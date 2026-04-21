//! `editor-ui` — minimal UI composition (gutter, status bar in V2).
//!
//! This crate stays free of GPU and windowing types. It owns layout for chrome
//! around the text canvas and will consume snapshots from `editor-core` in
//! later missions.

#![forbid(unsafe_code)]

pub mod chrome;
pub mod find_bar;
pub mod keybindings;
pub mod quick_open;
pub mod quick_open_palette;
pub mod search_panel;
pub mod settings_panel;
pub mod sidebar;
pub mod status_bar;
pub mod tab_strip;

pub use chrome::{ChromeQuad, ChromeTextLine, FrameChrome};
pub use find_bar::FindBar;
pub use quick_open::QuickOpenRanker;
pub use quick_open_palette::QuickOpenPalette;
pub use search_panel::SearchPanel;
pub use settings_panel::{
    format_settings_overlay, AboutStrings, Section, SettingsPanelState, SkillRow,
};
pub use sidebar::{FlatRow, Sidebar, DEFAULT_SIDEBAR_WIDTH, ROW_LINE_HEIGHT};
pub use status_bar::{SourceEncoding, StatusBarInfo, StatusBarInfoRef, StatusBarLayout};
pub use tab_strip::{paint_tab_strip, tab_label, TabHit, TAB_STRIP_HEIGHT};

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-ui v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-ui v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }
}
