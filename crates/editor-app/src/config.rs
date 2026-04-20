//! Cross-platform persisted session state (`state.json` under app config dir).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
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
    pub version: u32,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            last_file: None,
            last_cursor_byte: None,
            last_scroll_y: None,
            window_size: None,
            window_pos: None,
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
            version: STATE_SCHEMA_VERSION,
        };
        write_atomic(&path, serde_json::to_string(&s).unwrap().as_bytes()).unwrap();
        let read = fs::read_to_string(&path).unwrap();
        let got: PersistedState = serde_json::from_str(&read).unwrap();
        assert_eq!(got.last_cursor_byte, Some(42));
        assert_eq!(got.window_size, Some((800, 600)));
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
}
