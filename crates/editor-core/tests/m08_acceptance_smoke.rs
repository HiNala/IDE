//! Fast smoke toward M08 / MVP acceptance targets: large-buffer behavior without a GUI.
//!
//! Full p99 latency and 100 MB stress runs stay manual / Criterion; this guards regressions in CI.

use std::time::Instant;

use editor_core::{BytePos, EditKind, TextBuffer, UndoStack};

/// Enough text to catch pathological O(n²) behavior on naive buffers (~16 MiB).
const SMOKE_BYTES: usize = 16 * 1024 * 1024;

#[test]
fn large_buffer_edits_complete_under_budget() {
    let line: String = "x".repeat(79) + "\n";
    let mut raw = String::new();
    while raw.len() + line.len() <= SMOKE_BYTES {
        raw.push_str(&line);
    }

    let t0 = Instant::now();
    let mut buf = TextBuffer::from_str(&raw);
    let build_elapsed = t0.elapsed();
    assert!(
        build_elapsed < std::time::Duration::from_secs(10),
        "building rope from {} bytes took {:?}",
        raw.len(),
        build_elapsed
    );

    let end = BytePos(buf.len_bytes());
    let t1 = Instant::now();
    buf.byte_to_line_col(end).expect("byte_to_line_col at end");
    let nav_elapsed = t1.elapsed();
    assert!(
        nav_elapsed < std::time::Duration::from_secs(2),
        "line/col at EOF took {nav_elapsed:?}"
    );

    let mut undo = UndoStack::default();
    let t2 = Instant::now();
    let edit =
        buf.apply_edit(EditKind::Insert { pos: end, text: "tail\n".into() }).expect("append");
    undo.push(edit);
    let len = buf.len_bytes();
    undo.undo(&mut buf).expect("undo append");
    let edit_elapsed = t2.elapsed();
    assert!(buf.len_bytes() < len);
    assert!(
        edit_elapsed < std::time::Duration::from_secs(5),
        "append + undo on large buffer took {edit_elapsed:?}"
    );
}

/// MVP acceptance stress (NF-05 style): ~100 MiB of text in memory — not run in CI by default.
#[test]
#[ignore = "manual stress (~100 MiB RAM): cargo test -p editor-core --test m08_acceptance_smoke stress_100mb_buffer_smoke -- --ignored --nocapture"]
fn stress_100mb_buffer_smoke() {
    const STRESS_BYTES: usize = 100 * 1024 * 1024;
    let line: String = "x".repeat(79) + "\n";
    let mut raw = String::new();
    raw.reserve(STRESS_BYTES.saturating_add(line.len()));
    while raw.len() + line.len() <= STRESS_BYTES {
        raw.push_str(&line);
    }

    let t0 = Instant::now();
    let buf = TextBuffer::from_str(&raw);
    let build_elapsed = t0.elapsed();
    assert!(
        build_elapsed < std::time::Duration::from_secs(180),
        "building rope from {} bytes took {:?}",
        raw.len(),
        build_elapsed
    );

    let end = BytePos(buf.len_bytes());
    buf.byte_to_line_col(end).expect("byte_to_line_col at end of 100MB buffer");
}
