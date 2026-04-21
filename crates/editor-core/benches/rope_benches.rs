//! Criterion benchmarks for rope hot paths (M02).
//!
//! Large-buffer targets from the mission (e.g. ~10 MB) are scaled down here so
//! `cargo bench -p editor-core` stays practical on laptops; order-of-magnitude
//! checks still hold. Save a named baseline locally:  
//! `cargo bench -p editor-core -- --save-baseline m02-mvp`

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use editor_core::{BytePos, Cursor, CursorMotion, EditKind, TextBuffer, UndoStack};

fn bench_insert_end(c: &mut Criterion) {
    c.bench_function("text_buffer_insert_end_10k", |b| {
        b.iter_batched(
            TextBuffer::new,
            |mut buf| {
                for _ in 0..10_000 {
                    buf.insert(BytePos(buf.len_bytes()), "a").unwrap();
                }
                black_box(buf.len_bytes());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_byte_to_line_col(c: &mut Criterion) {
    let buf = TextBuffer::from_str(&(0..50_000).map(|_| "x").collect::<String>());
    c.bench_function("byte_to_line_col_mid", |b| {
        b.iter(|| {
            let p = BytePos(buf.len_bytes() / 2);
            black_box(buf.byte_to_line_col(p).unwrap());
        });
    });
}

fn bench_line_iteration(c: &mut Criterion) {
    let buf = TextBuffer::from_str(&(0..10_000).map(|_| "x\n").collect::<String>());
    c.bench_function("len_lines_10k", |b| {
        b.iter(|| black_box(buf.len_lines()));
    });
}

fn bench_snapshot_clone(c: &mut Criterion) {
    let buf = TextBuffer::from_str(&(0..100_000).map(|_| "y").collect::<String>());
    c.bench_function("snapshot_clone", |b| {
        b.iter(|| black_box(buf.snapshot()));
    });
}

/// ~0.5M UTF-8 bytes: random single-byte inserts (incoherent positions).
/// Sequential inserts at buffer end (coherent cursor); M07 regression gate target name.
fn bench_insert_random_coherent(c: &mut Criterion) {
    c.bench_function("insert_random_coherent_500", |b| {
        b.iter(|| {
            let mut buf = TextBuffer::new();
            for _ in 0..500 {
                buf.insert(BytePos(buf.len_bytes()), "z").unwrap();
            }
            black_box(buf.len_bytes());
        });
    });
}

fn bench_insert_random_incoherent(c: &mut Criterion) {
    let seed_buf = TextBuffer::from_str(&(0..500_000).map(|_| "a").collect::<String>());
    c.bench_function("insert_random_byte_incoherent_500", |b| {
        b.iter(|| {
            let mut buf = seed_buf.clone();
            for i in (0..500).step_by(7) {
                let p = (i * 997) % buf.len_bytes().max(1);
                buf.insert(BytePos(p), "z").unwrap();
            }
            black_box(buf.len_bytes());
        });
    });
}

fn bench_delete_random_ranges(c: &mut Criterion) {
    let base = TextBuffer::from_str(&(0..50_000).map(|_| "b").collect::<String>());
    c.bench_function("delete_random_ranges_200", |b| {
        b.iter(|| {
            let mut buf = base.clone();
            for i in 0..200 {
                let len = buf.len_bytes();
                if len < 2 {
                    break;
                }
                let start = (i * 131) % (len - 1);
                let del = 1 + (i % 20).min(len - start - 1);
                buf.delete_range(BytePos(start)..BytePos(start + del)).unwrap();
            }
            black_box(buf.len_bytes());
        });
    });
}

fn bench_cursor_motion_up_down(c: &mut Criterion) {
    let lines: String = (0..2_000).map(|_| "hello world\n").collect();
    let buf = TextBuffer::from_str(&lines);
    c.bench_function("cursor_up_down_8k", |b| {
        b.iter(|| {
            let mut c = Cursor::new(BytePos(buf.len_bytes() / 2));
            for _ in 0..8_000 {
                let _ = c.apply(CursorMotion::Up, &buf);
                let _ = c.apply(CursorMotion::Down, &buf);
            }
            black_box(c.pos().0);
        });
    });
}

fn bench_undo_push_with_checkpoint(c: &mut Criterion) {
    c.bench_function("undo_push_checkpoint_2k", |b| {
        b.iter(|| {
            let mut buf = TextBuffer::new();
            let mut u = UndoStack::default();
            for _ in 0..2_000 {
                let pos = BytePos(buf.len_bytes());
                let e = buf.apply_edit(EditKind::Insert { pos, text: "q".into() }).unwrap();
                u.push(e);
                u.checkpoint();
            }
            black_box(u.len_undo());
        });
    });
}

criterion_group!(
    benches,
    bench_insert_end,
    bench_byte_to_line_col,
    bench_line_iteration,
    bench_snapshot_clone,
    bench_insert_random_coherent,
    bench_insert_random_incoherent,
    bench_delete_random_ranges,
    bench_cursor_motion_up_down,
    bench_undo_push_with_checkpoint,
);
criterion_main!(benches);
