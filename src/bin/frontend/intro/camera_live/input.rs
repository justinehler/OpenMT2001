//! Process physical events, captured before any menu/Inspection routing.
//! No tutorial/Build/UI gate belongs in these event producers.
//! The camera reads relative device motion only; the absolute window pointer
//! stays with the UI, as in the original (0x4e51c0, 0x4e4f50).
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
    remainder: [f64; 2],
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

    /// Relative device motion (winit `DeviceEvent::MouseMotion`), standing in
    /// for the DirectInput mouse the original polls with `GetDeviceState` in
    /// relative-axis mode (0x4e51c0). It is accumulated until the next 50 Hz
    /// callback consumes it, and keeps arriving when the OS pointer is pinned
    /// at a screen edge. Fractional counts carry over to later motion.
    /// Unsupported magnitudes fail at the next boundary rather than saturating.
    pub fn motion(&mut self, movement: [f64; 2]) {
        if !movement.into_iter().all(f64::is_finite) {
            self.error = Some("Nonfinite physical camera motion".into());
            return;
        }
        for (axis, moved) in movement.into_iter().enumerate() {
            let total = self.remainder[axis] + moved;
            let whole = total.trunc();
            if whole < f64::from(i32::MIN) || whole > f64::from(i32::MAX) {
                self.error = Some("Physical camera delta leaves signed32 domain".into());
                return;
            }
            let Some(next) = self.delta[axis].checked_add(whole as i32) else {
                self.error = Some("Accumulated physical camera delta overflow".into());
                return;
            };
            self.delta[axis] = next;
            self.remainder[axis] = total - whole;
        }
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
        self.remainder = [0.; 2];
    }
}

#[cfg(test)]
mod tests {
    use super::PhysicalInput;
    use crate::intro::process_frontend::{PhysicalSample, project_camera_sample};
    use openmt2001::camera::BlockCamera;
    use winit::event::MouseButton;

    fn camera() -> BlockCamera {
        BlockCamera {
            center: [0.; 3],
            angle: 0.,
            height: 100.,
            radius: 50.,
        }
    }

    /// One 50 Hz callback: consume the pending delta as a right-drag orbit.
    fn poll(physical: &mut PhysicalInput, camera: &mut BlockCamera) -> [i32; 2] {
        let (_, buttons, delta) = physical.snapshot().unwrap();
        assert_eq!(buttons, [false, true]);
        let mut pose = camera.tick([false; 4]);
        camera
            .right_drag(&mut pose, delta, 50, false, false, false)
            .unwrap();
        physical.consumed();
        delta
    }

    #[test]
    fn relative_motion_past_a_pinned_pointer_keeps_rotating() {
        let mut physical = PhysicalInput::default();
        let mut camera = camera();
        let right = |down| PhysicalSample::Button {
            button: MouseButton::Right,
            down,
        };
        project_camera_sample(&mut physical, None, right(true));
        let mut angle = camera.angle;
        for _ in 0..20 {
            // The OS pointer is pinned at the window's right edge; only the
            // device keeps reporting motion.
            for _ in 0..3 {
                project_camera_sample(&mut physical, None, PhysicalSample::Pointer);
                project_camera_sample(&mut physical, None, PhysicalSample::Motion([2., 0.]));
            }
            assert_eq!(poll(&mut physical, &mut camera), [6, 0]);
            assert!(camera.angle < angle, "the camera keeps turning");
            angle = camera.angle;
        }
        // Window pointer moves alone never move the camera.
        for _ in 0..3 {
            project_camera_sample(&mut physical, None, PhysicalSample::Pointer);
        }
        assert_eq!(poll(&mut physical, &mut camera), [0, 0]);
        assert_eq!(camera.angle, angle);
    }

    #[test]
    fn yaw_clamp_applies_per_poll_and_focus_loss_clears_pending_motion() {
        let mut physical = PhysicalInput::default();
        physical.button(MouseButton::Right, true);
        let mut fast = camera();
        let mut clamped = camera();
        physical.motion([90., 0.]);
        physical.motion([40.5, 0.]);
        assert_eq!(poll(&mut physical, &mut fast), [130, 0]);
        let mut pose = clamped.tick([false; 4]);
        clamped
            .right_drag(&mut pose, [14, 0], 50, false, false, false)
            .unwrap();
        assert_eq!(
            fast.angle, clamped.angle,
            "one poll turns at most 14 counts"
        );
        // The excess is spent with that poll, not carried into the next.
        assert_eq!(poll(&mut physical, &mut fast), [0, 0]);
        assert_eq!(fast.angle, clamped.angle);

        // Only the half count carried from 40.5 remains; focus loss drops it
        // with the pending motion and the held button.
        physical.motion([7., -3.]);
        physical.unfocus();
        assert_eq!(
            physical.snapshot().unwrap(),
            ([false; 4], [false; 2], [0; 2])
        );
        physical.motion([0.5, 0.]);
        assert_eq!(physical.snapshot().unwrap().2, [0, 0]);
        physical.motion([0.5, 0.]);
        assert_eq!(physical.snapshot().unwrap().2, [1, 0]);
    }
}
