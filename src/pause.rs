//! Original4bac90/4bacc0 pause reasons and timer group transition latch.
//! Four reason bytes are tested together by the original unpause routine.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub bytes: [u8; 9],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timers {
    Suspend,
    Resume,
}

impl Timers {
    /// Original4eb720/4eb6c0 with group mask2. Each20-byte timer record
    /// has its enabled byte at0 and group mask at4; other fields stay intact.
    pub fn apply(self, tasks: &mut [[u8; 20]]) {
        for task in tasks {
            if u32::from_le_bytes(task[4..8].try_into().unwrap()) & 2 != 0 {
                task[0] = u8::from(self == Self::Resume);
            }
        }
    }
}

impl State {
    pub fn has_reason(&self) -> bool {
        self.bytes[..4] != [0; 4]
    }

    /// Pause slice of focus loss004bab90, AFTER device unacquire, input-disable
    /// and frontend suspend. Byte4 is a separate signed counter, not reason4.
    /// The network gate skips only this slice, not those preceding effects.
    pub fn focus_lost(
        &mut self,
        network_blocked: bool,
        mut timers: impl FnMut(Timers, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        if network_blocked || self.bytes[4] as i8 > 0 {
            return Ok(());
        }
        self.bytes[4] = self.bytes[4].wrapping_add(1);
        if self.bytes[4] == 1 && self.bytes[8] == 0 {
            timers(Timers::Suspend, self)?;
            self.bytes[8] = 1;
        }
        Ok(())
    }

    /// Pause slice of focus gain004bac00, AFTER device acquire, input-enable
    /// and frontend resume. Native tests only the ordinary word after decrement;
    /// even a remaining positive focus count does not prevent timer resumption.
    pub fn focus_gained(
        &mut self,
        network_blocked: bool,
        mut timers: impl FnMut(Timers, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        if network_blocked || self.bytes[4] as i8 <= 0 {
            return Ok(());
        }
        self.bytes[4] = self.bytes[4].wrapping_sub(1);
        if !self.has_reason() && self.bytes[8] != 0 {
            timers(Timers::Resume, self)?;
            self.bytes[8] = 0;
        }
        Ok(())
    }

    pub fn pause(
        &mut self,
        reason: usize,
        mut timers: impl FnMut(Timers, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        if reason >= 4 {
            return Err("unrecovered pause reason".into());
        }
        self.bytes[reason] = self.bytes[reason].wrapping_add(1);
        if self.bytes[reason] == 1 && self.bytes[8] == 0 {
            timers(Timers::Suspend, self)?;
            self.bytes[8] = 1;
        }
        Ok(())
    }

    pub fn unpause(
        &mut self,
        reason: usize,
        mut timers: impl FnMut(Timers, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        if reason >= 4 {
            return Err("unrecovered pause reason".into());
        }
        // Native JLE tests AL as signed; do not "repair" wrapped counters.
        if self.bytes[reason] as i8 > 0 {
            self.bytes[reason] -= 1;
            if !self.has_reason() && self.bytes[8] != 0 {
                timers(Timers::Resume, self)?;
                self.bytes[8] = 0;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "pause/focus_tests.rs"]
mod focus_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original pause counter fixture"]
    fn matches_original_pause_counters() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PAUSE")).unwrap();
        assert_eq!(&data[..8], b"MTPAUSE2");
        let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        assert_eq!(data.len(), 12 + count * 262);
        for (case, row) in data[12..].chunks_exact(262).enumerate() {
            let mut tasks: [[u8; 20]; 6] =
                std::array::from_fn(|i| row[22 + i * 20..42 + i * 20].try_into().unwrap());
            let mut state = State {
                bytes: row[2..11].try_into().unwrap(),
            };
            let mut trace = 0;
            let mut callback = |event, live: &mut State| {
                assert_eq!(live.bytes[8], row[10]);
                trace = match event {
                    Timers::Suspend => 1,
                    Timers::Resume => 2,
                };
                event.apply(&mut tasks);
                Ok(())
            };
            if row[0] == 0 {
                state.pause(row[1] as usize, &mut callback).unwrap();
            } else {
                state.unpause(row[1] as usize, &mut callback).unwrap();
            }
            assert_eq!(state.bytes, &row[11..20], "case {case}");
            assert_eq!(trace, row[20], "case {case} timers");
            assert_eq!(u8::from(state.has_reason()), row[21], "case {case} reasons");
            assert_eq!(
                tasks.as_flattened(),
                &row[142..262],
                "case {case} timer groups"
            );
        }
    }
}
