//! Throughput for sync load/save (M06). Fixtures are generated in-process.
//!
//! `cargo bench -p editor-io --bench file_io -- --save-baseline m06-mvp`

use std::hint::black_box;
use std::io::Write;

use criterion::{criterion_group, criterion_main, Criterion};
use editor_core::{LineEnding, TextBuffer};
use editor_io::{load_file_sync, save_file_sync, Encoding};

fn write_fixture(path: &std::path::Path, size: usize) {
    let mut f = std::fs::File::create(path).expect("create");
    let line = b"x".repeat(120);
    let mut written = 0usize;
    while written + line.len() + 1 < size {
        f.write_all(&line).expect("write");
        f.write_all(b"\n").expect("nl");
        written += line.len() + 1;
    }
}

fn bench_load_100k(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("a.txt");
    write_fixture(&p, 100_000);
    c.bench_function("load_file_sync_100kb", |b| {
        b.iter(|| black_box(load_file_sync(&p).expect("load")));
    });
}

fn bench_roundtrip_utf8(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("r.txt");
    let buf = TextBuffer::from_str(&(0..5000).map(|_| "a\n").collect::<String>());
    let snap = buf.snapshot();
    save_file_sync(&p, &snap, LineEnding::Lf, Encoding::Utf8).expect("save");
    c.bench_function("load_save_roundtrip_5k_lines", |b| {
        b.iter(|| {
            let l = load_file_sync(&p).expect("load");
            let s = l.buffer.snapshot();
            save_file_sync(&p, &s, LineEnding::Lf, Encoding::Utf8).expect("save");
            black_box(());
        });
    });
}

criterion_group!(benches, bench_load_100k, bench_roundtrip_utf8);
criterion_main!(benches);
