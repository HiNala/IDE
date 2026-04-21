//! Orchestrates embedding sidecars and code definitions into [`IndexStore`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

use editor_metadata::parse;
use editor_metadata::schema::Sidecar;
use editor_metadata::store::MetadataStore;

use crate::code_chunks::{extract_rust_definitions, is_indexable_code_file, should_skip_path};
use crate::config::IndexFile;
use crate::embedder::DynEmbedder;
use crate::error::{IndexError, Result};
use crate::incremental::is_sidecar_path;
use crate::schema::{Chunk, ChunkKind, ChunkMetadata};
use crate::store::IndexStore;

pub use crate::retrieve::{retrieve, Filter, RetrievalQuery, RetrievedChunk};

fn collect_md_paths(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(root)? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            collect_md_paths(&p, out)?;
        } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
            out.push(p);
        }
    }
    Ok(())
}

/// High-level indexing API (M22).
pub struct Indexer {
    workspace_root: PathBuf,
    store: Arc<IndexStore>,
    embedder: DynEmbedder,
    metadata: Arc<MetadataStore>,
    cfg: IndexFile,
}

impl std::fmt::Debug for Indexer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Indexer")
            .field("workspace_root", &self.workspace_root)
            .field("cfg", &self.cfg)
            .finish_non_exhaustive()
    }
}

impl Indexer {
    pub fn new(
        workspace_root: PathBuf,
        store: Arc<IndexStore>,
        embedder: DynEmbedder,
        metadata: Arc<MetadataStore>,
        cfg: IndexFile,
    ) -> Self {
        Self { workspace_root, store, embedder, metadata, cfg }
    }

    fn embedder_id(&self) -> String {
        self.embedder.id().to_string()
    }

    fn patch_meta(&self, m: &mut ChunkMetadata) {
        m.embedded_at = Utc::now();
        m.embedder_id = self.embedder_id();
    }

    /// Full rebuild: wipe derived index then walk sidecars + code files.
    pub async fn rebuild_all(&self) -> Result<IndexRebuildStats> {
        self.store.delete_all()?;
        let mut indexed_sidecars = 0u32;
        let mut indexed_code_files = 0u32;
        if self.cfg.index.index_sidecars {
            let meta_root = self.metadata.meta_root().to_path_buf();
            if meta_root.is_dir() {
                let mut md_files = Vec::new();
                collect_md_paths(&meta_root, &mut md_files).map_err(IndexError::Io)?;
                for p in md_files {
                    let raw = std::fs::read_to_string(&p).map_err(IndexError::Io)?;
                    if let Ok(sc) = parse(&raw) {
                        self.index_sidecar_for_source(&sc.frontmatter.source_path).await?;
                        indexed_sidecars += 1;
                    }
                }
            }
        }
        if self.cfg.index.index_code {
            for e in WalkBuilder::new(&self.workspace_root).standard_filters(true).build() {
                let e = e.map_err(|err| IndexError::Message(err.to_string()))?;
                let p = e.path();
                if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                let rel = p.strip_prefix(&self.workspace_root).unwrap_or(p);
                if rel.starts_with(".ide") {
                    continue;
                }
                if !is_indexable_code_file(rel) || should_skip_path(rel) {
                    continue;
                }
                if rel.extension().and_then(|x| x.to_str()) == Some("rs") {
                    self.index_code_file(rel).await?;
                    indexed_code_files += 1;
                }
            }
        }
        Ok(IndexRebuildStats { indexed_sidecars, indexed_code_files })
    }

    pub async fn index_sidecar_for_source(&self, source_rel: &Path) -> Result<()> {
        if !self.cfg.index.enabled || !self.cfg.index.index_sidecars {
            return Ok(());
        }
        let abs = self.workspace_root.join(source_rel);
        let Some(sc) = self.metadata.load(&abs)? else {
            return Ok(());
        };
        let sidecar_path = self.metadata.sidecar_path(&abs);
        let raw = std::fs::read_to_string(&sidecar_path).map_err(IndexError::Io)?;
        let hash = {
            let mut h = Sha256::new();
            h.update(raw.as_bytes());
            format!("{:x}", h.finalize())
        };
        let text = sidecar_embedding_text(&sc);
        let id = sidecar_chunk_id(source_rel);
        let path_key = posix_string(source_rel);
        self.store.delete_sidecar_for_path(&path_key)?;
        let mut chunk = Chunk {
            id,
            source_path: source_rel.to_path_buf(),
            chunk_kind: ChunkKind::Sidecar,
            text,
            source_hash: hash,
            metadata: ChunkMetadata {
                tags: sc.frontmatter.tags.clone(),
                line_start: None,
                line_end: None,
                embedded_at: Utc::now(),
                embedder_id: self.embedder_id(),
            },
        };
        self.patch_meta(&mut chunk.metadata);
        let embeddings =
            self.embedder.embed_batch(&[&chunk.text]).await.map_err(IndexError::Embedder)?;
        let emb = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| IndexError::Message("no embedding".into()))?;
        self.store.upsert_chunk(&chunk, &emb, self.embedder.id())?;
        Ok(())
    }

    pub async fn index_code_file(&self, source_rel: &Path) -> Result<()> {
        if !self.cfg.index.enabled || !self.cfg.index.index_code {
            return Ok(());
        }
        if should_skip_path(source_rel) {
            return Ok(());
        }
        if !is_indexable_code_file(source_rel) {
            return Ok(());
        }
        let abs = self.workspace_root.join(source_rel);
        let source = std::fs::read_to_string(&abs).map_err(IndexError::Io)?;
        let path_key = posix_string(source_rel);
        if source_rel.extension().and_then(|x| x.to_str()) != Some("rs") {
            return Ok(());
        }
        self.store.delete_code_for_path(&path_key)?;
        let mut chunks = extract_rust_definitions(&source, source_rel, self.embedder.id());
        for c in &mut chunks {
            self.patch_meta(&mut c.metadata);
        }
        if chunks.is_empty() {
            return Ok(());
        }
        let bs = self.cfg.embedder.batch_size.max(1);
        let mut start = 0usize;
        while start < chunks.len() {
            let end = (start + bs).min(chunks.len());
            let batch: Vec<&str> = chunks[start..end].iter().map(|c| c.text.as_str()).collect();
            let emb = self.embedder.embed_batch(&batch).await.map_err(IndexError::Embedder)?;
            if emb.len() != batch.len() {
                return Err(IndexError::Message("embedding batch mismatch".into()));
            }
            for (ch, e) in chunks[start..end].iter().zip(emb.iter()) {
                self.store.upsert_chunk(ch, e, self.embedder.id())?;
            }
            start = end;
        }
        Ok(())
    }

    /// Refresh sidecar + code for one path after an edit.
    pub async fn reindex_path(&self, path: &Path) -> Result<()> {
        let rel = path.strip_prefix(&self.workspace_root).unwrap_or(path);
        if is_sidecar_path(path) {
            let raw = std::fs::read_to_string(path).map_err(IndexError::Io)?;
            if let Ok(sc) = parse(&raw) {
                self.index_sidecar_for_source(&sc.frontmatter.source_path).await?;
            }
            return Ok(());
        }
        self.index_code_file(rel).await?;
        self.index_sidecar_for_source(rel).await?;
        Ok(())
    }

    pub async fn query(&self, q: RetrievalQuery) -> Result<Vec<RetrievedChunk>> {
        retrieve(&self.store, self.embedder.as_ref(), q).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IndexRebuildStats {
    pub indexed_sidecars: u32,
    pub indexed_code_files: u32,
}

fn sidecar_embedding_text(sc: &Sidecar) -> String {
    let mut t = String::new();
    if !sc.frontmatter.summary.is_empty() {
        t.push_str(&sc.frontmatter.summary);
    }
    if !sc.body.reasoning.is_empty() {
        if !t.is_empty() {
            t.push_str("\n\n");
        }
        t.push_str(&sc.body.reasoning);
    }
    t.chars().take(2000).collect()
}

fn sidecar_chunk_id(rel: &Path) -> String {
    let mut h = Sha256::new();
    h.update(posix_string(rel).as_bytes());
    h.update(b"|sidecar");
    format!("{:x}", h.finalize())
}

fn posix_string(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}
