//! High-level editor commands produced from raw OS input.

use editor_core::CursorMotion;

/// User-visible or internal editor actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorCommand {
    /// Insert UTF-8 text at the caret (typing, paste, IME commit).
    InsertText(String),
    /// Insert a newline (`\n`).
    InsertNewline,
    /// Delete one grapheme cluster before the caret.
    DeleteBackward,
    /// Delete one grapheme cluster after the caret.
    DeleteForward,
    /// Undo last edit.
    Undo,
    /// Redo last undone edit.
    Redo,
    /// Save buffer to disk.
    Save,
    /// Open file via native picker (Ctrl/Cmd+O).
    Open,
    /// Copy selection to system clipboard (Ctrl/Cmd+C).
    Copy,
    /// Cut selection to clipboard (Ctrl/Cmd+X).
    Cut,
    /// Paste from clipboard at caret (Ctrl/Cmd+V).
    Paste,
    /// Select entire buffer (Ctrl/Cmd+A).
    SelectAll,
    /// Exit the application (debug / minimal quit path).
    Quit,
    /// Toggle the developer HUD / metrics overlay (F11).
    ToggleDevHud,
    /// Apply a cursor motion (keyboard navigation).
    ApplyCursorMotion {
        motion: CursorMotion,
        /// When true, extend selection from anchor (M09+).
        extend_selection: bool,
    },
    /// Remove the word to the left of the caret (`Ctrl+Backspace` / `Alt+Backspace` on macOS).
    DeleteWordBackward,
    /// Remove the word to the right of the caret (`Ctrl+Delete` / `Alt+Delete` on macOS).
    DeleteWordForward,
    /// Scroll by one viewport page (PageUp / PageDown).
    PageUp,
    PageDown,
}
