# M22 — Local Vector Index & Semantic Retrieval

**Mission ID:** M22
**Prerequisites:** M21 complete. Sidecars exist and accumulate.
**Output:** A new `editor-index` crate that embeds sidecar metadata and tree-sitter-extracted code definitions into a local vector store (sqlite-vec). Incremental indexing on file changes. A simple semantic query API: "top-K chunks most relevant to this text." This is the retrieval backbone that makes M23's chat context-aware without blindly stuffing every file into the prompt. Local-first: embeddings can be produced by a local Ollama model (nomic-embed-text) or a cheap API (Voyage, OpenAI text-embedding-3-small).
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 3; this is what makes sidecars *useful*.
- M21 — the sidecars we're indexing.
- M15 — Tree-sitter integration; we reuse the parser to chunk code by definitions.
- `https://github.com/asg017/sqlite-vec` — the chosen storage backend. Brute-force vector search in SQLite, works on any platform including Raspberry Pi Zero.
- `https://docs.rs/rusqlite/` — SQLite bindings for Rust.
- `https://ollama.com/library/nomic-embed-text` — recommended local embedding model.

---

## The Situation In Plain English

Now that sidecars exist and contain rich summaries, we need a way for agents to find *the right ones* for a given question. If the user asks "how does authentication work in this project?" the agent shouldn't read every sidecar — it should retrieve the three or four sidecars most relevant to authentication. That's what vector retrieval does: each chunk of text gets embedded (turned into a dense high-dimensional vector), stored, and queried by cosine similarity against the embedding of the query.

Choice of backend: **`sqlite-vec`**. We considered:

- **LanceDB**: Rust-native, columnar, zero-copy, fast. But its Rust API is explicitly marked unstable; expecting churn. Good choice when it stabilizes.
- **Qdrant (embedded)**: Excellent quality but server-oriented. Overkill for our needs.
- **hnsw_rs / usearch**: Pure HNSW indexes. Fast but we'd have to build persistence and metadata storage around them.
- **sqlite-vec**: A SQLite extension doing brute-force cosine search. Works everywhere SQLite works (which is everywhere). For editor-scale use (a project has thousands, not millions, of chunks), brute-force is fast enough — a 10k-chunk search on 768-dim vectors is ~5ms.

sqlite-vec wins for V3. It's simple, stable, ships as a single loadable extension, and its "brute-force is fine" design matches our actual scale. If we ever need to index a 10-million-file monorepo, we'll revisit.

Embedding model: **nomic-embed-text** by default via Ollama. It's 768-dim, runs cheaply on CPU, produces high-quality embeddings for both prose and code. For users who don't run Ollama, we support calling Voyage's `voyage-code-3` or OpenAI's `text-embedding-3-small` — both very cheap.

What do we embed? Two things:
1. **Sidecar summaries** — each sidecar's `summary` frontmatter field plus the first ~500 tokens of the body. These are dense knowledge and the most valuable retrievals.
2. **Code definitions** — functions, classes, and top-level items extracted via Tree-sitter, with their signature and a short synthetic description. This is Aider's repo-map pattern, adapted for vector search.

Incremental indexing: a file watcher (we already have one) triggers re-embedding when a file changes. Hash-based invalidation — if the source hash matches the last-indexed hash, skip.

Query API: `retrieve(query: &str, top_k: usize, filter: Filter) -> Vec<Chunk>`. Filter lets callers restrict by file, tag, or recency. M23's chat panel calls this before assembling the prompt.

---

## Scope

**In scope:**
- New `editor-index` crate.
- sqlite-vec integration via rusqlite.
- `Chunk` representation: (id, text, source_path, chunk_kind, hash, embedding, metadata).
- Embedder trait with `OllamaEmbedder`, `ApiEmbedder`, `NoopEmbedder`.
- Sidecar indexing.
- Code definition extraction via Tree-sitter (reuse M15's grammars).
- Incremental indexing driven by workspace file watcher + sidecar update events.
- `retrieve()` API with basic filters.
- CLI command: `ide index --rebuild` to force a full re-index.

**Out of scope:**
- HNSW / approximate nearest neighbor (V4+; brute-force is fine at our scale).
- Multi-lingual query rewriting (V4+).
- Reranking stages (V4+).
- Hybrid search combining BM25 with vectors (V4+; plain vector is enough for V3).
- Embedding updates via background job scheduling beyond simple "on change, re-embed."

---

## North Star

Agent turn begins. User asks "how does our rate limiter work?" Before calling the frontier model, M23 calls `index.retrieve("rate limiter", top_k=5)`. Back come: `src/auth.rs` sidecar (tagged `rate-limiting`), a `governor` usage excerpt from M21's dependency list, `tests/auth_test.rs` sidecar (mentions rate limiter tests), and so on. Those ~1500 tokens go into the prompt. The frontier model answers with full context. Total token cost: low. Total answer quality: high.

---

## TODO List

### 1. Create `editor-index` crate

- [ ] 1.1. `cargo new --lib crates/editor-index`. Deps: `rusqlite` (with `bundled` feature to get SQLite 3.41+), `sqlite-vec` (binding crate if available, otherwise load as a dynamic extension — see 3.1), `serde`, `serde_json`, `sha2`, `tokio`, `crossbeam-channel`, `editor-core`, `editor-workspace`, `editor-syntax`, `editor-metadata`, `editor-ai-provider`.
- [ ] 1.2. Commit: `feat(index): scaffold editor-index crate`.

### 2. Schema

- [ ] 2.1. `src/schema.rs`:
  ```rust
  pub struct Chunk {
      pub id: String,                  // sha256(source_path + chunk_kind + chunk_offset)
      pub source_path: PathBuf,
      pub chunk_kind: ChunkKind,
      pub text: String,                // what gets embedded
      pub source_hash: String,         // sha256 of source content at embedding time
      pub metadata: ChunkMetadata,
  }
  pub enum ChunkKind {
      Sidecar,
      CodeDefinition { kind: &'static str, name: String },   // fn, class, impl, etc.
  }
  pub struct ChunkMetadata {
      pub tags: Vec<String>,
      pub line_start: Option<u32>,
      pub line_end: Option<u32>,
      pub embedded_at: DateTime<Utc>,
      pub embedder_id: String,
  }
  ```
- [ ] 2.2. Commit: `feat(index): chunk schema`.

### 3. SQLite + sqlite-vec setup

- [ ] 3.1. Storage location: `.ide/index.sqlite` in the workspace (git-ignored by default — it's derived, each dev rebuilds).
- [ ] 3.2. Schema:
  ```sql
  CREATE TABLE chunks (
      id TEXT PRIMARY KEY,
      source_path TEXT NOT NULL,
      chunk_kind TEXT NOT NULL,
      text TEXT NOT NULL,
      source_hash TEXT NOT NULL,
      metadata_json TEXT NOT NULL,
      embedded_at TEXT NOT NULL
  );
  CREATE INDEX idx_chunks_path ON chunks(source_path);
  CREATE VIRTUAL TABLE vec_chunks USING vec0(
      id TEXT PRIMARY KEY,
      embedding float[768]
  );
  ```
- [ ] 3.3. Load sqlite-vec as extension via rusqlite's extension loading. Pin the extension version.
- [ ] 3.4. Ship the compiled extension binary per-OS; bundle into the release artifacts from M11.
- [ ] 3.5. Commit: `feat(index): SQLite schema + sqlite-vec extension loading`.

### 4. Embedder trait

- [ ] 4.1. `src/embedder.rs`:
  ```rust
  #[async_trait::async_trait]
  pub trait Embedder: Send + Sync {
      fn id(&self) -> &str;
      fn dim(&self) -> usize;
      async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError>;
  }
  ```
- [ ] 4.2. Implementations:
  - `OllamaEmbedder` (default): posts to `http://localhost:11434/api/embeddings` with `model=nomic-embed-text`. 768-dim.
  - `VoyageEmbedder`: calls Voyage AI's API.
  - `OpenAIEmbedder`: calls `https://api.openai.com/v1/embeddings` with `text-embedding-3-small`.
  - `NoopEmbedder`: returns zero vectors; disables retrieval cleanly when nothing is configured.
- [ ] 4.3. Batch size 16 by default to amortize HTTP overhead.
- [ ] 4.4. Commit: `feat(index): Embedder implementations`.

### 5. Sidecar indexing

- [ ] 5.1. `src/indexer.rs`:
  ```rust
  pub struct Indexer {
      store: Arc<IndexStore>,
      embedder: Arc<dyn Embedder>,
      metadata_store: Arc<MetadataStore>,
  }
  impl Indexer {
      pub async fn index_sidecar(&self, source_path: &Path) -> Result<(), IndexError>;
      pub async fn rebuild_all(&self) -> Result<IndexStats, IndexError>;
  }
  ```
- [ ] 5.2. `index_sidecar`:
  1. Load the sidecar for `source_path`.
  2. Compute `source_hash = sha256(sidecar_markdown)`.
  3. Check existing `Chunk` with matching `id` and `source_hash` — if match, skip (idempotent).
  4. Construct chunk text: frontmatter summary + body's Summary section + body's Reasoning section. Cap at ~2000 chars.
  5. Call embedder.
  6. Upsert into `chunks` + `vec_chunks`.
- [ ] 5.3. Commit: `feat(index): sidecar embedding and upsert`.

### 6. Code definition extraction

- [ ] 6.1. `src/code_chunks.rs`:
  - Use M15's `Grammar` for the file's language. Walk the tree, find top-level definitions: functions, classes, impls, modules, const declarations.
  - For each definition, extract its signature (first line or first 200 chars) + the next few lines of body. Typical "repo-map" pattern from Aider.
  - Produce `Chunk { chunk_kind: CodeDefinition { kind, name }, text: signature_plus_snippet, ... }`.
- [ ] 6.2. Only index files whose extension we know; skip binaries, lock files, minified JS.
- [ ] 6.3. Cap per-file chunks at 200 (a crazy file with 500 tiny functions doesn't deserve that much index budget).
- [ ] 6.4. Commit: `feat(index): code definition chunking via Tree-sitter`.

### 7. Incremental indexing

- [ ] 7.1. Subscribe to `Workspace`'s `FileSystemEvent` stream. On `Modified(path)` or `Created(path)`:
  - If path is under `.ide/meta/`: re-index as a sidecar.
  - If path is a source file: re-index its code definitions *and* (if present) its sidecar.
  - On `Removed(path)`: delete all chunks where `source_path = path`.
- [ ] 7.2. Debounce re-indexing: a burst of edits to one file coalesces into one re-index 500 ms after the last edit.
- [ ] 7.3. Run indexing on a background worker (from `WorkerPool`) so it never blocks the UI.
- [ ] 7.4. Commit: `feat(index): incremental indexing driven by file watcher`.

### 8. Retrieval API

- [ ] 8.1. `src/retrieve.rs`:
  ```rust
  pub struct RetrievalQuery {
      pub text: String,
      pub top_k: usize,
      pub filter: Filter,
  }
  pub struct Filter {
      pub path_glob: Option<String>,
      pub tags_any: Vec<String>,
      pub chunk_kinds: Vec<ChunkKindSelector>,
  }
  pub struct RetrievedChunk {
      pub chunk: Chunk,
      pub similarity: f32,
  }
  impl Indexer {
      pub async fn retrieve(&self, query: RetrievalQuery) -> Result<Vec<RetrievedChunk>, IndexError>;
  }
  ```
- [ ] 8.2. Implementation:
  1. Embed the query text.
  2. `SELECT ... FROM vec_chunks ORDER BY vec_distance_cosine(embedding, ?) LIMIT ?`.
  3. Join with `chunks` for metadata; apply filters.
- [ ] 8.3. Target: retrieval latency < 30 ms for a 10k-chunk index.
- [ ] 8.4. Commit: `feat(index): retrieval API`.

### 9. CLI

- [ ] 9.1. `editor-app index --rebuild` runs a full re-index from scratch (drops the sqlite db, re-creates, indexes everything).
- [ ] 9.2. `editor-app index --status` prints a small report: N chunks indexed, last update timestamp, embedder in use.
- [ ] 9.3. Commit: `feat(app): index subcommand`.

### 10. Configuration

- [ ] 10.1. `.ide/index.toml`:
  ```toml
  [index]
  enabled = true
  index_code = true
  index_sidecars = true

  [embedder]
  kind = "ollama"   # or "voyage" or "openai" or "noop"
  model = "nomic-embed-text"
  batch_size = 16
  ```
- [ ] 10.2. Commit: `feat(index): configuration file`.

### 11. Benchmarks

- [ ] 11.1. Full index of a 1000-file project with Ollama embedder: target under 2 minutes (embedding is the bottleneck, not storage).
- [ ] 11.2. Incremental re-index on single-file change: < 500 ms including embedding.
- [ ] 11.3. Retrieval on 10k-chunk index: < 30 ms.
- [ ] 11.4. Storage: 10k chunks with 768-dim f32 vectors: ~30 MB. Acceptable.
- [ ] 11.5. Save baseline as `m22-v3`.
- [ ] 11.6. Commit: `bench(index): indexing and retrieval benchmarks`.

### 12. Quality gates + documentation

- [ ] 12.1. Standard gates.
- [ ] 12.2. Write `/docs/VECTOR_INDEX.md` covering architecture, storage layout, embedder selection, retrieval semantics.
- [ ] 12.3. Add `.ide/index.sqlite` to the default `.gitignore` recipe in M21.
- [ ] 12.4. Tag: `git tag -a m22-complete -m "M22 complete: local vector index"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. Full index of a small test project completes without errors.
3. Retrieval returns plausibly relevant chunks.
4. Incremental re-indexing updates the store correctly on file change.
5. Storage under `.ide/index.sqlite` is git-ignored by default.
6. `m22-complete` tag pushed.

## Testing Requirements

- Unit: chunk serialization, filter logic.
- Integration: full index → retrieve → known expected top result.
- Benchmark thresholds met.

## Git Commit Strategy

10-12 commits. Push after items 4, 7, 9, 12.

## Handoff to M23

M23 ties it all together. It uses the provider (M19), the tools (M20), the metadata capture (M21), and the retrieval (M22) in one coherent chat UI.

---

## Standing Orders Reminder

- Embeddings are versioned by `embedder_id`. If you change the embedder, invalidate and re-embed — mixing dimensions or models destroys retrieval quality silently.
- The index is *derived*. Losing it is annoying, not fatal — `ide index --rebuild` recovers in minutes. Never block editor startup on index readiness.
- Do not block the editor frame on embedding API calls. All indexing is off-thread.

Go.
