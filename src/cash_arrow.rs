//! Cash arrow pulse sequence (original 4c6290/4c62d0 start, 4ecc20 advance).
//! The binary's `cash_arrow::View` draws the frame this state selects.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub frame: i32,
    pub active: bool,
    end: i32,
}
impl State {
    ///4c6290/4c62d0 ignore another pulse while the existing sequence is alive.
    pub fn start(&mut self, positive: bool) {
        if !self.active {
            self.frame = if positive { 41 } else { 0 };
            self.end = if positive { 95 } else { 40 };
            self.active = true;
        }
    }
    pub fn advance(&mut self) {
        if self.active {
            let next = self.frame.wrapping_add(1);
            if next >= self.end {
                //4ecc20 restores the prior frame as it retires mode2 and
                //4c6260(2) clears the presentation latch.
                self.active = false;
            } else {
                self.frame = next;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original cash sequence/dispatcher observations"]
    fn matches_native_cash_sequence() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_CASH_ARROW_SEQUENCE")).unwrap();
        assert_eq!(&data[..8], b"MTCASH01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x037f);
        let mut at = 16;
        for _ in 0..word(12) {
            let positive = word(at) == 0x4c6290;
            let retirement = word(at + 4) as usize;
            let frames = word(at + 8) as usize;
            at += 12;
            let mut state = State::default();
            state.start(positive);
            state.start(!positive);
            for tick in 0..frames {
                if tick > 0 {
                    state.advance();
                }
                assert_eq!(state.frame, word(at) as i32, "native frame at{tick}");
                assert_eq!(if state.active { 0 } else { -1 }, word(at + 4) as i32);
                assert_eq!(state.active, word(at + 8) != 0);
                assert_eq!(state.active, tick < retirement);
                at += 12;
            }
        }
        assert_eq!(at, data.len());
    }
}
