//! Bounded undo/redo stack with insert coalescing.

use std::time::{Duration, Instant};

use crate::buffer::{Edit, EditKind, TextBuffer};
use crate::CoreResult;

/// Undo history with rapid single-character insert coalescing.
#[derive(Debug)]
pub struct UndoStack {
    history: Vec<Edit>,
    future: Vec<Edit>,
    last_push: Option<Instant>,
    capacity: usize,
    coalesce_window: Duration,
}

impl UndoStack {
    /// New stack with max `capacity` edits remembered and `coalesce_window` for typing merges.
    #[must_use]
    pub fn new(capacity: usize, coalesce_window: Duration) -> Self {
        Self { history: Vec::new(), future: Vec::new(), last_push: None, capacity, coalesce_window }
    }

    /// Push an edit from [`TextBuffer::apply_edit`]. Clears the redo stack.
    pub fn push(&mut self, edit: Edit) {
        self.future.clear();
        if self.should_coalesce_with_last(&edit) {
            if let Some(last) = self.history.pop() {
                let merged = merge_adjacent_inserts(last, edit);
                self.history.push(merged);
                self.last_push = Some(Instant::now());
                self.evict_if_needed();
                return;
            }
        }
        self.history.push(edit);
        self.last_push = Some(Instant::now());
        self.evict_if_needed();
    }

    /// Break coalescing so the next [`Self::push`] starts a fresh undo step.
    pub fn checkpoint(&mut self) {
        self.last_push = None;
    }

    /// Undo one edit; returns the inverse edit that was applied, if any.
    pub fn undo(&mut self, buffer: &mut TextBuffer) -> CoreResult<Option<Edit>> {
        let edit = match self.history.pop() {
            None => return Ok(None),
            Some(e) => e,
        };
        let inv = edit.inverse();
        inv.apply(buffer)?;
        self.future.push(edit);
        self.last_push = None;
        Ok(Some(inv))
    }

    /// Redo one undone edit; returns the reapplied edit when successful.
    pub fn redo(&mut self, buffer: &mut TextBuffer) -> CoreResult<Option<Edit>> {
        let edit = match self.future.pop() {
            None => return Ok(None),
            Some(e) => e,
        };
        let reapplied = edit.clone();
        edit.apply(buffer)?;
        self.history.push(edit);
        self.last_push = None;
        Ok(Some(reapplied))
    }

    /// Number of undo steps available.
    #[must_use]
    pub fn len_undo(&self) -> usize {
        self.history.len()
    }

    /// Number of redo steps available.
    #[must_use]
    pub fn len_redo(&self) -> usize {
        self.future.len()
    }

    fn should_coalesce_with_last(&self, next: &Edit) -> bool {
        let Some(last_t) = self.last_push else {
            return false;
        };
        if Instant::now().duration_since(last_t) > self.coalesce_window {
            return false;
        }
        let Some(last) = self.history.last() else {
            return false;
        };
        matches!(
            (&last.kind, &next.kind),
            (
                EditKind::Insert { pos: p1, text: t1 },
                EditKind::Insert { pos: p2, text: t2 },
            ) if t1.len() == 1 && t2.len() == 1 && p2.0 == p1.0 + t1.len()
        )
    }

    fn evict_if_needed(&mut self) {
        while self.history.len() > self.capacity {
            let _ = self.history.remove(0);
        }
    }
}

fn merge_adjacent_inserts(a: Edit, b: Edit) -> Edit {
    let seq = a.seq;
    match (a.kind, b.kind) {
        (EditKind::Insert { pos, text: mut t1 }, EditKind::Insert { text: t2, .. }) => {
            t1.push_str(&t2);
            Edit { kind: EditKind::Insert { pos, text: t1 }, seq }
        }
        _ => unreachable!("coalescing only for adjacent single-char inserts"),
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(10_000, Duration::from_millis(500))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::BytePos;

    #[test]
    fn undo_redo_round_trip() {
        let mut buf = TextBuffer::new();
        let mut u = UndoStack::new(100, Duration::from_secs(1));
        let e = buf.apply_edit(EditKind::Insert { pos: BytePos(0), text: "x".into() }).unwrap();
        u.push(e);
        assert_eq!(buf.slice_to_string(BytePos(0)..BytePos(1)).unwrap(), "x");
        u.undo(&mut buf).unwrap();
        assert_eq!(buf.len_bytes(), 0);
        u.redo(&mut buf).unwrap();
        assert_eq!(buf.slice_to_string(BytePos(0)..BytePos(1)).unwrap(), "x");
    }
}
