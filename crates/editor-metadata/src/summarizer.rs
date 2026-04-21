//! Pluggable summarizers for sidecar generation.

use std::path::Path;

use async_trait::async_trait;

use crate::error::SummarizerError;
use crate::schema::{blank_sidecar, HistoryEntry, Sidecar};
use crate::session::SessionLog;

/// Produces or refreshes a [`Sidecar`] for one source file after a session.
#[async_trait]
pub trait Summarizer: Send + Sync {
    async fn summarize(
        &self,
        file_path: &Path,
        prior_sidecar: Option<&Sidecar>,
        session: &SessionLog,
        current_source: &str,
    ) -> Result<Sidecar, SummarizerError>;
}

/// No LLM: skeleton sidecar + one history line from the session log.
#[derive(Debug, Default, Clone)]
pub struct NoopSummarizer;

#[async_trait]
impl Summarizer for NoopSummarizer {
    async fn summarize(
        &self,
        file_path: &Path,
        prior_sidecar: Option<&Sidecar>,
        session: &SessionLog,
        current_source: &str,
    ) -> Result<Sidecar, SummarizerError> {
        let rel = path_posix_string(file_path);
        let rel_path = Path::new(&rel);
        let mut sc = if let Some(p) = prior_sidecar {
            p.clone()
        } else {
            blank_sidecar(rel_path, current_source, "noop")
        };
        sc.frontmatter.generated_by_model = "noop".into();
        sc.frontmatter.last_updated = chrono::Utc::now();
        if sc.frontmatter.summary.is_empty() {
            sc.frontmatter.summary = format!("(auto skeleton) {rel}");
        }
        sc.body.history.push(HistoryEntry {
            timestamp: chrono::Utc::now(),
            summary: format!("session {} touched file", session.id),
            session_id: session.id.clone(),
        });
        Ok(sc)
    }
}

fn path_posix_string(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}
