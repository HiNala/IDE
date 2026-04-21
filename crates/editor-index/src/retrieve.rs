//! Semantic retrieval: embed query, cosine over stored vectors, filter, top-K.

use globset::{GlobBuilder, GlobSetBuilder};

use crate::error::{IndexError, Result};
use crate::schema::{ChunkKind, ChunkKindSelector};
use crate::store::{blob_to_f32, IndexStore, StoredRow};

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::NAN;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let d = na.sqrt() * nb.sqrt();
    if d == 0.0 {
        0.0
    } else {
        dot / d
    }
}

#[derive(Debug, Default)]
pub struct Filter {
    pub path_glob: Option<String>,
    pub tags_any: Vec<String>,
    pub chunk_kinds: ChunkKindSelector,
}

#[derive(Debug)]
pub struct RetrievalQuery {
    pub text: String,
    pub top_k: usize,
    pub filter: Filter,
}

#[derive(Debug)]
pub struct RetrievedChunk {
    pub chunk: crate::schema::Chunk,
    pub similarity: f32,
}

pub async fn retrieve(
    store: &IndexStore,
    embedder: &dyn crate::embedder::Embedder,
    q: RetrievalQuery,
) -> Result<Vec<RetrievedChunk>> {
    if !q.filter.chunk_kinds.include_sidecar && !q.filter.chunk_kinds.include_code {
        return Ok(vec![]);
    }
    let qv = embed_single(embedder, &q.text).await?;
    let rows = store.load_for_similarity(embedder.id(), qv.len())?;

    let path_matcher: Option<globset::GlobSet> = if let Some(ref g) = q.filter.path_glob {
        let mut b = GlobSetBuilder::new();
        b.add(GlobBuilder::new(g).build().map_err(|e| IndexError::Message(e.to_string()))?);
        Some(b.build().map_err(|e| IndexError::Message(e.to_string()))?)
    } else {
        None
    };

    let want_tags = &q.filter.tags_any;
    let mut scored: Vec<(f32, StoredRow)> = Vec::new();
    for row in rows {
        let emb = match blob_to_f32(&row.embedding_bytes) {
            Some(e) => e,
            None => continue,
        };
        let chunk = row.to_chunk()?;
        let kind_ok = match &chunk.chunk_kind {
            ChunkKind::Sidecar => q.filter.chunk_kinds.include_sidecar,
            ChunkKind::CodeDefinition { .. } => q.filter.chunk_kinds.include_code,
        };
        if !kind_ok {
            continue;
        }
        if let Some(ref set) = path_matcher {
            let s = chunk.source_path.to_string_lossy();
            if !set.is_match(&*s) {
                continue;
            }
        }
        if !want_tags.is_empty() {
            let have = chunk.metadata.tags.iter().any(|t| want_tags.contains(t));
            if !have {
                continue;
            }
        }
        let sim = cosine_sim(&qv, &emb);
        scored.push((sim, row));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let take = q.top_k.max(1);
    let mut out = Vec::new();
    for (sim, row) in scored.into_iter().take(take) {
        out.push(RetrievedChunk { chunk: row.to_chunk()?, similarity: sim });
    }
    Ok(out)
}

async fn embed_single(embedder: &dyn crate::embedder::Embedder, text: &str) -> Result<Vec<f32>> {
    let mut v = embedder.embed_batch(&[text]).await.map_err(IndexError::Embedder)?;
    v.pop().ok_or_else(|| IndexError::Message("empty embedding batch".into()))
}
