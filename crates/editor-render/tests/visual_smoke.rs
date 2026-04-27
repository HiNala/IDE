//! Off-screen / headless smoke: `TextLayer::prepare` succeeds when a GPU adapter exists.
//!
//! Does **not** read back pixels (would require a full render pass + copy). Run manually:
//!
//! ```text
//! cargo test -p editor-render --test visual_smoke -- --ignored --nocapture
//! ```
//!
//! For Cursor-style shell comparison, add a PNG under `docs/assets/cursor_style_reference.png`
//! (see `editor-ui` test `reference_visual`) and compare in a future pixel-diff harness.

#![allow(clippy::print_stderr)] // Skip message when no GPU in optional smoke test.

use editor_core::{ScrollOffset, TextBuffer};
use editor_render::{editor_syntax, TextLayer};
use wgpu::{DeviceDescriptor, Instance, InstanceDescriptor, TextureFormat};
use winit::dpi::PhysicalSize;

fn try_gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
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
        label: Some("visual_smoke"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    };
    pollster::block_on(adapter.request_device(&dev_desc)).ok()
}

#[test]
#[ignore = "requires GPU; run with: cargo test -p editor-render --test visual_smoke -- --ignored"]
fn text_layer_prepare_smoke_large_buffer() {
    let Some((device, queue)) = try_gpu() else {
        eprintln!("visual_smoke: skip — no adapter");
        return;
    };

    let mut layer = TextLayer::new(&device, &queue, TextureFormat::Bgra8UnormSrgb);
    layer.set_scale_factor(1.0);

    let mut s = String::new();
    for i in 0..500 {
        s.push_str(&format!("line {i:04} hello world\n"));
    }
    let buffer = TextBuffer::from_str(&s);
    let snap = buffer.snapshot();

    layer
        .prepare(
            &device,
            &queue,
            &snap,
            ScrollOffset::new(0.0),
            0,
            PhysicalSize::new(800, 600),
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
            0.0,
            0.0,
            editor_syntax::Language::Plain,
        )
        .expect("prepare should succeed");
}
