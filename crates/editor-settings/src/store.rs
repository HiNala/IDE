//! Load/save with atomic replace; debounced save scheduled by the app frame loop.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use std::collections::hash_map::Entry;

use tracing::info;

use crate::error::SettingsError;
use crate::paths::settings_file_path;
use crate::schema::{Settings, SettingsExport, SETTINGS_SCHEMA_VERSION};

/// Session fields from legacy `state.json` merged when creating fresh `settings.toml`.
#[derive(Debug, Clone, Default)]
pub struct LegacySessionMerge {
    pub undo_coalesce_ms: Option<u64>,
    pub skills_disabled: Vec<String>,
    pub extra_skill_dirs: Vec<std::path::PathBuf>,
}

impl LegacySessionMerge {
    #[must_use]
    pub fn from_persisted(
        undo_coalesce_ms: u64,
        skills_disabled: &[String],
        extra_skill_dirs: &[std::path::PathBuf],
    ) -> Self {
        Self {
            undo_coalesce_ms: Some(undo_coalesce_ms),
            skills_disabled: skills_disabled.to_vec(),
            extra_skill_dirs: extra_skill_dirs.to_vec(),
        }
    }
}

/// Owns in-memory settings and coordinates throttled disk writes.
#[derive(Debug)]
pub struct SettingsStore {
    settings: Settings,
    path: Option<PathBuf>,
    /// When set, disk write should happen no earlier than this instant.
    save_after: Option<Instant>,
}

const DEBOUNCE: Duration = Duration::from_millis(200);

impl SettingsStore {
    /// Load from default path, apply migrations, or create defaults and persist once.
    ///
    /// When a new `settings.toml` is written (`dirty`), optional `legacy` merges undo and skill
    /// fields from legacy `state.json` (M28 migration).
    #[must_use]
    pub fn load_or_create(legacy: Option<&LegacySessionMerge>) -> Self {
        let path = settings_file_path();
        let (mut settings, dirty) = match &path {
            Some(p) if p.exists() => match load_from_path(p) {
                Ok(s) => migrate_with_flag(s),
                Err(e) => {
                    info!(error = %e, ?p, "settings: load failed, using defaults");
                    (Settings::default(), true)
                }
            },
            Some(_p) => (Settings::default(), true),
            None => (Settings::default(), false),
        };
        if dirty {
            if let Some(leg) = legacy {
                merge_legacy(&mut settings, leg);
            }
        }
        let mut s = Self { settings, path, save_after: None };
        if dirty {
            if let Err(e) = s.save_immediate() {
                tracing::warn!(error = %e, "settings: initial save failed");
            }
        }
        s
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        self.schedule_save();
        &mut self.settings
    }

    pub fn replace(&mut self, settings: Settings) -> Result<(), SettingsError> {
        self.settings = migrate(settings);
        self.save_immediate()
    }

    /// Mark a future save (call after direct `settings` mutation if needed).
    pub fn schedule_save(&mut self) {
        self.save_after = Some(Instant::now() + DEBOUNCE);
    }

    /// Call from the run loop each frame; performs at most one flush when debounce elapsed.
    pub fn flush_pending_save(&mut self) -> Result<(), SettingsError> {
        if self.save_after.is_none() {
            return Ok(());
        }
        if Instant::now() < self.save_after.expect("checked") {
            return Ok(());
        }
        self.save_after = None;
        self.save_immediate()
    }

    pub fn save_immediate(&mut self) -> Result<(), SettingsError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        write_atomic(
            path,
            &toml::to_string_pretty(&self.settings).map_err(SettingsError::TomlSerialize)?,
        )
    }

    pub fn export_json_to_path(&self, path: &Path) -> Result<(), SettingsError> {
        let exp = SettingsExport::from(&self.settings);
        let json = serde_json::to_string_pretty(&exp)?;
        write_atomic(path, &json)?;
        Ok(())
    }

    pub fn import_json_from_path(&mut self, path: &Path) -> Result<(), SettingsError> {
        let data = fs::read_to_string(path)
            .map_err(|e| SettingsError::Io { path: path.to_path_buf(), source: e })?;
        let exp: SettingsExport = serde_json::from_str(&data)?;
        validate_import(&exp)?;
        self.settings = migrate(exp.into());
        self.save_immediate()
    }
}

fn validate_import(exp: &SettingsExport) -> Result<(), SettingsError> {
    if exp.version > SETTINGS_SCHEMA_VERSION {
        return Err(SettingsError::Import(format!(
            "unsupported settings version {} (max {})",
            exp.version, SETTINGS_SCHEMA_VERSION
        )));
    }
    if !(8.0..=32.0).contains(&exp.editor.font_size) {
        return Err(SettingsError::Import(format!(
            "editor.font_size out of range: {}",
            exp.editor.font_size
        )));
    }
    if exp.terminal.scrollback_lines == 0 || exp.terminal.scrollback_lines > 1_000_000 {
        return Err(SettingsError::Import("terminal.scrollback_lines out of range".into()));
    }
    Ok(())
}

fn migrate_with_flag(mut s: Settings) -> (Settings, bool) {
    let mut dirty = false;
    let prev = s.version;
    match s.version {
        0..=SETTINGS_SCHEMA_VERSION => {
            s.version = SETTINGS_SCHEMA_VERSION;
        }
        _ => {
            info!(have = s.version, "settings: unknown version; clamping to current");
            s.version = SETTINGS_SCHEMA_VERSION;
        }
    }
    if prev != SETTINGS_SCHEMA_VERSION {
        dirty = true;
    }
    // Ensure all known providers exist (add missing keys with defaults from empty map entries).
    let defaults = Settings::default();
    for (k, v) in defaults.ai.providers {
        match s.ai.providers.entry(k) {
            Entry::Vacant(e) => {
                e.insert(v);
                dirty = true;
            }
            Entry::Occupied(_) => {}
        }
    }
    (s, dirty)
}

fn migrate(s: Settings) -> Settings {
    migrate_with_flag(s).0
}

fn merge_legacy(settings: &mut Settings, leg: &LegacySessionMerge) {
    if let Some(ms) = leg.undo_coalesce_ms {
        settings.editor.undo_coalesce_ms = ms;
    }
    for name in &leg.skills_disabled {
        settings.skills.disabled.insert(name.clone());
    }
    if settings.extra_skill_dirs.is_empty() && !leg.extra_skill_dirs.is_empty() {
        settings.extra_skill_dirs = leg.extra_skill_dirs.clone();
    }
}

fn load_from_path(path: &Path) -> Result<Settings, SettingsError> {
    let data = fs::read_to_string(path)
        .map_err(|e| SettingsError::Io { path: path.to_path_buf(), source: e })?;
    let s: Settings = toml::from_str(&data)?;
    Ok(s)
}

fn write_atomic(path: &Path, text: impl AsRef<[u8]>) -> Result<(), SettingsError> {
    let data = text.as_ref();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| SettingsError::Io { path: dir.to_path_buf(), source: e })?;
    }
    let tmp = path.with_extension("part");
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| SettingsError::Io { path: tmp.clone(), source: e })?;
        f.write_all(data).map_err(|e| SettingsError::Io { path: tmp.clone(), source: e })?;
        f.sync_all().map_err(|e| SettingsError::Io { path: tmp.clone(), source: e })?;
    }
    fs::rename(&tmp, path)
        .map_err(|e| SettingsError::Io { path: path.to_path_buf(), source: e })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trip_toml() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("settings.toml");
        let s = Settings::default();
        write_atomic(&p, toml::to_string_pretty(&s).unwrap().as_bytes()).unwrap();
        let read = fs::read_to_string(&p).unwrap();
        let got: Settings = toml::from_str(&read).unwrap();
        assert_eq!(got.version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(got.editor.font_size, 14.0);
    }

    #[test]
    fn load_or_create_defaults() {
        let _ = SettingsStore::load_or_create(None);
    }
}
