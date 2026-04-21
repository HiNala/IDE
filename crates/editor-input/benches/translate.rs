//! Throughput of [`editor_input::map_keyboard_input`] (M05).
//!
//! Save a named baseline locally:
//! `cargo bench -p editor-input --bench translate -- --save-baseline m05-mvp`

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use editor_input::map_keyboard_input;
use editor_input::EditorCommand;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

fn bench_map_enter(c: &mut Criterion) {
    let m = ModifiersState::default();
    c.bench_function("map_keyboard_input_enter_newline", |b| {
        b.iter(|| {
            black_box(map_keyboard_input(
                PhysicalKey::Code(KeyCode::Enter),
                None,
                ElementState::Pressed,
                m,
            ));
        });
    });
}

fn bench_map_typed_char(c: &mut Criterion) {
    let m = ModifiersState::default();
    c.bench_function("map_keyboard_input_text_insert", |b| {
        b.iter(|| {
            black_box(map_keyboard_input(
                PhysicalKey::Code(KeyCode::KeyA),
                Some("x"),
                ElementState::Pressed,
                m,
            ));
        });
    });
}

fn bench_map_ctrl_z(c: &mut Criterion) {
    let m = ModifiersState::CONTROL;
    c.bench_function("map_keyboard_input_ctrl_z_undo", |b| {
        b.iter(|| {
            black_box(map_keyboard_input(
                PhysicalKey::Code(KeyCode::KeyZ),
                None,
                ElementState::Pressed,
                m,
            ));
        });
    });
}

/// Chained: map then branch (mirrors app dispatch overhead without winit `KeyEvent`).
fn bench_command_dispatch_smoke(c: &mut Criterion) {
    let m = ModifiersState::default();
    c.bench_function("map_then_match_insert_branch", |b| {
        b.iter(|| {
            let cmd = map_keyboard_input(
                PhysicalKey::Code(KeyCode::KeyB),
                Some("y"),
                ElementState::Pressed,
                m,
            );
            black_box(match cmd {
                Some(EditorCommand::InsertText(ref s)) => s.len(),
                Some(_) => 1,
                None => 0,
            });
        });
    });
}

fn bench_burst_batch(c: &mut Criterion) {
    let m = ModifiersState::default();
    c.bench_function("map_keyboard_input_burst_32_batch", |b| {
        b.iter_batched(
            || (),
            |()| {
                for _ in 0..32 {
                    black_box(map_keyboard_input(
                        PhysicalKey::Code(KeyCode::KeyA),
                        Some("c"),
                        ElementState::Pressed,
                        m,
                    ));
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_map_enter,
    bench_map_typed_char,
    bench_map_ctrl_z,
    bench_command_dispatch_smoke,
    bench_burst_batch
);
criterion_main!(benches);
