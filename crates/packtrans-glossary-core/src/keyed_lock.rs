use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;

type KeyLock = Arc<Mutex<()>>;

fn lock_table() -> &'static Mutex<HashMap<String, KeyLock>> {
    static TABLE: OnceLock<Mutex<HashMap<String, KeyLock>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_for(key: &str) -> KeyLock {
    let mut table = lock_table()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    table
        .entry(key.to_owned())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Runs `f` while holding an exclusive lock for `key`.
///
/// Concurrent callers with the same key wait for the in-flight work to finish;
/// only one runs `f` at a time per key.
pub fn with_key_lock<R>(key: &str, f: impl FnOnce() -> Result<R>) -> Result<R> {
    let lock = lock_for(key);
    let _guard = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[test]
    fn serializes_work_per_key() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let hits = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let hits = Arc::clone(&hits);
                thread::spawn(move || {
                    with_key_lock("test-key", || {
                        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
                        if n == 0 {
                            thread::sleep(Duration::from_millis(50));
                            hits.fetch_add(1, Ordering::SeqCst);
                        }
                        Ok::<_, anyhow::Error>(())
                    })
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap().unwrap();
        }

        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
