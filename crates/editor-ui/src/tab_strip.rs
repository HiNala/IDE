//! Horizontal buffer tabs (M14).

use editor_workspace::{BufferId, BufferManager};

use crate::chrome::{ChromeQuad, FrameChrome};

/// Logical height of the tab strip.
pub const TAB_STRIP_HEIGHT: f32 = 32.0;
const TAB_MIN_W: f32 = 120.0;
const TAB_MAX_W: f32 = 240.0;
const TAB_PAD: f32 = 8.0;
const INACTIVE_TAB: [f32; 4] = [0.12, 0.12, 0.14, 1.0];
const ACTIVE_TAB: [f32; 4] = [0.16, 0.17, 0.22, 1.0];
const ACTIVE_TOP_BAR: [f32; 4] = [0.35, 0.55, 0.95, 1.0];

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

/// Paint tabs (oldest left) and return hit regions.
pub fn paint_tab_strip(
    chrome: &mut FrameChrome,
    buffers: &BufferManager,
    scale: f32,
    origin_x: f32,
    origin_y: f32,
    scroll_x: f32,
) -> Vec<TabHit> {
    let mut hits = Vec::new();
    let order = buffers.order_oldest_first();
    if order.is_empty() {
        return hits;
    }
    let h = TAB_STRIP_HEIGHT * scale;
    let mut x = origin_x - scroll_x;
    let active = buffers.active();
    let close_w = 20.0 * scale;

    for id in &order {
        let label = tab_label(*id, buffers, &order);
        let mut display: String = label.chars().take(48).collect();
        if buffers.get(*id).map(|s| s.dirty).unwrap_or(false) {
            display = format!("● {display}");
        }
        let w = (display.chars().count() as f32 * 7.2 * scale + TAB_PAD * 2.0 * scale)
            .clamp(TAB_MIN_W * scale, TAB_MAX_W * scale);
        let is_active = active == Some(*id);
        let tab_bg = if is_active { ACTIVE_TAB } else { INACTIVE_TAB };
        chrome.push_quad(ChromeQuad { left: x, top: origin_y, width: w, height: h, rgba: tab_bg });
        if is_active {
            chrome.push_quad(ChromeQuad {
                left: x,
                top: origin_y,
                width: w,
                height: 3.0 * scale,
                rgba: ACTIVE_TOP_BAR,
            });
        }
        chrome.push_line(x + 8.0 * scale, origin_y + 8.0 * scale, display, [0xe4, 0xe4, 0xe8]);
        let cx0 = x + w - close_w;
        chrome.push_line(cx0 + 5.0 * scale, origin_y + 6.0 * scale, "×", [0xa8, 0xa8, 0xb0]);
        hits.push(TabHit { id: *id, x0: x, x1: x + w - close_w, close_x0: cx0, close_x1: x + w });
        x += w + 2.0 * scale;
    }
    hits
}
