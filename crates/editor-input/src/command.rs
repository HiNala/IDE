//! High-level editor commands produced from raw OS input.

use editor_core::CursorMotion;

/// User-visible or internal editor actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    /// Toggle the developer HUD / metrics overlay (F11).
    ToggleDevHud,
    /// Apply a cursor motion (keyboard navigation).
    ApplyCursorMotion {
        motion: CursorMotion,
        /// When true, extend selection from anchor (M09+); stored for wiring.
        extend_selection: bool,
    },
    /// Remove the word to the left of the caret (`Ctrl+Backspace` / `Alt+Backspace` on macOS).
    DeleteWordBackward,
    /// Remove the word to the right of the caret (`Ctrl+Delete` / `Alt+Delete` on macOS).
    DeleteWordForward,
}
