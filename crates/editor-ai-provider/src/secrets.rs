//! OS keychain storage for provider credentials (never log key material).

use keyring::Entry;

use crate::error::{KeyringFailure, ProviderError};

/// Persists API keys via the [`keyring`](https://docs.rs/keyring/) crate (`ide` / per-provider account).
#[derive(Debug, Clone)]
pub struct SecretStore {
    service_name: String,
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore {
    pub fn new() -> Self {
        Self { service_name: "ide".into() }
    }

    fn entry(&self, account: &str) -> Result<Entry, ProviderError> {
        Entry::new(&self.service_name, account)
            .map_err(|e| ProviderError::message(format!("keyring entry: {e}")))
    }

    /// Store a secret for `provider` (e.g. `"anthropic"`, `"openai"`, or `"custom:my-vllm"`).
    pub fn set_key(&self, account: &str, value: &str) -> Result<(), ProviderError> {
        self.entry(account)?
            .set_password(value)
            .map_err(|e| ProviderError::message(format!("keyring set: {e}")))
    }

    /// Read a secret; on missing entry, fall back to environment variables when applicable.
    pub fn get_key(&self, account: &str) -> Result<Option<String>, ProviderError> {
        match self.entry(account)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(env_fallback(account)),
            Err(e) => {
                tracing::warn!(error = %e, %account, "keyring read failed; trying environment");
                Ok(env_fallback(account))
            }
        }
    }

    pub fn delete_key(&self, account: &str) -> Result<(), ProviderError> {
        match self.entry(account)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(ProviderError::message(format!("keyring delete: {e}"))),
        }
    }

    /// Same as [`Self::get_key`] but reports whether env fallback was used (for user-visible hints).
    pub fn get_key_detail(
        &self,
        account: &str,
    ) -> Result<(Option<String>, Option<KeyringFailure>), ProviderError> {
        match self.entry(account)?.get_password() {
            Ok(v) => Ok((Some(v), None)),
            Err(keyring::Error::NoEntry) => {
                let v = env_fallback(account);
                if v.is_some() {
                    Ok((
                        v,
                        Some(KeyringFailure {
                            message: "No OS keychain entry; used environment variable.".into(),
                            used_env_fallback: true,
                        }),
                    ))
                } else {
                    Ok((None, None))
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, %account, "keyring read failed; trying environment");
                let v = env_fallback(account);
                let hint = Some(KeyringFailure {
                    message: format!("keyring: {e}"),
                    used_env_fallback: v.is_some(),
                });
                Ok((v, hint))
            }
        }
    }
}

fn env_fallback(account: &str) -> Option<String> {
    if let Some(rest) = account.strip_prefix("custom:") {
        let slug = rest
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .to_ascii_uppercase();
        let key = format!("IDE_CUSTOM_{slug}_API_KEY");
        return std::env::var(&key).ok();
    }
    let key = match account {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        "ollama" => return None,
        _ => return None,
    };
    std::env::var(key).ok()
}
