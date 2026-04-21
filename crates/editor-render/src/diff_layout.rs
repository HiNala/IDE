//! Translucent quads for inline diff (M17): line tints + intra-line spans.

use std::cmp::min;

use editor_core::ScrollOffset;
use editor_diff::{DiffGutter, InlineDiffLine};
use ropey::Rope;
use winit::dpi::PhysicalSize;

/// Full-line delete / old-side tint (≈ rgb 255,80,80 @ 25% alpha).
const DIFF_RED: [f32; 4] = [1.0, 0.31, 0.31, 0.25];
/// Stronger red for intra-line deletes.
const DIFF_RED_STRONG: [f32; 4] = [1.0, 0.22, 0.22, 0.42];
/// Full-line insert / new-side tint (≈ rgb 80,255,128 @ 25% alpha).
const DIFF_GREEN: [f32; 4] = [0.31, 1.0, 0.5, 0.25];
const DIFF_GREEN_STRONG: [f32; 4] = [0.22, 0.95, 0.4, 0.42];

#[allow(clippy::too_many_arguments)]
pub fn inline_diff_quads_into(
    out: &mut Vec<(f32, f32, f32, f32, [f32; 4])>,
    rope: &Rope,
    line_meta: &[InlineDiffLine],
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
    let vw = physical_size.width as f32;
    let vh = physical_size.height as f32;
    let term_h = terminal_pane_height_px.max(0.0);
    let max_content_y = vh - status_h - term_h;
    let total_lines = rope.len_lines();
    let first = (scroll.y_px / line_h).floor().max(0.0) as usize;
    let visible = ((vh - status_h - term_h).max(1.0) / line_h).ceil() as usize + 2;
    let last = min(first + visible, total_lines);

    let body_left = content_inset_left_px + gutter_w + 8.0;

    for line_idx in first..last {
        let Some(meta) = line_meta.get(line_idx) else {
            continue;
        };
        let (base, strong_del, strong_ins) = match meta.gutter {
            DiffGutter::Neutral => continue,
            DiffGutter::Delete => (DIFF_RED, DIFF_RED_STRONG, DIFF_GREEN_STRONG),
            DiffGutter::Insert => (DIFF_GREEN, DIFF_RED_STRONG, DIFF_GREEN_STRONG),
            DiffGutter::ReplaceOld => (DIFF_RED, DIFF_RED_STRONG, DIFF_GREEN_STRONG),
            DiffGutter::ReplaceNew => (DIFF_GREEN, DIFF_RED_STRONG, DIFF_GREEN_STRONG),
        };

        let top = content_inset_top_px + line_idx as f32 * line_h - scroll.y_px + 4.0;
        let bottom = (top + line_h).min(max_content_y);
        if top >= max_content_y || bottom <= top + 0.5 {
            continue;
        }

        // Full-line wash
        out.push((body_left, top, vw, bottom, base));

        let _line_start = rope.line_to_byte(line_idx);
        for r in &meta.delete_spans {
            let left = body_left + r.start as f32 * char_w;
            let right = (body_left + r.end as f32 * char_w).min(vw);
            if right > left + 0.5 {
                out.push((left, top, right, bottom, strong_del));
            }
        }
        for r in &meta.insert_spans {
            let left = body_left + r.start as f32 * char_w;
            let right = (body_left + r.end as f32 * char_w).min(vw);
            if right > left + 0.5 {
                out.push((left, top, right, bottom, strong_ins));
            }
        }
    }
}
