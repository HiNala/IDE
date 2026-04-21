//! In-buffer find with regex / literal / case / whole-word options.

use std::ops::Range;

use editor_core::{BytePos, Edit, EditKind, TextBuffer, TextBufferSnapshot};
use regex::Regex;

use crate::error::SearchError;

/// Maximum matches returned (UI / perf cap).
pub const IN_FILE_MATCH_CAP: usize = 5000;

/// Find options for the active document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InFileSearch {
    pub query: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// One occurrence in the buffer (UTF-8 byte offsets into LF-normalized storage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InFileMatch {
    pub byte_range: Range<usize>,
    pub line: usize,
    pub col_start: usize,
}

/// Result of [`search_buffer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InFileSearchResult {
    pub matches: Vec<InFileMatch>,
    pub capped: bool,
}

/// Build the `regex` pattern string from UI options.
pub fn build_pattern(params: &InFileSearch) -> Result<String, SearchError> {
    if params.query.is_empty() {
        return Ok(String::new());
    }
    let core = if params.is_regex { params.query.clone() } else { regex::escape(&params.query) };
    let wrapped = if params.whole_word { format!(r"\b(?:{})\b", core) } else { core };
    let mut pat = String::new();
    if !params.case_sensitive {
        pat.push_str("(?i)");
    }
    pat.push_str(&wrapped);
    Ok(pat)
}

/// Run find over a snapshot (matches unsaved buffer content).
pub fn search_buffer(
    params: &InFileSearch,
    snapshot: &TextBufferSnapshot,
) -> Result<InFileSearchResult, SearchError> {
    if params.query.is_empty() {
        return Ok(InFileSearchResult { matches: Vec::new(), capped: false });
    }
    let pat = build_pattern(params)?;
    let re = Regex::new(&pat).map_err(|e| SearchError::InvalidRegex(e.to_string()))?;
    let rope = snapshot.rope();
    let haystack = rope.to_string();
    let mut matches = Vec::new();
    let mut capped = false;
    for m in re.find_iter(&haystack) {
        if matches.len() >= IN_FILE_MATCH_CAP {
            capped = true;
            break;
        }
        let start = m.start();
        let end = m.end();
        let line = rope.byte_to_line(start);
        let line_start = rope.line_to_byte(line);
        let col_start = start - line_start;
        matches.push(InFileMatch { byte_range: start..end, line, col_start });
    }
    Ok(InFileSearchResult { matches, capped })
}

fn map_core_err(e: editor_core::CoreError) -> SearchError {
    SearchError::Io(e.to_string())
}

/// Applies one replacement (delete + insert). Push returned [`Edit`]s onto [`editor_core::UndoStack`]
/// in order for correct undo/redo.
pub fn replace_one(
    buffer: &mut TextBuffer,
    match_idx: usize,
    matches: &[InFileMatch],
    replacement: &str,
) -> Result<(Vec<Edit>, isize), SearchError> {
    let m = matches.get(match_idx).ok_or(SearchError::MatchIndexOutOfRange)?;
    let start = m.byte_range.start;
    let end = m.byte_range.end;
    let old_len = end - start;
    let deleted_text =
        buffer.slice_to_string(BytePos(start)..BytePos(end)).map_err(map_core_err)?;
    let e_del = buffer
        .apply_edit(EditKind::Delete { range: BytePos(start)..BytePos(end), deleted_text })
        .map_err(map_core_err)?;
    let e_ins = buffer
        .apply_edit(EditKind::Insert { pos: BytePos(start), text: replacement.to_string() })
        .map_err(map_core_err)?;
    let new_len = replacement.len();
    let delta = new_len as isize - old_len as isize;
    Ok((vec![e_del, e_ins], delta))
}

/// Replace every match in reverse byte order so indices stay valid.
pub fn replace_all(
    buffer: &mut TextBuffer,
    matches: &[InFileMatch],
    replacement: &str,
) -> Result<(Vec<Edit>, usize), SearchError> {
    let mut order: Vec<usize> = (0..matches.len()).collect();
    order.sort_by_key(|&i| matches[i].byte_range.start);
    let mut edits = Vec::new();
    let mut count = 0usize;
    for &idx in order.iter().rev() {
        let m = &matches[idx];
        let start = m.byte_range.start;
        let end = m.byte_range.end;
        let deleted_text =
            buffer.slice_to_string(BytePos(start)..BytePos(end)).map_err(map_core_err)?;
        edits.push(
            buffer
                .apply_edit(EditKind::Delete { range: BytePos(start)..BytePos(end), deleted_text })
                .map_err(map_core_err)?,
        );
        edits.push(
            buffer
                .apply_edit(EditKind::Insert { pos: BytePos(start), text: replacement.to_string() })
                .map_err(map_core_err)?,
        );
        count += 1;
    }
    Ok((edits, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_core::TextBuffer;

    fn snap(s: &str) -> TextBufferSnapshot {
        TextBuffer::from_str(s).snapshot()
    }

    #[test]
    fn empty_query_no_matches() {
        let r = search_buffer(&InFileSearch::default(), &snap("hello hello")).unwrap();
        assert!(r.matches.is_empty());
    }

    #[test]
    fn literal_two_matches() {
        let r = search_buffer(
            &InFileSearch {
                query: "ll".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: false,
            },
            &snap("hello hallo"),
        )
        .unwrap();
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn case_insensitive() {
        let r = search_buffer(
            &InFileSearch {
                query: "a".into(),
                is_regex: false,
                case_sensitive: false,
                whole_word: false,
            },
            &snap("A b a"),
        )
        .unwrap();
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn whole_word() {
        let r = search_buffer(
            &InFileSearch {
                query: "foo".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: true,
            },
            &snap("foo foobar foo"),
        )
        .unwrap();
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn regex_digit() {
        let r = search_buffer(
            &InFileSearch {
                query: r"\d+".into(),
                is_regex: true,
                case_sensitive: true,
                whole_word: false,
            },
            &snap("a 12 b 3"),
        )
        .unwrap();
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn unicode_literal() {
        let r = search_buffer(
            &InFileSearch {
                query: "日本".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: false,
            },
            &snap("hi 日本語 日本"),
        )
        .unwrap();
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn invalid_regex_errors() {
        let err = search_buffer(
            &InFileSearch {
                query: "(".into(),
                is_regex: true,
                case_sensitive: true,
                whole_word: false,
            },
            &snap("("),
        )
        .err()
        .unwrap();
        assert!(matches!(err, SearchError::InvalidRegex(_)));
    }

    #[test]
    fn replace_one_updates_buffer() {
        let mut buf = TextBuffer::from_str("foo bar foo");
        let res = search_buffer(
            &InFileSearch {
                query: "foo".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: true,
            },
            &buf.snapshot(),
        )
        .unwrap();
        assert_eq!(res.matches.len(), 2);
        let (_edits, _) = replace_one(&mut buf, 0, &res.matches, "ZZ").unwrap();
        assert_eq!(buf.to_text(), "ZZ bar foo");
    }

    #[test]
    fn replace_all_updates_buffer() {
        let mut buf = TextBuffer::from_str("foo bar foo");
        let res = search_buffer(
            &InFileSearch {
                query: "foo".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: true,
            },
            &buf.snapshot(),
        )
        .unwrap();
        let (_edits, n) = replace_all(&mut buf, &res.matches, "x").unwrap();
        assert_eq!(n, 2);
        assert_eq!(buf.to_text(), "x bar x");
    }
}
