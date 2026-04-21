//! AI provider abstraction: one [`AiProvider`] trait over Anthropic, OpenAI, Ollama, and OpenAI-compatible HTTP APIs.
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

pub mod anthropic;
pub mod config;
pub mod custom;
pub mod error;
mod http;
pub mod ledger;
pub mod ollama;
pub mod openai;
mod openai_compat;
pub mod probe;
pub mod provider;
pub mod rate_limit;
pub mod registry;
pub mod secrets;
mod sse;

pub mod types;

pub use config::{load_or_create_default, providers_config_path, ActiveSection, ProviderConfig};
pub use error::ProviderError;
pub use ledger::TokenLedger;
pub use provider::{AiProvider, ChatEventStream};
pub use registry::{ProviderRegistry, RegistryError};
pub use secrets::SecretStore;
pub use types::{
    ChatEvent, ChatRequest, ContentBlock, Message, ModelInfo, StopReason, ToolDef, Usage,
};
