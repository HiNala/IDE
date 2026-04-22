//! Single-line JSON tokenizer.
//!
//! Covers the RFC 8259 grammar adequately for highlight purposes — strict
//! validation is the JSON parser's job, not ours. Recognizes:
//!   - Object keys (string immediately followed by `:`) colored as `Attribute`.
//!   - String values colored as `String`. Escape sequences (`\"`, `\\`) don't
//!     terminate the span.
//!   - Numbers: `-?`, integer, optional fraction, optional exponent.
//!   - Literals `true` / `false` / `null` colored as `Keyword`.
//!   - Line comments `//` and block comments `/* ... */` — not valid JSON
//!     but ubiquitous in JSON5 / tsconfig / vscode settings, so we color them.
//!   - Punctuation (`,`, `:`, `{`, `}`, `[`, `]`) stays as `Text`.

use crate::{TokenKind, TokenSpan};

/// Tokenize one line of JSON into [`TokenSpan`]s.
#[must_use]
pub fn tokenize_line(line: &str) -> Vec<TokenSpan> {
    if line.is_empty() {
        return Vec::new();
    }
    let bytes = line.as_bytes();
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

    while i < bytes.len() {
        let b = bytes[i];

        // Block comment (JSON5 / tsconfig extension). Single-line form only here.
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let start = i;
            let mut j = i + 2;
            while j + 1 < bytes.len() {
                if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                    j += 2;
                    push(&mut spans, start, j, TokenKind::Comment);
                    i = j;
                    break;
                }
                j += 1;
            }
            if i != start {
                continue;
            }
            // Unterminated — rest of line is comment.
            push(&mut spans, start, bytes.len(), TokenKind::Comment);
            return spans;
        }

        // Line comment — swallow to EOL.
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            push(&mut spans, i, bytes.len(), TokenKind::Comment);
            return spans;
        }

        // String. Look past the closing quote for `:` to decide key vs value.
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
            // Peek forward skipping whitespace for `:` → this string is a key.
            let mut k = j;
            while k < bytes.len() && matches!(bytes[k], b' ' | b'\t') {
                k += 1;
            }
            let kind = if k < bytes.len() && bytes[k] == b':' {
                TokenKind::Attribute
            } else {
                TokenKind::String
            };
            push(&mut spans, start, j, kind);
            i = j;
            continue;
        }

        // Number: optional `-`, digits, optional fraction, optional exponent.
        if b.is_ascii_digit() || (b == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_digit()
                    || c == b'.'
                    || c == b'e'
                    || c == b'E'
                    || c == b'+'
                    || c == b'-'
                {
                    j += 1;
                } else {
                    break;
                }
            }
            push(&mut spans, start, j, TokenKind::Number);
            i = j;
            continue;
        }

        // Literal true / false / null.
        if b.is_ascii_alphabetic() {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                j += 1;
            }
            let word = &line[start..j];
            let kind = match word {
                "true" | "false" | "null" => TokenKind::Keyword,
                _ => TokenKind::Text,
            };
            push(&mut spans, start, j, kind);
            i = j;
            continue;
        }

        push(&mut spans, i, i + 1, TokenKind::Text);
        i += 1;
    }

    spans
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
    fn keys_color_as_attribute() {
        let s = spans(r#""name": "ide","#);
        assert!(s.iter().any(|&(t, k)| t == r#""name""# && k == T::Attribute));
        assert!(s.iter().any(|&(t, k)| t == r#""ide""# && k == T::String));
    }

    #[test]
    fn value_string_without_colon_is_plain_string() {
        let s = spans(r#"["a", "b"]"#);
        let strings: Vec<_> = s.iter().filter(|(_, k)| *k == T::String).collect();
        assert_eq!(strings.len(), 2);
    }

    #[test]
    fn numbers_cover_negative_decimal_exponent() {
        for literal in ["42", "-7", "3.14", "1e10", "2.5e-3"] {
            let s = spans(literal);
            assert_eq!(s.len(), 1, "literal {literal}");
            assert_eq!(s[0].1, T::Number, "literal {literal}");
        }
    }

    #[test]
    fn true_false_null_are_keywords() {
        for literal in ["true", "false", "null"] {
            let s = spans(literal);
            assert_eq!(s[0], (literal, T::Keyword));
        }
    }

    #[test]
    fn line_comment_swallows_rest_of_line() {
        let s = spans(r#"{} // trailing"#);
        assert!(s.iter().any(|&(t, k)| t == "// trailing" && k == T::Comment));
    }

    #[test]
    fn block_comment_single_line_closed() {
        let s = spans(r#"1 /* mid */ 2"#);
        assert!(s.iter().any(|&(t, k)| t == "/* mid */" && k == T::Comment));
    }

    #[test]
    fn block_comment_unterminated_runs_to_eol() {
        let s = spans(r#"1 /* never closed"#);
        assert!(s.iter().any(|&(t, k)| t == "/* never closed" && k == T::Comment));
    }

    #[test]
    fn escaped_quote_does_not_terminate_string() {
        let s = spans(r#""a\"b""#);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, T::String);
        assert_eq!(s[0].0, r#""a\"b""#);
    }

    #[test]
    fn punctuation_stays_text() {
        let s = spans("{}");
        assert!(s.iter().all(|(_, k)| *k == T::Text));
    }

    #[test]
    fn spans_cover_every_byte() {
        let line = r#"{"a": [1, true, null, "x"]}"#;
        let out = tokenize_line(line);
        let mut cursor = 0;
        for s in &out {
            assert_eq!(s.start, cursor);
            cursor = s.end;
        }
        assert_eq!(cursor, line.len());
    }
}
