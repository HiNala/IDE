//! Chunk representation for embeddings and retrieval.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One retrievable unit (sidecar digest or code definition).
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub id: String,
    pub source_path: PathBuf,
    pub chunk_kind: ChunkKind,
    pub text: String,
    /// Hash of underlying bytes at indexing time (sidecar markdown or full source file).
    pub source_hash: String,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkKind {
    Sidecar,
    CodeDefinition { kind: String, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub tags: Vec<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub embedded_at: DateTime<Utc>,
    pub embedder_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkKindSelector {
    #[serde(default)]
    pub include_sidecar: bool,
    #[serde(default)]
    pub include_code: bool,
}

impl Default for ChunkKindSelector {
    fn default() -> Self {
        Self { include_sidecar: true, include_code: true }
    }
}

pub fn chunk_kind_storage_key(kind: &ChunkKind) -> String {
    match kind {
        ChunkKind::Sidecar => "sidecar".into(),
        ChunkKind::CodeDefinition { kind: k, name } => format!("code:{k}:{name}"),
    }
}

pub fn parse_chunk_kind_storage(s: &str) -> Option<ChunkKind> {
    if s == "sidecar" {
        return Some(ChunkKind::Sidecar);
    }
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    if parts.len() == 3 && parts[0] == "code" {
        return Some(ChunkKind::CodeDefinition {
            kind: parts[1].to_string(),
            name: parts[2].to_string(),
        });
    }
    None
}
