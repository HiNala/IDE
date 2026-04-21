# Local vector index (M22)

The **`editor-index`** crate stores embeddings for **sidecar markdown** (from `editor-metadata`) and **Rust code definitions** (Tree-sitter) under **`.ide/index.sqlite`**. That file is **derived**; delete it or run `editor-app index --rebuild` to recreate it. It is git-ignored by default.

## Architecture

1. **Chunks** — Each row is one retrievable unit: either a sidecar digest (`chunk_kind = sidecar`) or a code snippet (`chunk_kind = code:…`). Text + metadata live in SQLite; **embedding vectors** are stored as `f32` **BLOBs** (little-endian).

2. **Retrieval** — The query string is embedded with the **same embedder** as indexing. Similarity is **cosine similarity** between the query vector and each stored vector, with the same **`embedder_id`** and **dimension**. Results are sorted by similarity and the top **K** are returned after optional **filters** (path glob, tags, chunk kinds). At editor scale this **brute-force** scan is acceptable; the mission keeps the door open to **sqlite-vec** or other ANN backends later.

3. **Embedders** — Vectors are **not comparable across models** or dimensions. The store records `embedder_id` per row; retrieval only loads rows matching the active embedder. If you change `.ide/index.toml` or model, run **`editor-app index --rebuild`** so everything is re-embedded consistently.

## Layout

| Path | Role |
|------|------|
| `.ide/index.sqlite` | SQLite DB (chunks + embedding blobs) |
| `.ide/index.toml` | Optional config (see below) |
| `.ide/meta/` | Sidecar sources (indexed when `index_sidecars = true`) |

## Configuration (`.ide/index.toml`)

Defaults apply if the file is missing.

```toml
[index]
enabled = true
index_code = true
index_sidecars = true

[embedder]
kind = "ollama"   # "ollama" | "openai" | "voyage" | "noop"
model = "nomic-embed-text"
batch_size = 16
# ollama_base_url = "http://localhost:11434"
```

- **`ollama`** — `POST /api/embeddings` (default model `nomic-embed-text`, 768-d). Local-first.
- **`openai`** — `text-embedding-3-small` (or `model`); requires **`OPENAI_API_KEY`**.
- **`voyage`** — Voyage embeddings API; requires **`VOYAGE_API_KEY`**.
- **`noop`** — Zero vectors; useful for tests or when retrieval is disabled but wiring should stay intact.

## CLI

From the **workspace root** (current working directory):

```bash
cargo run -p editor-app -- index --rebuild
cargo run -p editor-app -- index --status
```

`--status` prints chunk count and last embedding metadata.

## API

Use **`Indexer::query`** / **`retrieve`** with a **`RetrievalQuery`** and **`Filter`** from `editor_index`. Incremental indexing hooks live in **`incremental`** (debounced file events); the workspace watcher emits changes for **`.ide/meta/**/*.md`** sidecars as well as normal source files (other `.ide/` paths stay filtered as tooling noise). Wire debounced `Indexer::reindex_path` to your **`WorkerPool`** in the app when you integrate M23 — never on the render thread.

## References

- [sqlite-vec](https://github.com/asg017/sqlite-vec) — optional future extension; current implementation uses in-process cosine over stored blobs.
- [Ollama nomic-embed-text](https://ollama.com/library/nomic-embed-text)
