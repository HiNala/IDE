//! `editor-syntax` — tiny lexers that emit [`TokenSpan`]s for one line.
//!
//! This crate is intentionally minimal: no `tree-sitter`, no regex engine,
//! no stateful parser. Each line is tokenized independently, which lets the
//! renderer recolor a single edit in `O(line_len)` without coordinating
//! with a parse tree.
//!
//! A richer tree-sitter backend can later replace [`rust::tokenize_line`]
//! behind the same [`TokenSpan`] contract without touching callers.

#![forbid(unsafe_code)]

pub mod json;
pub mod markdown;
pub mod rust;
pub mod toml;

/// Semantic category of a token. Renderers map these to theme colors.
///
/// Only categories that actually affect visible coloring are kept —
/// punctuation and whitespace fall under [`TokenKind::Text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Default foreground (punctuation, identifiers, whitespace).
    Text,
    /// Reserved words: `fn`, `let`, `match`, `pub`, `impl`, `use`, etc.
    Keyword,
    /// Primitive or user-defined type identifiers (heuristic: `UpperCamelCase`).
    Type,
    /// String literals (`"..."`, raw strings, byte strings).
    String,
    /// Char literals (`'x'`, `'\n'`, lifetimes intentionally excluded).
    Char,
    /// Line comments (`// ...`) + block comments (`/* ... */`).
    Comment,
    /// Numeric literals (`42`, `0xff`, `3.14`, `1_000`).
    Number,
    /// Attribute heads (`#[derive(Clone)]`, `#![forbid(unsafe_code)]`).
    Attribute,
    /// Macro invocations (`println!`, `vec!`).
    Macro,
    /// Lifetime identifiers (`'static`, `'a`).
    Lifetime,
}

/// A contiguous half-open byte range with a semantic color tag.
///
/// Ranges are line-relative: `start` and `end` are byte offsets into the
/// single-line string passed to the tokenizer (and therefore always < line_len).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

impl TokenSpan {
    #[must_use]
    pub const fn new(start: usize, end: usize, kind: TokenKind) -> Self {
        Self { start, end, kind }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Per-line lexer state carried between adjacent lines.
///
/// Languages that need multi-line context (Rust's nested `/* ... */`) read and
/// write this. Languages without multi-line constructs leave it untouched —
/// callers can still feed the same `LineState` through every line and pay no
/// extra cost.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineState {
    /// Depth of `/* ... */` nesting open at the start of the line. 0 means
    /// "not inside a block comment". Only used by [`Language::Rust`].
    pub block_comment_depth: u32,
}

/// Supported source languages. `Plain` yields a single `Text` span and is
/// the default fallback for unknown / binary files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Plain,
    Rust,
    Toml,
    Json,
    Markdown,
}

impl Language {
    /// Infer a language from a file extension (case-insensitive).
    ///
    /// Unknown extensions map to [`Language::Plain`].
    #[must_use]
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "rs" => Self::Rust,
            "toml" => Self::Toml,
            "json" | "jsonc" | "json5" => Self::Json,
            "md" | "markdown" | "mdown" | "mkd" => Self::Markdown,
            _ => Self::Plain,
        }
    }

    /// Match well-known config filenames (e.g. `Cargo.lock` uses TOML syntax).
    #[must_use]
    fn from_filename(name: &str) -> Option<Self> {
        match name {
            "Cargo.lock" | "rust-toolchain" => Some(Self::Toml),
            _ => None,
        }
    }

    /// Infer a language from a file path. Convenience wrapper around
    /// [`Self::from_extension`] for callers that already hold a `Path`.
    #[must_use]
    pub fn from_path(path: &std::path::Path) -> Self {
        if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
            if let Some(lang) = Self::from_filename(name) {
                return lang;
            }
        }
        path.extension().and_then(|e| e.to_str()).map_or(Self::Plain, Self::from_extension)
    }

    /// Tokenize `line` into non-empty, non-overlapping spans covering every
    /// byte in the input. Callers can trust `spans[i].end == spans[i+1].start`.
    ///
    /// Plain text returns a single `Text` span for the whole line.
    ///
    /// Stateless; for coherent multi-line block comments use
    /// [`Self::tokenize_line_with_state`].
    #[must_use]
    pub fn tokenize_line(self, line: &str) -> Vec<TokenSpan> {
        self.tokenize_line_with_state(line, LineState::default()).0
    }

    /// Tokenize `line` using the state carried from the previous line. Returns
    /// the spans plus the state leaving this line — feed that state into the
    /// next call to keep multi-line constructs coherent.
    #[must_use]
    pub fn tokenize_line_with_state(
        self,
        line: &str,
        state: LineState,
    ) -> (Vec<TokenSpan>, LineState) {
        match self {
            Self::Plain => {
                if line.is_empty() {
                    (Vec::new(), state)
                } else {
                    (vec![TokenSpan::new(0, line.len(), TokenKind::Text)], state)
                }
            }
            Self::Rust => rust::tokenize_line_with_state(line, state),
            // Languages with no multi-line state pass it through unchanged.
            Self::Toml => (toml::tokenize_line(line), state),
            Self::Json => (json::tokenize_line(line), state),
            Self::Markdown => (markdown::tokenize_line(line), state),
        }
    }
}

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_from_extension_covers_rs() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("RS"), Language::Rust);
        assert_eq!(Language::from_extension("toml"), Language::Toml);
        assert_eq!(Language::from_extension("json"), Language::Json);
        assert_eq!(Language::from_extension("JSONC"), Language::Json);
        assert_eq!(Language::from_extension("md"), Language::Markdown);
        assert_eq!(Language::from_extension("MARKDOWN"), Language::Markdown);
        assert_eq!(Language::from_extension("txt"), Language::Plain);
        assert_eq!(Language::from_extension(""), Language::Plain);
    }

    #[test]
    fn language_from_path_picks_extension() {
        use std::path::Path;
        assert_eq!(Language::from_path(Path::new("src/main.rs")), Language::Rust);
        assert_eq!(Language::from_path(Path::new("Cargo.toml")), Language::Toml);
        assert_eq!(Language::from_path(Path::new("Cargo.lock")), Language::Toml);
        assert_eq!(Language::from_path(Path::new("README.md")), Language::Markdown);
        assert_eq!(Language::from_path(Path::new("LICENSE")), Language::Plain);
    }

    #[test]
    fn plain_tokenize_empty() {
        assert!(Language::Plain.tokenize_line("").is_empty());
    }

    #[test]
    fn plain_tokenize_whole_line() {
        let spans = Language::Plain.tokenize_line("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].end, 11);
        assert_eq!(spans[0].kind, TokenKind::Text);
    }
}
