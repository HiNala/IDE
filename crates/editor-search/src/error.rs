//! Search errors (invalid regex, out-of-range replace, etc.).

use thiserror::Error;

/// Failures from search / replace operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// User-supplied regex did not compile.
    #[error("invalid regex: {0}")]
    InvalidRegex(String),
    /// Match index out of range for the current match list.
    #[error("match index out of range")]
    MatchIndexOutOfRange,
    /// Underlying I/O while scanning the workspace.
    #[error("io error: {0}")]
    Io(String),
}

impl From<std::io::Error> for SearchError {
    fn from(e: std::io::Error) -> Self {
        SearchError::Io(e.to_string())
    }
}

impl From<grep_regex::Error> for SearchError {
    fn from(e: grep_regex::Error) -> Self {
        SearchError::InvalidRegex(e.to_string())
    }
}
