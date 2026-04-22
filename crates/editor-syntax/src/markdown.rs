//! Single-line Markdown tokenizer.
//!
//! Markdown is a block language, but most visual cues are legible per-line:
//!   - ATX headings `#`-prefixed — entire line colors as `Keyword`.
//!   - Fenced code fences (` ``` ` / `~~~`) — the fence itself colors as `Type`.
//!     We don't carry "inside a code block" state here; a future multi-line
//!     pass can flip the whole line kind to `String` while inside.
//!   - List markers (`- `, `* `, `+ `, ordered `1. `) — marker as `Attribute`.
//!   - Blockquote `>` prefix — as `Comment`.
//!   - Inline backtick code spans `` `...` `` — contents as `String`.
//!   - Bold `**...**` / `__...__`, italic `*...*` / `_..._` — contents as
//!     `Keyword` / `Type` respectively (heuristic, avoids regex).
//!   - Link / image syntax `[text](url)` / `![alt](src)` — text as `String`,
//!     url as `Attribute`.
//!
//! Rules fire top-to-bottom; the first rule that matches at a byte position
//! wins.

use crate::{TokenKind, TokenSpan};

/// Tokenize one line of Markdown into [`TokenSpan`]s.
#[must_use]
pub fn tokenize_line(line: &str) -> Vec<TokenSpan> {
    if line.is_empty() {
        return Vec::new();
    }
    let bytes = line.as_bytes();

    // Leading whitespace offset — lets us match block prefixes after indent.
    let indent = bytes.iter().take_while(|&&c| c == b' ' || c == b'\t').count();

    // Heading: `#`, `##`, ... up to `######`, followed by space or EOL.
    if let Some(end) = heading_prefix(&bytes[indent..]) {
        let mut spans = Vec::with_capacity(2);
        if indent > 0 {
            spans.push(TokenSpan::new(0, indent, TokenKind::Text));
        }
        spans.push(TokenSpan::new(indent, indent + end, TokenKind::Keyword));
        if indent + end < bytes.len() {
            spans.push(TokenSpan::new(indent + end, bytes.len(), TokenKind::Keyword));
        }
        return spans;
    }

    // Fenced code block marker — entire line colors as Type.
    if is_fence(&bytes[indent..]) {
        let mut spans = Vec::with_capacity(2);
        if indent > 0 {
            spans.push(TokenSpan::new(0, indent, TokenKind::Text));
        }
        spans.push(TokenSpan::new(indent, bytes.len(), TokenKind::Type));
        return spans;
    }

    // Blockquote.
    if bytes.get(indent) == Some(&b'>') {
        let mut spans = Vec::with_capacity(2);
        if indent > 0 {
            spans.push(TokenSpan::new(0, indent, TokenKind::Text));
        }
        spans.push(TokenSpan::new(indent, bytes.len(), TokenKind::Comment));
        return spans;
    }

    // Body: tokenize inline markers.
    inline_tokens(line, bytes, indent)
}

/// Returns the number of `#` characters if this is an ATX heading, else None.
fn heading_prefix(bytes: &[u8]) -> Option<usize> {
    let hash_run = bytes.iter().take_while(|&&c| c == b'#').count();
    if hash_run == 0 || hash_run > 6 {
        return None;
    }
    // Must be followed by space or end of line.
    match bytes.get(hash_run) {
        Some(&b' ') | Some(&b'\t') | None => Some(hash_run),
        _ => None,
    }
}

/// Returns true if `bytes` starts with ```` ``` ```` or `~~~` (GFM fence).
fn is_fence(bytes: &[u8]) -> bool {
    let c = match bytes.first() {
        Some(&b) if b == b'`' || b == b'~' => b,
        _ => return false,
    };
    bytes.iter().take(3).filter(|&&b| b == c).count() == 3
}

fn inline_tokens(line: &str, bytes: &[u8], indent: usize) -> Vec<TokenSpan> {
    let mut spans: Vec<TokenSpan> = Vec::with_capacity(8);
    let mut i = 0usize;

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

    // List markers on the leading indent: `- `, `* `, `+ `, `N. `, `N) `.
    if i == 0 && indent < bytes.len() {
        if let Some(marker_end) = list_marker(&bytes[indent..]) {
            if indent > 0 {
                push(&mut spans, 0, indent, TokenKind::Text);
            }
            push(&mut spans, indent, indent + marker_end, TokenKind::Attribute);
            i = indent + marker_end;
        }
    }

    while i < bytes.len() {
        let b = bytes[i];

        // Inline code `...` — span as String, opening/closing backticks too.
        if b == b'`' {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'`' {
                j += 1;
            }
            if j < bytes.len() {
                j += 1;
            }
            push(&mut spans, start, j, TokenKind::String);
            i = j;
            continue;
        }

        // Image / link: `![alt](src)` or `[text](url)`.
        if (b == b'!' && bytes.get(i + 1) == Some(&b'['))
            || (b == b'[' && i > 0 && bytes[i - 1] != b'!')
            || (b == b'[' && i == 0)
        {
            if let Some((text_end, url_end)) = match_link(&bytes[i..]) {
                let text_start = if b == b'!' { i + 1 } else { i };
                // `!` (if present) + `[text]` colored as String.
                push(&mut spans, i, i + text_end, TokenKind::String);
                // `(url)` colored as Attribute. The match points url_end one
                // past the closing `)`.
                push(&mut spans, i + text_end, i + url_end, TokenKind::Attribute);
                i += url_end;
                let _ = text_start; // silence unused
                continue;
            }
        }

        // Bold `**...**` (two-char token).
        if b == b'*' && bytes.get(i + 1) == Some(&b'*') {
            if let Some(end) = scan_emphasis(&bytes[i + 2..], b'*', true) {
                let stop = i + 2 + end + 2;
                push(&mut spans, i, stop, TokenKind::Keyword);
                i = stop;
                continue;
            }
        }
        if b == b'_' && bytes.get(i + 1) == Some(&b'_') {
            if let Some(end) = scan_emphasis(&bytes[i + 2..], b'_', true) {
                let stop = i + 2 + end + 2;
                push(&mut spans, i, stop, TokenKind::Keyword);
                i = stop;
                continue;
            }
        }

        // Italic `*...*` or `_..._`.
        if b == b'*' || b == b'_' {
            if let Some(end) = scan_emphasis(&bytes[i + 1..], b, false) {
                let stop = i + 1 + end + 1;
                push(&mut spans, i, stop, TokenKind::Type);
                i = stop;
                continue;
            }
        }

        push(&mut spans, i, i + 1, TokenKind::Text);
        i += 1;
    }

    let _ = line; // keep signature clean; line is the byte source
    spans
}

/// Returns the byte count of a list marker (including trailing space) if
/// `bytes` starts with one.
fn list_marker(bytes: &[u8]) -> Option<usize> {
    if bytes.len() >= 2 && matches!(bytes[0], b'-' | b'*' | b'+') && bytes[1] == b' ' {
        return Some(2);
    }
    // Ordered list: digits + `.` or `)` + space.
    let digit_run = bytes.iter().take_while(|&&b| b.is_ascii_digit()).count();
    if digit_run > 0
        && bytes.len() > digit_run + 1
        && matches!(bytes[digit_run], b'.' | b')')
        && bytes[digit_run + 1] == b' '
    {
        return Some(digit_run + 2);
    }
    None
}

/// Byte offset (into `rest`) of the closing emphasis token of length `1` or
/// `2`. `wide = true` means look for two `marker` chars in a row.
fn scan_emphasis(rest: &[u8], marker: u8, wide: bool) -> Option<usize> {
    let mut j = 0;
    while j < rest.len() {
        if wide {
            if j + 1 < rest.len() && rest[j] == marker && rest[j + 1] == marker {
                return Some(j);
            }
        } else if rest[j] == marker {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// For a `[text](url)` (or `![alt](src)`) starting at the front of `bytes`,
/// return `(end_of_bracket_including_closing_bracket, end_of_paren_including_closing_paren)`
/// both offsets into `bytes`. Returns `None` if the shape doesn't match.
fn match_link(bytes: &[u8]) -> Option<(usize, usize)> {
    let start = if bytes.first() == Some(&b'!') { 1 } else { 0 };
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut j = start + 1;
    while j < bytes.len() && bytes[j] != b']' {
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    let text_end = j + 1; // past `]`
    if bytes.get(text_end) != Some(&b'(') {
        return None;
    }
    let mut k = text_end + 1;
    while k < bytes.len() && bytes[k] != b')' {
        k += 1;
    }
    if k >= bytes.len() {
        return None;
    }
    Some((text_end, k + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenKind as T;

    fn spans(line: &str) -> Vec<(&str, T)> {
        tokenize_line(line).into_iter().map(|s| (&line[s.start..s.end], s.kind)).collect()
    }

    #[test]
    fn empty_line() {
        assert!(tokenize_line("").is_empty());
    }

    #[test]
    fn heading_one_to_six() {
        for (prefix, text) in [("# ", "Title"), ("## ", "A"), ("### ", "A"), ("###### ", "Deepest")]
        {
            let line = format!("{prefix}{text}");
            let s = spans(&line);
            assert!(s.iter().any(|(_, k)| *k == T::Keyword), "heading `{line}`");
        }
    }

    #[test]
    fn not_heading_without_space() {
        let s = spans("#nospace");
        assert!(s.iter().all(|(_, k)| *k != T::Keyword));
    }

    #[test]
    fn fence_backtick_and_tilde() {
        assert!(spans("```rust").iter().any(|(_, k)| *k == T::Type));
        assert!(spans("~~~").iter().any(|(_, k)| *k == T::Type));
    }

    #[test]
    fn blockquote_whole_line_is_comment() {
        let s = spans("> quoted text");
        assert!(s.iter().any(|(t, k)| *k == T::Comment && t.starts_with(">")));
    }

    #[test]
    fn list_marker_dash() {
        let s = spans("- item");
        assert_eq!(s[0], ("- ", T::Attribute));
    }

    #[test]
    fn list_marker_ordered() {
        let s = spans("12. item");
        assert_eq!(s[0], ("12. ", T::Attribute));
    }

    #[test]
    fn list_marker_with_indent() {
        let s = spans("  * nested");
        assert_eq!(s[0], ("  ", T::Text));
        assert_eq!(s[1], ("* ", T::Attribute));
    }

    #[test]
    fn inline_code_spans() {
        let s = spans("use `cargo test` please");
        assert!(s.iter().any(|(t, k)| *k == T::String && t == &"`cargo test`"));
    }

    #[test]
    fn bold_and_italic() {
        let s = spans("a **bold** b *italic* c");
        assert!(s.iter().any(|(t, k)| *k == T::Keyword && t == &"**bold**"));
        assert!(s.iter().any(|(t, k)| *k == T::Type && t == &"*italic*"));
    }

    #[test]
    fn link_syntax() {
        let s = spans("see [docs](https://x.y)");
        assert!(s.iter().any(|(t, k)| *k == T::String && t == &"[docs]"));
        assert!(s.iter().any(|(t, k)| *k == T::Attribute && t == &"(https://x.y)"));
    }

    #[test]
    fn image_syntax() {
        let s = spans("![alt](img.png)");
        assert!(s.iter().any(|(t, k)| *k == T::String && t == &"![alt]"));
        assert!(s.iter().any(|(t, k)| *k == T::Attribute && t == &"(img.png)"));
    }

    #[test]
    fn spans_cover_every_byte() {
        for line in [
            "# heading",
            "plain line",
            "- item with `code` and **bold**",
            "> quoted",
            "1. first [link](u)",
            "```rust",
        ] {
            let out = tokenize_line(line);
            let mut cursor = 0;
            for s in &out {
                assert_eq!(s.start, cursor, "line `{line}`");
                cursor = s.end;
            }
            assert_eq!(cursor, line.len(), "line `{line}`");
        }
    }
}
