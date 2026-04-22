//! Horizontal buffer tabs (M14).

use editor_workspace::{BufferId, BufferManager};

use crate::chrome::{ChromeQuad, FrameChrome};

/// Logical height of the tab strip.
pub const TAB_STRIP_HEIGHT: f32 = 34.0;
const TAB_MIN_W: f32 = 120.0;
const TAB_MAX_W: f32 = 240.0;
const TAB_PAD: f32 = 10.0;
// VS Code Dark+ palette.
const STRIP_BG: [f32; 4] = [0.176, 0.176, 0.176, 1.0]; // #2d2d2d
const INACTIVE_TAB: [f32; 4] = [0.176, 0.176, 0.176, 1.0]; // #2d2d2d (same as strip)
const ACTIVE_TAB: [f32; 4] = [0.118, 0.118, 0.118, 1.0]; // #1e1e1e (matches editor)
const ACTIVE_TOP_BAR: [f32; 4] = [0.0, 0.48, 0.80, 1.0]; // #007acc
const TAB_SEPARATOR: [f32; 4] = [0.098, 0.098, 0.098, 1.0]; // #191919
const ACTIVE_TEXT: [u8; 3] = [0xff, 0xff, 0xff];
const INACTIVE_TEXT: [u8; 3] = [0x96, 0x96, 0x96];
const CLOSE_DIM: [u8; 3] = [0x7a, 0x7a, 0x7a];

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
        rgba: STRIP_BG,
    });
    // 1px bottom separator under the strip.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y + h - scale.max(1.0),
        width: strip_width_px,
        height: scale.max(1.0),
        rgba: TAB_SEPARATOR,
    });

    let order = buffers.order_oldest_first();
    if order.is_empty() {
        return hits;
    }
    let mut x = origin_x - scroll_x;
    let active = buffers.active();
    let close_w = 24.0 * scale;

    for id in &order {
        let label = tab_label(*id, buffers, &order);
        let mut display: String = label.chars().take(48).collect();
        if buffers.get(*id).map(|s| s.dirty).unwrap_or(false) {
            display = format!("● {display}");
        }
        let w = (display.chars().count() as f32 * 7.2 * scale + TAB_PAD * 2.0 * scale + close_w)
            .clamp(TAB_MIN_W * scale, TAB_MAX_W * scale);
        let is_active = active == Some(*id);
        let tab_bg = if is_active { ACTIVE_TAB } else { INACTIVE_TAB };
        chrome.push_quad(ChromeQuad { left: x, top: origin_y, width: w, height: h, rgba: tab_bg });
        if is_active {
            chrome.push_quad(ChromeQuad {
                left: x,
                top: origin_y,
                width: w,
                height: 2.0 * scale,
                rgba: ACTIVE_TOP_BAR,
            });
        } else {
            // Subtle 1px separator on the right edge to delimit inactive tabs.
            chrome.push_quad(ChromeQuad {
                left: x + w - scale.max(1.0),
                top: origin_y + 4.0 * scale,
                width: scale.max(1.0),
                height: h - 8.0 * scale,
                rgba: TAB_SEPARATOR,
            });
        }
        let text_rgb = if is_active { ACTIVE_TEXT } else { INACTIVE_TEXT };
        chrome.push_line(x + TAB_PAD * scale, origin_y + 10.0 * scale, display, text_rgb);
        let cx0 = x + w - close_w;
        chrome.push_line(cx0 + 8.0 * scale, origin_y + 9.0 * scale, "×", CLOSE_DIM);
        hits.push(TabHit { id: *id, x0: x, x1: x + w - close_w, close_x0: cx0, close_x1: x + w });
        x += w;
    }
    hits
}
