//! User settings: TOML on disk (`~/.config/ide/settings.toml`), JSON import/export without secrets.
//!
//! API keys are never part of this document; the app stores them in the OS keychain (M19/M28).

#![forbid(unsafe_code)]

pub mod error;
pub mod paths;
pub mod schema;
mod store;

pub use error::SettingsError;
pub use schema::{
    AiSettings, EditorSettings, LineEndingPreference, ProviderConfig, Settings, SettingsExport,
    SkillsSettings, TerminalSettings, KNOWN_PROVIDER_KEYS, SETTINGS_SCHEMA_VERSION,
};
pub use store::{LegacySessionMerge, SettingsStore};
