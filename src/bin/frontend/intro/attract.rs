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
                self.start_menu_presentation(now, false)?;
                break;
            }
        }
        if enabled && self.menu.as_mut().is_some_and(|menu| menu.take_credits()) {
            self.start_menu_presentation(now, true)?;
        }
        if self.attract_menu.is_some() && self.attract_input.skip() {
            self.opening.request_end();
        }
        Ok(())
    }
    fn start_menu_presentation(&mut self, now: Instant, credits: bool) -> Result<(), String> {
        let tick = (now.saturating_duration_since(self.started).as_secs_f64() * 50.) as usize;
        let current = self
            .menu_camera
            .as_mut()
            .ok_or("attract menu camera missing")?
            .sample(tick, 0.);
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as u32;
        let backdrop = openmt2001::flyby_script::select_seeded(
            &self.assets.flyby_hub,
            self.assets.flyby_quality.as_deref(),
            seed,
            || {
                self.assets
                    .process_runtime
                    .performance8b26cc()
                    .map_err(|e| e.to_string())
            },
        )?;
        let bytes = std::fs::read(
            self.assets
                .inputs
                .installer
                .join("Standard/Cut_scenes")
                .join(&backdrop.track),
        )
        .map_err(|e| e.to_string())?;
        self.assets
            .flyby
            .restart_track(&Flyby::parse(&bytes)?, current)?;
        self.assets.opening_logos = backdrop.logos;
        let camera = self.assets.flyby.current();
        for index in self
            .assets
            .scene
            .advance_daylight(backdrop.hour, 0, camera)?
        {
            let draw = &self.assets.scene.draws[index];
            self.renderer.update_dynamic_surface(
                &mut self.prepared,
                index,
                &draw.borrowed(),
                draw.view_projection.unwrap_or(self.assets.scene.camera),
                self.assets.scene.fog,
            )?;
        }
        self.credit_rows.clear();
        if credits {
            let seed = openmt2001::startup_rng::Snapshot::from_seed(seed);
            let mut random = seed.state(0x8b06e8);
            self.credit_rows = crate::fresh_city::result::credit_lines(
                &openmt2001::pe::PeImage::parse(&self.assets.inputs.executable)?,
                &mut random,
            )?;
        }
        self.opening =
            openmt2001::opening_sequence::Controller::with_credits(self.credit_rows.len());
        //4b1800 resets the media catalog before starting music_intro.wma.
        self.restart_intro_music = credits;
        self.ended = false;
        self.attract_menu = self.menu.take();
        self.menu_camera = None;
        self.started = now;
        self.last_sample = None;
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
        input.sample(PhysicalSample::Pointer);
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
