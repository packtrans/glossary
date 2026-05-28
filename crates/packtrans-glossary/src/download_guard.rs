use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;

/// Serializes in-flight downloads for the HTTP server.
pub struct DownloadCoordinator {
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl DownloadCoordinator {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_lock<R>(&self, key: &str, f: impl FnOnce() -> Result<R>) -> Result<R> {
        let lock = {
            let mut table = self
                .locks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            table
                .entry(key.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f()
    }
}

impl Default for DownloadCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn with_download_lock<R>(
    guard: Option<&DownloadCoordinator>,
    key: &str,
    f: impl FnOnce() -> Result<R>,
) -> Result<R> {
    match guard {
        Some(coordinator) => coordinator.with_lock(key, f),
        None => f(),
    }
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
        let coordinator = Arc::new(DownloadCoordinator::new());
        let hits = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let coordinator = Arc::clone(&coordinator);
                let hits = Arc::clone(&hits);
                thread::spawn(move || {
                    coordinator.with_lock("test-key", || {
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
