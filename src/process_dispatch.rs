//! Move-only process controllers and dispatch. World callback context never travels.
//! The game binary moves them between the menu and each city's Session.
use crate::{pause, scenario_objective, timers};
use std::time::Instant;

pub struct DispatchCustody {
    pub objective: scenario_objective::State,
    pub pause: pause::State,
    pub tasks: Vec<timers::Task>,
    pub frequency: timers::CalendarFrequency,
    pub frame: timers::Frame,
    pub epoch: Instant,
}
pub struct Seed {
    pub objective: scenario_objective::State,
    pub pause: pause::State,
    pub tasks: Vec<timers::Task>,
    pub frequency: timers::CalendarFrequency,
}
pub struct Timing {
    pub frame: timers::Frame,
    pub epoch: Instant,
}
impl Timing {
    /// Host milliseconds on the process clock every city frame samples.
    pub fn now(&self) -> u32 {
        self.epoch.elapsed().as_millis() as u32
    }

    /// One city frame: milliseconds since the previous sample (original4eb8b0).
    pub fn elapsed_now(&mut self) -> i32 {
        let now = self.now();
        self.frame.elapsed(Some(now))
    }

    /// Discard the interval since the previous sample (original4ebaa0).
    pub fn synchronize_now(&mut self) {
        let now = self.now();
        self.frame.synchronize(now);
    }

    /// Begin a city's frame loop, whether it was started fresh or loaded.
    /// The original ends every city start with4ebaa0 (call at4ba1b4), and its
    /// main loop samples this clock every frame (4b9c74), menus included, so no
    /// menu or loading time ever reaches a city. This process clock is not
    /// sampled between cities, so the first frame would otherwise replay the
    /// whole frontend interval as logic ticks. Registrations, accumulators and
    /// pending adjustments are retained.
    pub fn start_city(&mut self) {
        self.synchronize_now();
    }
}
impl DispatchCustody {
    pub fn startup(pause: pause::State) -> Result<Self, String> {
        // Explicit modern prior padding policy, not recovered native heap data.
        let mut value = Self {
            objective: scenario_objective::State::default(),
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
    pub fn prepare_start(self, fresh: bool) -> Result<(Seed, Timing), String> {
        if fresh && (self.pause.bytes[0] as i8) < 0 {
            return Err("Unsupported native nonterminating fresh start: negative reason0".into());
        }
        Ok((
            Seed {
                objective: self.objective,
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
    /// Take back the process controllers a departing city held, then retire
    /// that city's owners (see `retire_session`).
    pub fn from_parts(
        objective: scenario_objective::State,
        pause: pause::State,
        tasks: Vec<timers::Task>,
        frequency: timers::CalendarFrequency,
        timing: Timing,
    ) -> Result<Self, String> {
        let mut value = Self {
            objective,
            pause,
            tasks,
            frequency,
            frame: timing.frame,
            epoch: timing.epoch,
        };
        value.retire_session()?;
        Ok(value)
    }

    pub fn retire_session(&mut self) -> Result<(), String> {
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
    pub fn world_reset_accumulators(&mut self) {
        for task in &mut self.tasks {
            task[12..16].copy_from_slice(&0f32.to_le_bytes());
        }
    }

    pub fn fresh_reset(&mut self) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const LOGIC_TICK: u32 = 0x481680;

    fn logic_ticks(tasks: &mut Vec<timers::Task>, elapsed: i32) -> u32 {
        let mut ticks = 0;
        timers::dispatch(tasks, elapsed, |_, callback, _| {
            if callback == LOGIC_TICK {
                ticks += 1;
            }
            Ok(())
        })
        .unwrap();
        ticks
    }

    /// A process custody whose clock was last sampled `menu_ms` ago, as after
    /// sitting in the frontend (or a previous city's teardown) before Ok.
    fn custody_after_menus(menu_ms: u32, previous_city: bool) -> DispatchCustody {
        let mut custody = DispatchCustody::startup(pause::State::default()).unwrap();
        if previous_city {
            custody.retire_session().unwrap();
        }
        let now = custody.epoch.elapsed().as_millis() as u32;
        custody.frame.synchronize(now.wrapping_sub(menu_ms));
        custody
    }

    #[test]
    fn city_start_after_long_menu_delay_advances_only_from_start() {
        for fresh in [false, true] {
            for previous_city in [false, true] {
                let (mut seed, mut timing) = custody_after_menus(180_000, previous_city)
                    .prepare_start(fresh)
                    .unwrap();
                if fresh {
                    seed.fresh_reset().unwrap();
                } else {
                    // Saved adoption: shared accumulator reset, menu hold released.
                    seed.world_reset_accumulators();
                    seed.pause
                        .unpause(0, |change, _| {
                            change.apply(&mut seed.tasks);
                            Ok(())
                        })
                        .unwrap();
                }

                // Without the rebase the first frame replays the menu interval.
                let mut stale = Timing {
                    frame: timing.frame.clone(),
                    epoch: timing.epoch,
                };
                let mut stale_tasks = seed.tasks.clone();
                let stale_elapsed = stale.elapsed_now();
                assert!(stale_elapsed >= 180_000);
                assert!(logic_ticks(&mut stale_tasks, stale_elapsed) > 4_000);

                // Loaded at saved tick 84487; the first frames run at 25 Hz from
                // the moment the city loop starts.
                let saved_tick = 84_487;
                let mut tick = saved_tick;
                timing.start_city();
                let started = std::time::Instant::now();
                for frame in 0..5 {
                    let elapsed = timing.elapsed_now();
                    let ticks = logic_ticks(&mut seed.tasks, elapsed);
                    let allowed = started.elapsed().as_millis() as u32 / 40 + 1;
                    tick += ticks;
                    assert!(
                        tick - saved_tick <= allowed,
                        "fresh={fresh} previous_city={previous_city}: frame {frame} \
                         advanced {saved_tick} -> {tick}, at most {allowed} allowed"
                    );
                    std::thread::sleep(Duration::from_millis(16));
                }
            }
        }
    }
}
