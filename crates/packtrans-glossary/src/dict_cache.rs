use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use lindera::dictionary::Dictionary;
use lru::LruCache;
use packtrans_glossary_core::{dictionary, tokenizer};

/// Default number of loaded Lindera dictionaries kept in memory.
const DEFAULT_CAPACITY: usize = 4;

/// LRU cache of loaded Lindera dictionaries keyed by resolved on-disk path.
#[derive(Clone)]
pub struct DictionaryCache {
    inner: Arc<Mutex<LruCache<String, Dictionary>>>,
}

impl DictionaryCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(DEFAULT_CAPACITY).expect("cache capacity is non-zero"),
            ))),
        }
    }

    /// Returns a cached dictionary for `lang`, loading it from disk on a miss.
    ///
    /// Returns `None` when the language uses Tantivy's default tokenizer.
    pub fn get_or_load(&self, lang: &str, base: Option<&Path>) -> Result<Option<Dictionary>> {
        let name = tokenizer::target_tokenizer_name(lang);
        if name == "default" {
            return Ok(None);
        }

        let key = dictionary::dictionary_path(name, base)?
            .to_string_lossy()
            .into_owned();

        let mut cache = self.inner.lock().expect("dictionary cache mutex poisoned");
        if let Some(dict) = cache.get(&key) {
            return Ok(Some(dict.clone()));
        }

        let dict = tokenizer::load_dictionary(name, base)?;
        cache.put(key, dict.clone());
        Ok(Some(dict))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_lang_skips_cache() {
        let cache = DictionaryCache::new();
        assert!(cache.get_or_load("en_us", None).unwrap().is_none());
    }
}
