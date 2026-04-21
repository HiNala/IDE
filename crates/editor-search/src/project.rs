//! Workspace-wide search: `ignore` parallel walk + `grep-searcher` line scan + streaming events.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use editor_core::{JobToken, WorkerPool};
use editor_workspace::entry::is_binary_heuristic;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::{WalkBuilder, WalkState};
use regex::Regex;

use crate::error::SearchError;
use crate::in_file::{build_pattern, InFileSearch};

/// Options mirrored from the in-file bar (same semantics).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectSearch {
    pub query: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// One match in a file (byte offsets are within the searched UTF-8 string, LF-normalized).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMatch {
    pub path: PathBuf,
    pub line: usize,
    pub col_start: usize,
    pub byte_range: Range<usize>,
    pub line_content: String,
}

/// Events streamed to the UI (drain each frame).
#[derive(Debug, Clone)]
pub enum SearchEvent {
    FileStarted(PathBuf),
    Match(ProjectMatch),
    FileFinished { path: PathBuf, match_count: usize },
    Done { total_files_searched: usize, total_matches: usize },
    Error(SearchError),
}

/// Handle for a running search (receive events + cancel).
pub struct SearchJob {
    pub rx: Receiver<SearchEvent>,
    token: JobToken,
}

impl std::fmt::Debug for SearchJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchJob").finish_non_exhaustive()
    }
}

impl SearchJob {
    #[must_use]
    pub fn token(&self) -> &JobToken {
        &self.token
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn memory_hit<'a>(mem: &'a HashMap<PathBuf, String>, path: &Path) -> Option<&'a str> {
    for (k, v) in mem {
        if same_path(k, path) {
            return Some(v.as_str());
        }
    }
    None
}

fn line_info(content: &str, byte: usize) -> (usize, usize, usize, usize) {
    let line = content[..byte].bytes().filter(|&b| b == b'\n').count();
    let line_start = content[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = content[byte..].find('\n').map(|i| byte + i).unwrap_or(content.len());
    (line, byte - line_start, line_start, line_end)
}

fn line_start_table(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            content
                .as_bytes()
                .iter()
                .enumerate()
                .filter_map(|(i, b)| (*b == b'\n').then_some(i + 1)),
        )
        .collect()
}

/// Line-oriented scan via `grep-searcher`, emitting every [`regex::Regex`] span on each matching line.
fn search_utf8_buffer(
    path: PathBuf,
    content: &str,
    matcher: &RegexMatcher,
    re: &Regex,
    tx: &Sender<SearchEvent>,
    files_done: &AtomicUsize,
    matches_total: &AtomicUsize,
) -> Result<(), SearchError> {
    let _ = tx.send(SearchEvent::FileStarted(path.clone()));
    let line_starts = line_start_table(content);
    let mut file_matches = 0usize;
    let mut searcher = Searcher::new();
    let p = path.clone();
    searcher
        .search_slice(
            matcher,
            content.as_bytes(),
            UTF8(|lnum, line| {
                let line_idx = (lnum as usize).saturating_sub(1);
                let line_start = line_starts.get(line_idx).copied().unwrap_or(0);
                for m in re.find_iter(line) {
                    let abs_start = line_start + m.start();
                    let abs_end = line_start + m.end();
                    let (line_row, col, ls, le) = line_info(content, abs_start);
                    let line_content = content[ls..le].to_string();
                    let _ = tx.send(SearchEvent::Match(ProjectMatch {
                        path: p.clone(),
                        line: line_row,
                        col_start: col,
                        byte_range: abs_start..abs_end,
                        line_content,
                    }));
                    file_matches += 1;
                    matches_total.fetch_add(1, Ordering::Relaxed);
                }
                Ok(true)
            }),
        )
        .map_err(|e| SearchError::Io(e.to_string()))?;
    files_done.fetch_add(1, Ordering::Relaxed);
    let _ = tx.send(SearchEvent::FileFinished { path, match_count: file_matches });
    Ok(())
}

fn run_parallel(
    root: PathBuf,
    params: ProjectSearch,
    memory_overrides: HashMap<PathBuf, String>,
    tx: Sender<SearchEvent>,
    cancel: JobToken,
    pool_token: JobToken,
) {
    let params_for_pat = InFileSearch {
        query: params.query.clone(),
        is_regex: params.is_regex,
        case_sensitive: params.case_sensitive,
        whole_word: params.whole_word,
    };
    let pat = match build_pattern(&params_for_pat) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(SearchEvent::Error(e));
            return;
        }
    };
    let matcher = match RegexMatcher::new(&pat) {
        Ok(m) => m,
        Err(e) => {
            let _ = tx.send(SearchEvent::Error(e.into()));
            return;
        }
    };
    let re = match Regex::new(&pat) {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(SearchEvent::Error(SearchError::InvalidRegex(e.to_string())));
            return;
        }
    };
    let re = Arc::new(re);
    let matcher = Arc::new(matcher);
    let mem = Arc::new(memory_overrides);
    let files_done = Arc::new(AtomicUsize::new(0));
    let matches_total = Arc::new(AtomicUsize::new(0));

    let mut wb = WalkBuilder::new(&root);
    wb.standard_filters(true);
    wb.require_git(false);
    wb.hidden(false);
    let parallel = wb.build_parallel();

    parallel.run(|| {
        let tx = tx.clone();
        let cancel = cancel.clone();
        let pool_token = pool_token.clone();
        let re = Arc::clone(&re);
        let matcher = Arc::clone(&matcher);
        let mem = Arc::clone(&mem);
        let root = root.clone();
        let files_done = Arc::clone(&files_done);
        let matches_total = Arc::clone(&matches_total);
        Box::new(move |entry| {
            if cancel.is_cancelled() || pool_token.is_cancelled() {
                return WalkState::Quit;
            }
            let ent = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };
            if !ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
                return WalkState::Continue;
            }
            let path = ent.path();
            if path == root.as_path() {
                return WalkState::Continue;
            }
            if is_binary_heuristic(path) {
                return WalkState::Continue;
            }

            let content: String = if let Some(text) = memory_hit(&mem, path) {
                text.to_string()
            } else {
                match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => return WalkState::Continue,
                }
            };

            if let Err(e) = search_utf8_buffer(
                path.to_path_buf(),
                &content,
                &matcher,
                &re,
                &tx,
                &files_done,
                &matches_total,
            ) {
                let _ = tx.send(SearchEvent::Error(e));
                return WalkState::Quit;
            }
            WalkState::Continue
        })
    });

    let _ = tx.send(SearchEvent::Done {
        total_files_searched: files_done.load(Ordering::Relaxed),
        total_matches: matches_total.load(Ordering::Relaxed),
    });
}

/// Start a cancellable project search on a background worker from [`WorkerPool`].
///
/// `memory_overrides` maps paths to UTF-8 buffer text for dirty documents that must not be read
/// from disk.
pub fn start_project_search(
    params: ProjectSearch,
    workspace: &editor_workspace::Workspace,
    memory_overrides: HashMap<PathBuf, String>,
    pool: &WorkerPool,
) -> SearchJob {
    let (tx, rx) = crossbeam_channel::unbounded();
    let token = JobToken::new_detached();
    let token_bg = token.clone();

    if params.query.is_empty() {
        let _ = tx.send(SearchEvent::Done { total_files_searched: 0, total_matches: 0 });
        return SearchJob { rx, token };
    }

    let root = workspace.root().to_path_buf();
    let params_clone = params.clone();
    let mem = memory_overrides;

    let (_jt, done) = pool.spawn(move |pool_tok| {
        run_parallel(root, params_clone, mem, tx, token_bg, pool_tok.clone());
    });
    drop(done);

    SearchJob { rx, token }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Instant;

    use super::*;
    use editor_workspace::Workspace;
    use tempfile::TempDir;

    #[test]
    fn project_search_finds_across_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("a.txt"), "alpha beta\ngamma beta\n").unwrap();
        fs::write(root.join("b.txt"), "beta fish\n").unwrap();
        let ws = Workspace::open(root).unwrap();
        let job = start_project_search(
            ProjectSearch {
                query: "beta".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: false,
            },
            &ws,
            HashMap::new(),
            &WorkerPool::new(Some(2)),
        );
        let mut matches = 0usize;
        let mut files_finished = 0usize;
        let t0 = Instant::now();
        while let Ok(ev) = job.rx.recv() {
            match ev {
                SearchEvent::Match(_) => matches += 1,
                SearchEvent::FileFinished { .. } => files_finished += 1,
                SearchEvent::Done { total_matches, .. } => {
                    assert_eq!(total_matches, matches);
                    break;
                }
                SearchEvent::Error(e) => panic!("{e:?}"),
                _ => {}
            }
        }
        assert!(matches >= 3, "matches={matches}");
        assert!(files_finished >= 2);
        assert!(t0.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn memory_override_beats_disk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let p = root.join("x.txt");
        fs::write(&p, "disk\n").unwrap();
        let ws = Workspace::open(root).unwrap();
        let mut mem = HashMap::new();
        mem.insert(p.clone(), "memory beta\n".to_string());
        let job = start_project_search(
            ProjectSearch {
                query: "beta".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: false,
            },
            &ws,
            mem,
            &WorkerPool::new(Some(1)),
        );
        let mut saw = false;
        while let Ok(ev) = job.rx.recv() {
            match ev {
                SearchEvent::Match(m) => {
                    assert!(m.line_content.contains("memory"));
                    saw = true;
                }
                SearchEvent::Done { .. } => break,
                _ => {}
            }
        }
        assert!(saw);
    }
}
