//! `editor-app index` subcommand implementation.

use std::path::Path;
use std::sync::Arc;

use editor_metadata::store::MetadataStore;

use crate::config::load_index_config;
use crate::embedder::build_embedder_from_config;
use crate::error::{IndexError, Result};
use crate::indexer::Indexer;
use crate::store::IndexStore;

/// Run `index --rebuild` / `index --status` against `workspace_root`.
pub fn run_cli(workspace_root: &Path, args: &[String]) -> Result<()> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("usage: editor-app index [--rebuild | --status]");
        return Ok(());
    }

    let cfg = load_index_config(workspace_root);
    let store = Arc::new(IndexStore::open(workspace_root)?);
    let embedder = build_embedder_from_config(&cfg)?;
    let meta = Arc::new(MetadataStore::new(workspace_root.to_path_buf()));
    let indexer =
        Indexer::new(workspace_root.to_path_buf(), Arc::clone(&store), embedder, meta, cfg);

    if args.iter().any(|a| a == "--rebuild") {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| IndexError::Message(e.to_string()))?;
        let stats = rt.block_on(indexer.rebuild_all())?;
        println!(
            "index: rebuild done (sidecar files indexed: {}, code files: {})",
            stats.indexed_sidecars, stats.indexed_code_files
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--status") {
        let st = store.stats()?;
        println!("index.sqlite: {}", store.db_path().display());
        println!("chunks: {}", st.chunk_count);
        println!("last_embedded_at: {:?}", st.last_embedded_at);
        println!("last_embedder_id: {:?}", st.last_embedder_id);
        return Ok(());
    }

    eprintln!("usage: editor-app index [--rebuild | --status]");
    Err(IndexError::Message("missing --rebuild or --status for `index` subcommand".into()))
}
