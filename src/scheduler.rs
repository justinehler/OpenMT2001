//! Original00481680 logic-loop boundaries, independent of host frame timing.
//! Callbacks run synchronously, so later gates see their changes as native code does.

pub struct Controls {
    /// Captured once at entry from00541918; one host frame need not be one step.
    pub iterations: u32,
    pub abort: bool,
    pub alternate_script_active: bool,
    pub network_enabled: bool,
    pub network_override: bool,
    pub deadline: u32,
    pub gate_32: bool,
    pub gate_32_offset: bool,
    pub before_six_sound: u32,
}

#[derive(Clone, Copy)]
pub enum ClockDomain {
    Total,
    Normal,
}

impl ClockDomain {
    fn index(self) -> usize {
        match self {
            Self::Total => 0,
            Self::Normal => 1,
        }
    }
}

struct Scheduled<T> {
    due: u32,
    domain: usize,
    previous: Option<usize>,
    next: Option<usize>,
    payload: Option<T>,
}

/// Original480a80/480ec0 insertion and draining, with typed payloads in place
/// of native packet pointers. Two stable linked lists use signed tick ordering.
/// Callbacks may insert events and change clocks; captured next links ensure
/// a newly inserted earlier event is not accidentally visited in this pass.
pub struct Queue<T> {
    nodes: Vec<Option<Scheduled<T>>>,
    free: Vec<usize>,
    heads: [Option<usize>; 2],
    tails: [Option<usize>; 2],
    draining: bool,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            nodes: vec![],
            free: vec![],
            heads: [None; 2],
            tails: [None; 2],
            draining: false,
        }
    }
}

impl<T> Queue<T> {
    /// Visit retained nodes in native stream order (total list, normal list).
    /// Snapshotting during a callback would omit its temporarily owned payload.
    pub fn snapshot(&self) -> Result<Vec<(ClockDomain, u32, &T)>, String> {
        if self.draining {
            return Err("cannot save a queue during dispatch".into());
        }
        let mut rows = Vec::new();
        for domain in [ClockDomain::Total, ClockDomain::Normal] {
            let mut next = self.heads[domain.index()];
            while let Some(index) = next {
                let node = self.nodes[index]
                    .as_ref()
                    .ok_or("queue snapshot link missing")?;
                rows.push((
                    domain,
                    node.due,
                    node.payload
                        .as_ref()
                        .ok_or("queue snapshot payload missing")?,
                ));
                next = node.next;
            }
        }
        Ok(rows)
    }
    /// Insert an already admitted event. Original480900's immediate-dispatch
    /// admission paths are outside this primitive; all due bit patterns are
    /// ordered as signed integers, including -1.
    pub fn insert(&mut self, domain: ClockDomain, due: u32, payload: T) {
        let domain = domain.index();
        let mut next = self.heads[domain];
        while let Some(index) = next {
            let node = self.nodes[index].as_ref().unwrap();
            if node.due as i32 > due as i32 {
                break;
            }
            next = node.next;
        }
        let previous = next.map_or(self.tails[domain], |index| {
            self.nodes[index].as_ref().unwrap().previous
        });
        let index = self.free.pop().unwrap_or_else(|| {
            self.nodes.push(None);
            self.nodes.len() - 1
        });
        self.nodes[index] = Some(Scheduled {
            due,
            domain,
            previous,
            next,
            payload: Some(payload),
        });
        if let Some(previous) = previous {
            self.nodes[previous].as_mut().unwrap().next = Some(index);
        } else {
            self.heads[domain] = Some(index);
        }
        if let Some(next) = next {
            self.nodes[next].as_mut().unwrap().previous = Some(index);
        } else {
            self.tails[domain] = Some(index);
        }
    }

    /// Drain Total then Normal using the live clock before each comparison.
    /// A dispatch error retains the current node after preceding side effects.
    pub fn drain(
        &mut self,
        clocks: &mut [u32; 2],
        mut dispatch: impl FnMut(&mut T, &mut Self, &mut [u32; 2]) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.draining {
            return Err("reentrant scheduled queue drain is unsupported".into());
        }
        self.draining = true;
        let result = self.drain_inner(clocks, &mut dispatch);
        self.draining = false;
        result
    }

    fn drain_inner(
        &mut self,
        clocks: &mut [u32; 2],
        dispatch: &mut impl FnMut(&mut T, &mut Self, &mut [u32; 2]) -> Result<(), String>,
    ) -> Result<(), String> {
        for domain in 0..2 {
            let mut current = self.heads[domain];
            while let Some(index) = current {
                let node = self.nodes[index].as_mut().unwrap();
                if node.due as i32 > clocks[domain] as i32 {
                    break;
                }
                let next = node.next;
                let mut payload = node.payload.take().unwrap();
                if let Err(error) = dispatch(&mut payload, self, clocks) {
                    self.nodes[index].as_mut().unwrap().payload = Some(payload);
                    return Err(error);
                }
                drop(payload);
                let node = self.nodes[index].take().unwrap();
                if let Some(previous) = node.previous {
                    self.nodes[previous].as_mut().unwrap().next = node.next;
                } else {
                    self.heads[node.domain] = node.next;
                }
                if let Some(next) = node.next {
                    self.nodes[next].as_mut().unwrap().previous = node.previous;
                } else {
                    self.tails[node.domain] = node.previous;
                }
                self.free.push(index);
                current = next;
            }
        }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), String> {
        if self.draining {
            return Err("reset during scheduled dispatch is unsupported".into());
        }
        self.nodes.clear();
        self.free.clear();
        self.heads = [None; 2];
        self.tails = [None; 2];
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Residents,
    Animations,
    AdvanceHour,
    BeforeSixSound,
    /// Original deadline helper receives the captured next tick in its event.
    Deadline {
        next_tick: u32,
    },
    /// World/host boundaries whose internal behavior remains outside this loop.
    Helper(u32),
}

/// Saved timing words retain their original mapping: total logic ticks, normal
/// ticks, within-hour ticks, within-day ticks, speed setting, simulation mode.
/// AdvanceHour must run the clock/hourly behavior synchronously; Animations must
/// visit the pool once after the entire resident pass, not after each resident.
pub fn advance(
    clock: &mut crate::clock::Clock,
    controls: &mut Controls,
    mut callback: impl FnMut(Event, &mut crate::clock::Clock, &mut Controls) -> Result<u32, String>,
) -> Result<(), String> {
    let iterations = controls.iterations;
    for _ in 0..iterations {
        callback(Event::Helper(0x4ed6f0), clock, controls)?;
        clock.timing[0] = clock.timing[0].wrapping_add(1);
        if clock.timing[0].is_multiple_of(200) {
            callback(Event::Helper(0x481340), clock, controls)?;
        }
        let tick = clock.timing[0];
        controls.gate_32 = tick & 31 == 0;
        controls.gate_32_offset = tick.wrapping_sub(15) & 31 == 0;
        callback(Event::Helper(0x4ed6e0), clock, controls)?;
        callback(Event::Helper(0x480ec0), clock, controls)?;
        if controls.abort {
            break;
        }
        match clock.timing[5] {
            1 => {
                for event in [
                    Event::Helper(0x44ec50),
                    Event::Helper(0x4805b0),
                    Event::Residents,
                    Event::Helper(0x4eb5b0),
                    Event::Helper(0x44c210),
                    Event::Helper(0x450080),
                    Event::Helper(0x4b5db0),
                    Event::Helper(0x4b5d50),
                    Event::Helper(0x4b0810),
                ] {
                    callback(event, clock, controls)?;
                }
                if !controls.network_enabled || controls.network_override {
                    callback(Event::Helper(0x477f20), clock, controls)?;
                }
                for index in [2, 1, 3] {
                    clock.timing[index] = clock.timing[index].wrapping_add(1);
                }
                if clock.hour() == 5 && clock.timing[2] == 540 {
                    controls.before_six_sound = callback(Event::BeforeSixSound, clock, controls)?;
                }
                if clock.timing[1].is_multiple_of(625) {
                    callback(Event::AdvanceHour, clock, controls)?;
                }
                callback(Event::Helper(0x4d5b40), clock, controls)?;
                if clock.timing[0].is_multiple_of(50) {
                    callback(Event::Helper(0x4896e0), clock, controls)?;
                }
            }
            2 => {
                callback(Event::Helper(0x465230), clock, controls)?;
                if controls.alternate_script_active {
                    callback(Event::Helper(0x4d5b40), clock, controls)?;
                }
            }
            _ => {}
        }
        callback(Event::Animations, clock, controls)?;
        if clock.timing[0].is_multiple_of(5) {
            callback(Event::Helper(0x48cea0), clock, controls)?;
        }
        let tick = clock.timing[0];
        if tick >= controls.deadline {
            if controls.network_enabled && !controls.network_override {
                callback(Event::Helper(0x4bac90), clock, controls)?;
            } else {
                callback(
                    Event::Deadline {
                        next_tick: tick.wrapping_add(25),
                    },
                    clock,
                    controls,
                )?;
            }
        }
        callback(Event::Helper(0x4ed6f0), clock, controls)?;
    }
    Ok(())
}

/// Host-visible boundaries during a timed frame. Auxiliary timer bodies remain
/// explicit host work; only the recovered481680 logic callback is routed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimedEvent {
    Logic(Event),
    AuxiliaryTimer(u32),
}

/// Compose original4eb8b0 elapsed dispatch with its registered481680 callback.
/// A script/host callback may change the same live timer records, so group2
/// pause takes effect at the next native timer gate, without aborting a logic
/// iteration or already-entered timer catch-up. Host time acquisition and world
/// side effects stay with the caller; no unimplemented helper is silently run.
pub fn advance_timed(
    milliseconds: Option<u32>,
    frame: &mut crate::timers::Frame,
    tasks: &mut Vec<crate::timers::Task>,
    clock: &mut crate::clock::Clock,
    controls: &mut Controls,
    mut callback: impl FnMut(
        TimedEvent,
        &mut crate::clock::Clock,
        &mut Controls,
        &mut Vec<crate::timers::Task>,
    ) -> Result<u32, String>,
) -> Result<Vec<f32>, String> {
    let elapsed = frame.elapsed(milliseconds);
    crate::timers::dispatch(tasks, elapsed, |_, address, tasks| {
        if address == 0x481680 {
            advance(clock, controls, |event, clock, controls| {
                callback(TimedEvent::Logic(event), clock, controls, tasks)
            })
        } else {
            callback(TimedEvent::AuxiliaryTimer(address), clock, controls, tasks).map(|_| ())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original sorted queue insertion/drain observations"]
    fn matches_native_scheduled_core() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_SCHEDULED_CORE")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTCORE01");
        let mut at = 12;
        for _ in 0..word(8) {
            let config: [u32; 8] = std::array::from_fn(|i| word(at + 4 * i));
            at += 32;
            let expected_clocks = [word(at), word(at + 4)];
            at += 8;
            let mut queue = Queue::default();
            for _ in 0..config[3] {
                let action: [u32; 5] = std::array::from_fn(|i| word(at + 4 * i));
                at += 20;
                let domain = if action[0] == 0 {
                    ClockDomain::Total
                } else {
                    ClockDomain::Normal
                };
                queue.insert(domain, action[1], action);
            }
            // Allocator/free observations are retained in the oracle but are
            // outside this typed queue's contract. Compare actual callbacks,
            // category effects, remaining links and live clocks instead.
            let mut expected_trace = Vec::new();
            for _ in 0..config[4] {
                let trace: [u32; 6] = std::array::from_fn(|i| word(at + 4 * i));
                at += 24;
                if trace[0] == 0 {
                    expected_trace.push(trace);
                }
            }
            let mut expected_remaining = Vec::new();
            for _ in 0..config[5] {
                expected_remaining.push(std::array::from_fn::<_, 5, _>(|i| word(at + 4 * i)));
                at += 20;
            }
            let mut expected_categories = [0; 8];
            for _ in 0..config[6] {
                expected_categories[word(at) as usize] = word(at + 4);
                at += 8;
            }
            let mut clocks = [config[1], config[2]];
            let mut trace = Vec::new();
            let mut categories = [0; 8];
            queue
                .drain(&mut clocks, |action, _, clocks| {
                    match action[2] {
                        0 => {
                            trace.push([0, action[0], 0, action[3], action[1], clocks[0]]);
                            if action[4] != 0 {
                                clocks[0] = action[4];
                            }
                        }
                        1 => categories[action[3] as usize] = 2,
                        _ => {}
                    }
                    Ok(())
                })
                .unwrap();
            let mut remaining = Vec::new();
            for domain in 0..2 {
                let mut current = queue.heads[domain];
                let mut previous = None;
                while let Some(index) = current {
                    let node = queue.nodes[index].as_ref().unwrap();
                    assert_eq!(node.previous, previous);
                    let action = node.payload.as_ref().unwrap();
                    remaining.push([
                        domain as u32,
                        action[2],
                        action[3],
                        node.due,
                        remaining.len() as u32,
                    ]);
                    previous = current;
                    current = node.next;
                }
                assert_eq!(queue.tails[domain], previous);
            }
            assert_eq!(trace, expected_trace, "callbacks case{}", config[0]);
            assert_eq!(remaining, expected_remaining, "remaining case{}", config[0]);
            assert_eq!(
                categories, expected_categories,
                "categories case{}",
                config[0]
            );
            assert_eq!(clocks, expected_clocks, "clocks case{}", config[0]);
        }
        assert_eq!(at, bytes.len());
    }

    fn snapshot(clock: &crate::clock::Clock, controls: &Controls) -> [u32; 12] {
        [
            clock.timing[0],
            clock.timing[1],
            clock.timing[2],
            clock.timing[3],
            clock.calendar[0],
            clock.calendar[1],
            clock.calendar[2],
            clock.calendar[3],
            clock.calendar[4],
            controls.before_six_sound,
            u32::from(controls.gate_32),
            u32::from(controls.gate_32_offset),
        ]
    }

    #[test]
    #[ignore = "requires original logic-loop boundary observations"]
    fn matches_native_logic_loop() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_LOGIC_LOOP")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTLOOP01");
        let mut at = 12;
        for case in 0..word(8) {
            let config: [u32; 16] = std::array::from_fn(|i| word(at + i * 4));
            at += 64;
            let initial: [u32; 12] = std::array::from_fn(|i| word(at + i * 4));
            at += 48;
            let expected: [u32; 12] = std::array::from_fn(|i| word(at + i * 4));
            at += 48;
            let mut clock = crate::clock::Clock {
                calendar: initial[4..9].try_into().unwrap(),
                timing: [initial[0], initial[1], initial[2], initial[3], 0, config[1]],
            };
            let mut controls = Controls {
                iterations: config[0],
                abort: config[2] != 0,
                alternate_script_active: config[3] != 0,
                network_enabled: config[4] != 0,
                network_override: config[5] != 0,
                deadline: config[6],
                before_six_sound: initial[9],
                gate_32: initial[10] != 0,
                gate_32_offset: initial[11] != 0,
            };
            let mut trace = Vec::new();
            advance(&mut clock, &mut controls, |event, clock, controls| {
                let target = match event {
                    Event::Residents => 0x45bd40,
                    Event::Animations => 0x4ece30,
                    Event::AdvanceHour => 0x47f660,
                    Event::BeforeSixSound => 0x4ad760,
                    Event::Deadline { .. } => 0x450490,
                    Event::Helper(address) => address,
                };
                trace.push((target, snapshot(clock, controls)));
                if target == 0x45bd40 && config[8] != u32::MAX {
                    clock.timing[1] = config[8];
                }
                if target == 0x4ed6f0 && trace.len() == 1 && config[10] != u32::MAX {
                    controls.iterations = config[10];
                }
                if event == Event::AdvanceHour {
                    let mut calendar_controls = crate::clock::Controls {
                        years_per_day: config[7],
                        suppress_rollover_query: false,
                        rollover_events_enabled: false,
                        autosave_disabled: false,
                    };
                    clock.advance_hour(&mut calendar_controls, |event, clock, _| {
                        if event == crate::clock::Event::Hourly {
                            if config[9] != u32::MAX {
                                clock.calendar[0] = config[9];
                            }
                            // The hourly dispatcher is an explicit callback here;
                            // its hour-6 sound cleanup is observed by the outer loop.
                            if clock.hour() == 6 {
                                controls.before_six_sound = 0;
                            }
                        }
                        false
                    });
                }
                Ok(if event == Event::BeforeSixSound {
                    0xa0d10
                } else {
                    0
                })
            })
            .unwrap();
            assert_eq!(snapshot(&clock, &controls), expected, "case{case} state");
            let count = word(at) as usize;
            at += 4;
            assert_eq!(trace.len(), count, "case{case} trace count");
            for actual in trace {
                let expected = (word(at), std::array::from_fn(|i| word(at + 4 + i * 4)));
                assert_eq!(actual, expected, "case{case} callback");
                at += 52;
            }
        }
        assert_eq!(at, bytes.len());
    }
}
