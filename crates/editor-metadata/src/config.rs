//! `.ide/metadata.toml` — summarizer selection.

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use crate::api_summarizer::ApiSummarizer;
use crate::error::SummarizerError;
use crate::ollama_summarizer::OllamaSummarizer;
use crate::summarizer::{NoopSummarizer, Summarizer};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MetadataFileConfig {
    #[serde(default)]
    pub summarizer: SummarizerSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SummarizerSection {
    /// `noop` | `ollama` | `api`
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub ollama_base_url: Option<String>,
    #[serde(default)]
    pub api_base_url: Option<String>,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_true")]
    pub update_on_commit: bool,
}

fn default_kind() -> String {
    "noop".into()
}

fn default_max_concurrent() -> usize {
    2
}

fn default_true() -> bool {
    true
}

impl Default for SummarizerSection {
    fn default() -> Self {
        Self {
            kind: default_kind(),
            provider: None,
            model: None,
            ollama_base_url: None,
            api_base_url: None,
            max_concurrent: default_max_concurrent(),
            update_on_commit: default_true(),
        }
    }
}

#[must_use]
pub fn load_metadata_config(workspace_root: &Path) -> MetadataFileConfig {
    let p = workspace_root.join(".ide").join("metadata.toml");
    let raw = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => return MetadataFileConfig::default(),
    };
    toml::from_str(&raw).unwrap_or_default()
}

/// Build summarizer + concurrency from config and optional API key override.
pub fn summarizer_from_config(
    cfg: &SummarizerSection,
) -> Result<Arc<dyn Summarizer>, SummarizerError> {
    summarizer_from_config_with_key(cfg, std::env::var("EDITOR_SUMMARIZER_API_KEY").ok().as_deref())
}

pub fn summarizer_from_config_with_key(
    cfg: &SummarizerSection,
    api_key_override: Option<&str>,
) -> Result<Arc<dyn Summarizer>, SummarizerError> {
    match cfg.kind.as_str() {
        "noop" => Ok(Arc::new(NoopSummarizer)),
        "ollama" => {
            let m = cfg.model.as_deref().unwrap_or(OllamaSummarizer::default_model());
            Ok(Arc::new(OllamaSummarizer::new(cfg.ollama_base_url.as_deref(), m)?))
        }
        "api" => {
            let prov = cfg.provider.as_deref().unwrap_or("openai");
            let model = cfg.model.clone().unwrap_or_else(|| "gpt-4o-mini".into());
            match prov {
                "anthropic" => {
                    let key = api_key_override.ok_or_else(|| {
                        SummarizerError::Message(
                            "set EDITOR_SUMMARIZER_API_KEY for api summarizer".into(),
                        )
                    })?;
                    Ok(Arc::new(ApiSummarizer::anthropic(cfg.api_base_url.as_deref(), key, model)?))
                }
                _ => {
                    let key = api_key_override.ok_or_else(|| {
                        SummarizerError::Message(
                            "set EDITOR_SUMMARIZER_API_KEY for api summarizer".into(),
                        )
                    })?;
                    let base = cfg.api_base_url.as_deref().unwrap_or("https://api.openai.com");
                    Ok(Arc::new(ApiSummarizer::openai_compatible(base, key, model)?))
                }
            }
        }
        other => Err(SummarizerError::Message(format!("unknown summarizer.kind: {other}"))),
    }
}
