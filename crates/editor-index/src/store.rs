//! SQLite persistence for chunks and embedding blobs (brute-force cosine in Rust).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{IndexError, Result};
use crate::schema::{chunk_kind_storage_key, parse_chunk_kind_storage, Chunk, ChunkMetadata};

fn f32_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

pub fn blob_to_f32(b: &[u8]) -> Option<Vec<f32>> {
    if !b.len().is_multiple_of(4) {
        return None;
    }
    Some(b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// Stored under `.ide/index.sqlite` (derived; safe to delete).
#[derive(Debug)]
pub struct IndexStore {
    path: PathBuf,
    conn: Connection,
}

impl IndexStore {
    pub fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let ide = workspace_root.as_ref().join(".ide");
        std::fs::create_dir_all(&ide)?;
        let path = ide.join("index.sqlite");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            r"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                source_path TEXT NOT NULL,
                chunk_kind TEXT NOT NULL,
                text TEXT NOT NULL,
                source_hash TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                embedded_at TEXT NOT NULL,
                embedder_id TEXT NOT NULL,
                dim INTEGER NOT NULL,
                embedding BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_path ON chunks(source_path);
            CREATE INDEX IF NOT EXISTS idx_chunks_embedder ON chunks(embedder_id);
            ",
        )?;
        Ok(Self { path, conn })
    }

    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.path
    }

    pub fn delete_all(&self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM chunks; VACUUM;").map_err(IndexError::from)?;
        Ok(())
    }

    pub fn upsert_chunk(&self, chunk: &Chunk, embedding: &[f32], embedder_id: &str) -> Result<()> {
        if embedding.is_empty() {
            return Err(IndexError::Message("empty embedding".into()));
        }
        let dim = embedding.len() as i32;
        let meta = serde_json::to_string(&chunk.metadata)?;
        let kind_s = chunk_kind_storage_key(&chunk.chunk_kind);
        let path_s = chunk.source_path.to_string_lossy();
        self.conn.execute(
            r"INSERT OR REPLACE INTO chunks
            (id, source_path, chunk_kind, text, source_hash, metadata_json, embedded_at, embedder_id, dim, embedding)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &chunk.id,
                path_s.as_ref() as &str,
                kind_s,
                &chunk.text,
                &chunk.source_hash,
                meta,
                chunk.metadata.embedded_at.to_rfc3339(),
                embedder_id,
                dim,
                f32_blob(embedding),
            ],
        )?;
        Ok(())
    }

    pub fn delete_by_source_path(&self, workspace_relative: &str) -> Result<usize> {
        let n =
            self.conn.execute("DELETE FROM chunks WHERE source_path = ?1", [workspace_relative])?;
        Ok(n)
    }

    pub fn delete_sidecar_for_path(&self, workspace_relative: &str) -> Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM chunks WHERE source_path = ?1 AND chunk_kind = 'sidecar'",
            [workspace_relative],
        )?;
        Ok(n)
    }

    pub fn delete_code_for_path(&self, workspace_relative: &str) -> Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM chunks WHERE source_path = ?1 AND chunk_kind LIKE 'code:%'",
            [workspace_relative],
        )?;
        Ok(n)
    }

    pub fn chunk_count(&self) -> Result<i64> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        Ok(n)
    }

    pub fn stats(&self) -> Result<IndexStats> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        let last: Option<String> = self
            .conn
            .query_row("SELECT MAX(embedded_at) FROM chunks", [], |r| r.get(0))
            .optional()?;
        let embedder: Option<String> = self
            .conn
            .query_row(
                "SELECT embedder_id FROM chunks ORDER BY embedded_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(IndexStats {
            chunk_count: count,
            last_embedded_at: last
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            last_embedder_id: embedder,
        })
    }

    /// Rows for brute-force retrieval (same embedder + dimension as query).
    pub fn load_for_similarity(&self, embedder_id: &str, dim: usize) -> Result<Vec<StoredRow>> {
        let mut stmt = self.conn.prepare(
            r"SELECT id, source_path, chunk_kind, text, source_hash, metadata_json, dim, embedding
               FROM chunks WHERE embedder_id = ?1 AND dim = ?2",
        )?;
        let rows = stmt.query_map(params![embedder_id, dim as i64], |r| {
            let emb: Vec<u8> = r.get(7)?;
            Ok(StoredRow {
                id: r.get(0)?,
                source_path: PathBuf::from(r.get::<_, String>(1)?),
                chunk_kind_s: r.get(2)?,
                text: r.get(3)?,
                source_hash: r.get(4)?,
                metadata_json: r.get(5)?,
                dim: r.get(6)?,
                embedding_bytes: emb,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[derive(Debug)]
pub struct IndexStats {
    pub chunk_count: i64,
    pub last_embedded_at: Option<DateTime<Utc>>,
    pub last_embedder_id: Option<String>,
}

#[derive(Debug)]
pub struct StoredRow {
    pub id: String,
    pub source_path: PathBuf,
    pub chunk_kind_s: String,
    pub text: String,
    pub source_hash: String,
    pub metadata_json: String,
    pub dim: i32,
    pub embedding_bytes: Vec<u8>,
}

impl StoredRow {
    pub fn to_chunk(&self) -> Result<Chunk> {
        let kind = parse_chunk_kind_storage(&self.chunk_kind_s)
            .ok_or_else(|| IndexError::Message(format!("bad chunk_kind: {}", self.chunk_kind_s)))?;
        let meta: ChunkMetadata = serde_json::from_str(&self.metadata_json)?;
        Ok(Chunk {
            id: self.id.clone(),
            source_path: self.source_path.clone(),
            chunk_kind: kind,
            text: self.text.clone(),
            source_hash: self.source_hash.clone(),
            metadata: meta,
        })
    }
}
