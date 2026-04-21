//! Placeholder bench target so workspace `cargo bench --no-run` can resolve M22 retrieve baselines later.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};

fn index_retrieve_stub(c: &mut Criterion) {
    c.bench_function("index_retrieve_stub", |b| b.iter(|| black_box(0u8)));
}

criterion_group!(benches, index_retrieve_stub);
criterion_main!(benches);
