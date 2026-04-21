//! Minimal integration smoke for `editor-terminal` (M26 expands this).

#[test]
fn banner_smoke() {
    assert!(!editor_terminal::banner().is_empty());
}
