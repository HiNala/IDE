//! `.ide/index.toml` — embedder and feature flags.

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IndexFile {
    #[serde(default)]
    pub index: IndexSection,
    #[serde(default)]
    pub embedder: EmbedderSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexSection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub index_code: bool,
    #[serde(default = "default_true")]
    pub index_sidecars: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedderSection {
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_batch")]
    pub batch_size: usize,
    #[serde(default)]
    pub ollama_base_url: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_kind() -> String {
    "ollama".into()
}

fn default_model() -> String {
    "nomic-embed-text".into()
}

fn default_batch() -> usize {
    16
}

impl Default for IndexSection {
    fn default() -> Self {
        Self { enabled: true, index_code: true, index_sidecars: true }
    }
}

impl Default for EmbedderSection {
    fn default() -> Self {
        Self {
            kind: default_kind(),
            model: default_model(),
            batch_size: default_batch(),
            ollama_base_url: None,
        }
    }
}

#[must_use]
pub fn load_index_config(workspace_root: &Path) -> IndexFile {
    let p = workspace_root.join(".ide").join("index.toml");
    let Ok(raw) = std::fs::read_to_string(&p) else {
        return IndexFile::default();
    };
    toml::from_str(&raw).unwrap_or_default()
}
