# editor-syntax

Minimal per-language tokenizers that produce `Vec<TokenSpan>` for a single
line of source text. Used by `editor-render` to drive per-run colors in
`cosmic_text::Buffer::set_rich_text`.

## Status

- **Rust** — hand-written lexer covering keywords, types (heuristic),
  identifiers, strings / chars, line + block comments, numbers, attributes,
  macro calls, operators, punctuation.

## Non-goals (for now)

- Full `tree-sitter` integration. The mission doc calls for it; this crate
  is the regex-free fallback that makes M15 shippable on day one and gives
  `editor-render` a stable API to migrate onto `tree-sitter` later.
- Semantic highlighting (type inference, trait lookups). Out of scope.
- Incremental reparse. Each line is tokenized independently so a single
  edit only reshapes one line.
