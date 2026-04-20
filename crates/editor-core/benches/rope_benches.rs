//! Criterion benchmarks for rope hot paths (M02).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use editor_core::{BytePos, TextBuffer};

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

criterion_group!(
    benches,
    bench_insert_end,
    bench_byte_to_line_col,
    bench_line_iteration,
    bench_snapshot_clone
);
criterion_main!(benches);
