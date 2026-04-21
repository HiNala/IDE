//! M16 search throughput baselines (`cargo bench -p editor-search`).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use editor_core::TextBuffer;
use editor_search::{search_buffer, InFileSearch};
use editor_workspace::Workspace;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use tempfile::TempDir;

fn in_file_10mb_many_matches(c: &mut Criterion) {
    let mut s = String::with_capacity(10 * 1024 * 1024);
    while s.len() < 10 * 1024 * 1024 {
        s.push_str("needle line\n");
    }
    let buf = TextBuffer::from_str(&s);
    let snap = buf.snapshot();
    let params = InFileSearch {
        query: "needle".into(),
        is_regex: false,
        case_sensitive: true,
        whole_word: false,
    };
    c.bench_function("in_file_10mb_literal_needle", |b| {
        b.iter(|| {
            let r = search_buffer(black_box(&params), black_box(&snap)).unwrap();
            black_box(r.matches.len())
        });
    });
}

fn project_walk_fixture(c: &mut Criterion) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    for i in 0..100 {
        let p = root.join(format!("f{i}.txt"));
        std::fs::write(p, "alpha beta gamma\nbeta\n").unwrap();
    }
    let ws = Workspace::open(root).unwrap();
    c.bench_function("project_100_files_beta", |b| {
        b.iter(|| {
            let job = editor_search::start_project_search(
                editor_search::ProjectSearch {
                    query: "beta".into(),
                    is_regex: false,
                    case_sensitive: true,
                    whole_word: false,
                },
                &ws,
                std::collections::HashMap::new(),
                &editor_core::WorkerPool::new(Some(4)),
            );
            let mut n = 0usize;
            while let Ok(ev) = job.rx.recv() {
                if matches!(ev, editor_search::SearchEvent::Done { .. }) {
                    break;
                }
                if matches!(ev, editor_search::SearchEvent::Match(_)) {
                    n += 1;
                }
            }
            black_box(n)
        });
    });
}

fn grep_searcher_slice_throughput(c: &mut Criterion) {
    let content = "alpha beta\ngamma beta\n".repeat(10_000);
    let pat = "beta";
    let matcher = RegexMatcher::new(pat).unwrap();
    let re = regex::Regex::new(pat).unwrap();
    c.bench_function("grep_searcher_slice_20k_lines", |b| {
        b.iter(|| {
            let mut searcher = Searcher::new();
            let mut hits = 0usize;
            searcher
                .search_slice(
                    &matcher,
                    black_box(content.as_bytes()),
                    UTF8(|_, line| {
                        hits += re.find_iter(line).count();
                        Ok(true)
                    }),
                )
                .unwrap();
            black_box(hits)
        });
    });
}

criterion_group!(
    benches,
    in_file_10mb_many_matches,
    project_walk_fixture,
    grep_searcher_slice_throughput
);
criterion_main!(benches);
