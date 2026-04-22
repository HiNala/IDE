//! Hand-written single-line tokenizer for Rust source.
//!
//! Operates on one line at a time so a single edit only reshapes that line's
//! glyph runs. A tiny [`LineState`] carrier threads block-comment nesting
//! across lines so `/* ... */` spanning multiple lines highlights coherently.

use crate::{LineState, TokenKind, TokenSpan};

/// Reserved Rust keywords we color as `TokenKind::Keyword`.
/// Sorted for readability; lookups use a `.contains()` over the slice which
/// is fine for this size — `match` generates a linear chain either way.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield",
];

/// Primitive types — colored as `Type` even though they're lowercase.
const PRIMITIVES: &[&str] = &[
    "bool", "char", "f32", "f64", "i128", "i16", "i32", "i64", "i8", "isize", "str", "u128", "u16",
    "u32", "u64", "u8", "usize",
];

/// Tokenize a single line of Rust source into non-overlapping spans.
///
/// Convenience wrapper that discards the outgoing [`LineState`]. For coherent
/// multi-line block comments, use [`tokenize_line_with_state`].
///
/// Guarantees:
/// - Spans are sorted by `start`.
/// - Every input byte is covered by exactly one span (including whitespace
///   + punctuation, which get `TokenKind::Text`).
/// - `spans[i].end == spans[i+1].start`.
#[must_use]
pub fn tokenize_line(line: &str) -> Vec<TokenSpan> {
    tokenize_line_with_state(line, LineState::default()).0
}

/// Tokenize one line using `state` entering the line (typically the state
/// returned by the previous line's call). The second tuple element is the
/// state leaving the line — pass it to the next call to carry open block
/// comments across lines.
#[must_use]
pub fn tokenize_line_with_state(line: &str, mut state: LineState) -> (Vec<TokenSpan>, LineState) {
    if line.is_empty() {
        return (Vec::new(), state);
    }
    let bytes = line.as_bytes();
    let mut spans: Vec<TokenSpan> = Vec::with_capacity(16);
    let mut i = 0usize;

    // Carry an open `/* */` from the previous line: consume bytes until the
    // nesting balances (possibly opening new comments inside), or to EOL.
    if state.block_comment_depth > 0 {
        let (end, depth_after) =
            advance_block_comment(bytes, 0, state.block_comment_depth as usize);
        push(&mut spans, 0, end, TokenKind::Comment);
        state.block_comment_depth = depth_after as u32;
        i = end;
    }

    // Helper to push a span, merging into the previous one when both carry
    // `TokenKind::Text` so the output never fragments into one-byte filler
    // spans per punctuation glyph.
    fn push(spans: &mut Vec<TokenSpan>, start: usize, end: usize, kind: TokenKind) {
        if start >= end {
            return;
        }
        if kind == TokenKind::Text {
            if let Some(last) = spans.last_mut() {
                if last.kind == TokenKind::Text && last.end == start {
                    last.end = end;
                    return;
                }
            }
        }
        spans.push(TokenSpan::new(start, end, kind));
    }

    while i < bytes.len() {
        let b = bytes[i];

        // Line comment: `//` through end of line. Everything after is Comment.
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            push(&mut spans, i, bytes.len(), TokenKind::Comment);
            return (spans, state);
        }

        // Start of block comment `/*` — we color from here to the closing `*/`
        // on this line, or to EOL if the comment spans. Nested comments match
        // rustc's lexing rule. Depth remaining > 0 at EOL is carried in state.
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let start = i;
            let (end, depth_after) = advance_block_comment(bytes, i + 2, 1);
            push(&mut spans, start, end, TokenKind::Comment);
            state.block_comment_depth = depth_after as u32;
            i = end;
            continue;
        }

        // String literal: `"..."` with `\"` escapes. Raw strings (`r#"..."#`)
        // are handled as a separate arm below.
        if b == b'"' {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\\' && j + 1 < bytes.len() {
                    j += 2;
                    continue;
                }
                if bytes[j] == b'"' {
                    j += 1;
                    break;
                }
                j += 1;
            }
            push(&mut spans, start, j, TokenKind::String);
            i = j;
            continue;
        }

        // Raw / byte string heads: r"..." r#"..."# b"..." br"..." — conservative.
        // We only detect the leading `r` / `b` / `br` immediately followed by
        // `"` (optionally preceded by `#`s) and then consume through the matching
        // closing sequence. Anything more exotic falls back to identifier rules.
        if (b == b'r' || b == b'b')
            && i + 1 < bytes.len()
            && matches!(bytes[i + 1], b'"' | b'#' | b'r')
        {
            if let Some(end) = raw_string_end(bytes, i) {
                push(&mut spans, i, end, TokenKind::String);
                i = end;
                continue;
            }
        }

        // Char literal OR lifetime. `'` followed by something-then-`'` with at
        // most 2 glyphs inside (single char or escape) is a char; otherwise
        // treat `'name` as a lifetime.
        if b == b'\'' {
            if let Some(end) = char_literal_end(bytes, i) {
                push(&mut spans, i, end, TokenKind::Char);
                i = end;
                continue;
            }
            if let Some(end) = lifetime_end(bytes, i) {
                push(&mut spans, i, end, TokenKind::Lifetime);
                i = end;
                continue;
            }
        }

        // Numeric literal: `[0-9][0-9a-zA-Z_.]*` with a dot-for-float rule so
        // `foo.0` stays as identifier+dot+0, not one big number.
        if b.is_ascii_digit() {
            let start = i;
            let mut j = i + 1;
            let mut seen_dot = false;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    j += 1;
                } else if c == b'.'
                    && !seen_dot
                    && j + 1 < bytes.len()
                    && bytes[j + 1].is_ascii_digit()
                {
                    seen_dot = true;
                    j += 1;
                } else {
                    break;
                }
            }
            push(&mut spans, start, j, TokenKind::Number);
            i = j;
            continue;
        }

        // Attribute head: `#[...]` or `#![...]`. Color the `#` + opening bracket
        // + contents + closing bracket together so the visual anchor is whole.
        if b == b'#' && i + 1 < bytes.len() && matches!(bytes[i + 1], b'[' | b'!') {
            let start = i;
            let mut j = i + 1;
            if bytes[j] == b'!' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'[' {
                let mut depth = 1usize;
                j += 1;
                while j < bytes.len() && depth > 0 {
                    match bytes[j] {
                        b'[' => depth += 1,
                        b']' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                push(&mut spans, start, j, TokenKind::Attribute);
                i = j;
                continue;
            }
        }

        // Identifier or keyword. Unicode-XID would be stricter; ASCII
        // identifier rules are sufficient for highlighting 99.9% of files.
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    j += 1;
                } else {
                    break;
                }
            }
            let word = &line[start..j];
            let kind = classify_ident(word, bytes, j);
            push(&mut spans, start, j, kind);
            i = j;
            continue;
        }

        // Anything else — whitespace, punctuation, operators — carries the
        // default color. Let the `push` helper coalesce runs of Text.
        push(&mut spans, i, i + 1, TokenKind::Text);
        i += 1;
    }

    (spans, state)
}

/// Scan bytes for block-comment nesting, starting at `from` with `depth`
/// already open. Returns `(end_index, remaining_depth)` where `end_index`
/// is the byte just past the closing `*/` (or `bytes.len()` if the comment
/// ran off the end of the line).
fn advance_block_comment(bytes: &[u8], from: usize, mut depth: usize) -> (usize, usize) {
    let mut j = from;
    while depth > 0 && j + 1 < bytes.len() {
        if bytes[j] == b'/' && bytes[j + 1] == b'*' {
            depth += 1;
            j += 2;
        } else if bytes[j] == b'*' && bytes[j + 1] == b'/' {
            depth -= 1;
            j += 2;
        } else {
            j += 1;
        }
    }
    if depth > 0 {
        (bytes.len(), depth)
    } else {
        (j, 0)
    }
}

/// Decide whether an ASCII-ident word is a keyword, primitive, type
/// (UpperCamelCase), macro invocation (followed by `!`), or plain text.
fn classify_ident(word: &str, bytes: &[u8], end: usize) -> TokenKind {
    // Macro call: identifier followed immediately by `!` (but not `!=`).
    if end < bytes.len() && bytes[end] == b'!' && (end + 1 == bytes.len() || bytes[end + 1] != b'=')
    {
        return TokenKind::Macro;
    }
    if KEYWORDS.contains(&word) {
        return TokenKind::Keyword;
    }
    if PRIMITIVES.contains(&word) {
        return TokenKind::Type;
    }
    // UpperCamelCase → type. Single-letter uppercase (`T`, `E`) counts too
    // since those are conventional type-parameter names.
    if word.starts_with(|c: char| c.is_ascii_uppercase()) {
        return TokenKind::Type;
    }
    TokenKind::Text
}

/// Return the exclusive end byte-index of a `'x'`/`'\\n'`/etc. char literal
/// starting at `start` (where `bytes[start] == b'\''`), or `None` if the slice
/// is better interpreted as a lifetime or broken source.
fn char_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes[start], b'\'');
    let rest = &bytes[start + 1..];
    if rest.is_empty() {
        return None;
    }
    // `'\x' ... '`
    if rest[0] == b'\\' {
        // minimum is `'\n'` = 4 bytes; allow up to 8 for `'\u{1F600}'`-ish.
        // Find the closing quote within the next 10 bytes.
        let slice = &rest[..rest.len().min(10)];
        for (k, &b) in slice.iter().enumerate().skip(1) {
            if b == b'\'' {
                return Some(start + 1 + k + 1);
            }
        }
        return None;
    }
    // Ordinary char: either a single ASCII byte or a UTF-8 char; require a
    // closing quote within the next 5 bytes (covers 4-byte UTF-8 max).
    let slice = &rest[..rest.len().min(5)];
    for (k, &b) in slice.iter().enumerate().skip(1) {
        if b == b'\'' {
            return Some(start + 1 + k + 1);
        }
    }
    None
}

/// Return the exclusive end index of a lifetime name starting at `bytes[start] == b'\''`.
/// Lifetimes are `'` followed by one-or-more ASCII ident chars and NOT followed
/// immediately by another `'` (which would indicate a char literal instead).
fn lifetime_end(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes[start], b'\'');
    let rest = &bytes[start + 1..];
    if rest.is_empty() {
        return None;
    }
    if !(rest[0].is_ascii_alphabetic() || rest[0] == b'_') {
        return None;
    }
    let mut k = 1usize;
    while k < rest.len() && (rest[k].is_ascii_alphanumeric() || rest[k] == b'_') {
        k += 1;
    }
    // Char literal `'a'` would put a `'` immediately after the letter; reject.
    if k < rest.len() && rest[k] == b'\'' {
        return None;
    }
    Some(start + 1 + k)
}

/// Return exclusive end index of a raw/byte/byte-raw string literal head
/// starting at `start`. Supports: `r"..."`, `r#"..."#`, `b"..."`, `br"..."`,
/// `br#"..."#`. Returns `None` if this isn't a raw/byte string head after all.
fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start;
    // leading `b` / `br` / `r`
    if bytes[j] == b'b' {
        j += 1;
        if j < bytes.len() && bytes[j] == b'r' {
            j += 1;
        }
    } else if bytes[j] == b'r' {
        j += 1;
    } else {
        return None;
    }
    // optional `#`s
    let hash_start = j;
    while j < bytes.len() && bytes[j] == b'#' {
        j += 1;
    }
    let hashes = j - hash_start;
    if j >= bytes.len() || bytes[j] != b'"' {
        return None;
    }
    j += 1;
    // body: scan for `"` followed by matching number of `#`s.
    while j < bytes.len() {
        if bytes[j] == b'"' {
            let mut k = 1usize;
            while k <= hashes && j + k < bytes.len() && bytes[j + k] == b'#' {
                k += 1;
            }
            if k > hashes {
                return Some(j + 1 + hashes);
            }
        }
        j += 1;
    }
    Some(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenKind as T;

    fn kinds(line: &str) -> Vec<T> {
        tokenize_line(line).into_iter().map(|s| s.kind).collect()
    }

    fn spans_with_text(line: &str) -> Vec<(&str, T)> {
        tokenize_line(line).into_iter().map(|s| (&line[s.start..s.end], s.kind)).collect()
    }

    #[test]
    fn empty_line_yields_no_spans() {
        assert!(tokenize_line("").is_empty());
    }

    #[test]
    fn spans_cover_every_byte_and_are_ordered() {
        let line = "fn main() { let x = 42; }";
        let spans = tokenize_line(line);
        assert!(!spans.is_empty());
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans.last().unwrap().end, line.len());
        for pair in spans.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "gap between spans: {pair:?}");
            assert!(pair[0].start < pair[0].end, "empty span at {:?}", pair[0]);
        }
    }

    #[test]
    fn keywords_detected() {
        let s = spans_with_text("let mut x = if true { 1 } else { 2 };");
        assert!(s.contains(&("let", T::Keyword)));
        assert!(s.contains(&("mut", T::Keyword)));
        assert!(s.contains(&("if", T::Keyword)));
        assert!(s.contains(&("true", T::Keyword)));
        assert!(s.contains(&("else", T::Keyword)));
    }

    #[test]
    fn primitive_and_user_types() {
        let s = spans_with_text("fn f(n: u32) -> Result { }");
        assert!(s.contains(&("u32", T::Type)));
        assert!(s.contains(&("Result", T::Type)));
    }

    #[test]
    fn macro_vs_not_equal() {
        let s = spans_with_text("println!(\"hi\"); if a != b {}");
        assert!(s.contains(&("println", T::Macro)));
        // `a` is not a macro call despite `!=` appearing later. Text runs are
        // coalesced, so `a` lives inside a larger Text span together with the
        // surrounding spaces + `!=`.
        assert!(
            !s.iter().any(|(t, k)| *k == T::Macro && *t == "a"),
            "`a` must not be tagged as Macro: {s:?}"
        );
    }

    #[test]
    fn strings_and_chars() {
        let s = spans_with_text(r#"let s = "hi"; let c = 'a'; let esc = "\"inner\"";"#);
        assert!(s.iter().any(|(t, k)| *k == T::String && *t == r#""hi""#));
        assert!(s.iter().any(|(t, k)| *k == T::Char && *t == "'a'"));
        assert!(s.iter().any(|(t, k)| *k == T::String && t.starts_with("\"\\\"")));
    }

    #[test]
    fn raw_strings_with_hashes() {
        let s = spans_with_text(r###"let p = r#"con\tains no escapes"#;"###);
        assert!(s.iter().any(|(t, k)| *k == T::String && t.starts_with(r##"r#""##)));
    }

    #[test]
    fn byte_strings() {
        let s = spans_with_text(r##"let b = b"raw bytes"; let br = br#"raw bytes"#;"##);
        assert!(s.iter().any(|(t, k)| *k == T::String && t.starts_with("b\"")));
        assert!(s.iter().any(|(t, k)| *k == T::String && t.starts_with("br#\"")));
    }

    #[test]
    fn lifetimes_versus_chars() {
        let s = spans_with_text("fn f<'a>(x: &'a str) {}");
        assert!(s.iter().any(|(t, k)| *k == T::Lifetime && *t == "'a"));
        // 'a in `<'a>` and in `&'a str` should BOTH be lifetimes (not char).
        let lifetimes = s.iter().filter(|(_, k)| *k == T::Lifetime).count();
        assert_eq!(lifetimes, 2);
    }

    #[test]
    fn numbers_with_underscores_and_suffix() {
        let s = spans_with_text("let n = 1_000_000u64; let f = 3.14;");
        assert!(s.iter().any(|(t, k)| *k == T::Number && *t == "1_000_000u64"));
        assert!(s.iter().any(|(t, k)| *k == T::Number && *t == "3.14"));
    }

    #[test]
    fn line_comment_swallows_rest_of_line() {
        let s = spans_with_text("let x = 1; // trailing comment");
        let last = s.last().unwrap();
        assert_eq!(last.1, T::Comment);
        assert!(last.0.starts_with("//"));
    }

    #[test]
    fn block_comment_closed_on_same_line() {
        let s = spans_with_text("let /* hidden */ x = 1;");
        assert!(s.iter().any(|(t, k)| *k == T::Comment && *t == "/* hidden */"));
    }

    #[test]
    fn block_comment_unclosed_runs_to_eol() {
        let s = spans_with_text("let /* unterminated ");
        let last = s.last().unwrap();
        assert_eq!(last.1, T::Comment);
        assert!(last.0.starts_with("/*"));
    }

    #[test]
    fn block_comment_multi_line_state_tracks_depth() {
        let (_s1, state1) = tokenize_line_with_state("let x = /* start", LineState::default());
        assert_eq!(state1.block_comment_depth, 1);

        // Line 2: entirely inside comment, still open.
        let line2 = "still inside comment";
        let (s2, state2) = tokenize_line_with_state(line2, state1);
        assert_eq!(s2.len(), 1);
        assert_eq!(s2[0].kind, T::Comment);
        assert_eq!(s2[0].start, 0);
        assert_eq!(s2[0].end, line2.len());
        assert_eq!(state2.block_comment_depth, 1);

        // Line 3: closes the comment — depth back to 0, remaining code lexes.
        let line3 = "closing */ let y = 2;";
        let (s3, state3) = tokenize_line_with_state(line3, state2);
        assert_eq!(state3.block_comment_depth, 0);
        assert!(s3.iter().any(|span| span.kind == T::Comment));
        assert!(s3.iter().any(|span| span.kind == T::Keyword));
    }

    #[test]
    fn block_comment_nested_depth_increments() {
        let (_s, state) = tokenize_line_with_state("a /* outer /* inner", LineState::default());
        assert_eq!(state.block_comment_depth, 2);

        let (_s2, state2) = tokenize_line_with_state("*/ still open", state);
        assert_eq!(state2.block_comment_depth, 1);

        let (_s3, state3) = tokenize_line_with_state("*/ closed", state2);
        assert_eq!(state3.block_comment_depth, 0);
    }

    #[test]
    fn stateful_api_matches_stateless_on_plain_lines() {
        let line = "let x: u32 = 42;";
        let stateless = tokenize_line(line);
        let (stateful, _) = tokenize_line_with_state(line, LineState::default());
        assert_eq!(stateless, stateful);
    }

    #[test]
    fn attribute_detected() {
        let s = spans_with_text("#[derive(Debug, Clone)] struct S;");
        assert!(s.iter().any(|(t, k)| *k == T::Attribute && *t == "#[derive(Debug, Clone)]"));
        assert!(s.iter().any(|(t, k)| *k == T::Keyword && *t == "struct"));
    }

    #[test]
    fn inner_attribute_detected() {
        let s = spans_with_text("#![forbid(unsafe_code)]");
        assert!(s.iter().any(|(t, k)| *k == T::Attribute && *t == "#![forbid(unsafe_code)]"));
    }

    #[test]
    fn text_runs_coalesced() {
        // Three punctuation bytes in a row should collapse into ONE Text span.
        let spans = tokenize_line("();");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, T::Text);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].end, 3);
    }

    #[test]
    fn utf8_in_string_does_not_split_spans() {
        let line = "let s = \"café\";";
        let spans = tokenize_line(line);
        assert!(spans.iter().any(|s| s.kind == T::String));
        // Every span boundary must land on a UTF-8 char boundary.
        for span in &spans {
            assert!(line.is_char_boundary(span.start));
            assert!(line.is_char_boundary(span.end));
        }
    }

    #[test]
    fn kinds_covers_sane_sample() {
        // Smoke: a real-looking line covers all major categories.
        let k = kinds("pub fn add(a: i32, b: i32) -> i32 { a + b }");
        assert!(k.contains(&T::Keyword));
        assert!(k.contains(&T::Type));
        assert!(k.contains(&T::Text));
    }
}
