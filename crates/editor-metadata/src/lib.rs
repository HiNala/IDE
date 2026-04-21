//! Per-file metadata sidecars (`.ide/meta/…`), session logs, and `.ide/tasks.md` (M21).

#![forbid(unsafe_code)]

pub mod api_summarizer;
pub mod config;
pub mod error;
pub mod ollama_summarizer;
pub mod prompt;
pub mod schema;
pub mod session;
pub mod store;
pub mod summarizer;
pub mod tasks;
pub mod update;

pub use api_summarizer::{ApiProviderKind, ApiSummarizer};
pub use config::{
    load_metadata_config, summarizer_from_config, summarizer_from_config_with_key,
    MetadataFileConfig, SummarizerSection,
};
pub use error::{MetadataError, ParseError, Result, SummarizerError, TaskError};
pub use ollama_summarizer::OllamaSummarizer;
pub use schema::{
    blank_sidecar, parse, write_to_markdown, Frontmatter, HistoryEntry, Sidecar, SidecarBody,
};
pub use session::{SessionEvent, SessionLog};
pub use store::MetadataStore;
pub use summarizer::{NoopSummarizer, Summarizer};
pub use tasks::{tasks_path, Task, TaskList, TaskStatus};
pub use update::{workspace_relative, MetadataUpdater};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
