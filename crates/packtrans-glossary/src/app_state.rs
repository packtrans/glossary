use std::path::PathBuf;
use std::sync::Arc;

use crate::dict::DictionaryCache;
use crate::index::IndexCache;
use crate::util::download_guard::DownloadCoordinator;

#[derive(Clone)]
pub struct AppState {
    pub(crate) index_dir: Option<PathBuf>,
    pub(crate) dict_path: Option<PathBuf>,
    pub(crate) download_guard: Arc<DownloadCoordinator>,
    pub(crate) dict_cache: DictionaryCache,
    pub(crate) index_cache: IndexCache,
}

impl AppState {
    pub fn new(index_dir: Option<PathBuf>, dict_path: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            index_dir,
            dict_path,
            download_guard: Arc::new(DownloadCoordinator::new()),
            dict_cache: DictionaryCache::new(),
            index_cache: IndexCache::new(),
        })
    }
}
