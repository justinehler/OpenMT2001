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
