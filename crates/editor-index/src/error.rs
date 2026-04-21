//! Errors for the local vector index.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("metadata: {0}")]
    Metadata(#[from] editor_metadata::MetadataError),
    #[error("embedder: {0}")]
    Embedder(#[from] EmbedderError),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Error)]
pub enum EmbedderError {
    #[error("http: {0}")]
    Http(String),
    #[error("empty embedding response")]
    Empty,
    #[error("dimension mismatch (expected {expected}, got {got})")]
    DimMismatch { expected: usize, got: usize },
    #[error("{0}")]
    Message(String),
}

pub type Result<T, E = IndexError> = std::result::Result<T, E>;
