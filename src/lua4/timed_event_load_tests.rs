//! Absolute script events across a load: the saved global tick decides which
//! deadlines are queued again, the countdown runs on normal iterations, and a
//! pause raised by an event costs the rest of its scheduler batch one paused
//! iteration (0x4d53b0, 0x4d5b40, 0x481680, 0x4bcb90).

use super::{Runtime, Value};
use crate::clock::Clock;
use crate::scheduler::{self, Controls, Event};
use crate::script_events::Events;

const CLOCK: u32 = 50;
const ADMIT: u32 = 51;
const SHOW: u32 = 60;
const PAUSE: u32 = 61;

/// Nine timed tips, scheduled the way a campaign hub does: an absolute tick
/// computed from (day, hour, minute) of a 09:00 start, 625 ticks per hour.
const LEVEL: &[u8] = br#"
    function At(day, hour, minute)
        return (day - 1) * 15000 + (hour - 9) * 625 + minute * 10
    end
    function EVENTINITIALISE_Level()
        local times = { {1,9,30}, {1,11,0}, {1,13,10}, {1,16,5}, {1,18,15},
                        {1,23,0}, {2,0,0}, {2,2,0}, {2,6,0} }
        local n = 1
        while times[n] do
            local t = times[n]
            C_TriggerEventAbsolute('Tip', At(t[1], t[2], t[3]), n)
            n = n + 1
        end
    end
    function EVENT_Tip(n)
        Show(n)
        Pause()
    end
"#;

fn fresh_clock() -> Clock {
    let mut clock = Clock {
        calendar: [0; 5],
        timing: [0, 0, 0, 0, 0, 1],
    };
    clock.reset_world_clock();
    clock
}

/// Run the level's initializer against `clock`, as session start does.
fn start_level(clock: &Clock) -> (Runtime, Events) {
    let mut vm = Runtime::new().unwrap();
    vm.register_event_triggers(CLOCK).unwrap();
    vm.register("Show", SHOW).unwrap();
    vm.register("Pause", PAUSE).unwrap();
    let mut events = Events::default();
    let total = clock.timing[0];
    vm.execute("level", LEVEL, &mut |_, _| Err("unexpected binding".into()))
        .unwrap();
    vm.call("EVENTINITIALISE_Level", &[0.], &mut |id, args| {
        if id == CLOCK {
            return Ok(Some(f64::from(total)));
        }
        if id == ADMIT {
            let [
                Value::Number(reference),
                Value::Number(delay),
                Value::Number(argument),
            ] = args
            else {
                return Err("bad admission".into());
            };
            events.append(crate::script_events::Event {
                reference: *reference as u32,
                remaining: *delay as u32,
                argument: *argument as i32,
            });
        }
        Ok(None)
    })
    .unwrap();
    (vm, events)
}

/// A shown tip: (tip, global tick, normal tick).
type Shown = (i32, u32, u32);

/// Advance in batches of `speed` iterations until the normal tick reaches
/// `until`. A tip pauses the game; the pause latch is tested after the command
/// drain, as the session does, and the tip is dismissed between batches.
fn run(
    vm: &mut Runtime,
    events: &mut Events,
    clock: &mut Clock,
    speed: u32,
    until: u32,
) -> Vec<Shown> {
    let mut shown = vec![];
    let mut paused = false;
    let mut controls = Controls {
        iterations: speed,
        abort: false,
        alternate_script_active: false,
        network_enabled: false,
        network_override: false,
        deadline: u32::MAX,
        gate_32: false,
        gate_32_offset: false,
        before_six_sound: 0,
    };
    while clock.timing[1] < until {
        scheduler::advance(clock, &mut controls, |event, clock, controls| {
            match event {
                Event::Helper(0x480ec0) => controls.abort = paused,
                Event::Helper(0x4d5b40) => {
                    let ticks = (clock.timing[0], clock.timing[1]);
                    vm.tick_events(events, &mut |id, args, _| {
                        match (id, args) {
                            (SHOW, [Value::Number(n)]) => shown.push((*n as i32, ticks.0, ticks.1)),
                            (PAUSE, _) => paused = true,
                            _ => return Err("unexpected binding".into()),
                        }
                        Ok(None)
                    })?;
                }
                _ => {}
            }
            Ok(0)
        })
        .unwrap();
        paused = false;
    }
    shown
}

#[test]
fn loaded_save_queues_only_pending_tips_and_each_costs_one_paused_iteration() {
    // Starting Out, midnight after day 1: global 9381, normal 9375. Six global
    // ticks of earlier tips are already in the saved total.
    let mut clock = fresh_clock();
    clock.restore_load_timing([9381, 9375, 0, 0, 0, 1]);
    assert_eq!(clock.timing, [9381, 9375, 0, 0, 0, 1]);
    let (mut vm, mut events) = start_level(&clock);
    let queued: Vec<_> = events
        .pending()
        .iter()
        .map(|e| (e.argument, e.remaining))
        .collect();
    assert_eq!(queued, [(8, 10625 - 9381), (9, 13125 - 9381)]);

    let midnight = 9375 + 15000;
    let shown = run(&mut vm, &mut events, &mut clock, 16, midnight);
    // Each tip fires on its global deadline (the second one a tick later,
    // after the first tip's paused iteration) at normal 01:59 and 05:59.
    assert_eq!(shown, [(8, 10625, 10619), (9, 13126, 13119)]);
    // Two paused iterations: the original's total tick is 24383 at midnight.
    assert_eq!(clock.timing[0] - clock.timing[1], 9381 - 9375 + 2);
}

#[test]
fn a_pause_on_the_last_iteration_of_a_batch_costs_no_paused_iteration() {
    let mut clock = fresh_clock();
    clock.restore_load_timing([9381, 9375, 0, 0, 0, 1]);
    let (mut vm, mut events) = start_level(&clock);
    let midnight = 9375 + 15000;
    let shown = run(&mut vm, &mut events, &mut clock, 1, midnight);
    assert_eq!(shown, [(8, 10625, 10619), (9, 13125, 13119)]);
    assert_eq!(clock.timing[0] - clock.timing[1], 9381 - 9375);
}

#[test]
fn fresh_start_queues_every_tip() {
    let mut clock = fresh_clock();
    assert_eq!(clock.timing[..4], [0, 0, 0, 9 * 625]);
    let (mut vm, mut events) = start_level(&clock);
    assert_eq!(events.pending().len(), 9);
    let shown = run(&mut vm, &mut events, &mut clock, 1, 15000);
    let normal: Vec<_> = shown.iter().map(|&(n, _, normal)| (n, normal)).collect();
    assert_eq!(
        normal,
        [
            (1, 300),
            (2, 1250),
            (3, 2600),
            (4, 4425),
            (5, 5775),
            (6, 8750),
            (7, 9375),
            (8, 10625),
            (9, 13125)
        ]
    );
}
