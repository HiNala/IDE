//! Embedding backends: Ollama, OpenAI-compatible, Voyage, noop.

use std::sync::Arc;

use crate::config::IndexFile;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::EmbedderError;

#[async_trait]
pub trait Embedder: Send + Sync {
    fn id(&self) -> &str;
    fn dim(&self) -> usize;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError>;
}

/// Returns zero vectors of `dim` (retrieval disabled but pipeline stays wired).
#[derive(Debug)]
pub struct NoopEmbedder {
    dim: usize,
}

impl NoopEmbedder {
    #[must_use]
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

#[async_trait]
impl Embedder for NoopEmbedder {
    fn id(&self) -> &str {
        "noop"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        Ok(vec![vec![0.0f32; self.dim]; texts.len()])
    }
}

/// `POST /api/embeddings` on Ollama (default `nomic-embed-text`, 768-d).
#[derive(Debug)]
pub struct OllamaEmbedder {
    client: reqwest::Client,
    url: String,
    model: String,
    dim: usize,
}

impl OllamaEmbedder {
    pub fn new(base: Option<&str>, model: Option<&str>) -> Result<Self, EmbedderError> {
        let base = base.unwrap_or("http://localhost:11434").trim_end_matches('/');
        let url = format!("{base}/api/embeddings");
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-index/0.1")
                .build()
                .map_err(|e| EmbedderError::Http(e.to_string()))?,
            url,
            model: model.unwrap_or("nomic-embed-text").to_string(),
            dim: 768,
        })
    }
}

#[derive(Deserialize)]
struct OllamaEmbResp {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    fn id(&self) -> &str {
        "ollama"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            let body = json!({ "model": self.model, "prompt": t });
            let resp = self
                .client
                .post(&self.url)
                .json(&body)
                .send()
                .await
                .map_err(|e| EmbedderError::Http(e.to_string()))?;
            if !resp.status().is_success() {
                let t = resp.text().await.unwrap_or_default();
                return Err(EmbedderError::Http(format!("ollama embeddings {}", t)));
            }
            let parsed: OllamaEmbResp =
                resp.json().await.map_err(|e| EmbedderError::Http(e.to_string()))?;
            if parsed.embedding.len() != self.dim {
                return Err(EmbedderError::DimMismatch {
                    expected: self.dim,
                    got: parsed.embedding.len(),
                });
            }
            out.push(parsed.embedding);
        }
        Ok(out)
    }
}

/// OpenAI `text-embedding-3-small` (default 1536 dimensions).
#[derive(Debug)]
pub struct OpenAiEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dim: usize,
}

impl OpenAiEmbedder {
    pub fn new(api_key: impl Into<String>, model: Option<&str>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.unwrap_or("text-embedding-3-small").to_string(),
            dim: 1536,
        }
    }
}

#[derive(Deserialize)]
struct OpenAiEmbResp {
    data: Vec<OpenAiEmbData>,
}

#[derive(Deserialize)]
struct OpenAiEmbData {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    fn id(&self) -> &str {
        "openai"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let body = json!({
            "model": self.model,
            "input": texts,
        });
        let resp = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| EmbedderError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            return Err(EmbedderError::Http(format!("openai embeddings {}", t)));
        }
        let parsed: OpenAiEmbResp =
            resp.json().await.map_err(|e| EmbedderError::Http(e.to_string()))?;
        let mut out = Vec::with_capacity(parsed.data.len());
        for row in parsed.data {
            if row.embedding.len() != self.dim {
                return Err(EmbedderError::DimMismatch {
                    expected: self.dim,
                    got: row.embedding.len(),
                });
            }
            out.push(row.embedding);
        }
        if out.len() != texts.len() {
            return Err(EmbedderError::Message("openai embedding count mismatch".into()));
        }
        Ok(out)
    }
}

/// Voyage AI embeddings (`voyage-code-3` or `voyage-3`).
#[derive(Debug)]
pub struct VoyageEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dim: usize,
}

impl VoyageEmbedder {
    pub fn new(api_key: impl Into<String>, model: Option<&str>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.unwrap_or("voyage-code-3").to_string(),
            dim: 1024,
        }
    }
}

#[derive(Deserialize)]
struct VoyageEmbResp {
    data: Vec<VoyageEmbData>,
}

#[derive(Deserialize)]
struct VoyageEmbData {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedder for VoyageEmbedder {
    fn id(&self) -> &str {
        "voyage"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let body = json!({
            "model": self.model,
            "input": texts,
        });
        let resp = self
            .client
            .post("https://api.voyageai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| EmbedderError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            return Err(EmbedderError::Http(format!("voyage embeddings {}", t)));
        }
        let parsed: VoyageEmbResp =
            resp.json().await.map_err(|e| EmbedderError::Http(e.to_string()))?;
        let mut out = Vec::with_capacity(parsed.data.len());
        for row in parsed.data {
            if row.embedding.len() != self.dim {
                return Err(EmbedderError::DimMismatch {
                    expected: self.dim,
                    got: row.embedding.len(),
                });
            }
            out.push(row.embedding);
        }
        if out.len() != texts.len() {
            return Err(EmbedderError::Message("voyage embedding count mismatch".into()));
        }
        Ok(out)
    }
}

/// Type-erased embedder used by [`crate::Indexer`].
pub type DynEmbedder = Arc<dyn Embedder>;

pub fn noop_embedder() -> DynEmbedder {
    Arc::new(NoopEmbedder::new(768))
}

/// Instantiate embedder from `.ide/index.toml` + `OPENAI_API_KEY` / `VOYAGE_API_KEY` when needed.
pub fn build_embedder_from_config(cfg: &IndexFile) -> Result<DynEmbedder, EmbedderError> {
    match cfg.embedder.kind.as_str() {
        "noop" => Ok(noop_embedder()),
        "ollama" => Ok(Arc::new(OllamaEmbedder::new(
            cfg.embedder.ollama_base_url.as_deref(),
            Some(cfg.embedder.model.as_str()),
        )?)),
        "openai" => {
            let k = std::env::var("OPENAI_API_KEY").map_err(|_| {
                EmbedderError::Message("set OPENAI_API_KEY for openai embedder".into())
            })?;
            Ok(Arc::new(OpenAiEmbedder::new(k, Some(cfg.embedder.model.as_str()))))
        }
        "voyage" => {
            let k = std::env::var("VOYAGE_API_KEY").map_err(|_| {
                EmbedderError::Message("set VOYAGE_API_KEY for voyage embedder".into())
            })?;
            Ok(Arc::new(VoyageEmbedder::new(k, Some(cfg.embedder.model.as_str()))))
        }
        other => Err(EmbedderError::Message(format!("unknown embedder.kind: {other}"))),
    }
}
