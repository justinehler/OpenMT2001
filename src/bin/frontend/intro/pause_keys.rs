//! Foreground keyboard dispatch for the 4ba6f8 player-pause action.
use super::{focus, process_frontend::PhysicalSample};
use crate::live_host::process_runtime::{Byte, Error, Handle};
use winit::keyboard::{KeyCode, PhysicalKey};

/// 4ba5eb/4ba62b: the central input callback reads no keys while the media
/// byte 8fca98 or the camera-sequence byte 8b0700 is set.
///
/// This host never runs the 4b94eb/4b921d media probe, so startup leaves
/// 8fca98 awaiting a result for the whole process, in fresh and loaded cities
/// alike. With no probe result the media branch is never taken; treating the
/// missing result as an error here used to drop every F1/P press.
pub(super) fn suppressed(process: &Handle) -> Result<bool, String> {
    let media = match process.byte(Byte::Media8fca98) {
        Ok(byte) => byte,
        Err(Error::MediaProbeUnavailable) => 0,
        Err(error) => return Err(error.to_string()),
    };
    let sequence = process
        .byte(Byte::Suppressed8b0700)
        .map_err(|e| e.to_string())?;
    Ok(media != 0 || sequence != 0)
}

#[derive(Default)]
pub(super) struct Input {
    held: [bool; 2],
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
                self.held = [false; 2];
            }
            return Ok(());
        };
        let index = match key {
            PhysicalKey::Code(KeyCode::F1) => 0,
            PhysicalKey::Code(KeyCode::KeyP) => 1,
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
