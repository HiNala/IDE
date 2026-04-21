//! Status bar layout (V2): strings only — GPU draws in `editor-render`.

use std::path::{Path, PathBuf};

use editor_core::LineEnding;

/// On-disk / buffer encoding hint for the status line (UTF-8 internal; BOM/UTF-16 for future I/O).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

/// Snapshot for one frame of status chrome.
#[derive(Debug, Clone)]
pub struct StatusBarInfo {
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// Zero-based line index (display adds 1).
    pub cursor_line: usize,
    /// Zero-based UTF-8 byte column within the line.
    pub cursor_col: usize,
    pub total_lines: usize,
    pub encoding: SourceEncoding,
    pub line_ending: LineEnding,
    pub external_modified: bool,
    /// Short user-visible note (e.g. clipboard failure). Cleared by the app after a few seconds.
    pub status_message: Option<String>,
    /// Current git branch when the workspace is a repo (M18).
    pub git_branch: Option<String>,
    /// Count of paths differing from `HEAD` (best-effort; M18).
    pub git_modified_count: Option<usize>,
}

/// Borrowing status snapshot for one frame (avoids cloning [`PathBuf`] when handing off to the renderer).
#[derive(Debug, Clone, Copy)]
pub struct StatusBarInfoRef<'a> {
    pub path: Option<&'a Path>,
    pub dirty: bool,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub total_lines: usize,
    pub encoding: SourceEncoding,
    pub line_ending: LineEnding,
    pub external_modified: bool,
    pub status_message: Option<&'a str>,
    pub git_branch: Option<&'a str>,
    pub git_modified_count: Option<usize>,
}

impl<'a> StatusBarInfoRef<'a> {
    /// Clones path and transient message so the frame loop can own chrome while mutating other app state.
    #[must_use]
    pub fn into_owned(self) -> StatusBarInfo {
        StatusBarInfo {
            path: self.path.map(Path::to_path_buf),
            dirty: self.dirty,
            cursor_line: self.cursor_line,
            cursor_col: self.cursor_col,
            total_lines: self.total_lines,
            encoding: self.encoding,
            line_ending: self.line_ending,
            external_modified: self.external_modified,
            status_message: self.status_message.map(str::to_string),
            git_branch: self.git_branch.map(str::to_string),
            git_modified_count: self.git_modified_count,
        }
    }
}

/// Pre-shaped text for the bottom bar (single line for glyphon).
#[derive(Debug, Clone)]
pub struct StatusBarLayout {
    /// Logical height in **physical** pixels (includes scale).
    pub height_px: f32,
    pub line: String,
}

impl StatusBarLayout {
    /// Builds the status string: path (truncated), Ln/Col, encoding, line ending.
    #[must_use]
    pub fn from_info(info: &StatusBarInfo, scale_factor: f32) -> Self {
        let iref = StatusBarInfoRef {
            path: info.path.as_deref(),
            dirty: info.dirty,
            cursor_line: info.cursor_line,
            cursor_col: info.cursor_col,
            total_lines: info.total_lines,
            encoding: info.encoding,
            line_ending: info.line_ending,
            external_modified: info.external_modified,
            status_message: info.status_message.as_deref(),
            git_branch: info.git_branch.as_deref(),
            git_modified_count: info.git_modified_count,
        };
        Self::from_info_ref(&iref, scale_factor)
    }

    /// Same as [`Self::from_info`] but borrows the file path (see [`StatusBarInfoRef`]).
    #[must_use]
    pub fn from_info_ref(info: &StatusBarInfoRef<'_>, scale_factor: f32) -> Self {
        let height_px = 24.0 * scale_factor;
        let prefix = if info.dirty { "*" } else { "" };
        let ext = if info.external_modified { "⚠ " } else { "" };
        let path_str =
            info.path.map(|p| p.display().to_string()).unwrap_or_else(|| "untitled".to_string());
        let path_disp = truncate_path_tail(&path_str, 72);
        let line_n = info.cursor_line.saturating_add(1);
        let col_n = info.cursor_col.saturating_add(1);
        let enc = match info.encoding {
            SourceEncoding::Utf8 => "UTF-8",
            SourceEncoding::Utf8Bom => "UTF-8 BOM",
            SourceEncoding::Utf16Le => "UTF-16 LE",
            SourceEncoding::Utf16Be => "UTF-16 BE",
        };
        let le = match info.line_ending {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
            LineEnding::Cr => "CR",
            LineEnding::Mixed => "mixed",
        };
        let lines = info.total_lines.max(1);
        let mut line = format!("{prefix}{ext}{path_disp}");
        if let Some(b) = info.git_branch.filter(|s| !s.is_empty()) {
            line.push_str(&format!("    ·    {b}"));
        }
        if let Some(n) = info.git_modified_count {
            line.push_str(&format!("    ·    {n} modified"));
        }
        line.push_str(&format!(
            "    ·    Ln {line_n}, Col {col_n} · {lines} lines    ·    {enc} · {le}"
        ));
        if let Some(note) = info.status_message.filter(|s| !s.is_empty()) {
            let n = truncate_path_tail(note, 56);
            line = format!("{n}    ·    {line}");
        }
        Self { height_px, line }
    }
}

fn truncate_path_tail(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let skip = count.saturating_sub(max_chars.saturating_sub(1));
    format!("…{}", s.chars().skip(skip).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_contains_encoding_and_line_ending() {
        let info = StatusBarInfo {
            path: Some(PathBuf::from("C:/tmp/hello.rs")),
            dirty: true,
            cursor_line: 4,
            cursor_col: 11,
            total_lines: 100,
            encoding: SourceEncoding::Utf8,
            line_ending: LineEnding::Lf,
            external_modified: false,
            status_message: None,
            git_branch: None,
            git_modified_count: None,
        };
        let l = StatusBarLayout::from_info(&info, 1.0);
        assert!(l.line.contains('*'));
        assert!(l.line.contains("UTF-8"));
        assert!(l.line.contains("LF"));
        assert!(l.line.contains("Ln 5"));
        assert!(l.line.contains("Col 12"));
    }

    #[test]
    fn external_warn_symbol() {
        let info = StatusBarInfo {
            path: None,
            dirty: false,
            cursor_line: 0,
            cursor_col: 0,
            total_lines: 1,
            encoding: SourceEncoding::Utf8,
            line_ending: LineEnding::Crlf,
            external_modified: true,
            status_message: None,
            git_branch: None,
            git_modified_count: None,
        };
        let l = StatusBarLayout::from_info(&info, 1.0);
        assert!(l.line.contains('⚠'));
    }

    #[test]
    fn status_message_prefixes_line() {
        let info = StatusBarInfo {
            path: Some(PathBuf::from("src/main.rs")),
            dirty: false,
            cursor_line: 0,
            cursor_col: 0,
            total_lines: 10,
            encoding: SourceEncoding::Utf8,
            line_ending: LineEnding::Lf,
            external_modified: false,
            status_message: Some("Clipboard: could not read".into()),
            git_branch: None,
            git_modified_count: None,
        };
        let l = StatusBarLayout::from_info(&info, 1.0);
        assert!(l.line.starts_with("Clipboard"));
        assert!(l.line.contains("main.rs"));
    }

    #[test]
    fn from_info_ref_matches_from_info() {
        let info = StatusBarInfo {
            path: Some(PathBuf::from("C:/tmp/x.rs")),
            dirty: true,
            cursor_line: 1,
            cursor_col: 2,
            total_lines: 10,
            encoding: SourceEncoding::Utf8Bom,
            line_ending: LineEnding::Mixed,
            external_modified: false,
            status_message: None,
            git_branch: None,
            git_modified_count: None,
        };
        let iref = StatusBarInfoRef {
            path: info.path.as_deref(),
            dirty: info.dirty,
            cursor_line: info.cursor_line,
            cursor_col: info.cursor_col,
            total_lines: info.total_lines,
            encoding: info.encoding,
            line_ending: info.line_ending,
            external_modified: info.external_modified,
            status_message: None,
            git_branch: None,
            git_modified_count: None,
        };
        let a = StatusBarLayout::from_info(&info, 1.25);
        let b = StatusBarLayout::from_info_ref(&iref, 1.25);
        assert_eq!(a.line, b.line);
        assert!((a.height_px - b.height_px).abs() < f32::EPSILON);
    }
}
