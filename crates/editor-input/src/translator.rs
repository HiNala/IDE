//! Holds modifier snapshot for synchronous keyboard translation (M05).
//!
//! The app updates modifiers from [`winit::event::WindowEvent::ModifiersChanged`]. Keyboard events
//! are then mapped with [`Self::translate_key`](EventTranslator::translate_key), which delegates to
//! [`crate::map_keyboard_input`].

use winit::event::KeyEvent;
use winit::keyboard::ModifiersState;

use crate::command::EditorCommand;
use crate::map_keyboard_input;

/// Lightweight input translator: tracks the last modifier state and maps [`KeyEvent`] → [`EditorCommand`].
#[derive(Debug, Clone)]
pub struct EventTranslator {
    modifiers: ModifiersState,
}

impl Default for EventTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl EventTranslator {
    #[must_use]
    pub fn new() -> Self {
        Self { modifiers: ModifiersState::default() }
    }

    #[must_use]
    pub fn modifiers(&self) -> ModifiersState {
        self.modifiers
    }

    pub fn set_modifiers(&mut self, m: ModifiersState) {
        self.modifiers = m;
    }

    /// Maps a key event using the stored modifier snapshot (call after `ModifiersChanged` has been applied).
    #[must_use]
    pub fn translate_key(&self, event: &KeyEvent) -> Option<EditorCommand> {
        map_keyboard_input(
            event.physical_key,
            event.text.as_ref().map(|s| s.as_str()),
            event.state,
            self.modifiers,
        )
    }
}
