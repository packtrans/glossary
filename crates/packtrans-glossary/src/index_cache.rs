use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use lru::LruCache;
use tantivy::{Index, directory::MmapDirectory};

use crate::dict_cache::DictionaryCache;
use crate::tokenizer_native;

/// Default number of opened Tantivy indexes kept in memory.
const DEFAULT_CAPACITY: usize = 8;

/// LRU cache of opened Tantivy indexes keyed by resolved on-disk path.
#[derive(Clone)]
pub struct IndexCache {
    inner: Arc<Mutex<LruCache<String, Index>>>,
}

impl IndexCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(DEFAULT_CAPACITY).expect("cache capacity is non-zero"),
            ))),
        }
    }

    /// Returns a cached index for `index_dir`, opening it from disk on a miss.
    pub fn get_or_open(
        &self,
        index_dir: &Path,
        lang: &str,
        dict_path: Option<&Path>,
        dict_cache: Option<&DictionaryCache>,
    ) -> Result<Index> {
        let key = index_dir.to_string_lossy().into_owned();

        let mut cache = self.inner.lock().expect("index cache mutex poisoned");
        if let Some(index) = cache.get(&key) {
            return Ok(index.clone());
        }

        let index = open_index(index_dir, lang, dict_path, dict_cache)?;
        cache.put(key, index.clone());
        Ok(index)
    }
}

pub(crate) fn open_index(
    index_dir: &Path,
    lang: &str,
    dict_path: Option<&Path>,
    dict_cache: Option<&DictionaryCache>,
) -> Result<Index> {
    let dir = MmapDirectory::open(index_dir)
        .with_context(|| format!("failed to open index directory: {}", index_dir.display()))?;
    let index = Index::open(dir)
        .with_context(|| format!("failed to open index: {}", index_dir.display()))?;

    let cached_dict = match dict_cache {
        Some(cache) => cache.get_or_load(lang, dict_path)?,
        None => None,
    };
    tokenizer_native::register_for_language(&index, lang, dict_path, cached_dict.as_ref())?;

    Ok(index)
}
