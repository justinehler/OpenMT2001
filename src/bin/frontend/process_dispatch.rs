//! Move-only process dispatch image. Per-World callback context never travels.
use openmt2001::{pause, timers};
use std::time::Instant;

pub(crate) struct DispatchCustody {
    pub pause: pause::State,
    pub tasks: Vec<timers::Task>,
    pub frequency: timers::CalendarFrequency,
    pub frame: timers::Frame,
    pub epoch: Instant,
}
pub(crate) struct Seed {
    pub pause: pause::State,
    pub tasks: Vec<timers::Task>,
    pub frequency: timers::CalendarFrequency,
}
pub(crate) struct Timing {
    pub frame: timers::Frame,
    pub epoch: Instant,
}
impl DispatchCustody {
    pub(crate) fn startup(pause: pause::State) -> Result<Self, String> {
        // Explicit modern prior padding policy, not recovered native heap data.
        let mut value = Self {
            pause,
            tasks: timers::startup_tasks([0; 20]),
            frequency: timers::CalendarFrequency {
                frequency: 0.,
                calendar_timer: None,
                logic_timer: None,
            },
            frame: timers::Frame::default(),
            epoch: Instant::now(),
        };
        value.frame.initialize(0); // once-only4eb660
        value.pause.pause(0, |change, _| {
            change.apply(&mut value.tasks);
            Ok(())
        })?;
        Ok(value)
    }
    pub(crate) fn prepare_start(self, fresh: bool) -> Result<(Seed, Timing), String> {
        if fresh && (self.pause.bytes[0] as i8) < 0 {
            return Err("Unsupported native nonterminating fresh start: negative reason0".into());
        }
        Ok((
            Seed {
                pause: self.pause,
                tasks: self.tasks,
                frequency: self.frequency,
            },
            Timing {
                frame: self.frame,
                epoch: self.epoch,
            },
        ))
    }
    pub(crate) fn from_session(
        session: &mut crate::fresh_city::Session<'_>,
        timing: Timing,
    ) -> Result<Self, String> {
        let mut value = Self {
            pause: std::mem::take(&mut session.world.pause),
            tasks: std::mem::take(&mut session.hooks.timers),
            frequency: std::mem::replace(
                &mut session.hooks.frequency,
                timers::CalendarFrequency {
                    frequency: 0.,
                    calendar_timer: None,
                    logic_timer: None,
                },
            ),
            frame: timing.frame,
            epoch: timing.epoch,
        };
        value.retire_session()?;
        Ok(value)
    }

    fn retire_session(&mut self) -> Result<(), String> {
        // The worker's Options, player and script owners die with its World.
        // Returning their counts would orphan them in the next city. Native
        // UI teardown releases its owners separately (e.g. Options49e719),
        // while4ba1d0 keeps group2 suspended for the menu. Our joined-worker
        // boundary replaces all departing ordinary owners with ONE menu hold.
        // Focus, padding, timer records and process timing retain their owners.
        self.pause.bytes[..4].fill(0);
        self.pause.pause(0, |change, _| {
            change.apply(&mut self.tasks);
            Ok(())
        })
    }
}
impl Seed {
    ///4b9d40 ->4eb6f0 is shared by fresh and native saved load.
    pub(crate) fn world_reset_accumulators(&mut self) {
        for task in &mut self.tasks {
            task[12..16].copy_from_slice(&0f32.to_le_bytes());
        }
    }

    pub(crate) fn fresh_reset(&mut self) -> Result<(), String> {
        if (self.pause.bytes[0] as i8) < 0 {
            return Err("Unsupported negative reason0 before World reset".into());
        }
        self.world_reset_accumulators();
        //4b9fa0 drains only positive ordinary reason0.
        while self.pause.bytes[0] as i8 > 0 {
            self.pause.unpause(0, |change, _| {
                change.apply(&mut self.tasks);
                Ok(())
            })?;
        }
        Ok(())
    }
}

///4bcd6d..4bcd7b: native load completes with ONE unpause0, not fresh's
///positive-count drain. The later modern record12 transaction supersedes this
///numeric image literally, so this MUST run before restore_v2_extension.
pub(crate) fn native_load_completed(
    session: &mut crate::fresh_city::Session<'_>,
) -> Result<(), String> {
    session.world.pause.unpause(0, |change, _| {
        change.apply(&mut session.hooks.timers);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_switch_retires_options_player_and_script_owners() {
        for fresh in [false, true] {
            for focus in [0, 1] {
                let mut custody = DispatchCustody::startup(pause::State::default()).unwrap();
                custody
                    .pause
                    .unpause(0, |change, _| {
                        change.apply(&mut custody.tasks);
                        Ok(())
                    })
                    .unwrap();
                custody.pause.bytes[4..8].copy_from_slice(&[focus, 0xa5, 0x5a, 0xee]);
                let mut host = crate::live_host::Host::new([0.; 3]);
                host.set_running(true);
                host.toggle_player_pause(&mut custody.pause, &mut custody.tasks, |_, _| Ok(()))
                    .unwrap();
                for reason in 0..3 {
                    custody
                        .pause
                        .pause(reason, |change, _| {
                            change.apply(&mut custody.tasks);
                            Ok(())
                        })
                        .unwrap();
                }
                host.options(&mut custody.pause, &mut custody.tasks, true)
                    .unwrap();
                assert_eq!(&custody.pause.bytes[..4], &[2, 1, 1, 1]);
                let held_tasks = custody.tasks.clone();
                custody.retire_session().unwrap();
                assert_eq!(
                    custody.pause.bytes,
                    [1, 0, 0, 0, focus, 0xa5, 0x5a, 0xee, 1]
                );
                assert_eq!(
                    custody.tasks, held_tasks,
                    "retirement must not resume menu logic"
                );

                let (mut seed, _) = custody.prepare_start(fresh).unwrap();
                if fresh {
                    seed.fresh_reset().unwrap();
                } else {
                    seed.world_reset_accumulators();
                    seed.pause
                        .unpause(0, |change, _| {
                            change.apply(&mut seed.tasks);
                            Ok(())
                        })
                        .unwrap();
                }
                assert!(!seed.pause.has_reason());
                assert_eq!(seed.pause.bytes[8], 0);
                for task in seed
                    .tasks
                    .iter()
                    .filter(|t| u32::from_le_bytes(t[4..8].try_into().unwrap()) & 2 != 0)
                {
                    assert_eq!(task[0], 1, "the next city's logic timers must run");
                }
            }
        }
    }

    #[test]
    fn retiring_running_city_suspends_group_two_without_resetting_process_timers() {
        let mut custody = DispatchCustody::startup(pause::State::default()).unwrap();
        custody
            .pause
            .unpause(0, |change, _| {
                change.apply(&mut custody.tasks);
                Ok(())
            })
            .unwrap();
        let mut expected = custody.tasks.clone();
        pause::Timers::Suspend.apply(&mut expected);
        custody.retire_session().unwrap();
        assert_eq!(custody.pause.bytes, [1, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(custody.tasks, expected);
    }
}
