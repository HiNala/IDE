//! Error types for `editor-chat`.

use editor_ai_provider::ProviderError;

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("no AI provider configured — add an API key in Settings (Ctrl+,)")]
    NoProvider,
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("engine is already shut down")]
    Shutdown,
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ChatError>;
