//! Status bar layout (V2): strings only — GPU draws in `editor-render`.

use std::path::PathBuf;

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
        let height_px = 24.0 * scale_factor;
        let prefix = if info.dirty { "*" } else { "" };
        let ext = if info.external_modified { "⚠ " } else { "" };
        let path_str = info
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "untitled".to_string());
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
        let line = format!(
            "{prefix}{ext}{path_disp}    ·    Ln {line_n}, Col {col_n} · {lines} lines    ·    {enc} · {le}"
        );
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
        };
        let l = StatusBarLayout::from_info(&info, 1.0);
        assert!(l.line.contains('⚠'));
    }
}
