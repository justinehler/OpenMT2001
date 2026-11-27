//! Original simulation-clock transitions, independent of host time and I/O.
pub type Clock = crate::save::SavedClock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Hourly,
    RolloverQuery,
    Rollover,
    Era(u32),
    Autosave,
    Hour19,
    Hour7,
}

/// Runtime controls consumed by0047f660. Script/host callbacks own the actual
/// side effects; the query callback returns the original boolean gate result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controls {
    pub years_per_day: u32,
    /// Original008b0700 bypasses the0048dbf0 query when set.
    pub suppress_rollover_query: bool,
    pub rollover_events_enabled: bool,
    pub autosave_disabled: bool,
}

/// Pacing words reset alongside the world clock (00822d38/00822d4c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResetPacing {
    pub counter: u32,
    pub step: u32,
}

impl Clock {
    /// Clock portion of004b9d40 (004b9e2d onward), including0047f800.
    /// Unrelated world managers and the earlier simulation-enable store remain
    /// caller work. Absolute hour and unmentioned timing words retain their bytes.
    pub fn reset_world_clock(&mut self) -> ResetPacing {
        self.initialize();
        self.timing[3] = self.hour().wrapping_mul(625);
        self.timing[0] = 0;
        self.timing[1] = 0;
        ResetPacing {
            counter: 0,
            step: 25,
        }
    }

    ///0047f3d0 initializes selected words only. Absolute time and other timing
    /// words retain caller state; fresh-process zero storage is a separate input.
    pub fn initialize(&mut self) {
        self.calendar[0] = 9;
        self.calendar[1] = 0;
        self.calendar[2] = 0;
        self.calendar[4] = 0;
        self.timing[2] = 0;
    }

    pub fn hour(&self) -> u32 {
        self.calendar[0]
    }

    ///0047f660. Callbacks execute synchronously at the original boundaries and
    /// can change state before subsequent native-style reads. The year increment
    /// alone is captured before Hourly, as in the original ESI register.
    pub fn advance_hour(
        &mut self,
        controls: &mut Controls,
        mut callback: impl FnMut(Event, &mut Self, &mut Controls) -> bool,
    ) {
        self.try_advance_hour(controls, |event, clock, controls| {
            Ok::<_, std::convert::Infallible>(callback(event, clock, controls))
        })
        .unwrap();
    }

    /// Fallible host/world dispatch stops at the failing boundary, retaining
    /// preceding effects without running later hourly or rollover callbacks.
    pub fn try_advance_hour<E>(
        &mut self,
        controls: &mut Controls,
        mut callback: impl FnMut(Event, &mut Self, &mut Controls) -> Result<bool, E>,
    ) -> Result<(), E> {
        let increment = controls.years_per_day;
        self.calendar[0] = self.calendar[0].wrapping_add(1);
        self.timing[2] = 0;
        callback(Event::Hourly, self, controls)?;
        if self.calendar[0] as i32 >= 24 {
            self.calendar[1] = self.calendar[1].wrapping_add(increment);
            self.calendar[4] = self.calendar[4].wrapping_add(1);
            self.timing[3] = 0;
            self.calendar[0] = 0;
            if !controls.suppress_rollover_query
                && !callback(Event::RolloverQuery, self, controls)?
                && controls.rollover_events_enabled
            {
                callback(Event::Rollover, self, controls)?;
            }
            if self.calendar[1] as i32 >= 10 {
                self.calendar[1] = ((self.calendar[1] as i32) % 10) as u32;
                self.calendar[2] = self.calendar[2].wrapping_add(1);
                if (self.calendar[2] as i32) < 8 {
                    callback(Event::Era(self.calendar[2]), self, controls)?;
                }
            }
            if !controls.autosave_disabled {
                callback(Event::Autosave, self, controls)?;
            }
        }
        if self.hour() == 19 {
            callback(Event::Hour19, self, controls)?;
        }
        if self.hour() == 7 {
            callback(Event::Hour7, self, controls)?;
        }
        self.calendar[3] = self.calendar[0].wrapping_add(
            self.calendar[1]
                .wrapping_add(self.calendar[2].wrapping_mul(10))
                .wrapping_mul(24),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_hourly_dispatch_does_not_roll_over() {
        let mut clock = Clock {
            calendar: [23, 9, 2, 719, 6],
            timing: [10, 10, 624, 14999, 0, 1],
        };
        let mut controls = Controls {
            years_per_day: 1,
            suppress_rollover_query: false,
            rollover_events_enabled: true,
            autosave_disabled: false,
        };
        let mut calls = Vec::new();
        let result = clock.try_advance_hour(&mut controls, |event, _, _| {
            calls.push(event);
            Err::<bool, _>("hourly failed")
        });
        assert_eq!(result, Err("hourly failed"));
        assert_eq!(calls, [Event::Hourly]);
        assert_eq!(clock.calendar, [24, 9, 2, 719, 6]);
        assert_eq!(clock.timing[2], 0);
        assert_eq!(clock.timing[3], 14999);
    }

    #[test]
    #[ignore = "requires original simulation-clock observations"]
    fn matches_native_clock() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_CLOCK")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let read_clock = |at| Clock {
            calendar: std::array::from_fn(|i| word(at + i * 4)),
            timing: std::array::from_fn(|i| word(at + 20 + i * 4)),
        };
        assert_eq!(&bytes[..8], b"MTCLOCK2");
        assert_eq!((word(8), word(12)), (44, 2));
        let mut at = 16;
        for case in 0..word(8) {
            let mut clock = read_clock(at);
            at += 44;
            let mut controls = Controls {
                years_per_day: word(at),
                suppress_rollover_query: word(at + 4) != 0,
                rollover_events_enabled: word(at + 8) != 0,
                autosave_disabled: word(at + 12) != 0,
            };
            let query_result = word(at + 16) != 0;
            let mode = word(at + 20);
            at += 24;
            let mut trace = Vec::new();
            clock.advance_hour(&mut controls, |event, clock, controls| {
                let (id, argument) = match event {
                    Event::Hourly => (1, 0),
                    Event::RolloverQuery => (2, 0),
                    Event::Rollover => (3, 0),
                    Event::Era(value) => (4, value),
                    Event::Autosave => (5, 0),
                    Event::Hour19 => (6, 0),
                    Event::Hour7 => (7, 0),
                };
                trace.push((id, argument, *clock));
                match (mode, event) {
                    (1, Event::Hourly) => clock.calendar[0] = 23,
                    (2, Event::Hourly) => controls.years_per_day = 99,
                    (3, Event::Hour19) => clock.calendar[0] = 7,
                    (4, Event::RolloverQuery) => clock.calendar[1] = 12,
                    (5, Event::Autosave) => clock.calendar[0] = 19,
                    _ => {}
                }
                event == Event::RolloverQuery && query_result
            });
            assert_eq!(clock, read_clock(at), "case {case} clock");
            at += 44;
            assert_eq!(
                controls.years_per_day,
                word(at),
                "case {case} captured increment"
            );
            let count = word(at + 4) as usize;
            at += 8;
            assert_eq!(trace.len(), count, "case {case} callback count");
            for actual in trace {
                assert_eq!(
                    actual,
                    (word(at), word(at + 4), read_clock(at + 8)),
                    "case {case} callback snapshot"
                );
                at += 52;
            }
        }
        for case in 0..word(12) {
            let mut clock = read_clock(at);
            at += 44;
            clock.initialize();
            assert_eq!(clock, read_clock(at), "initializer {case}");
            at += 44;
        }
        for case in 0..2 {
            let mut clock = read_clock(at);
            at += 44;
            let pacing = clock.reset_world_clock();
            assert_eq!(clock, read_clock(at), "world reset {case}");
            at += 44;
            assert_eq!(
                pacing,
                ResetPacing {
                    counter: word(at),
                    step: word(at + 4)
                }
            );
            at += 8;
        }
        assert_eq!(at, bytes.len());
    }
}
