//! Integration tests for multi-buffer + workspace path coherence (M13).
//! These run without `editor-app` or a GPU — they lock in `BufferManager` + `Workspace` invariants.

use std::fs;

use editor_workspace::{BufferManager, Workspace};
use tempfile::tempdir;

#[test]
fn find_by_path_accepts_equivalent_path_spellings() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file = dir.path().join("doc.txt");
    fs::write(&file, "one\n")?;

    let mut mgr = BufferManager::new();
    let id = mgr.open_file(&file)?;
    let canon = file.canonicalize()?;

    assert_eq!(mgr.find_by_path(&file), Some(id));
    assert_eq!(mgr.find_by_path(&canon), Some(id));
    assert!(mgr.get(id).is_some_and(|s| s.path.is_some()));
    Ok(())
}

#[test]
fn reopen_same_path_reuses_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file = dir.path().join("x.txt");
    fs::write(&file, "a\n")?;

    let mut mgr = BufferManager::new();
    let first = mgr.open_file(&file)?;
    mgr.create_empty();
    let second = mgr.open_file(&file)?;
    assert_eq!(first, second);
    assert_eq!(mgr.len(), 2);
    Ok(())
}

#[test]
fn workspace_root_covers_open_file_for_walk() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let root = dir.path();
    let nested = root.join("src");
    fs::create_dir_all(&nested)?;
    let file = nested.join("main.rs");
    fs::write(&file, "fn main() {}\n")?;

    let ws = Workspace::open(root)?;
    let paths: Vec<_> = ws.walk_files()?.into_iter().map(|e| e.relative).collect();
    assert!(
        paths.iter().any(|r| r == &std::path::PathBuf::from("src/main.rs")),
        "walk_files should list src/main.rs, got {paths:?}"
    );

    let mut mgr = BufferManager::new();
    mgr.open_file(&file)?;
    assert!(mgr.active().is_some());
    Ok(())
}

#[test]
fn mru_next_prev_round_trips_two_buffers() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "a\n")?;
    fs::write(&b, "b\n")?;

    let mut mgr = BufferManager::new();
    let id_a = mgr.open_file(&a)?;
    let id_b = mgr.open_file(&b)?;
    assert_eq!(mgr.active(), Some(id_b));

    mgr.next_buffer();
    assert_eq!(mgr.active(), Some(id_a));
    mgr.next_buffer();
    assert_eq!(mgr.active(), Some(id_b));

    mgr.prev_buffer();
    assert_eq!(mgr.active(), Some(id_a));
    mgr.prev_buffer();
    assert_eq!(mgr.active(), Some(id_b));
    Ok(())
}
