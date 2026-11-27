//! Foreground keyboard dispatch for the 4ba6f8 player-pause action.
use super::{focus, process_frontend::PhysicalSample};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Default)]
pub(super) struct Input {
    held: [bool; 3],
}
#[derive(Default, Clone, Copy)]
pub(super) struct Gates {
    pub active: bool,
    pub modal: bool,
    pub text_entry: bool,
    pub suppressed: bool,
}
impl Input {
    pub(super) fn dispatch(
        &mut self,
        sample: PhysicalSample,
        gates: Gates,
        route: &mut focus::Route,
    ) -> Result<(), String> {
        let PhysicalSample::Key { key, down, repeat } = sample else {
            if matches!(sample, PhysicalSample::Unfocus) {
                self.held = [false; 3];
            }
            return Ok(());
        };
        let index = match key {
            PhysicalKey::Code(KeyCode::F1) => 0,
            PhysicalKey::Code(KeyCode::KeyP) => 1,
            // Retain the existing host convenience binding through the same owner.
            PhysicalKey::Code(KeyCode::Space) => 2,
            _ => return Ok(()),
        };
        let edge = down && !self.held[index] && !repeat;
        self.held[index] = down;
        // Native F1 bypasses the P text-entry guard. Neither bypasses mode4,
        // media/suppression or the worker's authoritative network restriction.
        if edge
            && gates.active
            && !gates.modal
            && !gates.suppressed
            && (index == 0 || !gates.text_entry)
        {
            route.toggle_player_pause()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "pause_keys_tests.rs"]
mod tests;
