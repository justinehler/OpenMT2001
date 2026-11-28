//! Original4b1980/4b1a50/4b1eb0 result transition clock (50 Hz).
//! The script callback is emitted only when the controller's fade ends.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub active: bool,
    pub mode: u32,
    pub phase: u32,
    pub fade: u32,
    pub alpha: u32,
}

pub struct Controller {
    state: State,
    bypass: bool,
    credits: bool,
    credit_rows: usize,
    shown_rows: usize,
    phase_ticks: u32,
    frame: u32,
    text_present: bool,
    credit_construction_edge: bool,
}

impl Controller {
    pub fn new(bypass: bool, credits: bool, credit_rows: usize) -> Self {
        Self {
            state: State {
                active: true,
                mode: 1,
                phase: 0,
                fade: 0,
                alpha: 0,
            },
            bypass,
            credits,
            credit_rows,
            shown_rows: 0,
            phase_ticks: 0,
            frame: 0,
            text_present: true,
            credit_construction_edge: false,
        }
    }
    #[cfg(test)]
    pub fn state(&self) -> State {
        self.state
    }
    /// Original4b22e8 uses the current controller alpha as the result title's
    /// high colour byte. Kept separate from the script's text identifier.
    pub fn alpha(&self) -> u8 {
        self.state.alpha.min(255) as u8
    }
    /// Original4b27c4 dispatches phase0/1 to the result title and13/14 to
    /// the current4b1b20 credit record. Other phases draw neither text.
    pub fn title_visible(&self) -> bool {
        self.state.active && self.state.phase <= 1
    }
    pub fn credit_row(&self) -> Option<usize> {
        (self.state.active
            && self.credits
            && matches!(self.state.phase, 13 | 14)
            && self.shown_rows > 0
            && self.shown_rows <= self.credit_rows)
            .then_some(self.shown_rows.saturating_sub(1))
    }
    /// Returns true once on native4b1f82's synchronous4b1bc0 call tick.
    pub fn take_credit_construction_edge(&mut self) -> bool {
        std::mem::take(&mut self.credit_construction_edge)
    }
    pub fn set_identifier(&mut self, identifier: Option<u32>) {
        self.text_present = identifier.is_some();
    }
    pub fn set_bypass(&mut self, enabled: bool) {
        self.bypass = enabled;
    }
    /// Returns the native CutSceneEnded dispatch edge for this service tick.
    pub fn tick(&mut self, bypass_input: bool) -> bool {
        if !self.state.active {
            return false;
        }
        self.frame = self.frame.wrapping_add(1);
        if bypass_input && self.bypass {
            self.state.mode = 2;
        }
        let mut ended = false;
        let mut alpha_underflow = 0i32;
        match self.state.mode {
            1 => {
                if self.state.fade >= 85 {
                    self.state.mode = 0;
                } else {
                    self.state.fade += 1;
                }
            }
            2 => {
                if self.state.alpha > 0 {
                    let alpha = self.state.alpha as i32 - 5;
                    alpha_underflow = alpha.min(0);
                    self.state.alpha = alpha.max(0) as u32;
                }
                if self.state.fade == 0 {
                    self.state.active = false;
                    self.state.mode = 0;
                    ended = true;
                } else {
                    self.state.fade -= 1;
                }
            }
            _ => (),
        }
        let previous_phase = self.state.phase;
        self.advance_text();
        if self.credits && previous_phase == 1 && self.state.phase == 12 {
            self.credit_construction_edge = true;
        }
        if alpha_underflow < 0 && self.state.phase == 0 && self.text_present {
            self.state.alpha = (alpha_underflow + 2).max(0) as u32;
        }
        ended
    }
    fn advance_text(&mut self) {
        if !self.credits {
            if self.text_present {
                self.state.alpha = (self.state.alpha + 2).min(254);
            }
            return;
        }
        match self.state.phase {
            0 => {
                if self.state.alpha >= 254 {
                    self.state.phase = 1;
                    self.phase_ticks = 0;
                } else {
                    self.state.alpha = (self.state.alpha + 2).min(254);
                }
            }
            1 => {
                self.phase_ticks += 1;
                self.state.alpha = self.state.alpha.saturating_sub(2);
                if self.phase_ticks >= 128 {
                    self.state.alpha = 0;
                    self.state.phase = 12;
                    self.phase_ticks = 0;
                }
            }
            14 => {
                self.phase_ticks += 1;
                self.state.alpha = self.state.alpha.saturating_sub(2);
                if self.phase_ticks >= 128 {
                    self.state.alpha = 0;
                    self.phase_ticks = 0;
                    if self.shown_rows >= self.credit_rows {
                        self.state.phase = 2;
                        self.state.mode = 2;
                    } else {
                        self.state.phase = 12;
                    }
                }
            }
            12 => {
                self.shown_rows += 1;
                if self.shown_rows > self.credit_rows {
                    self.state.phase = 2;
                    self.state.mode = 2;
                } else {
                    self.state.phase = 13;
                    self.phase_ticks = 0;
                }
            }
            13 => {
                self.phase_ticks += 1;
                self.state.alpha = (self.state.alpha + 2).min(254);
                if self.phase_ticks >= 128 {
                    self.state.phase = 14;
                    self.phase_ticks = 0;
                }
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Explicit-PC64 native v1.4 4b1980/4b1eb0 observations from
    // tools/native_result_controller.py (work/frontend/native-result-controller.json).
    #[test]
    fn native_result_transition_edges() {
        let cases = [
            ("wait", false, true, None, 300, None),
            ("skip101", false, true, Some(101), 300, Some(186)),
            ("skip1", false, true, Some(1), 300, Some(1)),
            ("disabled", false, false, Some(101), 300, None),
            ("credits", true, false, None, 1500, Some(1113)),
        ];
        for (name, credits, bypass, skip, frames, expected) in cases {
            let mut controller = Controller::new(bypass, credits, 3);
            let mut ended = None;
            for frame in 1..=frames {
                if controller.tick(skip == Some(frame)) {
                    ended = Some(frame);
                    break;
                }
            }
            assert_eq!(ended, expected, "{name}");
        }
    }
    #[test]
    #[ignore = "requires explicit-PC64 native result-controller fixture"]
    fn native_result_every_service_tick() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESULT_CONTROLLER")).unwrap();
        assert_eq!(&bytes[..8], b"MTRCTRL1");
        let mut at = 8;
        let word = |bytes: &[u8], at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&bytes, &mut at), 6);
        for case in 0..6 {
            let name_len = word(&bytes, &mut at) as usize;
            let frames = word(&bytes, &mut at) as usize;
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            let (credits, bypass, skip) = match case {
                0 => (false, true, None),
                1 => (false, true, Some(101)),
                2 => (false, true, Some(1)),
                3 => (false, false, Some(101)),
                4 => (true, false, None),
                5 => (false, true, None),
                _ => unreachable!(),
            };
            let mut controller = Controller::new(bypass, credits, 3);
            if case == 5 {
                controller.set_identifier(None);
            }
            for frame in 1..=frames {
                if case == 5 && frame == 501 {
                    controller.set_identifier(Some(1));
                }
                controller.tick(skip == Some(frame));
                let expected = std::array::from_fn::<_, 5, _>(|_| word(&bytes, &mut at));
                let state = controller.state();
                let actual = [
                    u32::from(state.active),
                    state.mode,
                    state.phase,
                    state.fade,
                    state.alpha,
                ];
                assert_eq!(actual, expected, "{name} tick{frame}");
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original explicit-PC64 full credit timing fixture"]
    fn original_complete_thirty_two_credit_rows() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESULT_CREDITS_TIMING")).unwrap();
        assert_eq!(&bytes[..8], b"MTRC32T2");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        let frames = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let construct_tick = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 20 + frames * 24);
        let mut controller = Controller::new(false, true, 32);
        controller.set_identifier(Some(3));
        let mut ended = None;
        for frame in 1..=frames {
            if controller.tick(false) {
                ended = Some(frame);
            }
            assert_eq!(
                controller.take_credit_construction_edge(),
                frame == construct_tick,
                "credit constructor edge tick{frame}"
            );
            let at = 20 + (frame - 1) * 24;
            let native: [u32; 6] = std::array::from_fn(|column| {
                u32::from_le_bytes(
                    bytes[at + column * 4..at + column * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            });
            let state = controller.state();
            assert_eq!(
                [
                    u32::from(state.active),
                    state.mode,
                    state.phase,
                    state.fade,
                    state.alpha
                ],
                native[..5],
                "complete result tick{frame}"
            );
            if matches!(state.phase, 13 | 14) {
                assert_eq!(
                    controller.credit_row(),
                    Some(native[5] as usize),
                    "credit row tick{frame}"
                );
            } else {
                assert_eq!(
                    controller.credit_row(),
                    None,
                    "non-credit phase tick{frame}"
                );
            }
        }
        assert_eq!(ended, Some(8566));
    }
}
