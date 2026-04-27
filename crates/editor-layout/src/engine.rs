//! `TaffyTree` wrapper — keeps raw `NodeId` inside this module.

use taffy::prelude::*;

use crate::{LayoutItem, LayoutRect, LayoutResult};

/// Public error type for layout failures.
pub type LayoutError = taffy::TaffyError;

/// Wraps [`TaffyTree`](taffy::TaffyTree) with our compute + collect API.
pub struct LayoutEngine {
    tree: TaffyTree<()>,
}

impl std::fmt::Debug for LayoutEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutEngine").finish_non_exhaustive()
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    #[must_use]
    pub fn new() -> Self {
        Self { tree: TaffyTree::new() }
    }

    /// Run flex/grid layout from `root` with a definite available size.
    pub fn compute(
        &mut self,
        root: taffy::NodeId,
        available_width: f32,
        available_height: f32,
    ) -> Result<(), LayoutError> {
        self.tree.compute_layout(
            root,
            taffy::Size {
                width: taffy::style::AvailableSpace::Definite(available_width),
                height: taffy::style::AvailableSpace::Definite(available_height),
            },
        )?;
        Ok(())
    }

    /// Low-level access for submodules (e.g. `status_bar`) that build a [`TaffyTree`](taffy::TaffyTree) per call site.
    pub(crate) fn taffy_mut(&mut self) -> &mut TaffyTree<()> {
        &mut self.tree
    }

    /// Collect `(NodeId, widget_id)` pairs into [`LayoutItem`]s with global coordinates.
    pub fn result(&self, node_ids: &[(taffy::NodeId, u64)]) -> Result<LayoutResult, LayoutError> {
        if node_ids.is_empty() {
            return Ok(LayoutResult::default());
        }
        let first = self.tree.layout(node_ids[0].0)?;
        let _root_id = node_ids[0].0;
        let root_width = first.size.width;
        let root_height = first.size.height;
        // First node is assumed to be the root; use its size as the reported root rect.
        let items: Result<Vec<LayoutItem>, _> = node_ids
            .iter()
            .map(|(nid, wid)| {
                let l = self.tree.layout(*nid)?;
                let rect = LayoutRect {
                    x: l.location.x,
                    y: l.location.y,
                    width: l.size.width,
                    height: l.size.height,
                };
                Ok(LayoutItem { widget_id: *wid, rect })
            })
            .collect();
        Ok(LayoutResult { items: items?, root_width, root_height })
    }
}

/// Demo tree: 200px | flex-1 | 100px in an 800×24 root (widget ids 1,2,3 for children; 0 = root).
#[cfg(test)]
fn demo_three_col_row() -> (LayoutEngine, taffy::NodeId, Vec<(taffy::NodeId, u64)>) {
    let mut eng = LayoutEngine::new();
    let left = eng
        .tree
        .new_leaf(Style {
            size: taffy::Size { width: length(200.0), height: length(24.0) },
            ..Default::default()
        })
        .expect("left");

    let center = eng
        .tree
        .new_leaf(Style {
            size: taffy::Size { width: auto(), height: length(24.0) },
            flex_grow: 1.0,
            min_size: taffy::Size { width: length(0.0), height: auto() },
            ..Default::default()
        })
        .expect("center");

    let right = eng
        .tree
        .new_leaf(Style {
            size: taffy::Size { width: length(100.0), height: length(24.0) },
            ..Default::default()
        })
        .expect("right");

    let root = eng
        .tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                size: taffy::Size { width: length(800.0), height: length(24.0) },
                ..Default::default()
            },
            &[left, center, right],
        )
        .expect("root");

    let id_map = vec![(root, 0), (left, 1), (center, 2), (right, 3)];
    (eng, root, id_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_col_flex_row_middle_takes_remainder() {
        let (mut eng, root, id_map) = demo_three_col_row();
        eng.compute(root, 800.0, 24.0).expect("compute");
        let r = eng.result(&id_map).expect("result");
        let left = r.items.iter().find(|i| i.widget_id == 1).expect("left");
        let mid = r.items.iter().find(|i| i.widget_id == 2).expect("mid");
        let right = r.items.iter().find(|i| i.widget_id == 3).expect("right");
        assert!((left.rect.width - 200.0).abs() < 0.5, "left = {:?}", left.rect);
        assert!((right.rect.width - 100.0).abs() < 0.5, "right = {:?}", right.rect);
        assert!((mid.rect.width - 500.0).abs() < 0.5, "mid = {:?}", mid.rect);
        assert!((r.root_width - 800.0).abs() < 0.5);
        assert!((r.root_height - 24.0).abs() < 0.5);
    }
}
