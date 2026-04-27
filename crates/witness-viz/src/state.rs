use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub reports_dir: PathBuf,
}

impl AppState {
    pub fn new(reports_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(AppStateInner { reports_dir }),
        }
    }

    pub fn reports_dir(&self) -> &std::path::Path {
        &self.inner.reports_dir
    }
}
