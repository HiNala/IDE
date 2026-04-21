//! `providers.toml` under the OS config directory (`~/.config/ide/` on Linux, etc.).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ProviderError;

/// Default relative to [`providers_config_path`](providers_config_path); same contents on all OSes except path.
pub const DEFAULT_PROVIDERS_TOML: &str = r#"
[active]
provider = "anthropic"
model = "claude-opus-4-7"

[providers.anthropic]
enabled = true

[providers.openai]
enabled = true

[providers.ollama]
enabled = true
base_url = "http://localhost:11434"

[limits]
max_tokens_default = 4096
rate_limit_per_minute = 60
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSection {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicSection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
}

impl Default for AnthropicSection {
    fn default() -> Self {
        Self { enabled: true, base_url: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiSection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
}

impl Default for OpenAiSection {
    fn default() -> Self {
        Self { enabled: true, base_url: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaSection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_ollama_base")]
    pub base_url: String,
}

fn default_true() -> bool {
    true
}

fn default_ollama_base() -> String {
    "http://localhost:11434".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSection {
    #[serde(default)]
    pub enabled: bool,
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvidersSection {
    #[serde(default)]
    pub anthropic: AnthropicSection,
    #[serde(default)]
    pub openai: OpenAiSection,
    #[serde(default)]
    pub ollama: OllamaSection,
    #[serde(default)]
    pub custom: std::collections::HashMap<String, CustomSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsSection {
    #[serde(default = "default_max_tokens")]
    pub max_tokens_default: u32,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_rate_limit() -> u32 {
    60
}

impl Default for LimitsSection {
    fn default() -> Self {
        Self {
            max_tokens_default: default_max_tokens(),
            rate_limit_per_minute: default_rate_limit(),
        }
    }
}

impl Default for OllamaSection {
    fn default() -> Self {
        Self { enabled: true, base_url: default_ollama_base() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub active: ActiveSection,
    pub providers: ProvidersSection,
    pub limits: LimitsSection,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        toml::from_str(DEFAULT_PROVIDERS_TOML).expect("embedded default TOML must parse")
    }
}

/// `%APPDATA%\\ide\\providers.toml`, `~/Library/Application Support/ide/providers.toml`, or `~/.config/ide/providers.toml`.
pub fn providers_config_path() -> PathBuf {
    if let Some(base) = directories::BaseDirs::new() {
        base.config_dir().join("ide").join("providers.toml")
    } else {
        PathBuf::from("providers.toml")
    }
}

/// Read configuration; create the default file if missing.
pub fn load_or_create_default(path: Option<&Path>) -> Result<ProviderConfig, ProviderError> {
    let path = path.map(|p| p.to_path_buf()).unwrap_or_else(providers_config_path);
    let data = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    ProviderError::config(format!("create config dir {}: {e}", parent.display()))
                })?;
            }
            fs::write(&path, DEFAULT_PROVIDERS_TOML.trim_start()).map_err(|e| {
                ProviderError::config(format!("write default {}: {e}", path.display()))
            })?;
            DEFAULT_PROVIDERS_TOML.to_string()
        }
        Err(e) => {
            return Err(ProviderError::config(format!("read {}: {e}", path.display())));
        }
    };
    toml::from_str(&data).map_err(|e| ProviderError::config(format!("parse TOML: {e}")))
}
