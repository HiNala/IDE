//! High-level editor commands produced from raw OS input.

use editor_core::CursorMotion;

/// User-visible or internal editor actions.
#[derive(Debug, Clone, PartialEq)]
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
    /// Toggle exclusive fullscreen (**F11**); **Primary+F11** toggles the developer HUD / metrics overlay.
    ToggleFullscreen,
    /// Toggle the developer HUD / metrics overlay (**Primary+F11**).
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
    /// Left mouse down over the window (physical pixels, top-left origin).
    ///
    /// `click_count` is 1–3 for single/double/triple within platform timing and distance.
    MouseClick {
        x_px: i32,
        y_px: i32,
        click_count: u8,
        shift: bool,
    },
    /// Left button held and pointer moved (physical pixels).
    MouseDrag {
        x_px: i32,
        y_px: i32,
    },
    /// Vertical scroll (positive content delta: move document up, decrease `ScrollOffset.y_px`).
    ScrollContent {
        delta_y_px: f32,
    },
    /// Toggle integrated terminal pane (M26: **Ctrl+`**).
    ToggleTerminalPane,
    /// Spawn another integrated terminal session (M26: **Ctrl+Shift+`**).
    NewIntegratedTerminal,
    /// Open settings (M28: **Ctrl+,** — VS Code convention).
    OpenSettings,
    /// Next open buffer in MRU order (**Ctrl+Tab**).
    NextBuffer,
    /// Previous open buffer (**Ctrl+Shift+Tab**).
    PrevBuffer,
    /// Close active buffer (**Ctrl+W**); host may refuse if dirty.
    CloseBuffer,
    /// New untitled buffer (**Ctrl+N**).
    NewBuffer,
    /// Toggle project sidebar (**Ctrl+B**).
    ToggleSidebar,
    /// Toggle quick-open palette (**Ctrl+P**).
    ToggleQuickOpen,
    /// Focus sidebar for keyboard navigation (**Ctrl+Shift+E**).
    FocusSidebar,
    /// Open in-buffer find bar (**Ctrl+F**). Hides replace row.
    FindInFile,
    /// Open in-buffer find bar with replace row visible (**Ctrl+H**).
    ReplaceInFile,
    /// Jump to the next in-buffer match (**F3** or Enter while find bar focused).
    FindNext,
    /// Jump to the previous in-buffer match (**Shift+F3** or Shift+Enter).
    FindPrev,
    /// Dismiss modal overlay / cancel (Escape); app decides vs quit.
    Cancel,
}
