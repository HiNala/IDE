//! Prompt fragments for local/API summarizers (see `prompts/summarize.md`).

use std::path::Path;

use crate::schema::{write_to_markdown, Sidecar};
use crate::session::SessionLog;

pub const SUMMARIZE_SYSTEM: &str = include_str!("../prompts/summarize.md");

pub const SUMMARIZE_USER_HEADER: &str =
    "Output ONLY the complete sidecar markdown document: YAML frontmatter between --- lines, then the body sections exactly as specified. No prose before or after the document.";

#[must_use]
pub fn build_summarizer_user_message(
    file_path: &Path,
    prior_sidecar: Option<&Sidecar>,
    session: &SessionLog,
    current_source: &str,
) -> String {
    let prior =
        prior_sidecar.and_then(|s| write_to_markdown(s).ok()).unwrap_or_else(|| "(none)".into());
    let session_txt = serde_json::to_string_pretty(session).unwrap_or_else(|_| "{}".into());
    format!(
        "{SUMMARIZE_USER_HEADER}\n\n## Prior sidecar markdown\n{prior}\n\n## Session log (JSON)\n{session_txt}\n\n## File: {}\n\n## Source\n```\n{current_source}\n```\n",
        file_path.display(),
    )
}

#[must_use]
pub fn strip_model_markdown_fence(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.strip_prefix("markdown").unwrap_or(rest).trim_start();
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
    }
    t.to_string()
}
