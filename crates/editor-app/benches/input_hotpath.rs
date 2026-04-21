//! CPU-only proxy for M05 input → mutation: `map_keyboard_input` + `TextBuffer::insert`.
//! Does **not** include layout, shaping, or GPU (see `editor-render` benches for GPU).
//!
//! Save baseline:
//! `cargo bench -p editor-app --bench input_hotpath -- --save-baseline m05-mvp`

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use editor_core::{BytePos, TextBuffer};
use editor_input::map_keyboard_input;
use editor_input::EditorCommand;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

fn bench_translate_then_insert_end(c: &mut Criterion) {
    let modifiers = ModifiersState::default();
    let key = PhysicalKey::Code(KeyCode::KeyA);

    c.bench_function("translate_char_then_insert_1kb_buffer", |b| {
        b.iter_batched(
            || TextBuffer::from_str(&(0..1024).map(|_| "z").collect::<String>()),
            |mut buf| {
                let pos = BytePos(buf.len_bytes());
                let cmd = map_keyboard_input(key, Some("a"), ElementState::Pressed, modifiers);
                match cmd {
                    Some(EditorCommand::InsertText(ref s)) => {
                        buf.insert(pos, s).unwrap();
                    }
                    Some(_) => {}
                    None => {}
                }
                black_box(buf.len_bytes());
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_translate_then_insert_end);
criterion_main!(benches);
