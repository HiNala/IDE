//! Pixel rectangles covering a byte range on visible lines (selection highlight).

use std::cmp::min;

use editor_core::ScrollOffset;
use ropey::Rope;
use winit::dpi::PhysicalSize;

/// Returns `(left, top, right, bottom)` in window pixels for each strip (Y-down, half-open right/bottom).
#[allow(clippy::too_many_arguments)] // Layout needs viewport, scroll, font metrics together.
pub fn selection_rects_pixels(
    rope: &Rope,
    sel_lo: usize,
    sel_hi: usize,
    scroll: ScrollOffset,
    physical_size: PhysicalSize<u32>,
    status_h: f32,
    line_h: f32,
    gutter_w: f32,
    char_w: f32,
) -> Vec<(f32, f32, f32, f32)> {
    if sel_lo >= sel_hi {
        return Vec::new();
    }
    let vw = physical_size.width as f32;
    let vh = physical_size.height as f32;
    let max_content_y = vh - status_h;

    let total_lines = rope.len_lines();
    let first = (scroll.y_px / line_h).floor().max(0.0) as usize;
    let visible = ((vh - status_h).max(1.0) / line_h).ceil() as usize + 2;
    let last = min(first + visible, total_lines);

    let mut out = Vec::new();

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
        let left = gutter_w + 8.0 + col0 as f32 * char_w;
        let mut right = gutter_w + 8.0 + col1 as f32 * char_w;
        right = right.min(vw);
        if right <= left + 0.5 {
            continue;
        }

        let top = line_idx as f32 * line_h - scroll.y_px + 4.0;
        let bottom = (top + line_h).min(max_content_y);

        if top >= max_content_y {
            continue;
        }
        out.push((left, top, right, bottom));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn rects_cover_first_line_slice() {
        let rope = Rope::from_str("abcdef\n");
        let rects = selection_rects_pixels(
            &rope,
            1,
            4,
            ScrollOffset { y_px: 0.0 },
            PhysicalSize::new(640, 480),
            0.0,
            20.0,
            32.0,
            8.4,
        );
        assert_eq!(rects.len(), 1);
        let (l, t, r, b) = rects[0];
        assert!(l < r && t < b);
        assert!(l > 32.0);
    }
}
