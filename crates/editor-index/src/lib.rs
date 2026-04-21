//! Local vector index for sidecars + code (M22). Storage: `.ide/index.sqlite` (derived).

#![forbid(unsafe_code)]
// Scaffold crate: allow noisy lints until the index API is finalized (docs + CLI ergonomics).
#![allow(missing_docs)]
#![allow(missing_debug_implementations)]
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::manual_is_multiple_of)]

pub mod cli;
pub mod code_chunks;
pub mod config;
pub mod embedder;
pub mod error;
pub mod incremental;
pub mod indexer;
pub mod retrieve;
pub mod schema;
pub mod store;

pub use config::{load_index_config, EmbedderSection, IndexFile, IndexSection};
pub use embedder::{
    build_embedder_from_config, noop_embedder, DynEmbedder, NoopEmbedder, OllamaEmbedder,
    OpenAiEmbedder, VoyageEmbedder,
};
pub use error::{EmbedderError, IndexError, Result};
pub use indexer::{IndexRebuildStats, Indexer};
pub use retrieve::{retrieve, Filter, RetrievalQuery, RetrievedChunk};
pub use schema::{Chunk, ChunkKind, ChunkKindSelector, ChunkMetadata};
pub use store::IndexStore;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
