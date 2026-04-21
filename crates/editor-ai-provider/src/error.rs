//! Provider-facing error type for streaming chat.

use std::sync::Arc;

/// Error surfaced over the [`ChatEvent::Error`](crate::types::ChatEvent::Error) stream and for
/// fallible setup APIs.
#[derive(Debug)]
pub struct ProviderError(pub(crate) ProviderErrorInner);

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<ProviderErrorInner> for ProviderError {
    fn from(value: ProviderErrorInner) -> Self {
        ProviderError(value)
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(value: serde_json::Error) -> Self {
        ProviderError(ProviderErrorInner::Json(value))
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderErrorInner {
    #[error("HTTP request failed")]
    Http(#[source] reqwest::Error),
    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("JSON error: {0}")]
    Json(#[source] serde_json::Error),
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("SSE: {0}")]
    Sse(String),
    #[error("stream ended before Done event")]
    IncompleteStream,
    #[error("{0}")]
    Message(String),
    #[error("configuration: {0}")]
    Config(String),
}

impl ProviderError {
    pub fn message(msg: impl Into<String>) -> Self {
        ProviderError(ProviderErrorInner::Message(msg.into()))
    }

    pub fn config(msg: impl Into<String>) -> Self {
        ProviderError(ProviderErrorInner::Config(msg.into()))
    }

    pub fn http_status(status: u16, body: String) -> Self {
        ProviderError(ProviderErrorInner::HttpStatus { status, body })
    }

    pub fn sse(msg: impl Into<String>) -> Self {
        ProviderError(ProviderErrorInner::Sse(msg.into()))
    }

    pub(crate) fn http(err: reqwest::Error) -> Self {
        ProviderError(ProviderErrorInner::Http(err))
    }

    pub fn into_shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn incomplete_stream() -> Self {
        ProviderError(ProviderErrorInner::IncompleteStream)
    }
}

impl Clone for ProviderError {
    fn clone(&self) -> Self {
        match &self.0 {
            ProviderErrorInner::Http(e) => {
                ProviderError(ProviderErrorInner::Message(e.to_string()))
            }
            ProviderErrorInner::HttpStatus { status, body } => {
                ProviderError(ProviderErrorInner::HttpStatus {
                    status: *status,
                    body: body.clone(),
                })
            }
            ProviderErrorInner::Json(e) => {
                ProviderError(ProviderErrorInner::Message(e.to_string()))
            }
            ProviderErrorInner::Url(e) => ProviderError(ProviderErrorInner::Message(e.to_string())),
            ProviderErrorInner::Sse(s) => ProviderError(ProviderErrorInner::Sse(s.clone())),
            ProviderErrorInner::IncompleteStream => {
                ProviderError(ProviderErrorInner::IncompleteStream)
            }
            ProviderErrorInner::Message(s) => ProviderError(ProviderErrorInner::Message(s.clone())),
            ProviderErrorInner::Config(s) => ProviderError(ProviderErrorInner::Config(s.clone())),
        }
    }
}

impl serde::Serialize for ProviderError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Wrapper so keyring failures on headless Linux can still surface something structured.
#[derive(Debug, Clone)]
pub struct KeyringFailure {
    pub message: String,
    pub used_env_fallback: bool,
}

impl std::fmt::Display for KeyringFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if self.used_env_fallback {
            write!(f, " (environment fallback was used)")?;
        }
        Ok(())
    }
}

pub type Result<T, E = ProviderError> = std::result::Result<T, E>;
