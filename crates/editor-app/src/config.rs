//! Cross-platform persisted session state (`state.json` under app config dir).

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use editor_skills::SkillPersistence;
use serde::{Deserialize, Serialize};
use tracing::info;

const STATE_SCHEMA_VERSION: u32 = 1;

/// User session fields restored on next launch (M10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub last_file: Option<PathBuf>,
    pub last_cursor_byte: Option<u64>,
    pub last_scroll_y: Option<f32>,
    pub window_size: Option<(u32, u32)>,
    pub window_pos: Option<(i32, i32)>,
    /// When `true`, do not cap redraw rate on battery (M12).
    #[serde(default)]
    pub power_uncap_on_battery: bool,
    /// Last seen scale factor per monitor name (winit `MonitorHandle::name`) for multi-monitor (M12).
    #[serde(default)]
    pub monitor_scale_by_name: HashMap<String, f32>,
    /// Undo coalescing window for rapid typing (ms); default matches [`editor_core::undo::COALESCE_MS`].
    #[serde(default = "default_undo_coalesce_ms")]
    pub undo_coalesce_ms: u64,
    /// Agent skill ids the user turned off (M27); all others stay enabled by default.
    #[serde(default)]
    pub skills_disabled: Vec<String>,
    /// Extra directories scanned for `*/SKILL.md` after built-in and user-global paths (M27).
    #[serde(default)]
    pub extra_skill_dirs: Vec<PathBuf>,
    /// Sidebar width in logical pixels (M14).
    #[serde(default)]
    pub sidebar_width: Option<f32>,
    /// Whether the project sidebar is visible (M14).
    #[serde(default)]
    pub sidebar_visible: Option<bool>,
    pub version: u32,
}

fn default_undo_coalesce_ms() -> u64 {
    editor_core::undo::COALESCE_MS
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            last_file: None,
            last_cursor_byte: None,
            last_scroll_y: None,
            window_size: None,
            window_pos: None,
            power_uncap_on_battery: false,
            monitor_scale_by_name: HashMap::new(),
            undo_coalesce_ms: editor_core::undo::COALESCE_MS,
            skills_disabled: Vec::new(),
            extra_skill_dirs: Vec::new(),
            sidebar_width: None,
            sidebar_visible: None,
            version: STATE_SCHEMA_VERSION,
        }
    }
}

fn project_state_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "HiNala", "IDE").map(|p| p.config_dir().join("state.json"))
}

impl PersistedState {
    /// Loads `state.json` or returns default on any I/O or parse error.
    #[must_use]
    pub fn load() -> Self {
        let Some(path) = project_state_path() else {
            info!("persisted state: no config directory (directories crate)");
            return Self::default();
        };
        let data = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                info!(error = %e, ?path, "persisted state: read failed, using defaults");
                return Self::default();
            }
        };
        match serde_json::from_str::<PersistedState>(&data) {
            Ok(mut s) => {
                if s.version != STATE_SCHEMA_VERSION {
                    info!(
                        have = s.version,
                        want = STATE_SCHEMA_VERSION,
                        "persisted state: schema mismatch, resetting"
                    );
                    return Self::default();
                }
                s.version = STATE_SCHEMA_VERSION;
                s
            }
            Err(e) => {
                info!(error = %e, "persisted state: parse failed, using defaults");
                Self::default()
            }
        }
    }

    /// Atomic write: temp file in same directory, `sync_all`, rename.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = project_state_path() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "no config directory for persisted state",
            ));
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        write_atomic(&path, json.as_bytes())
    }

    /// Maps persisted fields into [`SkillPersistence`] for [`editor_skills::SkillRegistry::load`].
    #[must_use]
    #[allow(dead_code)] // Used in unit tests; skills registry wiring is not in the binary yet.
    pub fn skill_persistence(&self) -> SkillPersistence {
        SkillPersistence {
            disabled: self.skills_disabled.iter().cloned().collect(),
            extra_dirs: self.extra_skill_dirs.clone(),
        }
    }

    /// Enable or disable a skill by id (M28 settings panel will call this).
    #[allow(dead_code)] // Used in unit tests; settings UI will call this when shipped.
    pub fn set_skill_enabled(&mut self, name: &str, enabled: bool) {
        if enabled {
            self.skills_disabled.retain(|n| n != name);
        } else if !self.skills_disabled.iter().any(|n| n == name) {
            self.skills_disabled.push(name.to_string());
        }
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trip_state() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let s = PersistedState {
            last_file: Some(PathBuf::from("/tmp/x.rs")),
            last_cursor_byte: Some(42),
            last_scroll_y: Some(12.5),
            window_size: Some((800, 600)),
            window_pos: Some((10, 20)),
            power_uncap_on_battery: false,
            monitor_scale_by_name: HashMap::new(),
            undo_coalesce_ms: editor_core::undo::COALESCE_MS,
            skills_disabled: vec!["writing-python".into()],
            extra_skill_dirs: Vec::new(),
            sidebar_width: None,
            sidebar_visible: None,
            version: STATE_SCHEMA_VERSION,
        };
        write_atomic(&path, serde_json::to_string(&s).unwrap().as_bytes()).unwrap();
        let read = fs::read_to_string(&path).unwrap();
        let got: PersistedState = serde_json::from_str(&read).unwrap();
        assert_eq!(got.last_cursor_byte, Some(42));
        assert_eq!(got.window_size, Some((800, 600)));
        assert_eq!(got.skills_disabled, vec!["writing-python".to_string()]);
        let sp = got.skill_persistence();
        assert!(sp.disabled.contains("writing-python"));
    }

    #[test]
    fn corrupt_file_falls_back() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, b"{not json").unwrap();
        let read = fs::read_to_string(&path);
        assert!(read.is_ok());
        assert!(serde_json::from_str::<PersistedState>(&read.unwrap()).is_err());
    }

    #[test]
    fn set_skill_enabled_round_trips_disable_list() {
        let mut s = PersistedState::default();
        s.set_skill_enabled("using-git", false);
        assert!(s.skills_disabled.iter().any(|n| n == "using-git"));
        s.set_skill_enabled("using-git", true);
        assert!(!s.skills_disabled.iter().any(|n| n == "using-git"));
    }
}
