//! Original timer records and elapsed-time dispatch (004eb620/004eb9b6).
//! Host timestamp acquisition stays outside this module. Task callbacks are
//! synchronous and can change the live task records or append another task.

pub type Task = [u8; 20];

/// Original4b9895..4b9904 registers these four timers before loading a level.
/// Calendar.SetFrequency's separate5414dc/e0 IDs remain absent on this path.
pub fn startup_tasks(prior: Task) -> Vec<Task> {
    [
        (0.5, 4u32, 0x4baa10),
        (10.0, 1, 0x4ba4d0),
        (50.0, 1, 0x4ba580),
        (25.0, 10, 0x481680),
    ]
    .into_iter()
    .map(|(frequency, group, callback)| {
        let mut task = prior;
        task[4..8].copy_from_slice(&group.to_le_bytes());
        configure(&mut task, frequency, callback);
        task
    })
    .collect()
}

// Error-free TwoSum keeps the low part that an f64 addition loses. Inputs here
// are exact f32 values, integer deltas, or the exact product1000*f32.
fn sum_parts(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let virtual_b = sum - a;
    (sum, (a - (sum - virtual_b)) + (b - virtual_b))
}

// A binary64 sum has eleven trailing zero bits in the original64-bit x87
// significand. At a half-ULP tie it is therefore even. Account for the smaller
// spacing toward zero at exact powers of two.
fn retained_residual(sum: f64, error: f64) -> bool {
    if error == 0. || !sum.is_finite() {
        return false;
    }
    if sum == 0. {
        return true;
    }
    let bits = sum.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
    let toward_zero = sum.is_sign_negative() != error.is_sign_negative();
    let power = bits & ((1_u64 << 52) - 1) == 0;
    let half = 2_f64.powi(exponent - 64 - i32::from(power && toward_zero));
    error.abs() > half
}

fn sum_at_least(a: f64, b: f64, threshold: f64) -> bool {
    let (sum, error) = sum_parts(a, b);
    if sum != threshold {
        return sum >= threshold;
    }
    !(error < 0. && retained_residual(sum, error))
}

fn adjusted_ticks(seconds: f32, delta: i32) -> i32 {
    let (sum, error) = sum_parts(1000. * f64::from(seconds), f64::from(delta));
    // At this magnitude both original operands are integers, and x87's64-bit
    // significand represents their sum exactly throughout the signed64 range.
    if sum.abs() >= 4503599627370496. && sum.abs() < 36893488147419103232. {
        let exact = sum as i128 + error as i128;
        return if exact < i128::from(i64::MIN) || exact > i128::from(i64::MAX) {
            0
        } else {
            exact as i64 as i32
        };
    }
    let mut result = crate::economy::ftol_low_word(sum);
    if sum.is_finite() && sum.trunc() == sum && retained_residual(sum, error) {
        if sum > 0. && error < 0. {
            result = result.wrapping_sub(1);
        }
        if sum < 0. && error > 0. {
            result = result.wrapping_add(1);
        }
    }
    result
}

/// Original manager timing words. The host supplies timeGetTime-equivalent
/// wrapping milliseconds; no wall-clock API belongs in the shared simulation.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub previous: u64,
    pub current: u64,
    pub statistics_since: u64,
    pub rate: f32,
    pub frames: u32,
    pub pending_seconds: f32,
    pub pending_groups: u32,
}

impl Frame {
    /// Original4ebaa0 updates only previous/current. Not manager initialization.
    pub fn synchronize(&mut self, milliseconds:u32) {
        self.previous=u64::from(milliseconds); self.current=u64::from(milliseconds);
    }

    /// Timing part of original4eb660. The host must also clear active timer
    /// registrations. Rate and pending-group words are deliberately retained.
    pub fn initialize(&mut self, milliseconds: u32) {
        self.previous = u64::from(milliseconds);
        self.current = u64::from(milliseconds);
        self.statistics_since = u64::from(milliseconds);
        self.frames = 0;
        self.pending_seconds = 0.;
    }

    /// Original4eb8b0..4eb9b6. None selects its synthetic-time path. Arithmetic
    /// follows the original64-bit x87 precision selected by004ed6e0.
    pub fn elapsed(&mut self, milliseconds: Option<u32>) -> i32 {
        if let Some(now) = milliseconds {
            self.previous = self.current;
            self.current = u64::from(now);
        } else {
            self.previous = 0;
            self.current = crate::economy::ftol_low_word(1000. * f64::from(0.04_f32)) as u32 as u64;
        }
        let before = self.frames;
        self.frames = self.frames.wrapping_add(1);
        if before == 19 {
            self.frames = 0;
            let difference = self.current.wrapping_sub(self.statistics_since);
            if difference != 0 {
                // Original tests all64 bits but FIDIV consumes only the signed
                // low word. Keep that asymmetry, including a zero denominator.
                self.rate = (20000. / f64::from(difference as u32 as i32)) as f32;
            }
            self.statistics_since = self.current;
        }
        let mut delta = (self.current as u32).wrapping_sub(self.previous as u32) as i32;
        // FCOMP's equality bit also skips unordered values, retaining NaN.
        if matches!(
            self.pending_seconds.partial_cmp(&0.),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Greater)
        ) {
            delta = adjusted_ticks(self.pending_seconds, delta);
            self.pending_seconds = 0.;
        }
        delta
    }
}

fn number(task: &Task, at: usize) -> f32 {
    f32::from_le_bytes(task[at..at + 4].try_into().unwrap())
}
fn put(task: &mut Task, at: usize, value: f32) {
    task[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Original4eb620. Keep padding and the existing group mask intact.
pub fn configure(task: &mut Task, frequency: f32, callback: u32) {
    task[0] = 1;
    put(task, 8, (1000. / f64::from(frequency)) as f32);
    put(task, 12, 0.);
    task[16..20].copy_from_slice(&callback.to_le_bytes());
}

/// Original4eb7c0: replace frequency and reset accumulated time while retaining
/// the enabled byte, callback, group mask and padding.
pub fn reconfigure(task: &mut Task, frequency: f32) {
    let enabled = task[0];
    let callback = u32::from_le_bytes(task[16..20].try_into().unwrap());
    configure(task, frequency, callback);
    task[0] = enabled;
}

/// Timer globals used by Calendar.SetFrequency (004d9870).
pub struct CalendarFrequency {
    pub frequency: f32,
    pub calendar_timer: Option<usize>,
    pub logic_timer: Option<usize>,
}

impl CalendarFrequency {
    /// Returns false when the original primary timer ID is -1; in that case
    /// even the stored frequency and script constants remain unchanged.
    /// On true, the Lua binding also restores GAME_MINUTE/HOUR/YEAR/DECADE.
    pub fn set(&mut self, tasks: &mut [Task], value: f64) -> Result<bool, String> {
        let Some(primary) = self.calendar_timer else {
            return Ok(false);
        };
        if primary >= tasks.len() || self.logic_timer.is_some_and(|i| i >= tasks.len()) {
            return Err("calendar timer ID is outside the live timer array".into());
        }
        self.frequency = value as f32;
        reconfigure(&mut tasks[primary], self.frequency);
        if let Some(logic) = self.logic_timer {
            reconfigure(&mut tasks[logic], (f64::from(self.frequency) * 625.) as f32);
        }
        Ok(true)
    }
}

/// Original4eb840: consume the pending seconds adjustment for matching groups,
/// including disabled tasks. The shared delta is rounded before each addition.
pub fn apply_pending(tasks: &mut [Task], seconds: &mut f32, groups: u32) {
    let delta = (1000. * f64::from(*seconds)) as f32;
    for task in tasks {
        if u32::from_le_bytes(task[4..8].try_into().unwrap()) & groups != 0 {
            put(
                task,
                12,
                (f64::from(number(task, 12)) + f64::from(delta)) as f32,
            );
        }
    }
    *seconds = 0.;
}

/// Dispatch the elapsed-millisecond word produced by4eb8b0. Native signed
/// division is followed by a wrapping unsigned loop count; negative elapsed
/// values below-19 therefore retain the original very large number of passes.
/// Callbacks may append or edit records, but must not remove/reorder them.
/// Returned fractions are4eb7f0's accumulator/interval for every task, including
/// disabled tasks. This does not model host clocks or the frame-rate statistic.
pub fn dispatch(
    tasks: &mut Vec<Task>,
    elapsed: i32,
    mut callback: impl FnMut(usize, u32, &mut Vec<Task>) -> Result<(), String>,
) -> Result<Vec<f32>, String> {
    let chunks = (elapsed / 20) as u32;
    for pass in 0..=chunks {
        let delta = if pass == chunks { elapsed % 20 } else { 20 };
        let mut index = 0;
        while index < tasks.len() {
            if tasks[index][0] != 0 {
                // Native FST stores single precision but leaves the unrounded
                // x87 sum for the first comparison (FPCW037f,64-bit precision).
                let accumulator = f64::from(number(&tasks[index], 12));
                let sum = accumulator + f64::from(delta);
                put(&mut tasks[index], 12, sum as f32);
                let mut due = sum_at_least(
                    accumulator,
                    f64::from(delta),
                    f64::from(number(&tasks[index], 8)),
                );
                while due {
                    let period = number(&tasks[index], 8);
                    let remainder =
                        (f64::from(number(&tasks[index], 12)) - f64::from(period)) as f32;
                    put(&mut tasks[index], 12, remainder);
                    let address = u32::from_le_bytes(tasks[index][16..20].try_into().unwrap());
                    let length = tasks.len();
                    callback(index, address, tasks)?;
                    if tasks.len() < length {
                        return Err("timer callback removed task records".into());
                    }
                    // No enabled check inside this loop: a pause affects later
                    // tasks/passes, not this task's already-entered catch-up.
                    due = number(&tasks[index], 12) >= number(&tasks[index], 8);
                }
            }
            index += 1;
        }
    }
    Ok(tasks
        .iter()
        .map(|task| (f64::from(number(task, 12)) / f64::from(number(task, 8))) as f32)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl Reader<'_> {
        fn wide(&mut self) -> u64 {
            u64::from(self.word()) | (u64::from(self.word()) << 32)
        }
        fn frame(&mut self) -> Frame {
            Frame {
                previous: self.wide(),
                current: self.wide(),
                statistics_since: self.wide(),
                rate: f32::from_bits(self.word()),
                frames: self.word(),
                pending_seconds: f32::from_bits(self.word()),
                pending_groups: self.word(),
            }
        }
        fn word(&mut self) -> u32 {
            let (a, b) = self.0.split_at(4);
            self.0 = b;
            u32::from_le_bytes(a.try_into().unwrap())
        }
        fn task(&mut self) -> Task {
            let (a, b) = self.0.split_at(20);
            self.0 = b;
            a.try_into().unwrap()
        }
    }
    #[test]
    #[ignore = "requires original Calendar.SetFrequency observations"]
    fn matches_original_calendar_frequency() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CALENDAR_FREQUENCY").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTCALF01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        assert_eq!(word(8), 40);
        for case in 0..word(8) {
            let primary = word(at);
            let logic = word(at + 4);
            let value = f64::from_le_bytes(data[at + 8..at + 16].try_into().unwrap());
            at += 16;
            let mut tasks: Vec<Task> = data[at..at + 40]
                .chunks_exact(20)
                .map(|v| v.try_into().unwrap())
                .collect();
            at += 40;
            let expected_frequency = word(at);
            at += 4;
            let expected: Vec<Task> = data[at..at + 40]
                .chunks_exact(20)
                .map(|v| v.try_into().unwrap())
                .collect();
            at += 40;
            let constants = word(at);
            at += 4;
            let mut frequency = CalendarFrequency {
                frequency: 3.,
                calendar_timer: (primary != u32::MAX).then_some(primary as usize),
                logic_timer: (logic != u32::MAX).then_some(logic as usize),
            };
            assert_eq!(
                frequency.set(&mut tasks, value).unwrap(),
                constants == 4,
                "case {case} constants gate"
            );
            assert_eq!(
                frequency.frequency.to_bits(),
                expected_frequency,
                "case {case} frequency"
            );
            assert_eq!(tasks, expected, "case {case} records");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original elapsed timer dispatch fixture"]
    fn matches_original_elapsed_dispatch() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_TIMERS").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTTIMER6");
        let mut reader = Reader(&data[8..]);
        assert_eq!(reader.word(), 0x37f);
        let count = reader.word();
        assert!(count >= 9);
        for case in 0..count {
            let initial = reader.word();
            let elapsed = reader.word();
            let trace_count = reader.word();
            let final_count = reader.word();
            let mut tasks: Vec<_> = (0..initial).map(|_| reader.task()).collect();
            let modes: Vec<_> = (0..initial).map(|_| reader.word()).collect();
            let expected_trace: Vec<_> = (0..trace_count)
                .map(|_| (reader.word() as usize, reader.word()))
                .collect();
            let expected: Vec<_> = (0..final_count).map(|_| reader.task()).collect();
            let expected_fractions: Vec<_> = (0..final_count).map(|_| reader.word()).collect();
            let mut calls = vec![0; 100];
            let mut trace = vec![];
            let fractions = dispatch(&mut tasks, elapsed as i32, |index, address, tasks| {
                trace.push((index, address));
                calls[index] += 1;
                match modes.get(index).copied().unwrap_or(0) {
                    1 => tasks[index][0] = 0,
                    2 => crate::pause::Timers::Suspend.apply(tasks),
                    3 if calls[index] == 1 => {
                        let mut task = [0; 20];
                        task[1..4].copy_from_slice(&[0xa5, 0x6c, 0xef]);
                        configure(&mut task, 100., 0x100b0000 + tasks.len() as u32 * 16);
                        task[4..8].copy_from_slice(&1u32.to_le_bytes());
                        tasks.push(task);
                    }
                    4 if calls[index] == 1 => put(&mut tasks[index], 8, 80.),
                    _ => {}
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(trace, expected_trace, "case {case} callbacks");
            assert_eq!(tasks, expected, "case {case} records");
            assert_eq!(
                fractions.into_iter().map(f32::to_bits).collect::<Vec<_>>(),
                expected_fractions,
                "case {case} fractions"
            );
        }
        let configurations = reader.word();
        assert_eq!(configurations, 7);
        for case in 0..configurations {
            let mut task = reader.task();
            let frequency = f32::from_bits(reader.word());
            let callback = reader.word();
            let expected = reader.task();
            configure(&mut task, frequency, callback);
            assert_eq!(task, expected, "configuration {case}");
        }
        let adjustments = reader.word();
        assert_eq!(adjustments, 8);
        for case in 0..adjustments {
            let length = reader.word();
            let mut seconds = f32::from_bits(reader.word());
            let groups = reader.word();
            let mut tasks: Vec<_> = (0..length).map(|_| reader.task()).collect();
            let expected: Vec<_> = (0..length).map(|_| reader.task()).collect();
            let remaining = reader.word();
            apply_pending(&mut tasks, &mut seconds, groups);
            assert_eq!(tasks, expected, "adjustment {case}");
            assert_eq!(seconds.to_bits(), remaining, "adjustment {case} consumed");
        }
        let frames = reader.word();
        assert_eq!(frames, 14);
        for case in 0..frames {
            let real = reader.word() != 0;
            let now = reader.word();
            let mut state = reader.frame();
            let expected = reader.frame();
            let elapsed = reader.word();
            assert_eq!(
                state.elapsed(real.then_some(now)) as u32,
                elapsed,
                "frame {case} elapsed"
            );
            let fields = |f: Frame| {
                [
                    f.previous,
                    f.current,
                    f.statistics_since,
                    u64::from(f.rate.to_bits()),
                    u64::from(f.frames),
                    u64::from(f.pending_seconds.to_bits()),
                    u64::from(f.pending_groups),
                ]
            };
            assert_eq!(fields(state), fields(expected), "frame {case} fields");
        }
        let resets = reader.word();
        assert_eq!(resets, 3);
        for _ in 0..resets {
            let now = reader.word();
            let mut frame = reader.frame();
            let expected = reader.frame();
            frame.initialize(now);
            assert_eq!(frame.previous, expected.previous);
            assert_eq!(frame.current, expected.current);
            assert_eq!(frame.statistics_since, expected.statistics_since);
            assert_eq!(frame.rate.to_bits(), expected.rate.to_bits());
            assert_eq!(frame.frames, expected.frames);
            assert_eq!(
                frame.pending_seconds.to_bits(),
                expected.pending_seconds.to_bits()
            );
            assert_eq!(frame.pending_groups, expected.pending_groups);
        }
        assert!(reader.0.is_empty());
    }
}
