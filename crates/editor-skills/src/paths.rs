//! User-global skill directories (per M27 platform layout).

use std::path::PathBuf;

/// `%APPDATA%\\ide\\skills`, `~/.config/ide/skills`, or `~/Library/Application Support/ide/skills`.
#[must_use]
pub fn user_global_skills_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("ide").join("skills"))
    } else if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library").join("Application Support").join("ide").join("skills"))
    } else {
        home_dir().map(|h| h.join(".config").join("ide").join("skills"))
    }
}

fn home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
