//! Session side of the process dispatch controllers; the controllers themselves
//! live in the library (`openmt2001::process_dispatch`, #99).
pub(crate) use openmt2001::process_dispatch::{DispatchCustody, Seed, Timing};

/// Take the controllers back from a departing city (`DispatchCustody::from_parts`).
pub(crate) fn custody_from_session(
    session: &mut crate::fresh_city::Session<'_>,
    timing: Timing,
) -> Result<DispatchCustody, String> {
    DispatchCustody::from_parts(
        std::mem::take(&mut session.world.objective),
        std::mem::take(&mut session.world.pause),
        std::mem::take(&mut session.hooks.timers),
        std::mem::replace(
            &mut session.hooks.frequency,
            openmt2001::timers::CalendarFrequency {
                frequency: 0.,
                calendar_timer: None,
                logic_timer: None,
            },
        ),
        timing,
    )
}

///4bcd6d..4bcd7b: native load completes with ONE unpause0, not fresh's
///positive-count drain. Modern record12 leaves these process-owned pause and
///timer fields intact, including any remaining load-time script owners.
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
    use openmt2001::pause;

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
