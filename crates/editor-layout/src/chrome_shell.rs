//! Single source of truth for main IDE shell geometry (physical pixels).
//!
//! Callers pass logical dimensions and visibility flags; [`compute_main_chrome_layout`]
//! returns every major band used by `editor-app` for painting and hit testing.
//! Kept as pure math (no taffy) for determinism; taffy-based trees can be layered
//! in a later pass if we need flex/grid for sub-panes.

use crate::{LayoutItem, LayoutRect, LayoutResult};

/// Stable ids for [`LayoutItem::widget_id`] (main shell; extended in later missions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChromeWidgetId;

impl ChromeWidgetId {
    pub const TITLE_BAR: u64 = 1;
    pub const SIDEBAR: u64 = 2;
    pub const TAB_STRIP: u64 = 3;
    pub const BREADCRUMBS: u64 = 4;
    pub const EDITOR_VIEWPORT: u64 = 5;
    pub const AGENT_PANEL: u64 = 6;
    pub const TERMINAL_PANE: u64 = 7;
    pub const STATUS_BAR: u64 = 8;
}

/// Inputs for a frame — all `*_logical` values are **unscaled** CSS-like px;
/// `*_px` are already in physical pixels where noted.
#[derive(Debug, Clone, Copy)]
pub struct MainChromeParams {
    pub window_width_px: f32,
    pub window_height_px: f32,
    pub scale: f32,
    pub title_bar_height_logical: f32,
    pub tab_strip_height_logical: f32,
    pub breadcrumbs_height_logical: f32,
    pub show_tab_strip: bool,
    pub show_breadcrumbs: bool,
    pub activity_bar_width_logical: f32,
    pub sidebar_width_logical: f32,
    pub sidebar_visible: bool,
    /// Logical width of the agent rail (`AgentPanel::width`); ignored if hidden.
    pub agent_width_logical: f32,
    pub agent_panel_visible: bool,
    pub status_bar_height_px: f32,
    pub terminal_pane_height_px: f32,
}

/// Laid-out shell regions in **physical pixels** (window coordinates, origin top-left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainChromeLayout {
    pub title_h: f32,
    pub tab_strip_h: f32,
    pub breadcrumbs_h: f32,
    pub activity_w: f32,
    pub sidebar_w: f32,
    /// Activity + expanded sidebar; x origin for tab strip and editor body.
    pub inset_left_px: f32,
    pub agent_w: f32,
    /// Y of the tab strip (under the title bar).
    pub tab_strip_y: f32,
    /// Y of the breadcrumb band (under the tab strip when present).
    pub breadcrumbs_y: f32,
    /// Top of the editor / scrollbar / diff marks region (below breadcrumbs).
    pub inset_top_px: f32,
    /// Column height for sidebar: `window - status - terminal - title`.
    pub main_column_h: f32,
    /// Width available for the tab / breadcrumb strips (editor center column only).
    pub editor_strip_width: f32,
    pub agent_panel_left: f32,
    pub agent_panel_top: f32,
    pub agent_panel_height: f32,
    /// Bottom y of the editor/scrollbar **viewport** (stops above the bottom terminal when visible).
    pub content_bottom_px: f32,
    pub status_h: f32,
    pub term_h: f32,
    /// `window_h - status_h - title_h` region height above status (includes terminal band on the left).
    pub column_h: f32,
}

/// Compute the full main-window chrome layout. Matches `EditorApp::build_frame_chrome` math.
#[must_use]
pub fn compute_main_chrome_layout(p: &MainChromeParams) -> MainChromeLayout {
    let s = p.scale;
    let title_h = p.title_bar_height_logical * s;
    let tab_strip_h = if p.show_tab_strip { p.tab_strip_height_logical * s } else { 0.0 };
    let breadcrumbs_h = if p.show_breadcrumbs { p.breadcrumbs_height_logical * s } else { 0.0 };
    let activity_w = p.activity_bar_width_logical * s;
    let sidebar_w = if p.sidebar_visible { p.sidebar_width_logical * s } else { 0.0 };
    let inset_left_px = activity_w + sidebar_w;
    let agent_w = if p.agent_panel_visible { p.agent_width_logical * s } else { 0.0 };
    let tab_strip_y = title_h;
    let breadcrumbs_y = title_h + tab_strip_h;
    let inset_top_px = breadcrumbs_y + breadcrumbs_h;
    let status_h = p.status_bar_height_px;
    let term_h = p.terminal_pane_height_px;
    let w = p.window_width_px;
    let h = p.window_height_px;
    let column_h = (h - status_h - term_h).max(1.0);
    let main_column_h = (column_h - title_h).max(1.0);
    let editor_strip_width = (w - inset_left_px - agent_w).max(0.0);
    let agent_panel_left = w - agent_w;
    let agent_panel_top = title_h;
    let agent_panel_height = (h - status_h - title_h).max(1.0);
    let content_bottom_px = (h - status_h - term_h).max(inset_top_px);
    MainChromeLayout {
        title_h,
        tab_strip_h,
        breadcrumbs_h,
        activity_w,
        sidebar_w,
        inset_left_px,
        agent_w,
        tab_strip_y,
        breadcrumbs_y,
        inset_top_px,
        main_column_h,
        editor_strip_width,
        agent_panel_left,
        agent_panel_top,
        agent_panel_height,
        content_bottom_px,
        status_h,
        term_h,
        column_h,
    }
}

/// Alias for mission/docs naming (`build_chrome_tree` → layout result).
#[inline]
#[must_use]
pub fn build_chrome_tree(p: &MainChromeParams) -> MainChromeLayout {
    compute_main_chrome_layout(p)
}

/// Convert shell geometry to flat [`LayoutResult`] for hit testing / paint order.
/// Regions omit zero-sized strips (e.g. tab strip when `show_tab_strip` is false).
#[must_use]
pub fn main_chrome_to_layout_result(p: &MainChromeParams) -> LayoutResult {
    let l = compute_main_chrome_layout(p);
    let w = p.window_width_px;
    let h = p.window_height_px;
    let mut items = Vec::new();

    items.push(LayoutItem {
        widget_id: ChromeWidgetId::TITLE_BAR,
        rect: LayoutRect::from_xywh(0.0, 0.0, w, l.title_h),
    });

    if l.sidebar_w > 0.5 {
        items.push(LayoutItem {
            widget_id: ChromeWidgetId::SIDEBAR,
            rect: LayoutRect::from_xywh(l.activity_w, l.title_h, l.sidebar_w, l.main_column_h),
        });
    }

    if l.tab_strip_h > 0.5 {
        items.push(LayoutItem {
            widget_id: ChromeWidgetId::TAB_STRIP,
            rect: LayoutRect::from_xywh(
                l.inset_left_px,
                l.tab_strip_y,
                l.editor_strip_width,
                l.tab_strip_h,
            ),
        });
    }

    if l.breadcrumbs_h > 0.5 {
        items.push(LayoutItem {
            widget_id: ChromeWidgetId::BREADCRUMBS,
            rect: LayoutRect::from_xywh(
                l.inset_left_px,
                l.breadcrumbs_y,
                l.editor_strip_width,
                l.breadcrumbs_h,
            ),
        });
    }

    let editor_body_h = (l.content_bottom_px - l.inset_top_px).max(0.0);
    items.push(LayoutItem {
        widget_id: ChromeWidgetId::EDITOR_VIEWPORT,
        rect: LayoutRect::from_xywh(
            l.inset_left_px,
            l.inset_top_px,
            l.editor_strip_width,
            editor_body_h,
        ),
    });

    if l.agent_w > 0.5 {
        items.push(LayoutItem {
            widget_id: ChromeWidgetId::AGENT_PANEL,
            rect: LayoutRect::from_xywh(
                l.agent_panel_left,
                l.agent_panel_top,
                l.agent_w,
                l.agent_panel_height,
            ),
        });
    }

    if l.term_h > 0.5 {
        let pane_top = h - l.status_h - l.term_h;
        let pane_w = (w - l.inset_left_px - l.agent_w).max(0.0);
        items.push(LayoutItem {
            widget_id: ChromeWidgetId::TERMINAL_PANE,
            rect: LayoutRect::from_xywh(l.inset_left_px, pane_top, pane_w, l.term_h),
        });
    }

    items.push(LayoutItem {
        widget_id: ChromeWidgetId::STATUS_BAR,
        rect: LayoutRect::from_xywh(0.0, h - l.status_h, w, l.status_h),
    });

    LayoutResult { items, root_width: w, root_height: h }
}

/// Deterministic multiline string for checked-in layout goldens (see `editor-ui` tests).
#[must_use]
pub fn format_main_chrome_layout_golden(l: &MainChromeLayout) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(&mut s, "title_h={:.2}", l.title_h);
    let _ = writeln!(&mut s, "tab_strip_h={:.2}", l.tab_strip_h);
    let _ = writeln!(&mut s, "breadcrumbs_h={:.2}", l.breadcrumbs_h);
    let _ = writeln!(&mut s, "activity_w={:.2}", l.activity_w);
    let _ = writeln!(&mut s, "sidebar_w={:.2}", l.sidebar_w);
    let _ = writeln!(&mut s, "inset_left_px={:.2}", l.inset_left_px);
    let _ = writeln!(&mut s, "agent_w={:.2}", l.agent_w);
    let _ = writeln!(&mut s, "tab_strip_y={:.2}", l.tab_strip_y);
    let _ = writeln!(&mut s, "breadcrumbs_y={:.2}", l.breadcrumbs_y);
    let _ = writeln!(&mut s, "inset_top_px={:.2}", l.inset_top_px);
    let _ = writeln!(&mut s, "main_column_h={:.2}", l.main_column_h);
    let _ = writeln!(&mut s, "editor_strip_width={:.2}", l.editor_strip_width);
    let _ = writeln!(&mut s, "agent_panel_left={:.2}", l.agent_panel_left);
    let _ = writeln!(&mut s, "agent_panel_top={:.2}", l.agent_panel_top);
    let _ = writeln!(&mut s, "agent_panel_height={:.2}", l.agent_panel_height);
    let _ = writeln!(&mut s, "content_bottom_px={:.2}", l.content_bottom_px);
    let _ = writeln!(&mut s, "status_h={:.2}", l.status_h);
    let _ = writeln!(&mut s, "term_h={:.2}", l.term_h);
    let _ = writeln!(&mut s, "column_h={:.2}", l.column_h);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p1920() -> MainChromeParams {
        MainChromeParams {
            window_width_px: 1920.0,
            window_height_px: 1080.0,
            scale: 1.0,
            title_bar_height_logical: 34.0,
            tab_strip_height_logical: 32.0,
            breadcrumbs_height_logical: 24.0,
            show_tab_strip: true,
            show_breadcrumbs: true,
            activity_bar_width_logical: 0.0,
            sidebar_width_logical: 220.0,
            sidebar_visible: true,
            agent_width_logical: 360.0,
            agent_panel_visible: true,
            status_bar_height_px: 24.0,
            terminal_pane_height_px: 200.0,
        }
    }

    #[test]
    fn snapshot_1920x1080_typical() {
        let l = compute_main_chrome_layout(&p1920());
        assert_eq!(l.title_h, 34.0);
        assert_eq!(l.tab_strip_h, 32.0);
        assert_eq!(l.breadcrumbs_h, 24.0);
        assert_eq!(l.inset_left_px, 220.0);
        assert_eq!(l.agent_w, 360.0);
        assert_eq!(l.breadcrumbs_y, 34.0 + 32.0);
        assert_eq!(l.inset_top_px, 34.0 + 32.0 + 24.0);
        assert!((l.editor_strip_width - (1920.0 - 220.0 - 360.0)).abs() < 0.01);
        assert!((l.main_column_h - (1080.0 - 24.0 - 200.0 - 34.0)).abs() < 0.01);
        assert!((l.agent_panel_height - (1080.0 - 24.0 - 34.0)).abs() < 0.01);
    }

    #[test]
    fn snapshot_960x600() {
        let l = compute_main_chrome_layout(&MainChromeParams {
            window_width_px: 960.0,
            window_height_px: 600.0,
            scale: 1.0,
            title_bar_height_logical: 34.0,
            tab_strip_height_logical: 32.0,
            breadcrumbs_height_logical: 24.0,
            show_tab_strip: true,
            show_breadcrumbs: true,
            activity_bar_width_logical: 0.0,
            sidebar_width_logical: 220.0,
            sidebar_visible: true,
            agent_width_logical: 360.0,
            agent_panel_visible: true,
            status_bar_height_px: 24.0,
            terminal_pane_height_px: 160.0,
        });
        assert!((l.inset_top_px - 90.0).abs() < 0.01);
        assert!((l.editor_strip_width - (960.0 - 220.0 - 360.0)).abs() < 0.01);
    }

    #[test]
    fn main_chrome_layout_result_widget_count() {
        let p = p1920();
        let r = main_chrome_to_layout_result(&p);
        // title, sidebar, tab, breadcrumbs, editor, agent, terminal, status
        assert_eq!(r.items.len(), 8, "{:?}", r.items);
    }

    #[test]
    fn snapshot_2560x1440() {
        let l = compute_main_chrome_layout(&MainChromeParams {
            window_width_px: 2560.0,
            window_height_px: 1440.0,
            scale: 1.0,
            title_bar_height_logical: 34.0,
            tab_strip_height_logical: 32.0,
            breadcrumbs_height_logical: 24.0,
            show_tab_strip: true,
            show_breadcrumbs: true,
            activity_bar_width_logical: 0.0,
            sidebar_width_logical: 220.0,
            sidebar_visible: true,
            agent_width_logical: 360.0,
            agent_panel_visible: true,
            status_bar_height_px: 24.0,
            terminal_pane_height_px: 220.0,
        });
        assert!((l.editor_strip_width - (2560.0 - 220.0 - 360.0)).abs() < 0.01);
        assert!((l.content_bottom_px - (1440.0 - 24.0 - 220.0)).abs() < 0.01);
    }
}
