//! Mouse button chords and scroll → [`EditorCommand`](crate::EditorCommand) (M09).

use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::ModifiersState;

use crate::EditorCommand;

/// Double/triple-click grouping (Windows default is 500 ms).
const MULTI_CLICK_MS: u128 = 500;
/// Squared distance threshold (physical px) for multi-click grouping.
const MULTI_CLICK_MAX_DIST_SQ: f64 = 4.0 * 4.0;

/// Tracks left-button state and multi-click sequence.
#[derive(Debug, Clone, Default)]
pub struct MouseChordState {
    left_down: bool,
    last_click_time: Option<Instant>,
    last_click_pos: Option<(f64, f64)>,
    last_click_count: u8,
}

impl MouseChordState {
    /// Left button press/release. Pass the last known cursor position from [`WindowEvent::CursorMoved`].
    #[must_use]
    pub fn on_left_button(
        &mut self,
        state: ElementState,
        button: MouseButton,
        pos: PhysicalPosition<f64>,
        modifiers: ModifiersState,
    ) -> Option<EditorCommand> {
        if button != MouseButton::Left {
            return None;
        }
        match state {
            ElementState::Pressed => {
                self.left_down = true;
                let now = Instant::now();
                let (x, y) = (pos.x, pos.y);
                let count = if let (Some(t0), Some((lx, ly))) =
                    (self.last_click_time, self.last_click_pos)
                {
                    if now.duration_since(t0).as_millis() <= MULTI_CLICK_MS {
                        let dx = x - lx;
                        let dy = y - ly;
                        if dx * dx + dy * dy <= MULTI_CLICK_MAX_DIST_SQ {
                            (self.last_click_count % 3) + 1
                        } else {
                            1
                        }
                    } else {
                        1
                    }
                } else {
                    1
                };
                self.last_click_time = Some(now);
                self.last_click_pos = Some((x, y));
                self.last_click_count = count;
                Some(EditorCommand::MouseClick {
                    x_px: x.round() as i32,
                    y_px: y.round() as i32,
                    click_count: count,
                    shift: modifiers.shift_key(),
                })
            }
            ElementState::Released => {
                self.left_down = false;
                None
            }
        }
    }

    /// Drag updates while the left button is down.
    #[must_use]
    pub fn on_cursor_moved(&self, pos: PhysicalPosition<f64>) -> Option<EditorCommand> {
        if !self.left_down {
            return None;
        }
        Some(EditorCommand::MouseDrag { x_px: pos.x.round() as i32, y_px: pos.y.round() as i32 })
    }
}

/// Converts winit scroll delta to physical pixels (Y only). Subtract from [`editor_core::ScrollOffset`] `y_px`.
#[must_use]
pub fn scroll_delta_y_pixels(delta: MouseScrollDelta, scale_factor: f32) -> f32 {
    match delta {
        MouseScrollDelta::LineDelta(_, y) => y * 48.0 * scale_factor,
        MouseScrollDelta::PixelDelta(p) => p.y as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::ModifiersState;

    #[test]
    fn triple_click_then_single_resets() {
        let mut m = MouseChordState::default();
        let p0 = PhysicalPosition::new(10.0, 10.0);
        let mods = ModifiersState::default();
        let c1 = m.on_left_button(ElementState::Pressed, MouseButton::Left, p0, mods).unwrap();
        assert!(matches!(c1, EditorCommand::MouseClick { click_count: 1, x_px: 10, y_px: 10, .. }));
        let c2 = m.on_left_button(ElementState::Released, MouseButton::Left, p0, mods);
        assert!(c2.is_none());

        let c3 = m.on_left_button(ElementState::Pressed, MouseButton::Left, p0, mods).unwrap();
        assert!(matches!(c3, EditorCommand::MouseClick { click_count: 2, .. }));
        let _ = m.on_left_button(ElementState::Released, MouseButton::Left, p0, mods);

        let c4 = m.on_left_button(ElementState::Pressed, MouseButton::Left, p0, mods).unwrap();
        assert!(matches!(c4, EditorCommand::MouseClick { click_count: 3, .. }));
        let _ = m.on_left_button(ElementState::Released, MouseButton::Left, p0, mods);

        let c5 = m.on_left_button(ElementState::Pressed, MouseButton::Left, p0, mods).unwrap();
        assert!(matches!(c5, EditorCommand::MouseClick { click_count: 1, .. }));
    }
}
