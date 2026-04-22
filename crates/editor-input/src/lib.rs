//! `editor-input` — OS input → editor operations.

#![forbid(unsafe_code)]

pub mod command;
pub mod mouse;
pub mod settings_keys;

use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

pub use command::EditorCommand;
pub use mouse::{scroll_delta_y_pixels, MouseChordState};
pub use settings_keys::{map_settings_key, SettingsCommand};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[must_use]
pub fn banner() -> String {
    format!("editor-input v{VERSION}")
}

/// Primary shortcut modifier: **Command** on macOS, **Ctrl** on Windows/Linux.
#[inline]
#[must_use]
pub fn primary_modifier_active(m: &ModifiersState) -> bool {
    if cfg!(target_os = "macos") {
        m.super_key()
    } else {
        m.control_key()
    }
}

/// Word-move modifier: **Alt** on macOS, **Ctrl** elsewhere (matches common IDEs).
#[inline]
fn word_mod(m: &ModifiersState) -> bool {
    if cfg!(target_os = "macos") {
        m.alt_key()
    } else {
        m.control_key()
    }
}

/// Maps a keypress plus current modifier state to an [`EditorCommand`].
///
/// - Uses **physical keys** for shortcuts and navigation (stable across layouts).
/// - Uses [`KeyEvent::text`] for typed characters when no primary/ctrl/super held.
/// - **Repeat** events are passed through so key-repeat works for typing and navigation.
#[must_use]
pub fn map_key_event(event: &KeyEvent, modifiers: ModifiersState) -> Option<EditorCommand> {
    map_keyboard_input(
        event.physical_key,
        event.text.as_ref().map(|t| t.as_str()),
        event.state,
        modifiers,
    )
}

/// Same semantics as [`map_key_event`], without requiring a full [`KeyEvent`] (benchmarks / tests).
#[doc(hidden)]
#[must_use]
pub fn map_keyboard_input(
    physical_key: PhysicalKey,
    text: Option<&str>,
    state: ElementState,
    modifiers: ModifiersState,
) -> Option<EditorCommand> {
    if state != ElementState::Pressed {
        return None;
    }

    let wm = word_mod(&modifiers);
    let pm = primary_modifier_active(&modifiers);

    if let PhysicalKey::Code(code) = physical_key {
        if matches!(code, KeyCode::F11) {
            if pm {
                return Some(EditorCommand::ToggleDevHud);
            }
            return Some(EditorCommand::ToggleFullscreen);
        }
        // Global shortcuts
        if pm {
            match code {
                KeyCode::Tab => {
                    return Some(if modifiers.shift_key() {
                        EditorCommand::PrevBuffer
                    } else {
                        EditorCommand::NextBuffer
                    });
                }
                KeyCode::KeyN => return Some(EditorCommand::NewBuffer),
                KeyCode::KeyW => return Some(EditorCommand::CloseBuffer),
                KeyCode::Backquote if modifiers.shift_key() => {
                    return Some(EditorCommand::NewIntegratedTerminal);
                }
                KeyCode::Backquote => return Some(EditorCommand::ToggleTerminalPane),
                KeyCode::KeyZ if !modifiers.shift_key() => return Some(EditorCommand::Undo),
                KeyCode::KeyZ if modifiers.shift_key() => return Some(EditorCommand::Redo),
                KeyCode::KeyY => return Some(EditorCommand::Redo),
                KeyCode::KeyS => return Some(EditorCommand::Save),
                KeyCode::KeyO => return Some(EditorCommand::Open),
                KeyCode::KeyC => return Some(EditorCommand::Copy),
                KeyCode::KeyX => return Some(EditorCommand::Cut),
                KeyCode::KeyV => return Some(EditorCommand::Paste),
                KeyCode::KeyA if modifiers.shift_key() => {
                    return Some(EditorCommand::ToggleAgentPanel)
                }
                KeyCode::KeyA => return Some(EditorCommand::SelectAll),
                KeyCode::Comma => return Some(EditorCommand::OpenSettings),
                KeyCode::KeyB => return Some(EditorCommand::ToggleSidebar),
                KeyCode::KeyP => {
                    if modifiers.shift_key() {
                        return Some(EditorCommand::OpenCommandPalette);
                    }
                    return Some(EditorCommand::ToggleQuickOpen);
                }
                KeyCode::KeyE if modifiers.shift_key() => return Some(EditorCommand::FocusSidebar),
                KeyCode::KeyF => return Some(EditorCommand::FindInFile),
                KeyCode::KeyH => return Some(EditorCommand::ReplaceInFile),
                KeyCode::KeyQ => return Some(EditorCommand::Quit),
                KeyCode::Home => {
                    return Some(EditorCommand::ApplyCursorMotion {
                        motion: editor_core::CursorMotion::BufferStart,
                        extend_selection: modifiers.shift_key(),
                    });
                }
                KeyCode::End => {
                    return Some(EditorCommand::ApplyCursorMotion {
                        motion: editor_core::CursorMotion::BufferEnd,
                        extend_selection: modifiers.shift_key(),
                    });
                }
                _ => {}
            }
        }

        if matches!(code, KeyCode::Escape) {
            return Some(EditorCommand::Cancel);
        }

        if matches!(code, KeyCode::F3) {
            return Some(if modifiers.shift_key() {
                EditorCommand::FindPrev
            } else {
                EditorCommand::FindNext
            });
        }

        if wm {
            match code {
                KeyCode::ArrowLeft => {
                    return Some(EditorCommand::ApplyCursorMotion {
                        motion: editor_core::CursorMotion::WordLeft,
                        extend_selection: modifiers.shift_key(),
                    });
                }
                KeyCode::ArrowRight => {
                    return Some(EditorCommand::ApplyCursorMotion {
                        motion: editor_core::CursorMotion::WordRight,
                        extend_selection: modifiers.shift_key(),
                    });
                }
                KeyCode::Backspace => return Some(EditorCommand::DeleteWordBackward),
                KeyCode::Delete => return Some(EditorCommand::DeleteWordForward),
                _ => {}
            }
        }

        // Line home/end without primary (primary+Home/End handled above as buffer).
        if !pm {
            if matches!(code, KeyCode::Home) {
                return Some(EditorCommand::ApplyCursorMotion {
                    motion: editor_core::CursorMotion::LineStart,
                    extend_selection: modifiers.shift_key(),
                });
            }
            if matches!(code, KeyCode::End) {
                return Some(EditorCommand::ApplyCursorMotion {
                    motion: editor_core::CursorMotion::LineEnd,
                    extend_selection: modifiers.shift_key(),
                });
            }
        }

        match code {
            KeyCode::Enter => return Some(EditorCommand::InsertNewline),
            KeyCode::Tab => return Some(EditorCommand::InsertText("    ".into())),
            KeyCode::Backspace if !wm => return Some(EditorCommand::DeleteBackward),
            KeyCode::Delete if !wm => return Some(EditorCommand::DeleteForward),
            KeyCode::ArrowDown if !wm => {
                return Some(EditorCommand::ApplyCursorMotion {
                    motion: editor_core::CursorMotion::Down,
                    extend_selection: modifiers.shift_key(),
                });
            }
            KeyCode::ArrowUp if !wm => {
                return Some(EditorCommand::ApplyCursorMotion {
                    motion: editor_core::CursorMotion::Up,
                    extend_selection: modifiers.shift_key(),
                });
            }
            KeyCode::ArrowLeft if !wm => {
                return Some(EditorCommand::ApplyCursorMotion {
                    motion: editor_core::CursorMotion::Left,
                    extend_selection: modifiers.shift_key(),
                });
            }
            KeyCode::ArrowRight if !wm => {
                return Some(EditorCommand::ApplyCursorMotion {
                    motion: editor_core::CursorMotion::Right,
                    extend_selection: modifiers.shift_key(),
                });
            }
            KeyCode::PageUp if !wm => return Some(EditorCommand::PageUp),
            KeyCode::PageDown if !wm => return Some(EditorCommand::PageDown),
            _ => {}
        }
    }

    // Typed characters (layout-aware); skip when chorded with primary/ctrl/super.
    if let Some(t) = text {
        if t.is_empty() || pm || modifiers.control_key() || modifiers.super_key() {
            return None;
        }
        if t == "\r" || t == "\n" || t == "\u{7f}" {
            return None;
        }
        if t.chars().all(|c| c.is_control()) {
            return None;
        }
        return Some(EditorCommand::InsertText(t.to_string()));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_ok() {
        assert!(banner().contains("editor-input"));
    }
}
