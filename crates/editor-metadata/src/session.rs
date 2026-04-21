//! Session log passed to summarizers (serialized into prompts as JSON).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// High-level event in an agent session (extensible for M23+).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    Note { text: String },
}

/// One agent turn: identity plus files that were committed to disk from tool output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionLog {
    pub id: String,
    #[serde(default)]
    pub committed_changes: Vec<PathBuf>,
    #[serde(default)]
    pub events: Vec<SessionEvent>,
}

impl Default for SessionLog {
    fn default() -> Self {
        Self { id: "default-session".into(), committed_changes: Vec::new(), events: Vec::new() }
    }
}
