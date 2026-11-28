//! Main menu -> shared opening presentation -> retained main menu.
use super::*;
use process_frontend::PhysicalSample;
use winit::{
    event::MouseButton,
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Default)]
pub(super) struct Input {
    held: [bool; 6],
}
impl Input {
    pub(super) fn sample(&mut self, sample: PhysicalSample) {
        let (index, down) = match sample {
            PhysicalSample::Key { key, down, .. } | PhysicalSample::SyntheticKey { key, down } => {
                let index = match key {
                    PhysicalKey::Code(KeyCode::Escape) => 0,
                    PhysicalKey::Code(KeyCode::Space) => 1,
                    PhysicalKey::Code(KeyCode::Enter) => 2,
                    PhysicalKey::Code(KeyCode::NumpadEnter) => 3,
                    _ => return,
                };
                (index, down)
            }
            PhysicalSample::Button {
                button: MouseButton::Left,
                down,
            } => (4, down),
            PhysicalSample::Button {
                button: MouseButton::Right,
                down,
            } => (5, down),
            PhysicalSample::Unfocus => {
                self.held.fill(false);
                return;
            }
            _ => return,
        };
        self.held[index] = down;
    }
    fn skip(&self) -> bool {
        self.held.contains(&true)
    }
}

impl Intro {
    pub(super) fn service_attract(&mut self, now: Instant) -> Result<(), String> {
        let elapsed = now.saturating_duration_since(self.menu_updated);
        self.menu_updated = now;
        if self.attract_menu.is_some() && (!self.menu_focused || self.focus_pending()) {
            self.started += elapsed;
        }
        if (self.inspection.is_some() || self.loading.is_some())
            && let Some(menu) = &mut self.menu
        {
            menu.suspend_attract();
        }
        let enabled = self.menu.is_some()
            && self.inspection.is_none()
            && self.loading.is_none()
            && self.menu_focused
            && !self.focus_pending();
        for _ in 0..self.menu_clock.delivered(elapsed, enabled) {
            if self.menu.as_mut().is_some_and(|menu| menu.attract_event1()) {
                let tick =
                    (now.saturating_duration_since(self.started).as_secs_f64() * 50.) as usize;
                let current = self
                    .menu_camera
                    .as_mut()
                    .ok_or("attract menu camera missing")?
                    .sample(tick, 0.);
                self.assets.flyby.restart(current)?;
                self.opening = Default::default();
                self.ended = false;
                self.attract_menu = self.menu.take();
                self.menu_camera = None;
                self.started = now;
                self.last_sample = None;
                break;
            }
        }
        if self.attract_menu.is_some() && self.attract_input.skip() {
            self.opening.request_end();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_skip_inputs_are_held_keys_and_both_mouse_buttons_not_motion() {
        let mut input = Input::default();
        for code in [
            KeyCode::Escape,
            KeyCode::Space,
            KeyCode::Enter,
            KeyCode::NumpadEnter,
        ] {
            let sample = |down| PhysicalSample::Key {
                key: PhysicalKey::Code(code),
                down,
                repeat: false,
            };
            input.sample(sample(true));
            assert!(input.skip());
            input.sample(sample(false));
            assert!(!input.skip());
        }
        for button in [MouseButton::Left, MouseButton::Right] {
            input.sample(PhysicalSample::Button { button, down: true });
            assert!(input.skip());
            input.sample(PhysicalSample::Unfocus);
            assert!(!input.skip());
        }
        input.sample(PhysicalSample::Pointer([100., 200.]));
        input.sample(PhysicalSample::Key {
            key: PhysicalKey::Code(KeyCode::ArrowDown),
            down: true,
            repeat: false,
        });
        input.sample(PhysicalSample::Button {
            button: MouseButton::Middle,
            down: true,
        });
        assert!(!input.skip());
    }
}
