//! Measures [`editor_render::TextLayer::prepare`] on a large buffer (~10 MiB UTF-8) with a
//! 1080p-class viewport (~55 visible lines). Target: steady-state &lt; 3 ms per M04.
//!
//! Save a baseline: `cargo bench -p editor-render --bench text_layer_prepare -- --save-baseline m04-mvp`
//!
//! Skips gracefully when no wgpu adapter is available (headless CI).

#![allow(clippy::print_stderr)] // Skip message when no GPU on bench host.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use editor_core::{ScrollOffset, TextBuffer};
use editor_render::{editor_syntax, TextLayer};
use wgpu::{DeviceDescriptor, Instance, InstanceDescriptor, TextureFormat};
use winit::dpi::PhysicalSize;

/// ~100-byte lines; repeat until ~10 MiB (M04 acceptance: large file scroll).
fn build_large_corpus() -> String {
    const LINE: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n";
    let mut s = String::with_capacity(11 * 1024 * 1024);
    while s.len() < 10 * 1024 * 1024 {
        s.push_str(LINE);
    }
    s
}

struct BenchCtx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layer: TextLayer,
    buffer: TextBuffer,
    viewport: PhysicalSize<u32>,
}

fn try_init() -> Option<BenchCtx> {
    let mut desc = InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
    let instance = Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))
    .ok()?;

    let dev_desc = DeviceDescriptor {
        label: Some("text_layer_prepare_bench"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&dev_desc)).ok()?;
    let format = TextureFormat::Bgra8UnormSrgb;
    let mut layer = TextLayer::new(&device, &queue, format);
    layer.set_scale_factor(1.0);

    let corpus = build_large_corpus();
    let buffer = TextBuffer::from_str(&corpus);
    let viewport = PhysicalSize::new(1920, 1080);

    Some(BenchCtx { device, queue, layer, buffer, viewport })
}

fn bench_prepare_warm(ctx: &mut BenchCtx) {
    let snap = ctx.buffer.snapshot();
    ctx.layer
        .prepare(
            &ctx.device,
            &ctx.queue,
            &snap,
            ScrollOffset::new(0.0),
            0,
            ctx.viewport,
            true,
            None,
            None,
            0.0,
            0.0,
            None,
            None,
            None,
            0.0,
            0.0,
            0.0,
            editor_syntax::Language::Plain,
        )
        .expect("prepare warmup");
}

fn run() {
    let Some(mut ctx) = try_init() else {
        eprintln!("text_layer_prepare: skip — no wgpu adapter");
        return;
    };

    let mut criterion = Criterion::default().configure_from_args();
    bench_prepare_warm(&mut ctx);

    let mut group = criterion.benchmark_group("text_layer_prepare");
    group.throughput(Throughput::Elements(1));
    group.bench_function("warm_10mb_1080p", |b| {
        b.iter(|| {
            let snap = ctx.buffer.snapshot();
            ctx.layer
                .prepare(
                    black_box(&ctx.device),
                    black_box(&ctx.queue),
                    black_box(&snap),
                    ScrollOffset::new(0.0),
                    0,
                    ctx.viewport,
                    true,
                    None,
                    None,
                    0.0,
                    0.0,
                    None,
                    None,
                    None,
                    0.0,
                    0.0,
                    0.0,
                    editor_syntax::Language::Plain,
                )
                .expect("prepare");
        });
    });

    let line_h = ctx.layer.line_height_px();
    let mut scroll_y = 0.0_f32;
    group.bench_function("scroll_line_by_line_10mb", |b| {
        b.iter(|| {
            scroll_y += line_h;
            let snap = ctx.buffer.snapshot();
            ctx.layer
                .prepare(
                    black_box(&ctx.device),
                    black_box(&ctx.queue),
                    black_box(&snap),
                    ScrollOffset::new(scroll_y),
                    0,
                    ctx.viewport,
                    true,
                    None,
                    None,
                    0.0,
                    0.0,
                    None,
                    None,
                    None,
                    0.0,
                    0.0,
                    0.0,
                    editor_syntax::Language::Plain,
                )
                .expect("prepare");
        });
    });

    group.finish();
    criterion.final_summary();
}

fn main() {
    run();
}
