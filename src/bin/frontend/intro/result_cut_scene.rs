//! Result (win/loss/complete) cut-scene input and letterbox, shared with the
//! opening sequence's 4b1980 input test and 4b27e0 bars.
use crate::ui::Surface;
use winit::{event::MouseButton, keyboard::NamedKey};

/// Inputs that 4b1980 tests while the sequence permits interruption: held
/// Escape, Return, Space, keypad Enter, and the left or right button. They
/// are levels, not edges, so an input held when bypass becomes enabled ends
/// the cut scene too. A press released before the next service tick still
/// counts once.
#[derive(Default)]
pub(super) struct BypassInputs {
    escape: bool,
    space: bool,
    // winit reports main and keypad Enter as the same named key; a flag
    // stays correct under key auto-repeat.
    enter: bool,
    buttons: [bool; 2],
    pressed: bool,
}

impl BypassInputs {
    /// Records a key edge. Returns false for keys 4b1980 does not test.
    pub(super) fn key(&mut self, key: NamedKey, pressed: bool) -> bool {
        match key {
            NamedKey::Escape => self.escape = pressed,
            NamedKey::Space => self.space = pressed,
            NamedKey::Enter => self.enter = pressed,
            _ => return false,
        }
        self.pressed |= pressed;
        true
    }

    /// Records a mouse button edge. Returns false for other buttons.
    pub(super) fn button(&mut self, button: MouseButton, pressed: bool) -> bool {
        let index = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            _ => return false,
        };
        self.buttons[index] = pressed;
        self.pressed |= pressed;
        true
    }

    /// The level seen by one 50 Hz sequence service tick.
    pub(super) fn sample(&mut self) -> bool {
        let held = self.escape || self.space || self.enter || self.buttons.contains(&true);
        std::mem::take(&mut self.pressed) || held
    }

    /// Focus loss delivers no release events.
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// The result clock's bar height (+0x14) grows to 85 and retracts before
/// CutSceneEnded; 4b27e0 draws it whenever the clock is active.
pub(super) fn letterbox(controller: &crate::result_controller::Controller) -> Surface {
    super::opening::letterbox_bars(controller.letterbox_height())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result_controller::Controller;

    fn bar_edges(controller: &Controller) -> (f32, f32) {
        let bars = letterbox(controller);
        assert_eq!(bars.vertices.len(), 12);
        assert!(bars.vertices.iter().all(|v| v[3..6] == [0.; 3]));
        assert!(bars.alpha.iter().all(|&a| a == 255));
        (bars.vertices[1][1], bars.vertices[6][1])
    }

    #[test]
    fn result_letterbox_grows_from_the_first_tick_and_retracts_before_cut_scene_ended() {
        let mut controller = Controller::new(true, false, 0);
        assert_eq!(bar_edges(&controller), (0., 480.));
        assert!(!controller.tick(false));
        assert_eq!(bar_edges(&controller), (1., 479.));
        for tick in 2..=85 {
            assert!(!controller.tick(false));
            assert_eq!(controller.letterbox_height(), tick as u8);
        }
        for _ in 0..500 {
            assert!(!controller.tick(false));
        }
        assert_eq!(bar_edges(&controller), (85., 395.));
        // The bypass tick already retracts one row.
        assert!(!controller.tick(true));
        assert_eq!(controller.letterbox_height(), 84);
        for height in (0..84).rev() {
            assert!(!controller.tick(false));
            assert_eq!(controller.letterbox_height(), height);
        }
        // The last cut-scene frame still has the bar quads, at height zero.
        assert_eq!(bar_edges(&controller), (0., 480.));
        assert!(
            controller.tick(false),
            "CutSceneEnded follows full retraction"
        );
        assert_eq!(bar_edges(&controller), (0., 480.));
    }

    #[test]
    fn original_bypass_inputs_end_a_bypass_enabled_cut_scene() {
        #[derive(Clone, Copy, Debug)]
        enum Input {
            Key(NamedKey),
            Button(MouseButton),
        }
        let send = |sampler: &mut BypassInputs, input, pressed| match input {
            Input::Key(key) => sampler.key(key, pressed),
            Input::Button(button) => sampler.button(button, pressed),
        };
        // Main and keypad Enter both arrive as NamedKey::Enter.
        let inputs = [
            Input::Key(NamedKey::Escape),
            Input::Key(NamedKey::Space),
            Input::Key(NamedKey::Enter),
            Input::Button(MouseButton::Left),
            Input::Button(MouseButton::Right),
        ];
        for input in inputs {
            for bypass in [true, false] {
                let mut controller = Controller::new(bypass, false, 0);
                let mut sampler = BypassInputs::default();
                for _ in 0..100 {
                    assert!(!controller.tick(sampler.sample()));
                }
                // A click released before the next service tick still counts.
                assert!(send(&mut sampler, input, true));
                assert!(send(&mut sampler, input, false));
                let ended = (0..200).find(|_| controller.tick(sampler.sample()));
                assert_eq!(ended.is_some(), bypass, "{input:?}, bypass {bypass}");
            }
        }
    }

    #[test]
    fn held_bypass_input_is_a_level_and_other_inputs_are_ignored() {
        let mut sampler = BypassInputs::default();
        assert!(!sampler.key(NamedKey::Tab, true));
        assert!(!sampler.key(NamedKey::ArrowUp, true));
        assert!(!sampler.button(MouseButton::Middle, true));
        assert!(!sampler.sample());
        // Held through a disabled phase, it ends the scene once bypass is enabled.
        let mut controller = Controller::new(false, false, 0);
        sampler.button(MouseButton::Left, true);
        for _ in 0..50 {
            assert!(!controller.tick(sampler.sample()));
        }
        controller.set_bypass(true);
        assert!((0..200).any(|_| controller.tick(sampler.sample())));
        // Auto-repeat presses still release on one key-up.
        let mut sampler = BypassInputs::default();
        sampler.key(NamedKey::Enter, true);
        sampler.key(NamedKey::Enter, true);
        assert!(sampler.sample());
        assert!(sampler.sample());
        sampler.key(NamedKey::Enter, false);
        assert!(!sampler.sample());
        sampler.key(NamedKey::Space, true);
        sampler.clear();
        assert!(!sampler.sample());
    }
}
