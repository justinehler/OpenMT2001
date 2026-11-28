//! Opening presentation clock from 4b1a50/4b1eb0, serviced at 50 Hz.
//! Camera tracks outlive these cards; their final frame is not the menu trigger.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Publisher,
    Developer,
    Title,
}

impl Card {
    pub fn texture(self) -> usize {
        match self {
            Self::Publisher => 274,
            Self::Developer => 273,
            Self::Title => 287,
        }
    }
}

#[derive(Default)]
pub struct Controller {
    tick: usize,
    phase: u8,
    phase_ticks: u32,
    alpha: u8,
    height: u8,
    closing: bool,
    ended: bool,
    credits: usize,
    credit_index: usize,
}

impl Controller {
    /// Settings uses the same logo phases followed by 4b1bc0's credit rows.
    pub fn with_credits(rows: usize) -> Self {
        Self {
            credits: rows,
            ..Self::default()
        }
    }
    pub fn credit_row(&self) -> Option<(usize, u8)> {
        (!self.ended && matches!(self.phase, 13 | 14) && self.credit_index < self.credits)
            .then_some((self.credit_index, self.alpha))
    }
    pub fn height(&self) -> u8 {
        self.height
    }
    pub fn card(&self) -> Option<(Card, u8)> {
        let card = match self.phase {
            3..=5 => Card::Publisher,
            6..=8 => Card::Developer,
            9..=11 => Card::Title,
            _ => return None,
        };
        (!self.ended).then_some((card, self.alpha))
    }
    pub fn ended(&self) -> bool {
        self.ended
    }
    /// 4b1980 requests state2: fade the current caption and retract the bars.
    /// Repeated held input must not restart or postpone the closing animation.
    pub fn request_end(&mut self) {
        self.closing = true;
    }
    /// The caller supplies elapsed camera-service ticks, independently of FPS.
    pub fn advance_to(&mut self, tick: usize) {
        while self.tick < tick && !self.ended {
            self.tick += 1;
            self.advance();
        }
    }
    fn phase(&mut self, phase: u8) {
        self.phase = phase;
        self.phase_ticks = 0;
        if matches!(phase, 3 | 6 | 9) {
            self.alpha = 0;
        } else if matches!(phase, 4 | 7 | 10) {
            self.alpha = 255;
        }
    }
    fn advance(&mut self) {
        // Letterbox update precedes the caption update. Closing therefore
        // starts on the service tick after the last card reaches zero.
        if self.closing {
            self.alpha = self.alpha.saturating_sub(5);
            if self.height == 0 {
                self.ended = true;
            } else {
                self.height -= 1;
            }
        } else {
            self.height = (self.height + 1).min(85);
        }
        self.phase_ticks += 1;
        match self.phase {
            0 => self.phase(2),
            2 if self.phase_ticks == 100 => self.phase(3),
            3 | 6 | 9 => {
                if self.alpha == 255 {
                    self.phase(self.phase + 2);
                } else {
                    self.alpha += 1;
                }
            }
            5 | 8 | 11 if self.phase_ticks == 25 => self.phase(self.phase - 1),
            4 | 7 | 10 => {
                if self.alpha > 0 {
                    self.alpha -= 1;
                } else if self.phase == 10 {
                    if self.credits > 0 {
                        self.phase(12);
                    } else {
                        self.closing = true;
                    }
                } else {
                    self.phase(self.phase + 2);
                }
            }
            12 => self.phase(13),
            13 => {
                if self.alpha == 254 {
                    self.phase(14);
                } else {
                    self.alpha = self.alpha.saturating_add(2).min(254);
                }
            }
            14 => {
                if self.alpha > 0 {
                    self.alpha = self.alpha.saturating_sub(2);
                } else {
                    self.credit_index += 1;
                    if self.credit_index < self.credits {
                        self.phase(12);
                    } else {
                        self.phase(2);
                        self.closing = true;
                    }
                }
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_credits_follow_logos_fade_every_row_then_return() {
        let mut clock = Controller::with_credits(32);
        clock.advance_to(1711);
        assert_eq!(clock.card(), Some((Card::Title, 0)));
        assert_eq!(clock.credit_row(), None);
        clock.advance_to(1712);
        assert_eq!(clock.card(), None);
        assert_eq!(clock.credit_row(), None);
        for row in 0..32 {
            let start = 1713 + row * 257;
            for (offset, alpha) in [(0, 0), (1, 2), (127, 254), (128, 254), (255, 0)] {
                clock.advance_to(start + offset);
                assert_eq!(
                    clock.credit_row(),
                    Some((row, alpha)),
                    "row {row}, offset {offset}"
                );
                assert_eq!(clock.height(), 85);
                assert!(!clock.ended());
            }
        }
        clock.advance_to(9936);
        assert_eq!(clock.credit_row(), None);
        clock.advance_to(10021);
        assert_eq!(clock.height(), 0);
        assert!(!clock.ended());
        clock.advance_to(10022);
        assert!(clock.ended());
    }

    #[test]
    fn settings_credits_skip_during_logos_or_staff_retracts_bars_once() {
        for start in [300, 1800, 8000] {
            let mut clock = Controller::with_credits(32);
            clock.advance_to(start);
            for tick in start + 1..=start + 85 {
                clock.request_end();
                clock.advance_to(tick);
                assert!(!clock.ended());
            }
            clock.advance_to(start + 86);
            assert!(clock.ended());
            assert_eq!(clock.card(), None);
            assert_eq!(clock.credit_row(), None);
        }
    }

    #[test]
    fn attract_skip_retracts_the_shared_bars_without_restarting_on_held_input() {
        let mut clock = Controller::default();
        clock.advance_to(300);
        assert_eq!(clock.height(), 85);
        for tick in 301..=385 {
            clock.request_end();
            clock.advance_to(tick);
            assert!(!clock.ended());
        }
        assert_eq!(clock.height(), 0);
        clock.request_end();
        clock.advance_to(386);
        assert!(clock.ended());
        assert_eq!(clock.card(), None);
    }

    #[test]
    fn original_card_edges_and_automatic_finish() {
        let mut clock = Controller::default();
        for (tick, card) in [
            (100, None),
            (101, Some((Card::Publisher, 0))),
            (102, Some((Card::Publisher, 1))),
            (356, Some((Card::Publisher, 255))),
            (382, Some((Card::Publisher, 255))),
            (637, Some((Card::Publisher, 0))),
            (638, Some((Card::Developer, 0))),
            (893, Some((Card::Developer, 255))),
            (1175, Some((Card::Title, 0))),
            (1430, Some((Card::Title, 255))),
            (1712, Some((Card::Title, 0))),
        ] {
            clock.advance_to(tick);
            assert_eq!(clock.card(), card, "tick {tick}");
            assert!(!clock.ended());
        }
        clock.advance_to(1797);
        assert_eq!(clock.height(), 0);
        assert!(!clock.ended());
        clock.advance_to(1798);
        assert!(clock.ended());
        assert_eq!(clock.card(), None);
    }

    #[test]
    fn letterbox_growth_and_retraction_are_camera_ticks_not_render_frames() {
        let mut clock = Controller::default();
        for (tick, height) in [
            (0, 0),
            (1, 1),
            (50, 50),
            (85, 85),
            (1000, 85),
            (1712, 85),
            (1713, 84),
            (1750, 47),
            (1797, 0),
        ] {
            clock.advance_to(tick);
            assert_eq!(clock.height(), height, "tick {tick}");
            clock.advance_to(tick);
            assert_eq!(clock.height(), height);
        }
    }
}
