//! Integration: store + retrieve with a deterministic test embedder.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use editor_index::embedder::Embedder;
use editor_index::retrieve::{retrieve, Filter, RetrievalQuery};
use editor_index::schema::{Chunk, ChunkKind, ChunkMetadata};
use editor_index::store::IndexStore;

#[derive(Debug)]
struct FixedEmbedder {
    v: Vec<f32>,
}

#[async_trait]
impl Embedder for FixedEmbedder {
    fn id(&self) -> &str {
        "test-fixed"
    }

    fn dim(&self) -> usize {
        self.v.len()
    }

    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, editor_index::EmbedderError> {
        let n = texts.len();
        Ok(vec![self.v.clone(); n])
    }
}

#[tokio::test]
async fn retrieve_ranks_by_cosine_against_fixed_query_embedding() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = IndexStore::open(dir.path()).expect("open store");

    let v_query = vec![1.0f32, 0.0, 0.0, 0.0];
    let v_other = vec![0.0f32, 1.0, 0.0, 0.0];

    let chunk_a = Chunk {
        id: "c_a".into(),
        source_path: Path::new("src/auth.rs").to_path_buf(),
        chunk_kind: ChunkKind::Sidecar,
        text: "auth rate limiter".into(),
        source_hash: "ha".into(),
        metadata: ChunkMetadata {
            tags: vec!["auth".into()],
            line_start: None,
            line_end: None,
            embedded_at: Utc::now(),
            embedder_id: "test-fixed".into(),
        },
    };
    store.upsert_chunk(&chunk_a, &v_query, "test-fixed").expect("upsert a");

    let chunk_b = Chunk {
        id: "c_b".into(),
        source_path: Path::new("src/other.rs").to_path_buf(),
        chunk_kind: ChunkKind::Sidecar,
        text: "unrelated".into(),
        source_hash: "hb".into(),
        metadata: ChunkMetadata {
            tags: Vec::new(),
            line_start: None,
            line_end: None,
            embedded_at: Utc::now(),
            embedder_id: "test-fixed".into(),
        },
    };
    store.upsert_chunk(&chunk_b, &v_other, "test-fixed").expect("upsert b");

    let embedder: Arc<dyn Embedder> = Arc::new(FixedEmbedder { v: v_query.clone() });
    let q = RetrievalQuery { text: "anything".into(), top_k: 2, filter: Filter::default() };
    let out = retrieve(&store, embedder.as_ref(), q).await.expect("retrieve");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].chunk.id, "c_a");
    assert!(out[0].similarity > out[1].similarity);
}
