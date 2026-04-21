//! Platform config path: `<config_dir>/ide/settings.toml` (mission: human-readable alongside `providers.toml`).

use std::path::PathBuf;

#[must_use]
pub fn settings_file_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|b| b.config_dir().join("ide").join("settings.toml"))
}
