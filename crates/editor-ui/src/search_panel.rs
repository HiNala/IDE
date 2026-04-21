//! Project-wide search sidebar chrome (M16).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use editor_search::{ProjectMatch, ProjectSearch, SearchEvent, SearchJob};

/// Streaming workspace search UI model.
#[derive(Debug, Default)]
pub struct SearchPanel {
    pub visible: bool,
    /// When true, keyboard goes to the query field; `Esc` clears this first.
    pub focused: bool,
    pub query: String,
    pub query_cursor: usize,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub current_job: Option<SearchJob>,
    pub results: BTreeMap<PathBuf, Vec<ProjectMatch>>,
    pub expanded_files: HashSet<PathBuf>,
    pub list_cursor: usize,
    /// After this instant, (re)start search with [`Self::query`].
    pub debounce_deadline: Option<std::time::Instant>,
    pub last_scheduled_query: String,
}

impl SearchPanel {
    pub fn project_params(&self) -> ProjectSearch {
        ProjectSearch {
            query: self.query.clone(),
            is_regex: self.is_regex,
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
        }
    }

    /// Drain streaming events (call each frame).
    pub fn poll_job(&mut self) {
        let Some(job) = &self.current_job else {
            return;
        };
        while let Ok(ev) = job.rx.try_recv() {
            match ev {
                SearchEvent::FileFinished { .. } => {}
                SearchEvent::Match(m) => {
                    self.results.entry(m.path.clone()).or_default().push(m);
                }
                SearchEvent::Done { .. } | SearchEvent::Error(_) => {
                    // Keep job until replaced so `cancel` remains meaningful.
                }
                SearchEvent::FileStarted(_) => {}
            }
        }
    }

    pub fn cancel_job(&mut self) {
        if let Some(j) = &self.current_job {
            j.cancel();
        }
        self.current_job = None;
    }

    pub fn clear_results(&mut self) {
        self.results.clear();
        self.list_cursor = 0;
    }

    /// Flattened rows for keyboard navigation `(path, match_index_in_vec)`.
    pub fn flat_index(&self) -> Vec<(PathBuf, usize)> {
        let mut v = Vec::new();
        for (p, ms) in &self.results {
            for i in 0..ms.len() {
                v.push((p.clone(), i));
            }
        }
        v
    }

    pub fn clamp_list_cursor(&mut self) {
        let n = self.flat_index().len();
        if n == 0 {
            self.list_cursor = 0;
        } else if self.list_cursor >= n {
            self.list_cursor = n - 1;
        }
    }

    /// Multiline overlay below the find strip.
    #[must_use]
    pub fn format_overlay(&self) -> String {
        if !self.visible {
            return String::new();
        }
        let mut s = String::new();
        let re_m = if self.is_regex { "[.*]" } else { "[lit]" };
        let case_m = if self.case_sensitive { "Aa" } else { "aA" };
        let ww_m = if self.whole_word { "ab̸" } else { "ab" };
        s.push_str("Project search  Ctrl+Shift+E explorer · Esc unfocus\n");
        s.push_str(&format!("{re_m} {case_m} {ww_m}  Query: {}\n", self.query));
        s.push_str("Enter run search · ↑↓ pick · Enter open\n\n");
        let flat = self.flat_index();
        if flat.is_empty() {
            s.push_str("(no results yet)\n");
            return s;
        }
        for (li, (path, mi)) in flat.iter().enumerate() {
            let mark = if li == self.list_cursor { "› " } else { "  " };
            let m = &self.results[path][*mi];
            let rel = path.file_name().map(|x| x.to_string_lossy()).unwrap_or_default();
            s.push_str(&format!("{mark}{rel}:{}: {}\n", m.line + 1, m.line_content));
        }
        s
    }
}
