//! Original4958f4/495670 main-menu confirmation; ordinary single-modal owner.
//! UI samples are supplied by App redraws, not OS keydown callbacks.
use crate::human_dialog::Dialog;
pub const OFFSET: [f32; 2] = [-40., 0.];
pub const COLORS: [[u8; 3]; 2] = [[80, 148, 34], [175, 6, 6]];
pub const CALLBACK: u32 = 0x495670;

pub fn dialog() -> Dialog {
    Dialog {
        portrait: u32::MAX,
        title: 1765,
        body: 1766,
        buttons: [1636, 1637],
        button_count: 2,
        callback: CALLBACK,
        context: 0,
        timeout: false,
        handler_written: true,
        context_written: true,
        timeout_written: true,
        ..Dialog::default()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    current: [bool; 2],
    previous: [bool; 2],
}
/// App owns these arrays across menu construction, modal opening and closing.
/// Only physical main Return and Backspace are admitted. A repeated keydown
/// never fabricates an up/down edge; keypad Enter is a different physical code.
#[derive(Default)]
pub struct Keys {
    held: [bool; 2],
    current: [bool; 2],
    right: bool,
}
impl Keys {
    pub fn physical(&mut self, key: winit::keyboard::PhysicalKey, down: bool, repeat: bool) {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let index = match key {
            PhysicalKey::Code(KeyCode::Enter) => 0,
            PhysicalKey::Code(KeyCode::Backspace) => 1,
            _ => return,
        };
        if !repeat {
            self.held[index] = down;
        }
    }
    pub fn mouse_right(&mut self, down: bool) {
        self.right = down;
    }
    pub fn right(&self) -> bool {
        self.right
    }
    pub fn release_on_unfocus(&mut self) {
        self.held = [false; 2];
        self.right = false;
    }
    pub fn sample(&mut self) -> Sample {
        let sample = Sample {
            previous: self.current,
            current: self.held,
        };
        self.current = self.held;
        sample
    }
}
#[derive(Default)]
pub struct Modal {
    active: bool,
    blocking: bool,
    armed: bool,
    cursor: Option<[f32; 2]>,
    left: bool,
    right: bool,
    origin: Option<usize>,
}
impl Modal {
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn open(&mut self, cursor: Option<[f32; 2]>) {
        // Does not touch App's physical current/previous/held arrays.
        *self = Self {
            active: true,
            blocking: true,
            cursor,
            right: self.right,
            ..Self::default()
        };
    }
    pub fn cursor(&mut self, cursor: [f32; 2]) {
        self.cursor = Some(cursor);
    }
    pub fn pointer(&self) -> Option<[f32; 2]> {
        self.cursor
    }
    pub fn blocked_by_right(&self) -> bool {
        self.right
    }
    pub fn left_held(&self) -> bool {
        self.left
    }
    pub fn right(&mut self, down: bool) {
        self.right = down;
    }
    pub fn pressed_button(&self) -> Option<usize> {
        if self.left { self.origin } else { None }
    }
    pub fn button_state(&self, button: usize, hover: bool) -> openmt2001::ui_button::State {
        openmt2001::ui_button::State {
            flag_330: self.left && hover,
            flag_334: self.left && self.origin == Some(button) && hover,
            flag_335: hover,
            flag_333: false,
        }
    }
    pub fn cancel_pointer(&mut self) {
        self.cursor = None;
        self.left = false;
        self.right = false;
        self.origin = None;
    }
    /// Called once per eligible menu UI event1, with samples taken BEFORE it.
    /// Two independent raw branches, ordered Return then Backspace.
    pub fn event1(&mut self, sample: Sample) -> [bool; 2] {
        if !self.active {
            return [false; 2];
        }
        if !self.armed {
            self.armed = true;
            return [false; 2];
        }
        [
            sample.current[0] && !sample.previous[0],
            sample.current[1] && !sample.previous[1],
        ]
    }
    pub fn left(&mut self, down: bool, hit: Option<usize>) -> Option<usize> {
        if !self.active {
            return None;
        }
        let hit = if self.right { None } else { hit };
        if down {
            if !self.left {
                self.origin = hit;
            }
            self.left = true;
            None
        } else {
            let answer = self
                .left
                .then_some(self.origin)
                .flatten()
                .filter(|origin| Some(*origin) == hit);
            self.left = false;
            self.origin = None;
            answer
        }
    }
    /// Callback runs with the record still linked, after releasing modal
    /// blocking, as495670 calls43cbf0 before testing its reply argument.
    pub fn reply_with(
        &mut self,
        button: usize,
        callback: impl FnOnce(&Self, crate::human_dialog::Reply),
    ) {
        if !self.active || button > 1 {
            return;
        }
        self.blocking = false;
        callback(
            self,
            crate::human_dialog::Reply {
                callback: CALLBACK,
                context: 0,
                button: button as u32,
            },
        );
        // The ordinary owner retires this record AFTER the callback. No claim
        // about original full queues/reentrant replacement or optional8aaa24.
        self.active = false;
        self.left = false;
        self.origin = None;
    }
}
pub fn hit(bounds: [f32; 4], cursor: [f32; 2]) -> bool {
    cursor[0] > bounds[0] + OFFSET[0]
        && cursor[0] < bounds[2] + OFFSET[0]
        && cursor[1] >= bounds[1] + OFFSET[1]
        && cursor[1] <= bounds[3] + OFFSET[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{KeyCode, PhysicalKey};
    fn key(keys: &mut Keys, code: KeyCode, down: bool) {
        keys.physical(PhysicalKey::Code(code), down, false);
    }
    #[test]
    fn held_opening_return_and_repeat_need_release_then_repress() {
        let mut keys = Keys::default();
        let mut modal = Modal::default();
        key(&mut keys, KeyCode::Enter, true);
        modal.open(None);
        assert_eq!(modal.event1(keys.sample()), [false; 2]); // first UI event1
        for _ in 0..4 {
            keys.physical(PhysicalKey::Code(KeyCode::Enter), true, true);
            assert_eq!(modal.event1(keys.sample()), [false; 2]);
        }
        key(&mut keys, KeyCode::Enter, false);
        assert_eq!(modal.event1(keys.sample()), [false; 2]);
        key(&mut keys, KeyCode::Enter, true);
        assert_eq!(modal.event1(keys.sample()), [true, false]);
    }
    #[test]
    fn arming_is_a_ui_tick_and_physical_backspace_is_negative() {
        let mut keys = Keys::default();
        let mut modal = Modal::default();
        modal.open(None);
        // OS events alone cannot arm. A tap on the suppressed first sample
        // is ignored, and logical Escape/keypad Enter never alias our keys.
        key(&mut keys, KeyCode::Backspace, true);
        assert!(!modal.armed);
        assert_eq!(modal.event1(keys.sample()), [false; 2]);
        key(&mut keys, KeyCode::Backspace, false);
        assert_eq!(modal.event1(keys.sample()), [false; 2]);
        for code in [
            KeyCode::Escape,
            KeyCode::NumpadEnter,
            KeyCode::Space,
            KeyCode::Tab,
        ] {
            key(&mut keys, code, true);
            assert_eq!(modal.event1(keys.sample()), [false; 2]);
        }
        for _ in 0..1050 {
            assert_eq!(modal.event1(keys.sample()), [false; 2]);
            assert!(modal.active()); // original quit has no1000-tick timeout
        }
        key(&mut keys, KeyCode::Backspace, true);
        assert_eq!(modal.event1(keys.sample()), [false, true]);
    }
    #[test]
    fn reply_before_unlink_and_fresh_reopen_preserve_global_samples() {
        let mut keys = Keys::default();
        let mut modal = Modal::default();
        modal.open(Some([250., 280.]));
        modal.event1(keys.sample());
        key(&mut keys, KeyCode::Backspace, true);
        assert_eq!(modal.event1(keys.sample()), [false, true]);
        let mut trace = vec![];
        modal.reply_with(1, |record, reply| {
            assert!(record.active);
            assert!(!record.blocking);
            trace.push((reply.callback, reply.context, reply.button));
        });
        assert!(!modal.active());
        modal.open(None);
        assert_eq!(modal.event1(keys.sample()), [false; 2]);
        assert_eq!(modal.event1(keys.sample()), [false; 2]); // still held Backspace
        key(&mut keys, KeyCode::Backspace, false);
        modal.event1(keys.sample());
        key(&mut keys, KeyCode::Enter, true);
        key(&mut keys, KeyCode::Backspace, true);
        assert_eq!(modal.event1(keys.sample()), [true, true]); // independent branches
        modal.reply_with(0, |record, reply| {
            assert!(record.active);
            trace.push((reply.callback, reply.context, reply.button));
        });
        modal.reply_with(1, |_, _| {
            panic!("retired ordinary modal cannot answer again")
        });
        assert_eq!(trace, vec![(0x495670, 0, 1), (0x495670, 0, 0)]);
    }
    #[test]
    fn mouse_identity_right_block_and_focus_cancel_cannot_leak_release() {
        let mut modal = Modal::default();
        modal.open(None);
        assert_eq!(modal.left(false, Some(0)), None); // opening release isn't a modal press
        modal.left(true, Some(0));
        assert_eq!(modal.left(false, Some(1)), None); // drag to different button
        modal.left(true, Some(1));
        modal.right(true);
        assert_eq!(modal.left(false, Some(1)), None);
        modal.right(false);
        modal.left(true, Some(1));
        modal.cancel_pointer();
        assert_eq!(modal.left(false, Some(1)), None);
        modal.left(true, Some(1));
        assert_eq!(modal.left(false, Some(1)), Some(1));
        assert!(modal.active()); // answer is dispatched separately before unlink
    }
    #[test]
    fn original_layout_boundaries_use_one_shift_and_strict_horizontal_edges() {
        let main = [224.800003, 180.292099, 505.183777, 303.631714];
        let buttons = crate::human_dialog::buttons(main, 2);
        assert_eq!(
            buttons[0].map(f32::to_bits),
            [
                278.2612609863281_f32,
                274.79681396484375,
                338.2612609863281,
                298.79681396484375
            ]
            .map(f32::to_bits)
        );
        assert_eq!(buttons[1][0].to_bits(), 391.7225341796875_f32.to_bits());
        let b = buttons[0];
        assert!(!hit(b, [b[0] - 40., b[1]]));
        assert!(!hit(b, [b[2] - 40., b[1]]));
        assert!(hit(b, [250., b[1]]));
        assert!(hit(b, [250., b[3]]));
        assert!(!hit(b, [250., b[3] + 1.]));
        assert!(!hit(b, [b[2] - 1., b[1]])); // unshifted geometry is not clickable
    }
}
