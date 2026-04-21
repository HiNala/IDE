//! Baseline tag: `m21-v3`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use editor_metadata::schema::{blank_sidecar, parse, write_to_markdown};
use editor_metadata::{workspace_relative, MetadataStore};
use std::path::Path;
use tempfile::tempdir;

fn bench_roundtrip(c: &mut Criterion) {
    let mut sc = blank_sidecar(Path::new("src/x.rs"), "// demo", "bench");
    sc.frontmatter.summary = "s".into();
    sc.body.reasoning = "body".into();
    let md = write_to_markdown(&sc).unwrap();

    c.bench_function("sidecar_parse_serialize_roundtrip", |b| {
        b.iter(|| {
            let p = parse(black_box(&md)).unwrap();
            let _ = write_to_markdown(black_box(&p)).unwrap();
        });
    });
}

fn bench_list_1000(c: &mut Criterion) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let store = MetadataStore::new(root.clone());
    for i in 0..1000 {
        let p = Path::new("src").join(format!("m{i}.rs"));
        let mut sc = blank_sidecar(&p, &format!("fn f{i}() {{}}"), "bench");
        sc.frontmatter.summary = format!("file {i}");
        store.save(&sc).unwrap();
    }

    c.bench_function("metadata_store_list_1000", |b| {
        b.iter(|| {
            let v = store.list_all().unwrap();
            black_box(v.len());
        });
    });
}

fn bench_update_overhead(c: &mut Criterion) {
    c.bench_function("workspace_relative_hot_path", |b| {
        let root = Path::new("C:/dev/proj");
        let f = Path::new("C:/dev/proj/crates/x/src/lib.rs");
        b.iter(|| {
            black_box(workspace_relative(black_box(root), black_box(f)));
        });
    });
}

criterion_group!(benches, bench_roundtrip, bench_list_1000, bench_update_overhead);
criterion_main!(benches);
