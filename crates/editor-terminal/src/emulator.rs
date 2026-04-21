//! Alacritty `Term` + VTE `ansi::Processor` wrapper.

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{point_to_viewport, viewport_to_point, Config, Term};
use alacritty_terminal::vte::ansi;
use crossbeam_channel::Sender;

use crate::color_resolve::{self, DEFAULT_BG, DEFAULT_FG};
use crate::events::TerminalEvent;

/// Forwards alacritty events to the app (title, bell, redraw).
#[derive(Clone)]
pub struct ProxyListener(pub Sender<TerminalEvent>);

impl std::fmt::Debug for ProxyListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyListener").finish_non_exhaustive()
    }
}

impl EventListener for ProxyListener {
    fn send_event(&self, event: Event) {
        match event {
            Event::Title(t) => {
                let _ = self.0.send(TerminalEvent::TitleChanged(t));
            }
            Event::ResetTitle => {
                let _ = self.0.send(TerminalEvent::TitleChanged(String::new()));
            }
            Event::Bell => {
                let _ = self.0.send(TerminalEvent::Bell);
            }
            Event::Wakeup => {
                let _ = self.0.send(TerminalEvent::Dirty);
            }
            _ => {}
        }
    }
}

/// Terminal emulator state (grid + escape processing).
pub struct TerminalEmulator {
    term: Term<ProxyListener>,
    processor: ansi::Processor,
}

impl std::fmt::Debug for TerminalEmulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalEmulator").finish_non_exhaustive()
    }
}

impl TerminalEmulator {
    pub fn new(cols: usize, rows: usize, scrollback: usize, listener: ProxyListener) -> Self {
        let config = Config { scrolling_history: scrollback, ..Default::default() };
        let size = TermSize::new(cols, rows);
        let term = Term::new(config, &size, listener);
        Self { term, processor: ansi::Processor::new() }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let size = TermSize::new(cols, rows);
        self.term.resize(size);
    }

    #[must_use]
    pub fn term(&self) -> &Term<ProxyListener> {
        &self.term
    }

    #[must_use]
    pub fn term_mut(&mut self) -> &mut Term<ProxyListener> {
        &mut self.term
    }

    /// Scroll the viewport through history (`delta` &gt; 0 moves toward older lines).
    pub fn scroll_lines(&mut self, delta: i32) {
        if delta != 0 {
            self.term.scroll_display(Scroll::Delta(delta));
        }
    }

    /// Start a block selection at viewport cell `(line, col)`.
    pub fn begin_block_selection(&mut self, viewport_line: usize, col: usize) {
        let off = self.term.grid().display_offset();
        let gp = viewport_to_point(off, Point::<usize>::new(viewport_line, Column(col)));
        self.term.selection = Some(Selection::new(SelectionType::Block, gp, Side::Left));
    }

    pub fn update_block_selection(&mut self, viewport_line: usize, col: usize) {
        let off = self.term.grid().display_offset();
        let gp = viewport_to_point(off, Point::<usize>::new(viewport_line, Column(col)));
        if let Some(sel) = self.term.selection.as_mut() {
            sel.update(gp, Side::Right);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    #[must_use]
    pub fn selection_to_string(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    /// Build a render snapshot for the GPU text layer (row runs with RGB fg/bg).
    pub fn render_snapshot(&self) -> TerminalRenderSnapshot {
        let grid = self.term.grid();
        let screen_lines = grid.screen_lines();
        let cols = grid.columns();
        let off = grid.display_offset();
        let palette = self.term.colors();

        let mut rows = Vec::with_capacity(screen_lines);
        for row in 0..screen_lines {
            let mut runs: Vec<(String, [u8; 3], [u8; 3])> = Vec::new();
            let mut cur_text = String::new();
            let mut cur_fg = [0u8; 3];
            let mut cur_bg = [0u8; 3];
            let mut have_run = false;

            for col in 0..cols {
                let gp = viewport_to_point(off, Point::<usize>::new(row, Column(col)));
                let cell: &Cell = &grid[gp];
                if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let (fg, bg) =
                    color_resolve::resolve_cell_colors(cell, palette, DEFAULT_FG, DEFAULT_BG);
                let fg_a = [fg.r, fg.g, fg.b];
                let bg_a = [bg.r, bg.g, bg.b];
                let ch = if cell.c == '\0' || cell.c == ' ' { ' ' } else { cell.c };
                if !have_run {
                    cur_fg = fg_a;
                    cur_bg = bg_a;
                    cur_text.push(ch);
                    have_run = true;
                } else if fg_a == cur_fg && bg_a == cur_bg {
                    cur_text.push(ch);
                } else {
                    runs.push((std::mem::take(&mut cur_text), cur_fg, cur_bg));
                    cur_fg = fg_a;
                    cur_bg = bg_a;
                    cur_text.push(ch);
                }
            }
            if have_run {
                runs.push((cur_text, cur_fg, cur_bg));
            }
            rows.push(TerminalRowRuns { runs });
        }

        let cursor = self.term.renderable_content().cursor;
        let vp = point_to_viewport(off, cursor.point)
            .unwrap_or_else(|| Point::<usize>::new(0, Column(0)));

        TerminalRenderSnapshot {
            rows,
            cursor_row: vp.line,
            cursor_col: vp.column.0,
            cursor_hidden: matches!(
                cursor.shape,
                alacritty_terminal::vte::ansi::CursorShape::Hidden
            ),
        }
    }
}

/// One row of text runs for glyph shaping.
#[derive(Debug, Clone)]
pub struct TerminalRowRuns {
    pub runs: Vec<(String, [u8; 3], [u8; 3])>,
}

/// Pre-shaped view of the viewport for the render crate (M26).
#[derive(Debug, Clone)]
pub struct TerminalRenderSnapshot {
    pub rows: Vec<TerminalRowRuns>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub cursor_hidden: bool,
}
