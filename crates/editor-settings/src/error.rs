//! Errors from load/save/import.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("IO error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse settings TOML: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("failed to serialize settings: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("failed to parse settings JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("settings import validation failed: {0}")]
    Import(String),
}
