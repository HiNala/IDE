//! Single-line TOML tokenizer.
//!
//! Targets Cargo.toml / rustfmt.toml / clippy.toml / editorconfig-adjacent
//! files that dominate this workspace. Covers:
//!   - `[table.headers]` and `[[array.tables]]` — colored as `Type`.
//!   - Bare + dotted keys — colored as `Attribute` when they're on the LHS
//!     of `=`, plain `Text` otherwise.
//!   - Strings: basic `"..."`, literal `'...'`, multi-line tokens fall back
//!     to single-line rules (open quote colors the rest of the line).
//!   - Numbers, dates (heuristic: starts with 4 digits + `-`).
//!   - Booleans `true` / `false` — colored as `Keyword`.
//!   - Line comments `#`.

use crate::{TokenKind, TokenSpan};

/// Tokenize one line of TOML into [`TokenSpan`]s.
#[must_use]
pub fn tokenize_line(line: &str) -> Vec<TokenSpan> {
    if line.is_empty() {
        return Vec::new();
    }
    let bytes = line.as_bytes();
    let mut spans: Vec<TokenSpan> = Vec::with_capacity(8);
    let mut i = 0usize;
    // "Have we passed an `=` yet on this line?" flips key-detection off so the
    // value side doesn't get highlighted as if it were another key.
    let mut past_equals = false;

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

        // Comment swallows to EOL.
        if b == b'#' {
            push(&mut spans, i, bytes.len(), TokenKind::Comment);
            return spans;
        }

        // Table header: `[...]` / `[[...]]`. The entire bracketed span colors
        // as Type so `[dependencies.foo]` reads as one visual unit.
        if b == b'[' {
            let start = i;
            let mut j = i + 1;
            let mut depth = 1usize;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            push(&mut spans, start, j, TokenKind::Type);
            i = j;
            continue;
        }

        // Basic string `"..."` with `\"` escapes.
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

        // Literal string `'...'` — no escape processing.
        if b == b'\'' {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'\'' {
                j += 1;
            }
            if j < bytes.len() {
                j += 1;
            }
            push(&mut spans, start, j, TokenKind::String);
            i = j;
            continue;
        }

        // `=` — mark we've passed it so remaining identifiers aren't keys.
        if b == b'=' {
            past_equals = true;
            push(&mut spans, i, i + 1, TokenKind::Text);
            i += 1;
            continue;
        }

        // Numeric literal (also matches ISO dates in practice).
        if b.is_ascii_digit() || (b == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-' | b'+' | b':') {
                    j += 1;
                } else {
                    break;
                }
            }
            push(&mut spans, start, j, TokenKind::Number);
            i = j;
            continue;
        }

        // Bare identifier / key name. Dotted keys (`a.b.c`) consumed as one run
        // when BEFORE the `=`; after `=`, the same syntax could be a value.
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() {
                let c = bytes[j];
                // Before `=`: keys can have dots/dashes. After `=`: stop at `.`
                // so identifiers like `true` + dot + ident get handled right.
                let is_ident_byte = c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
                let is_dotted_key = !past_equals && c == b'.';
                if is_ident_byte || is_dotted_key {
                    j += 1;
                } else {
                    break;
                }
            }
            let word = &line[start..j];
            let kind = classify_ident(word, past_equals);
            push(&mut spans, start, j, kind);
            i = j;
            continue;
        }

        push(&mut spans, i, i + 1, TokenKind::Text);
        i += 1;
    }

    spans
}

fn classify_ident(word: &str, past_equals: bool) -> TokenKind {
    match word {
        "true" | "false" => TokenKind::Keyword,
        "inf" | "nan" => TokenKind::Number,
        _ if past_equals => TokenKind::Text,
        _ => TokenKind::Attribute,
    }
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
    fn table_header_colored_as_type() {
        let s = spans("[package]");
        assert!(s.iter().any(|(t, k)| *k == T::Type && *t == "[package]"));
    }

    #[test]
    fn nested_table_header() {
        let s = spans("[dependencies.serde]");
        assert!(s.iter().any(|(t, k)| *k == T::Type && *t == "[dependencies.serde]"));
    }

    #[test]
    fn array_of_tables_header() {
        let s = spans("[[bin]]");
        assert!(s.iter().any(|(t, k)| *k == T::Type && *t == "[[bin]]"));
    }

    #[test]
    fn simple_key_value_pair() {
        let s = spans(r#"name = "editor-syntax""#);
        assert!(s.iter().any(|(t, k)| *k == T::Attribute && *t == "name"));
        assert!(s.iter().any(|(t, k)| *k == T::String && *t == r#""editor-syntax""#));
    }

    #[test]
    fn dotted_key_stays_attribute() {
        let s = spans("package.edition = \"2021\"");
        assert!(s.iter().any(|(t, k)| *k == T::Attribute && *t == "package.edition"));
    }

    #[test]
    fn booleans_colored_as_keyword() {
        let s = spans("workspace = true");
        assert!(s.iter().any(|(t, k)| *k == T::Keyword && *t == "true"));
    }

    #[test]
    fn numbers_and_negative() {
        let s = spans("port = 8080");
        assert!(s.iter().any(|(t, k)| *k == T::Number && *t == "8080"));
        let s = spans("delta = -3");
        assert!(s.iter().any(|(t, k)| *k == T::Number && *t == "-3"));
    }

    #[test]
    fn comment_swallows_rest_of_line() {
        let s = spans("name = \"x\" # trailing");
        let last = s.last().unwrap();
        assert_eq!(last.1, T::Comment);
        assert!(last.0.starts_with('#'));
    }

    #[test]
    fn spans_cover_every_byte() {
        let line = r#"[package] name = "editor-syntax" # note"#;
        let sp = tokenize_line(line);
        assert_eq!(sp[0].start, 0);
        assert_eq!(sp.last().unwrap().end, line.len());
        for pair in sp.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn literal_string_single_quotes() {
        let s = spans(r"path = 'C:\temp\x.txt'");
        assert!(s.iter().any(|(t, k)| *k == T::String && *t == r"'C:\temp\x.txt'"));
    }
}
