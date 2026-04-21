//! Grouped undo/redo stack ([`UndoStack`]) with insert coalescing and depth cap.

use std::mem;
use std::time::{Duration, Instant};

use crate::buffer::{Edit, EditKind, TextBuffer};
use crate::CoreResult;

/// Default coalescing window for typed character groups (milliseconds).
pub const COALESCE_MS: u64 = 300;

/// Undo history: groups of [`Edit`]s; each [`UndoStack::checkpoint`] ends one group.
#[derive(Debug)]
pub struct UndoStack {
    groups: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
    current: Vec<Edit>,
    coalesce: Duration,
    max_steps: usize,
    last_event: Option<Instant>,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(10_000, Duration::from_millis(COALESCE_MS))
    }
}

impl UndoStack {
    /// `max_steps` caps **group** count (oldest dropped when exceeded). `coalesce` bounds insert merging.
    #[must_use]
    pub fn new(max_steps: usize, coalesce: Duration) -> Self {
        Self {
            groups: Vec::new(),
            redo: Vec::new(),
            current: Vec::new(),
            coalesce,
            max_steps: max_steps.max(1),
            last_event: None,
        }
    }

    /// Append an edit; clears redo. Merges consecutive inserts (typing) when allowed.
    pub fn push(&mut self, edit: Edit) {
        self.redo.clear();
        let now = Instant::now();
        if let Some(t) = self.last_event {
            if now.duration_since(t) > self.coalesce {
                self.flush_current();
            }
        }
        self.last_event = Some(now);

        if let Some(last) = self.current.last_mut() {
            if let (
                EditKind::Insert { pos: p1, text: t1 },
                EditKind::Insert { pos: p2, text: t2 },
            ) = (&mut last.kind, &edit.kind)
            {
                if p2.0 == p1.0 + t1.len() {
                    t1.push_str(t2);
                    return;
                }
            }
        }
        self.current.push(edit);
    }

    /// Ends the current edit group so the next [`Self::push`] starts a new undo step.
    pub fn checkpoint(&mut self) {
        self.flush_current();
    }

    /// Approximate count of undo steps available (groups + open group).
    #[must_use]
    pub fn len_undo(&self) -> usize {
        self.groups.len() + usize::from(!self.current.is_empty())
    }

    /// Undo the last group. Returns `Ok(None)` when the stack is empty.
    pub fn undo(&mut self, buf: &mut TextBuffer) -> CoreResult<Option<()>> {
        if !self.current.is_empty() {
            self.flush_current();
        }
        let Some(group) = self.groups.pop() else {
            return Ok(None);
        };
        for edit in group.iter().rev() {
            edit.inverse().apply(buf)?;
        }
        self.redo.push(group);
        Ok(Some(()))
    }

    /// Redo the last undone group. Returns `Ok(None)` when redo is empty.
    pub fn redo(&mut self, buf: &mut TextBuffer) -> CoreResult<Option<()>> {
        let Some(group) = self.redo.pop() else {
            return Ok(None);
        };
        for edit in &group {
            edit.apply(buf)?;
        }
        self.groups.push(group);
        Ok(Some(()))
    }

    fn flush_current(&mut self) {
        if self.current.is_empty() {
            return;
        }
        while self.groups.len() >= self.max_steps {
            self.groups.remove(0);
        }
        self.groups.push(mem::take(&mut self.current));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::BytePos;

    #[test]
    fn undo_redo_single_insert() {
        let mut buf = TextBuffer::new();
        let mut u = UndoStack::default();
        let e =
            buf.apply_edit(EditKind::Insert { pos: BytePos(0), text: "a".into() }).expect("insert");
        u.push(e);
        u.checkpoint();
        assert_eq!(u.len_undo(), 1);
        u.undo(&mut buf).expect("undo").expect("some");
        assert_eq!(buf.to_text(), "");
        u.redo(&mut buf).expect("redo").expect("some");
        assert_eq!(buf.to_text(), "a");
    }
}
