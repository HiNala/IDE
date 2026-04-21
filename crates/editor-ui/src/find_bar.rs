//! In-editor find / replace chrome (M16).

use editor_core::TextBufferSnapshot;
use editor_search::{search_buffer, InFileMatch, InFileSearch, SearchError};

/// Find / replace UI state; rendering is a monospace overlay string + top backdrop quad.
#[derive(Debug, Clone, Default)]
pub struct FindBar {
    pub visible: bool,
    pub query: String,
    pub query_cursor: usize,
    pub replace: String,
    pub replace_cursor: usize,
    /// Second row (replace field) visible.
    pub replace_row_visible: bool,
    /// When true, arrow keys / typing target replace field.
    pub focus_replace: bool,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub matches: Vec<InFileMatch>,
    pub current_match: Option<usize>,
    pub regex_error: Option<String>,
}

impl FindBar {
    fn search_params(&self) -> InFileSearch {
        InFileSearch {
            query: self.query.clone(),
            is_regex: self.is_regex,
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
        }
    }

    /// Recompute [`Self::matches`] from the active buffer snapshot.
    pub fn rerun_search(&mut self, snapshot: &TextBufferSnapshot) {
        self.regex_error = None;
        if !self.visible {
            self.matches.clear();
            self.current_match = None;
            return;
        }
        if self.query.is_empty() {
            self.matches.clear();
            self.current_match = None;
            return;
        }
        match search_buffer(&self.search_params(), snapshot) {
            Ok(r) => {
                self.matches = r.matches;
                if self.matches.is_empty() {
                    self.current_match = None;
                } else {
                    let next = self.current_match.unwrap_or(0).min(self.matches.len() - 1);
                    self.current_match = Some(next);
                }
            }
            Err(SearchError::InvalidRegex(msg)) => {
                self.regex_error = Some(msg);
                self.matches.clear();
                self.current_match = None;
            }
            Err(_) => {
                self.matches.clear();
                self.current_match = None;
            }
        }
    }

    /// Physical pixels for the dark strip behind the find UI (`0` when hidden).
    #[must_use]
    pub fn backdrop_height_px(&self, scale_factor: f32) -> f32 {
        if !self.visible {
            return 0.0;
        }
        if self.replace_row_visible {
            66.0 * scale_factor
        } else {
            36.0 * scale_factor
        }
    }

    /// Multiline overlay for [`editor_render::TextLayer`] (same slot as quick-open).
    #[must_use]
    pub fn format_overlay(&self, blink_on: bool) -> String {
        if !self.visible {
            return String::new();
        }
        let mut s = String::new();
        let re_m = if self.is_regex { "[.*]" } else { "[lit]" };
        let case_m = if self.case_sensitive { "Aa" } else { "aA" };
        let ww_m = if self.whole_word { "ab̸" } else { "ab" };
        s.push_str("Find  Esc close · Enter next · Shift+Enter prev\n");
        if let Some(ref err) = self.regex_error {
            s.push_str(&format!("Regex error: {err}\n"));
        }
        let qdisp =
            insert_cursor_byte(&self.query, self.query_cursor, blink_on, !self.focus_replace);
        s.push_str(&format!("{re_m} {case_m} {ww_m}  Find: {qdisp}\n"));
        if self.replace_row_visible {
            let rdisp = insert_cursor_byte(
                &self.replace,
                self.replace_cursor,
                blink_on,
                self.focus_replace,
            );
            s.push_str(&format!("Replace: {rdisp}   [Enter=replace] [Ctrl+Enter=all]\n"));
        }
        let (cur, tot) = match (self.current_match, self.matches.len()) {
            (Some(i), n) if n > 0 => (i + 1, n),
            (_, n) if n > 0 => (1, n),
            _ => (0, 0),
        };
        s.push_str(&format!("Matches: {cur} / {tot}\n"));
        s
    }

    pub fn next_match(&mut self) {
        let n = self.matches.len();
        if n == 0 {
            self.current_match = None;
            return;
        }
        let i = self.current_match.unwrap_or(0);
        self.current_match = Some((i + 1) % n);
    }

    pub fn prev_match(&mut self) {
        let n = self.matches.len();
        if n == 0 {
            self.current_match = None;
            return;
        }
        let i = self.current_match.unwrap_or(0);
        self.current_match = Some((i + n - 1) % n);
    }
}

fn insert_cursor_byte(s: &str, pos: usize, blink_on: bool, active: bool) -> String {
    if !active {
        return s.to_string();
    }
    let pos = s.floor_char_boundary(pos.min(s.len()));
    let c = if blink_on { '|' } else { ' ' };
    format!("{}{}{}", &s[..pos], c, &s[pos..])
}
