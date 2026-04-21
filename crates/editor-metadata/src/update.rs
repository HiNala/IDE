//! Run summarization for every path in [`SessionLog::committed_changes`](crate::session::SessionLog).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::error::MetadataError;
use crate::session::SessionLog;
use crate::store::MetadataStore;
use crate::summarizer::Summarizer;

/// Runs sidecar updates concurrently with bounded parallelism.
pub struct MetadataUpdater {
    store: Arc<MetadataStore>,
    summarizer: Arc<dyn Summarizer>,
    concurrency: usize,
}

impl std::fmt::Debug for MetadataUpdater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataUpdater")
            .field("concurrency", &self.concurrency)
            .finish_non_exhaustive()
    }
}

impl MetadataUpdater {
    #[must_use]
    pub fn new(
        store: Arc<MetadataStore>,
        summarizer: Arc<dyn Summarizer>,
        concurrency: usize,
    ) -> Self {
        Self { store, summarizer, concurrency: concurrency.max(1) }
    }

    /// Updates sidecars for each committed file. Failures are logged; successful paths are returned.
    pub async fn update_for_session(
        &self,
        session: &SessionLog,
    ) -> Result<Vec<PathBuf>, MetadataError> {
        if session.committed_changes.is_empty() {
            return Ok(Vec::new());
        }
        let sem = Arc::new(Semaphore::new(self.concurrency));
        let mut handles = Vec::new();
        let session = session.clone();
        let store = Arc::clone(&self.store);
        let summarizer = Arc::clone(&self.summarizer);
        for abs_path in session.committed_changes.iter().cloned() {
            let permit = sem
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| MetadataError::Message(e.to_string()))?;
            let store = Arc::clone(&store);
            let summarizer = Arc::clone(&summarizer);
            let session = session.clone();
            handles.push(tokio::spawn(async move {
                let _p = permit;
                let source_text = std::fs::read_to_string(&abs_path).unwrap_or_default();
                let prior = store.load(&abs_path).ok().flatten();
                let rel = workspace_relative(store.workspace_root(), &abs_path);
                match summarizer
                    .summarize(rel.as_path(), prior.as_ref(), &session, &source_text)
                    .await
                {
                    Ok(sc) => match store.save(&sc) {
                        Ok(()) => Some(abs_path),
                        Err(e) => {
                            tracing::warn!(path = ?abs_path, error = %e, "failed to save sidecar");
                            None
                        }
                    },
                    Err(e) => {
                        tracing::warn!(path = ?abs_path, error = %e, "sidecar summarization failed; leaving prior");
                        None
                    }
                }
            }));
        }
        let mut out = Vec::new();
        for h in handles {
            if let Ok(Some(p)) = h.await {
                out.push(p);
            }
        }
        Ok(out)
    }
}

#[must_use]
pub fn workspace_relative(workspace_root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(workspace_root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| file.to_path_buf())
}
