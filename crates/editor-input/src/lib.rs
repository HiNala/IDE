//! `editor-input` — OS input → editor operations.

#![forbid(unsafe_code)]

pub mod command;

use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

pub use command::EditorCommand;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[must_use]
pub fn banner() -> String {
    format!("editor-input v{VERSION}")
}

/// Maps a keypress plus current modifier state to an [`EditorCommand`].
///
/// Word navigation: **Ctrl+Left/Right** on Windows/Linux; **Alt+Left/Right** on
/// macOS (Apple convention). With **Shift**, `extend_selection` is set for
/// future selection-anchor wiring (M09).
#[must_use]
pub fn map_key_event(event: &KeyEvent, modifiers: ModifiersState) -> Option<EditorCommand> {
    if event.state != ElementState::Pressed || event.repeat {
        return None;
    }
    let word_mod =
        if cfg!(target_os = "macos") { modifiers.alt_key() } else { modifiers.control_key() };
    match event.physical_key {
        PhysicalKey::Code(KeyCode::F11) => Some(EditorCommand::ToggleDevHud),
        PhysicalKey::Code(KeyCode::ArrowLeft) if word_mod => {
            Some(EditorCommand::ApplyCursorMotion {
                motion: editor_core::CursorMotion::WordLeft,
                extend_selection: modifiers.shift_key(),
            })
        }
        PhysicalKey::Code(KeyCode::ArrowRight) if word_mod => {
            Some(EditorCommand::ApplyCursorMotion {
                motion: editor_core::CursorMotion::WordRight,
                extend_selection: modifiers.shift_key(),
            })
        }
        PhysicalKey::Code(KeyCode::Backspace) if word_mod => {
            Some(EditorCommand::DeleteWordBackward)
        }
        PhysicalKey::Code(KeyCode::Delete) if word_mod => Some(EditorCommand::DeleteWordForward),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_ok() {
        assert!(banner().contains("editor-input"));
    }
}
