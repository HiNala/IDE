//! Read-only shortcut list for the settings panel (M28). Primary = Ctrl on Windows/Linux, Command on macOS.

/// `(command_label, shortcut_display)`.
pub static DEFAULT_KEYMAP: &[(&str, &str)] = &[
    ("Save buffer", "Primary+S"),
    ("Open file", "Primary+O"),
    ("Undo", "Primary+Z"),
    ("Redo", "Primary+Shift+Z or Primary+Y"),
    ("Copy", "Primary+C"),
    ("Cut", "Primary+X"),
    ("Paste", "Primary+V"),
    ("Select all", "Primary+A"),
    ("Buffer start / end", "Primary+Home / Primary+End"),
    ("Toggle terminal pane", "Primary+`"),
    ("New terminal session", "Primary+Shift+`"),
    ("Toggle fullscreen", "F11"),
    ("Toggle metrics overlay", "Primary+F11"),
    ("Open settings", "Primary+,"),
    ("Test active AI connection (settings open)", "Primary+T"),
    ("Exit application", "Escape"),
    ("Insert tab", "Tab"),
    ("Page up / page down", "PageUp / PageDown"),
    ("Move by word", "Alt+Arrow (macOS Primary+Arrow for word in some layouts)"),
];
