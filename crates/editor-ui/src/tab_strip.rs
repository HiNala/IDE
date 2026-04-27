//! Horizontal buffer tabs (M14).

use editor_workspace::{BufferId, BufferManager};

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::icons::{paint_icon, Icon};
use crate::text_fit;
use crate::theme::palette;

/// Logical height of the tab strip.
pub const TAB_STRIP_HEIGHT: f32 = 32.0;
const TAB_MIN_W: f32 = 120.0;
const TAB_MAX_W: f32 = 240.0;
const TAB_PAD: f32 = 10.0;
const CLOSE_ICON_SIZE: f32 = 12.0;
/// Leading status dot (active = accent; inactive = status hints).
const STATUS_TAB_DOT: f32 = 6.0;

/// Hit regions for a frame (for mouse routing).
#[derive(Debug, Clone)]
pub struct TabHit {
    pub id: BufferId,
    pub x0: f32,
    pub x1: f32,
    pub close_x0: f32,
    pub close_x1: f32,
}

/// Display label with duplicate basename disambiguation.
pub fn tab_label(id: BufferId, buffers: &BufferManager, order: &[BufferId]) -> String {
    let st = match buffers.get(id) {
        Some(s) => s,
        None => return "?".into(),
    };
    let path = st.path.as_ref();
    let base = path
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".into());
    let names: Vec<String> = order
        .iter()
        .filter_map(|oid| buffers.get(*oid).and_then(|x| x.path.as_ref()))
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned())
        .collect();
    let dup = names.iter().filter(|n| *n == &base).count() > 1;
    if dup {
        path.and_then(|p| {
            p.parent().and_then(|parent| {
                parent.file_name().map(|d| format!("{} ({})", base, d.to_string_lossy()))
            })
        })
        .unwrap_or(base)
    } else {
        base
    }
}

/// Paint the strip background plus the tabs (oldest left) and return hit regions.
/// `strip_width_px` should cover the full area from `origin_x` to the window's right edge.
pub fn paint_tab_strip(
    chrome: &mut FrameChrome,
    buffers: &BufferManager,
    scale: f32,
    origin_x: f32,
    origin_y: f32,
    scroll_x: f32,
    strip_width_px: f32,
) -> Vec<TabHit> {
    let mut hits = Vec::new();
    let h = TAB_STRIP_HEIGHT * scale;

    // Always paint the strip background so the empty right side doesn't leak editor bg.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y,
        width: strip_width_px,
        height: h,
        rgba: palette::TAB_STRIP_BG,
    });
    // 1px bottom separator under the strip.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y + h - scale.max(1.0),
        width: strip_width_px,
        height: scale.max(1.0),
        rgba: palette::TAB_SEPARATOR,
    });

    let order = buffers.order_oldest_first();
    if order.is_empty() {
        return hits;
    }
    let mut x = origin_x - scroll_x;
    let active = buffers.active();
    let close_w = 24.0 * scale;
    let dot_pad = (STATUS_TAB_DOT + 6.0) * scale;
    let mut inactive_ordinal: usize = 0;

    let fixed = TAB_PAD * 2.0 * scale + dot_pad + close_w;
    let max_inner = (TAB_MAX_W * scale) - fixed;
    for id in &order {
        let label = tab_label(*id, buffers, &order);
        let is_active = active == Some(*id);
        // Fit label to the maximum tab body first, then re-fit if the min width clamps.
        let mut display = text_fit::ellipsize_mono(&label, max_inner, scale, 7.2);
        let mut w = (display.chars().count() as f32 * 7.2 * scale + fixed)
            .clamp(TAB_MIN_W * scale, TAB_MAX_W * scale);
        let inner2 = w - fixed;
        display = text_fit::ellipsize_mono(&label, inner2, scale, 7.2);
        w = (display.chars().count() as f32 * 7.2 * scale + fixed)
            .clamp(TAB_MIN_W * scale, TAB_MAX_W * scale);
        let tab_bg = if is_active { palette::TAB_ACTIVE_BG } else { palette::TAB_INACTIVE_BG };
        chrome.push_quad(ChromeQuad { left: x, top: origin_y, width: w, height: h, rgba: tab_bg });
        if is_active {
            // Cursor / VS Code style: accent line along the bottom of the active tab.
            chrome.push_quad(ChromeQuad {
                left: x,
                top: origin_y + h - 2.0 * scale,
                width: w,
                height: 2.0 * scale,
                rgba: palette::ACCENT_BLUE,
            });
        } else {
            // Subtle 1px separator on the right edge to delimit inactive tabs.
            chrome.push_quad(ChromeQuad {
                left: x + w - scale.max(1.0),
                top: origin_y + 4.0 * scale,
                width: scale.max(1.0),
                height: h - 8.0 * scale,
                rgba: palette::TAB_SEPARATOR,
            });
        }
        let text_rgb = if is_active { palette::TAB_ACTIVE_FG } else { palette::TAB_INACTIVE_FG };
        let mut text_x = x + TAB_PAD * scale;
        // Status dot: active = primary accent; inactive = alternate secondary hints.
        let rgba = if is_active {
            palette::ACCENT_BLUE
        } else {
            // Warm / cool alternation so inactive tabs don’t mirror the active purple dot.
            let c = if inactive_ordinal.is_multiple_of(2) {
                palette::DIFF_MODIFIED
            } else {
                palette::DIFF_ADDED
            };
            inactive_ordinal += 1;
            c
        };
        {
            let cx = text_x + STATUS_TAB_DOT * scale / 2.0;
            let cy = origin_y + h / 2.0;
            paint_icon(chrome, Icon::Dot, cx, cy, STATUS_TAB_DOT * scale, rgba);
        }
        text_x += dot_pad;
        let tab_clip = [x, origin_y, x + w, origin_y + h];
        chrome.push_line_clipped(text_x, origin_y + 10.0 * scale, display, text_rgb, tab_clip);
        let cx0 = x + w - close_w;
        // Close glyph: centered X drawn from rects (no Unicode).
        let rgb = palette::TAB_CLOSE_DIM;
        paint_icon(
            chrome,
            Icon::Close,
            cx0 + close_w / 2.0,
            origin_y + h / 2.0,
            CLOSE_ICON_SIZE * scale,
            [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0],
        );
        hits.push(TabHit { id: *id, x0: x, x1: x + w - close_w, close_x0: cx0, close_x1: x + w });
        x += w;
    }
    hits
}
