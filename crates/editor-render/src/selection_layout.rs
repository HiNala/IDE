//! Pixel rectangles covering a byte range on visible lines (selection highlight).

use std::cmp::min;

use editor_core::ScrollOffset;
use ropey::Rope;
use winit::dpi::PhysicalSize;

/// Premultiplied-friendly RGBA for selection quads (passed through to [`crate::solid_quads::SolidQuadLayer`]).
pub const SELECTION_FILL_RGBA: [f32; 4] = [0.25, 0.55, 0.95, 0.22];

/// Search hit highlight (drawn under selection).
#[allow(dead_code)] // Used by `search_match_rects_pixels_into` (optional find overlay path).
pub const SEARCH_MATCH_FILL_RGBA: [f32; 4] = [0.95, 0.72, 0.18, 0.14];

/// Active search hit (current match).
#[allow(dead_code)]
pub const SEARCH_CURRENT_FILL_RGBA: [f32; 4] = [0.98, 0.78, 0.22, 0.35];

/// Allocates a fresh vec (tests only; the frame loop uses [`selection_rects_pixels_into`]).
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn selection_rects_pixels(
    rope: &Rope,
    sel_lo: usize,
    sel_hi: usize,
    scroll: ScrollOffset,
    physical_size: PhysicalSize<u32>,
    status_h: f32,
    line_h: f32,
    gutter_w: f32,
    char_w: f32,
) -> Vec<(f32, f32, f32, f32, [f32; 4])> {
    let mut out = Vec::new();
    selection_rects_pixels_into(
        &mut out,
        rope,
        sel_lo,
        sel_hi,
        scroll,
        physical_size,
        status_h,
        0.0,
        line_h,
        gutter_w,
        char_w,
        0.0,
        0.0,
    );
    out
}

/// Fills `out` with `(left, top, right, bottom, rgba)` in window pixels (Y-down). Clears `out` first.
///
/// Reuse `out` across frames to avoid allocating a fresh [`Vec`] every redraw (M05).
/// The main compositor stacks backdrop + search + selection and uses [`append_range_rects`] for
/// selection so this helper is optional; kept for tests and embedders.
#[allow(clippy::too_many_arguments)] // Layout needs viewport, scroll, font metrics together.
pub fn selection_rects_pixels_into(
    out: &mut Vec<(f32, f32, f32, f32, [f32; 4])>,
    rope: &Rope,
    sel_lo: usize,
    sel_hi: usize,
    scroll: ScrollOffset,
    physical_size: PhysicalSize<u32>,
    status_h: f32,
    terminal_pane_height_px: f32,
    line_h: f32,
    gutter_w: f32,
    char_w: f32,
    content_inset_left_px: f32,
    content_inset_top_px: f32,
) {
    out.clear();
    if sel_lo >= sel_hi {
        return;
    }
    let cap_hint = {
        let vh = physical_size.height as f32;
        let first = (scroll.y_px / line_h).floor().max(0.0) as usize;
        let visible = ((vh - status_h - terminal_pane_height_px.max(0.0)).max(1.0) / line_h).ceil()
            as usize
            + 2;
        let last = min(first + visible, rope.len_lines());
        last.saturating_sub(first)
    };
    if cap_hint > 0 {
        out.reserve(cap_hint);
    }
    append_range_rects(
        out,
        rope,
        sel_lo,
        sel_hi,
        scroll,
        physical_size,
        status_h,
        terminal_pane_height_px,
        line_h,
        gutter_w,
        char_w,
        content_inset_left_px,
        content_inset_top_px,
        SELECTION_FILL_RGBA,
    );
}

/// Non-current search matches (amber, low opacity).
#[allow(dead_code)] // Optional find overlay; default frame path uses selection + diff only.
#[allow(clippy::too_many_arguments)]
pub fn search_match_rects_pixels_into(
    out: &mut Vec<(f32, f32, f32, f32, [f32; 4])>,
    rope: &Rope,
    ranges: &[(usize, usize)],
    current: Option<usize>,
    scroll: ScrollOffset,
    physical_size: PhysicalSize<u32>,
    status_h: f32,
    terminal_pane_height_px: f32,
    line_h: f32,
    gutter_w: f32,
    char_w: f32,
    content_inset_left_px: f32,
    content_inset_top_px: f32,
) {
    for (i, &(lo, hi)) in ranges.iter().enumerate() {
        if lo >= hi {
            continue;
        }
        let rgba =
            if Some(i) == current { SEARCH_CURRENT_FILL_RGBA } else { SEARCH_MATCH_FILL_RGBA };
        append_range_rects(
            out,
            rope,
            lo,
            hi,
            scroll,
            physical_size,
            status_h,
            terminal_pane_height_px,
            line_h,
            gutter_w,
            char_w,
            content_inset_left_px,
            content_inset_top_px,
            rgba,
        );
    }
}

/// Append rectangles for one UTF-8 byte range without clearing `out` (stack with search / backdrop).
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_range_rects(
    out: &mut Vec<(f32, f32, f32, f32, [f32; 4])>,
    rope: &Rope,
    sel_lo: usize,
    sel_hi: usize,
    scroll: ScrollOffset,
    physical_size: PhysicalSize<u32>,
    status_h: f32,
    terminal_pane_height_px: f32,
    line_h: f32,
    gutter_w: f32,
    char_w: f32,
    content_inset_left_px: f32,
    content_inset_top_px: f32,
    rgba: [f32; 4],
) {
    if sel_lo >= sel_hi {
        return;
    }
    let vw = physical_size.width as f32;
    let vh = physical_size.height as f32;
    let term_h = terminal_pane_height_px.max(0.0);
    let max_content_y = vh - status_h - term_h;

    let total_lines = rope.len_lines();
    let first = (scroll.y_px / line_h).floor().max(0.0) as usize;
    let visible = ((vh - status_h - term_h).max(1.0) / line_h).ceil() as usize + 2;
    let last = min(first + visible, total_lines);

    for line_idx in first..last {
        let line_start = rope.line_to_byte(line_idx);
        let line_end = if line_idx + 1 < total_lines {
            rope.line_to_byte(line_idx + 1)
        } else {
            rope.len_bytes()
        };
        let a = sel_lo.max(line_start);
        let b = sel_hi.min(line_end);
        if a >= b {
            continue;
        }

        let col0 = a - line_start;
        let col1 = b - line_start;
        let left = content_inset_left_px + gutter_w + 8.0 + col0 as f32 * char_w;
        let mut right = content_inset_left_px + gutter_w + 8.0 + col1 as f32 * char_w;
        right = right.min(vw);
        if right <= left + 0.5 {
            continue;
        }

        let top = content_inset_top_px + line_idx as f32 * line_h - scroll.y_px + 4.0;
        let bottom = (top + line_h).min(max_content_y);

        if top >= max_content_y {
            continue;
        }
        out.push((left, top, right, bottom, rgba));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn rects_cover_first_line_slice() {
        let rope = Rope::from_str("abcdef\n");
        let mut rects = Vec::new();
        selection_rects_pixels_into(
            &mut rects,
            &rope,
            1,
            4,
            ScrollOffset { y_px: 0.0 },
            PhysicalSize::new(640, 480),
            0.0,
            0.0,
            20.0,
            32.0,
            8.4,
            0.0,
            0.0,
        );
        assert_eq!(rects.len(), 1);
        let (l, t, r, b, _) = rects[0];
        assert!(l < r && t < b);
        assert!(l > 32.0);
    }

    #[test]
    fn allocating_wrapper_matches_into() {
        let rope = Rope::from_str("abcdef\n");
        let scroll = ScrollOffset { y_px: 0.0 };
        let size = PhysicalSize::new(640, 480);
        let allocated = selection_rects_pixels(&rope, 1, 4, scroll, size, 0.0, 20.0, 32.0, 8.4);
        let mut into_vec = Vec::new();
        selection_rects_pixels_into(
            &mut into_vec,
            &rope,
            1,
            4,
            scroll,
            size,
            0.0,
            0.0,
            20.0,
            32.0,
            8.4,
            0.0,
            0.0,
        );
        assert_eq!(allocated, into_vec);
    }
}
