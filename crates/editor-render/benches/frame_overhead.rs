//! Measures CPU-side cost of [`FrameTimer`] tick + rolling statistics (M03 baseline).
//! Full GPU `render_frame` throughput is covered by interactive smoke tests and `--dry-run`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use editor_render::FrameTimer;

fn bench_frame_timer_cycle(c: &mut Criterion) {
    let mut timer = FrameTimer::new();
    c.bench_function("frame_timer_tick_and_percentiles", |b| {
        b.iter(|| {
            timer.tick();
            black_box(timer.last_delta());
            black_box(timer.average_fps());
            black_box(timer.p95_frame_time());
            black_box(timer.p99_frame_time());
        });
    });
}

criterion_group!(benches, bench_frame_timer_cycle);
criterion_main!(benches);
