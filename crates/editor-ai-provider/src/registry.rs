//! Load enabled providers from [`ProviderConfig`](crate::config::ProviderConfig) and secrets.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use url::Url;

use crate::anthropic::AnthropicProvider;
use crate::config::ProviderConfig;
use crate::custom::CustomHttpProvider;
use crate::error::ProviderError;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAiProvider;
use crate::provider::AiProvider;
use crate::rate_limit::MinuteRateLimit;
use crate::secrets::SecretStore;

#[derive(Debug, Clone, thiserror::Error)]
pub enum RegistryError {
    #[error("unknown provider id `{0}`")]
    Unknown(String),
}

/// Holds constructed providers and tracks which id is active.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
    active_id: Option<String>,
}

impl fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .field("active_id", &self.active_id)
            .finish()
    }
}

impl ProviderRegistry {
    /// Build all enabled providers (skips entries that need missing API keys with a warning).
    pub fn from_config(
        config: &ProviderConfig,
        secrets: &SecretStore,
    ) -> Result<Self, ProviderError> {
        let mut providers: HashMap<String, Arc<dyn AiProvider>> = HashMap::new();
        let rate = MinuteRateLimit::new(config.limits.rate_limit_per_minute);

        if config.providers.anthropic.enabled {
            match secrets.get_key("anthropic")? {
                Some(key) => {
                    let p = AnthropicProvider::new(
                        key,
                        config.providers.anthropic.base_url.as_deref().and_then(|s| s.parse().ok()),
                        rate.clone(),
                    )?;
                    providers.insert("anthropic".into(), Arc::new(p));
                }
                None => {
                    tracing::warn!(
                        "anthropic enabled in config but no API key in keychain/env — skipping"
                    );
                }
            }
        }

        if config.providers.openai.enabled {
            match secrets.get_key("openai")? {
                Some(key) => {
                    let p = OpenAiProvider::new(
                        key,
                        config.providers.openai.base_url.as_deref().and_then(|s| s.parse().ok()),
                        rate.clone(),
                    )?;
                    providers.insert("openai".into(), Arc::new(p));
                }
                None => {
                    tracing::warn!("openai enabled in config but no API key — skipping");
                }
            }
        }

        if config.providers.ollama.enabled {
            let p = OllamaProvider::new(
                Some(config.providers.ollama.base_url.parse().map_err(|e: url::ParseError| {
                    ProviderError::message(format!("ollama base_url: {e}"))
                })?),
                rate.clone(),
            )?;
            providers.insert("ollama".into(), Arc::new(p));
        }

        for (name, entry) in &config.providers.custom {
            if !entry.enabled {
                continue;
            }
            let account = format!("custom:{name}");
            let key = secrets.get_key(&account)?;
            let url: Url = entry.base_url.parse().map_err(|e: url::ParseError| {
                ProviderError::message(format!("custom {name} base_url: {e}"))
            })?;
            let p = CustomHttpProvider::new(name, url, key, entry.models.clone(), rate.clone())?;
            providers.insert(format!("custom:{name}"), Arc::new(p) as Arc<dyn AiProvider>);
        }

        let active_key = normalize_active_key(&config.active.provider)?;
        let active_id = if providers.contains_key(&active_key) {
            Some(active_key)
        } else {
            tracing::warn!(
                active = %config.active.provider,
                "active provider from config not loaded — set a key or enable the provider"
            );
            providers.keys().next().cloned()
        };

        Ok(Self { providers, active_id })
    }

    pub fn active(&self) -> Option<Arc<dyn AiProvider>> {
        let id = self.active_id.as_ref()?;
        self.providers.get(id).cloned()
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn set_active(&mut self, id: &str) -> Result<(), RegistryError> {
        if self.providers.contains_key(id) {
            self.active_id = Some(id.into());
            return Ok(());
        }
        Err(RegistryError::Unknown(id.into()))
    }

    pub fn list(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.providers.keys().map(|s| s.as_str()).collect();
        v.sort_unstable();
        v
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }
}

fn normalize_active_key(raw: &str) -> Result<String, ProviderError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(ProviderError::config("active.provider is empty"));
    }
    if s.contains(':') || matches!(s, "anthropic" | "openai" | "ollama") {
        Ok(s.into())
    } else {
        Ok(format!("custom:{s}"))
    }
}
