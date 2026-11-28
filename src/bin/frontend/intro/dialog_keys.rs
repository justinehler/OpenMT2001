//! Physical Return/Backspace history, sampled once per frontend UI event1.
use super::process_frontend::PhysicalSample;
use winit::keyboard::{KeyCode, PhysicalKey};

/// Lives across prompt replacement and city presentation changes. An edge is
/// consumed even when there is no eligible dialog; it is never saved for one.
pub(super) struct Input {
    held: [bool; 2],
    previous: [bool; 2],
    focused: bool,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            held: [false; 2],
            previous: [false; 2],
            focused: true,
        }
    }
}
impl Input {
    pub(super) fn focus(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.held = [false; 2];
        }
        self.previous = self.held;
    }
    pub(super) fn physical(&mut self, sample: PhysicalSample) {
        let (key, down, repeat, synthetic) = match sample {
            PhysicalSample::Key { key, down, repeat } => (key, down, repeat, false),
            PhysicalSample::SyntheticKey { key, down } => (key, down, false, true),
            PhysicalSample::Unfocus => {
                self.held = [false; 2];
                self.previous = [false; 2];
                return;
            }
            _ => return,
        };
        let index = match key {
            PhysicalKey::Code(KeyCode::Enter) => 0,
            PhysicalKey::Code(KeyCode::Backspace) => 1,
            _ => return,
        };
        if !repeat {
            self.held[index] = down;
        }
        if synthetic {
            self.previous[index] = down;
        }
    }
    /// Returns a notice's deferred script dismissal id, if Return started one.
    pub(super) fn event1(&mut self, prompts: Option<&mut crate::prompt::State>) -> Option<u32> {
        let edges = std::array::from_fn(|i| self.held[i] && !self.previous[i]);
        self.previous = self.held;
        if self.focused
            && let Some(prompts) = prompts
        {
            return prompts.dialog_event1(edges);
        }
        None
    }
}

#[cfg(test)]
#[path = "dialog_keys_tests.rs"]
mod tests;
