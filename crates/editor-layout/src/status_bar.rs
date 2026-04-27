//! One-row flex layout for the IDE status bar: `min left | flex-1 center | min right`.
//!
//! Widget ids: `0` = root, `1` = left, `2` = center, `3` = right (see [`LayoutResult`](crate::LayoutResult)).

use taffy::prelude::*;

use crate::{LayoutEngine, LayoutError, LayoutResult};

const MIN_CENTER_PX: f32 = 48.0;

/// Lays out a full-width status row with two fixed (min-content) sides and a flexing middle.
/// `left_width` and `right_width` are *desired* intrinsic widths; they are clamped so the
/// center column is at least [`MIN_CENTER_PX`] and the three columns sum to `bar_width`.
#[must_use = "this returns layout rectangles for paint"]
pub fn layout_status_bar_row(
    bar_width: f32,
    bar_height: f32,
    left_intrinsic_width: f32,
    right_intrinsic_width: f32,
) -> Result<LayoutResult, LayoutError> {
    let bar_width = bar_width.max(0.0);
    let bar_height = bar_height.max(0.0);
    // Clamp side columns so the center can breathe (matches UI_STRATEGY §5.1).
    let max_each = ((bar_width - MIN_CENTER_PX) * 0.5).max(0.0);
    let mut lw = left_intrinsic_width.min(max_each);
    let mut rw = right_intrinsic_width.min(max_each);
    let rest = (bar_width - MIN_CENTER_PX).max(0.0);
    if lw + rw > rest {
        let sum = lw + rw;
        if sum > 0.0 {
            lw *= rest / sum;
            rw *= rest / sum;
        } else {
            lw = 0.0;
            rw = 0.0;
        }
    }

    let mut eng = LayoutEngine::new();
    let t = eng.taffy_mut();

    let left = t.new_leaf(Style {
        size: taffy::Size { width: length(lw), height: length(bar_height) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;

    let center = t.new_leaf(Style {
        size: taffy::Size { width: auto(), height: length(bar_height) },
        flex_grow: 1.0,
        min_size: taffy::Size { width: length(0.0), height: auto() },
        ..Default::default()
    })?;

    let right = t.new_leaf(Style {
        size: taffy::Size { width: length(rw), height: length(bar_height) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;

    let root = t.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: taffy::Size { width: length(bar_width), height: length(bar_height) },
            ..Default::default()
        },
        &[left, center, right],
    )?;

    let id_map = vec![(root, 0), (left, 1), (center, 2), (right, 3)];
    eng.compute(root, bar_width, bar_height)?;
    eng.result(&id_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_columns_shrink_before_center_too_narrow() {
        let r = layout_status_bar_row(400.0, 24.0, 200.0, 200.0).expect("layout");
        let l = r.items.iter().find(|i| i.widget_id == 1).expect("L");
        let c = r.items.iter().find(|i| i.widget_id == 2).expect("C");
        let rgt = r.items.iter().find(|i| i.widget_id == 3).expect("R");
        assert!((c.rect.width - MIN_CENTER_PX).abs() < 2.0, "center = {:?}", c.rect);
        // 400 = l + c + r; l ≈ r after symmetric trim
        assert!((l.rect.width - rgt.rect.width).abs() < 1.0, "l={:?} r={:?}", l, rgt);
    }
}
