//! Agent tool-use surface (M20): workspace-bound tools, transactions, JSON Schemas.
//!
//! Writes stage into [`WorkspaceTx`] and commit through [`editor_core::TextBuffer::apply_edit`].
//! See `docs/AI_TOOLS.md` for tool reference.

#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

pub mod config;
pub mod error;
pub mod path;
pub mod registry;
pub mod tool;
pub mod tools;
pub mod transaction;

pub use config::ToolConfig;
pub use editor_ai_provider::ToolDef;
pub use error::{Result, ToolError};
pub use registry::ToolRegistry;
pub use tool::{Tool, ToolOutput};
pub use transaction::{BufferEdit, PendingChange, WorkspaceTx};
