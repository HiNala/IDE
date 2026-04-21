//! Map winit key events to bytes for the PTY.

use winit::event::ElementState;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

/// Encode a key press into bytes for the shell. `None` means the caller handles it (shortcuts).
pub fn encode_key(
    physical: PhysicalKey,
    logical: &Key,
    state: ElementState,
    modifiers: ModifiersState,
) -> Option<Vec<u8>> {
    if state != ElementState::Pressed {
        return None;
    }

    let PhysicalKey::Code(code) = physical else {
        return None;
    };

    let ctrl = modifiers.control_key();
    let alt = modifiers.alt_key();

    if ctrl && !alt {
        match code {
            KeyCode::KeyC => return Some(vec![0x03]),
            KeyCode::KeyD => return Some(vec![0x04]),
            KeyCode::KeyZ => return Some(vec![0x1a]),
            KeyCode::KeyH => return Some(vec![0x08]),
            KeyCode::KeyI => return Some(vec![0x09]),
            KeyCode::KeyJ => return Some(vec![0x0a]),
            KeyCode::KeyM => return Some(vec![0x0d]),
            KeyCode::BracketLeft => return Some(vec![0x1b]),
            KeyCode::Backslash => return Some(vec![0x1c]),
            KeyCode::BracketRight => return Some(vec![0x1d]),
            KeyCode::KeyA => return Some(vec![0x01]),
            KeyCode::KeyB => return Some(vec![0x02]),
            KeyCode::KeyE => return Some(vec![0x05]),
            KeyCode::KeyF => return Some(vec![0x06]),
            KeyCode::KeyG => return Some(vec![0x07]),
            KeyCode::KeyK => return Some(vec![0x0b]),
            KeyCode::KeyL => return Some(vec![0x0c]),
            KeyCode::KeyN => return Some(vec![0x0e]),
            KeyCode::KeyO => return Some(vec![0x0f]),
            KeyCode::KeyP => return Some(vec![0x10]),
            KeyCode::KeyQ => return Some(vec![0x11]),
            KeyCode::KeyR => return Some(vec![0x12]),
            KeyCode::KeyS => return Some(vec![0x13]),
            KeyCode::KeyT => return Some(vec![0x14]),
            KeyCode::KeyU => return Some(vec![0x15]),
            KeyCode::KeyV => return Some(vec![0x16]),
            KeyCode::KeyW => return Some(vec![0x17]),
            KeyCode::KeyX => return Some(vec![0x18]),
            KeyCode::KeyY => return Some(vec![0x19]),
            _ => {}
        }
    }

    if ctrl && matches!(code, KeyCode::Space) {
        return Some(vec![0x00]);
    }

    if !ctrl {
        if let Key::Character(s) = logical {
            return Some(s.as_str().as_bytes().to_vec());
        }
    }

    match logical {
        Key::Named(NamedKey::Enter) => return Some(vec![b'\r']),
        Key::Named(NamedKey::Tab) => return Some(vec![b'\t']),
        Key::Named(NamedKey::Backspace) => return Some(vec![0x7f]),
        Key::Named(NamedKey::Escape) => return Some(vec![0x1b]),
        Key::Named(NamedKey::ArrowUp) => return Some(csi_arrow('A', modifiers)),
        Key::Named(NamedKey::ArrowDown) => return Some(csi_arrow('B', modifiers)),
        Key::Named(NamedKey::ArrowRight) => return Some(csi_arrow('C', modifiers)),
        Key::Named(NamedKey::ArrowLeft) => return Some(csi_arrow('D', modifiers)),
        Key::Named(NamedKey::Home) => return Some(csi_simple('H', modifiers)),
        Key::Named(NamedKey::End) => return Some(csi_simple('F', modifiers)),
        Key::Named(NamedKey::PageUp) => return Some(csi_tilde(5, modifiers)),
        Key::Named(NamedKey::PageDown) => return Some(csi_tilde(6, modifiers)),
        Key::Named(NamedKey::Delete) => return Some(vec![0x1b, b'[', b'3', b'~']),
        Key::Named(NamedKey::Insert) => return Some(vec![0x1b, b'[', b'2', b'~']),
        _ => {}
    }

    match code {
        KeyCode::F1 => return Some(vec![0x1b, b'O', b'P']),
        KeyCode::F2 => return Some(vec![0x1b, b'O', b'Q']),
        KeyCode::F3 => return Some(vec![0x1b, b'O', b'R']),
        KeyCode::F4 => return Some(vec![0x1b, b'O', b'S']),
        _ => {}
    }

    None
}

fn mod_suffix(m: ModifiersState) -> Option<u8> {
    let mut v: u8 = 1;
    if m.shift_key() {
        v += 1;
    }
    if m.alt_key() {
        v += 2;
    }
    if m.control_key() {
        v += 4;
    }
    if m.super_key() {
        v += 8;
    }
    if v > 1 {
        Some(v + 1)
    } else {
        None
    }
}

fn csi_arrow(dir: char, m: ModifiersState) -> Vec<u8> {
    if let Some(suf) = mod_suffix(m) {
        format!("\x1b[1;{suf}{dir}").into_bytes()
    } else {
        format!("\x1b[{dir}").into_bytes()
    }
}

fn csi_simple(letter: char, m: ModifiersState) -> Vec<u8> {
    if let Some(suf) = mod_suffix(m) {
        format!("\x1b[1;{suf}{letter}").into_bytes()
    } else {
        format!("\x1b[{letter}").into_bytes()
    }
}

fn csi_tilde(n: u8, m: ModifiersState) -> Vec<u8> {
    if let Some(suf) = mod_suffix(m) {
        format!("\x1b[{n};{suf}~").into_bytes()
    } else {
        format!("\x1b[{n}~").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_is_cr() {
        assert_eq!(
            encode_key(
                PhysicalKey::Code(KeyCode::Enter),
                &Key::Named(NamedKey::Enter),
                ElementState::Pressed,
                ModifiersState::default(),
            ),
            Some(vec![b'\r'])
        );
    }

    #[test]
    fn ctrl_c() {
        assert_eq!(
            encode_key(
                PhysicalKey::Code(KeyCode::KeyC),
                &Key::Character("c".into()),
                ElementState::Pressed,
                ModifiersState::CONTROL,
            ),
            Some(vec![0x03])
        );
    }

    #[test]
    fn letter_a() {
        assert_eq!(
            encode_key(
                PhysicalKey::Code(KeyCode::KeyA),
                &Key::Character("a".into()),
                ElementState::Pressed,
                ModifiersState::default(),
            ),
            Some(vec![b'a'])
        );
    }
}
