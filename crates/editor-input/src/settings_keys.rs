//! Keyboard routing when the settings overlay has focus (M28).

use winit::event::ElementState;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

/// Actions for the settings panel (handled in `editor-app`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsCommand {
    /// Close overlay (**Esc**).
    Close,
    /// Previous section (**Up**).
    SectionPrev,
    /// Next section (**Down**).
    SectionNext,
    /// Previous control (**Shift+Tab** or **Up** in body — app interprets).
    FocusPrev,
    /// Next control (**Tab** or **Down** in body).
    FocusNext,
    /// Activate primary control (**Enter**).
    Activate,
    /// Toggle checkbox / cycle dropdown (**Space**).
    Toggle,
    /// Focus the section filter field (**/** when Keybindings or Skills is active).
    FocusSectionFilter,
    /// Insert text while editing an API key field.
    InsertChar(char),
    /// Delete backward in API key field.
    DeleteBackward,
    /// Paste (handled separately with clipboard in app).
    Paste,
    /// Test the active AI provider (**Primary+T**).
    TestActiveProvider,
}

#[must_use]
pub fn map_settings_key(
    physical_key: PhysicalKey,
    text: Option<&str>,
    state: ElementState,
    modifiers: ModifiersState,
) -> Option<SettingsCommand> {
    if state != ElementState::Pressed {
        return None;
    }

    let pm = crate::primary_modifier_active(&modifiers);

    if let PhysicalKey::Code(code) = physical_key {
        if pm && matches!(code, KeyCode::KeyT) {
            return Some(SettingsCommand::TestActiveProvider);
        }
        if matches!(code, KeyCode::Escape) {
            return Some(SettingsCommand::Close);
        }
        if matches!(code, KeyCode::ArrowUp) {
            return Some(SettingsCommand::SectionPrev);
        }
        if matches!(code, KeyCode::ArrowDown) {
            return Some(SettingsCommand::SectionNext);
        }
        if matches!(code, KeyCode::Tab) {
            if modifiers.shift_key() {
                return Some(SettingsCommand::FocusPrev);
            }
            return Some(SettingsCommand::FocusNext);
        }
        if matches!(code, KeyCode::Enter) {
            return Some(SettingsCommand::Activate);
        }
        if matches!(code, KeyCode::Space) {
            return Some(SettingsCommand::Toggle);
        }
        if matches!(code, KeyCode::Slash) && !modifiers.control_key() && !modifiers.super_key() {
            return Some(SettingsCommand::FocusSectionFilter);
        }
        if matches!(code, KeyCode::Backspace) {
            return Some(SettingsCommand::DeleteBackward);
        }
        if matches!(code, KeyCode::KeyV) && pm {
            return Some(SettingsCommand::Paste);
        }
    }

    if let Some(t) = text {
        if t.is_empty() || modifiers.control_key() || modifiers.super_key() {
            return None;
        }
        let mut it = t.chars();
        if let Some(c) = it.next() {
            if it.next().is_none() && !c.is_control() {
                return Some(SettingsCommand::InsertChar(c));
            }
        }
    }

    None
}
