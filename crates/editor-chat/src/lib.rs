//! `editor-chat` — AI chat session management and agent loop (M23).
//!
//! This crate wires the [`editor_ai_provider`] streaming API and
//! [`editor_ai_tools`] execution surface into a higher-level
//! `ChatSession` that can be driven from the winit event loop.
//!
//! Key types:
//! - [`ChatMessage`] — a single turn (user / assistant / tool) with optional streaming state.
//! - [`Conversation`] — an ordered list of messages for one session.
//! - [`ChatEngine`] — owns a tokio runtime + provider registry; spawns async tasks
//!   and sends [`ChatEvent`] updates back via a crossbeam channel.

#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

pub mod conversation;
pub mod engine;
pub mod error;

pub use conversation::{ChatMessage, ChatRole, Conversation, MessageId};
pub use engine::{ChatEngine, ChatEngineConfig, EngineEvent};
pub use error::{ChatError, Result};
