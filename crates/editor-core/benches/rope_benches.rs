//! Criterion benchmarks for rope hot paths (expanded in M02).
//!
//! This bench exists in M01 so `cargo bench --no-run` can compile the harness
//! in CI.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn noop_insert_baseline(c: &mut Criterion) {
    c.bench_function("rope_baseline_noop", |b| {
        b.iter(|| black_box(1u64));
    });
}

criterion_group!(benches, noop_insert_baseline);
criterion_main!(benches);
