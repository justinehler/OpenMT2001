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
        //4ba1d0: only reason0 and its real timer/latch transition, no normalization.
        if session.world.pause.bytes[8] == 0 {
            session.world.pause.pause(0, |change, _| {
                change.apply(&mut session.hooks.timers);
                Ok(())
            })?;
        }
        Ok(Self {
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
