//! Error types for metadata, summarization, and tasks.

use thiserror::Error;

/// Sidecar file / store errors.
#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] ParseError),
    #[error("{0}")]
    Message(String),
}

/// Markdown / YAML sidecar parse failures.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("missing YAML frontmatter delimiter ---")]
    MissingFrontmatter,
    #[error("YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid UTF-8")]
    Utf8,
}

/// Summarizer failures (logged; must not abort commits).
#[derive(Debug, Error)]
pub enum SummarizerError {
    #[error("HTTP / provider: {0}")]
    Http(String),
    #[error("empty model response")]
    EmptyResponse,
    #[error("parse sidecar from model output: {0}")]
    Parse(#[from] ParseError),
    #[error("{0}")]
    Message(String),
}

/// Task list parse / write issues.
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Message(String),
}

impl From<std::str::Utf8Error> for ParseError {
    fn from(_: std::str::Utf8Error) -> Self {
        ParseError::Utf8
    }
}

pub type Result<T, E = MetadataError> = std::result::Result<T, E>;
