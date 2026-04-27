//! `editor-ui` — minimal UI composition (gutter, status bar in V2).
//!
//! This crate stays free of GPU and windowing types. It owns layout for chrome
//! around the text canvas and will consume snapshots from `editor-core` in
//! later missions.

#![forbid(unsafe_code)]

pub mod activity_bar;
pub mod agent_panel;
pub mod breadcrumbs;
pub mod chrome;
pub mod chrome_layout;
pub mod command_palette;
pub mod diff_panel;
pub mod find_bar;
pub mod gutter_marks;
pub mod icons;
pub mod keybindings;
pub mod quick_open;
pub mod quick_open_palette;
pub mod scrollbar;
pub mod search_panel;
pub mod settings_panel;
pub mod sidebar;
pub mod status_bar;
pub mod tab_strip;
pub mod terminal_header;
pub mod text_fit;
pub mod theme;

pub use activity_bar::{paint_activity_bar, ActivityIcon, ACTIVITY_BAR_WIDTH};
pub use agent_panel::{
    wrap_text, AgentPanel, AgentPanelHits, AgentSession, AgentSessionStatus, AgentTabHit,
    ChatDisplayMsg, ChatDisplayRole, ContextChip, AGENT_PANEL_MAX_WIDTH, AGENT_PANEL_MIN_WIDTH,
    AGENT_PANEL_WIDTH,
};
pub use breadcrumbs::{paint_breadcrumbs, BreadcrumbHit, BREADCRUMBS_HEIGHT};
pub use chrome::{ChromeQuad, ChromeTextLine, FrameChrome};
pub use chrome_layout::{
    build_chrome_tree, compute_main_chrome_layout, format_main_chrome_layout_golden,
    main_chrome_to_layout_result, ChromeWidgetId, MainChromeLayout, MainChromeParams,
};
pub use command_palette::{CommandEntry, CommandPalette};
pub use diff_panel::{DiffLine, DiffLineKind, DiffPanel};
pub use editor_layout::{LayoutItem, LayoutResult};
pub use find_bar::FindBar;
pub use gutter_marks::{compute_gutter_marks, paint_gutter_marks, GutterMark};
pub use icons::{paint_icon, Icon};
pub use quick_open::QuickOpenRanker;
pub use quick_open_palette::QuickOpenPalette;
pub use scrollbar::{paint_scrollbar, ScrollbarInput, ScrollbarMetrics, SCROLLBAR_WIDTH};
pub use search_panel::SearchPanel;
pub use settings_panel::{
    format_settings_overlay, AboutStrings, Section, SettingsPanelState, SkillRow,
};
pub use sidebar::{FlatRow, Sidebar, SidebarGitStatus, DEFAULT_SIDEBAR_WIDTH, ROW_LINE_HEIGHT};
pub use status_bar::{SourceEncoding, StatusBarInfo, StatusBarInfoRef, StatusBarLayout};
pub use tab_strip::{paint_tab_strip, tab_label, TabHit, TAB_STRIP_HEIGHT};
pub use terminal_header::{paint_terminal_header, TerminalHeaderHits, TERMINAL_HEADER_HEIGHT};
pub use theme::{palette, spacing, typography};

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-ui v{VERSION}")
}

/// Top menu strip (IDE, File, …, search pill).
mod title_bar {
    use super::chrome::{ChromeQuad, FrameChrome};
    use super::icons::{paint_icon, Icon};
    use super::theme::palette as pal;

    /// Logical height of the title / menu / search bar (unscaled).
    pub const TITLE_BAR_HEIGHT: f32 = 34.0;

    const LEFT_PAD: f32 = 12.0;
    const MENU_GAP: f32 = 14.0;
    const SEARCH_PILL_H: f32 = 24.0;
    const RIGHT_ICON_GAP: f32 = 8.0;

    fn ui_icon_rgba() -> [f32; 4] {
        [
            pal::AGENT_HEADER_FG[0] as f32 / 255.0,
            pal::AGENT_HEADER_FG[1] as f32 / 255.0,
            pal::AGENT_HEADER_FG[2] as f32 / 255.0,
            1.0,
        ]
    }

    /// Paint at **y = 0**. `search_text` — active file basename; empty → placeholder.
    pub fn paint_title_bar(
        chrome: &mut FrameChrome,
        scale: f32,
        window_w_px: f32,
        height_px: f32,
        search_text: &str,
    ) {
        if height_px < 0.5 || window_w_px < 1.0 {
            return;
        }
        chrome.push_quad(ChromeQuad {
            left: 0.0,
            top: 0.0,
            width: window_w_px,
            height: height_px,
            rgba: pal::TAB_STRIP_BG,
        });
        let hair = scale.max(1.0);
        chrome.push_quad(ChromeQuad {
            left: 0.0,
            top: height_px - hair,
            width: window_w_px,
            height: hair,
            rgba: pal::TAB_SEPARATOR,
        });

        let mid_y = (height_px - 9.0 * scale) / 2.0;
        let y_center = height_px / 2.0;
        let title_clip = [0.0, 0.0, window_w_px, height_px];
        let mut x = LEFT_PAD * scale;
        chrome.push_line_clipped(x, mid_y, "IDE".to_string(), pal::ACCENT_TEXT, title_clip);
        x += 30.0 * scale;
        chrome.push_line_clipped(x, mid_y, "|".to_string(), pal::SIDEBAR_HEADER_FG, title_clip);
        x += 10.0 * scale;

        for label in ["File", "Edit", "View", "Go", "Run"] {
            chrome.push_line_clipped(x, mid_y, label.to_string(), pal::SIDEBAR_ROW_FG, title_clip);
            x += label.len() as f32 * 6.2 * scale + MENU_GAP * scale;
        }

        let pill_w = 300.0 * scale;
        let pill_h = SEARCH_PILL_H * scale;
        let pill_x = (window_w_px - pill_w) / 2.0;
        let pill_y = (height_px - pill_h) / 2.0;
        chrome.push_quad(ChromeQuad {
            left: pill_x,
            top: pill_y,
            width: pill_w,
            height: pill_h,
            rgba: pal::AGENT_INPUT_BG,
        });
        // Inner top highlight (soft depth on the search field).
        chrome.push_quad(ChromeQuad {
            left: pill_x + hair,
            top: pill_y + hair,
            width: pill_w - 2.0 * hair,
            height: hair,
            rgba: pal::rgba_u8(0xff, 0xff, 0xff, 0x0a),
        });
        chrome.push_quad(ChromeQuad {
            left: pill_x,
            top: pill_y + pill_h - hair,
            width: pill_w,
            height: hair,
            rgba: pal::AGENT_BORDER,
        });
        // Magnifier in pill (left).
        let search_icon_sz = 12.0 * scale;
        paint_icon(
            chrome,
            Icon::Search,
            pill_x + 10.0 * scale + search_icon_sz * 0.5,
            y_center,
            search_icon_sz,
            ui_icon_rgba(),
        );
        let display = if search_text.is_empty() {
            "Search or jump to file\u{2026}".to_string()
        } else {
            search_text.chars().take(40).collect()
        };
        let fg = if search_text.is_empty() { pal::AGENT_HEADER_FG } else { pal::EDITOR_FG };
        let pill_clip = [pill_x, pill_y, pill_x + pill_w, pill_y + pill_h];
        chrome.push_line_clipped(
            pill_x + 30.0 * scale,
            pill_y + (pill_h - 9.0 * scale) / 2.0,
            display,
            fg,
            pill_clip,
        );

        // Right: primary surfaces + user (rect icons).
        let ico = ui_icon_rgba();
        let sz = 15.0 * scale;
        let mut rx = window_w_px - 10.0 * scale - sz * 0.5;
        // Left-to-right: explorer → settings → chat → user (right edge).
        for kind in [Icon::User, Icon::Chat, Icon::Settings, Icon::Explorer] {
            paint_icon(chrome, kind, rx, y_center, sz, ico);
            rx -= sz + RIGHT_ICON_GAP * scale;
        }
    }
}

pub use title_bar::paint_title_bar;
pub use title_bar::TITLE_BAR_HEIGHT;

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
