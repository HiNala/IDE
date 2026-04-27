//! Optional golden: place `docs/assets/cursor_style_reference.png` (Cursor-style shell screenshot)
//! for manual / future pixel diff tooling. CI does not require the file.

use std::path::PathBuf;

#[test]
fn optional_cursor_style_reference_png() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let p = repo_root.join("docs/assets/cursor_style_reference.png");
    if p.is_file() {
        let len = std::fs::metadata(&p).expect("stat").len();
        assert!(len > 8 * 1024, "reference PNG should be non-trivial ({p:?})");
    }
}
