//! Process physical events, captured before any menu/Inspection routing.
//! No tutorial/Build/UI gate belongs in these event producers.
use winit::{
    event::MouseButton,
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Default)]
pub struct PhysicalInput {
    held: [bool; 4],
    taps: [bool; 4],
    buttons: [bool; 2],
    delta: [i32; 2],
    pointer: Option<[f64; 2]>,
    error: Option<String>,
}

impl PhysicalInput {
    pub fn key(&mut self, key: PhysicalKey, down: bool) {
        let index = match key {
            PhysicalKey::Code(KeyCode::ArrowRight) => 0,
            PhysicalKey::Code(KeyCode::ArrowLeft) => 1,
            PhysicalKey::Code(KeyCode::ArrowDown) => 2,
            PhysicalKey::Code(KeyCode::ArrowUp) => 3,
            _ => return,
        };
        self.taps[index] |= down && !self.held[index];
        self.held[index] = down;
    }

    pub fn button(&mut self, button: MouseButton, down: bool) {
        match button {
            MouseButton::Left => self.buttons[0] = down,
            MouseButton::Right => self.buttons[1] = down,
            _ => (),
        }
    }

    /// WindowEvent::CursorMoved uses the same physical-window pixel units as
    /// Intro's old drag producer. This is an explicit host adapter, not proof
    /// of DirectInput/device acceleration equivalence. Unsupported magnitudes
    /// fail at the next boundary rather than saturating or inventing a delta.
    pub fn pointer(&mut self, position: [f64; 2]) {
        if !position.into_iter().all(f64::is_finite) {
            self.error = Some("Nonfinite physical camera pointer".into());
            return;
        }
        if let Some(previous) = self.pointer {
            for axis in 0..2 {
                let movement = (position[axis] - previous[axis]).round();
                if movement < f64::from(i32::MIN) || movement > f64::from(i32::MAX) {
                    self.error = Some("Physical camera delta leaves signed32 domain".into());
                    return;
                }
                let Some(next) = self.delta[axis].checked_add(movement as i32) else {
                    self.error = Some("Accumulated physical camera delta overflow".into());
                    return;
                };
                self.delta[axis] = next;
            }
        }
        self.pointer = Some(position);
    }

    pub fn snapshot(&self) -> Result<([bool; 4], [bool; 2], [i32; 2]), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok((
            std::array::from_fn(|i| self.held[i] || self.taps[i]),
            self.buttons,
            self.delta,
        ))
    }

    /// Exactly one successful central consumes edges/delta, even mode4/6 or
    /// a disabled callback. Later catchup callbacks retain held input only.
    pub fn consumed(&mut self) {
        self.taps = [false; 4];
        self.delta = [0; 2];
    }

    pub fn unfocus(&mut self) {
        self.held = [false; 4];
        self.taps = [false; 4];
        self.buttons = [false; 2];
        self.delta = [0; 2];
        self.pointer = None;
    }
}
