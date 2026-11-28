//! Component contracts recovered from original4896e0/489df0. Expected events
//! are literal original control-flow outcomes, independent of modern metrics.
//! These tests do not establish natural target acquisition or result UI safety.
use openmt2001::{
    objective_tick::{self, Controls, Event},
    player_manager::{Manager, Player},
    scenario_objective::State,
    x87::Sum,
};

fn roster(active: &[bool]) -> Manager {
    let mut manager = Manager::initialize([0; 40]);
    for (slot, &active) in active.iter().enumerate() {
        let mut bytes = vec![0; 0x224];
        bytes[0x10] = u8::from(active);
        manager.players.push(Player {
            reference: 0x10010000 + slot as u32 * 0x400,
            bytes,
            auxiliaries: vec![],
        });
    }
    manager
}

fn controls(mode: u32) -> Controls {
    Controls {
        network_enabled: false,
        network_override: false,
        mode,
        year: 0,
        day: 0,
    }
}

#[test]
fn authored_sales_flag_zero_requires_target_with_one_survivor() {
    // SCENARIO3/4 explicitly set (SALES,100,0). 4896e0 must skip its
    // reason4 shortcut, then compare signed player+1b4 against the target.
    let mut manager = roster(&[false, true, false]);
    let mut state = State::default();
    state.set(6., 100., Some(0.));
    let mut controls = controls(1);
    for (sales, expected) in [
        (99u32, vec![]),
        (
            100,
            vec![Event::Outcome {
                winner: 1,
                reason: 2,
            }],
        ),
    ] {
        manager.players[1].bytes[0x1b4..0x1b8].copy_from_slice(&sales.to_le_bytes());
        let mut events = vec![];
        objective_tick::advance(
            &mut state,
            &mut manager,
            &mut controls,
            &mut |event, _, _, _| {
                assert!(
                    !matches!(event, Event::Metric { .. }),
                    "sales reads the player counter"
                );
                events.push(event);
                Ok(Sum::from_i32(0))
            },
        )
        .unwrap();
        assert_eq!(events, expected);
    }
    assert_eq!(state.reached, [0, 1, 0, 0, 0, 0]);
    assert_eq!(state.excess, [0; 6]);
}

#[test]
fn survivor_precedes_metrics_and_keeps_allocated_slot_identity() {
    // 4896e0 uses remaining manager+0c, scans allocated slots for active+10,
    // emits reason4, and returns without setting target/reached/excess.
    let mut manager = roster(&[false, false, true]);
    let mut state = State {
        kind: 5,
        target: 40000.,
        ..State::default()
    };
    let mut controls = controls(1);
    let mut events = vec![];
    objective_tick::advance(
        &mut state,
        &mut manager,
        &mut controls,
        &mut |event, _, _, _| {
            assert!(
                !matches!(event, Event::Metric { .. }),
                "survival bypasses target metrics"
            );
            events.push(event);
            Ok(Sum::from_i32(0))
        },
    )
    .unwrap();
    assert_eq!(
        events,
        [Event::Outcome {
            winner: 2,
            reason: 4
        }]
    );
    assert_eq!(state.reached, [0; 6]);
    assert_eq!(state.excess, [0; 6]);
}

#[test]
fn none_and_non_authoritative_network_never_emit_survivor() {
    // Native kind0 is free play; native93bbb9 && !93bbba returns before
    // every objective path. Neither may become a one-player victory.
    for (kind, network_enabled) in [(0, false), (5, true)] {
        let mut manager = roster(&[true]);
        let mut state = State {
            kind,
            target: 1.,
            ..State::default()
        };
        let mut controls = controls(1);
        controls.network_enabled = network_enabled;
        objective_tick::advance(
            &mut state,
            &mut manager,
            &mut controls,
            &mut |event, _, _, _| {
                panic!("gated objective emitted {event:?}");
            },
        )
        .unwrap();
        assert_eq!(state.reached, [0; 6]);
    }
}

#[test]
fn dated_rank_uses_native_decade_date_and_orders_notice_before_removal() {
    let mut manager = roster(&[true, true]);
    let mut state = State {
        kind: 4,
        target: 1940.,
        ..State::default()
    };
    let mut controls = controls(2);
    controls.day = 9; // 1930 + year*10 + day = 1939: no metric calls yet.
    objective_tick::advance(
        &mut state,
        &mut manager,
        &mut controls,
        &mut |event, _, _, _| {
            panic!("dated objective ran early: {event:?}");
        },
    )
    .unwrap();
    controls.year = 1;
    controls.day = 0;
    let mut events = vec![];
    objective_tick::advance(
        &mut state,
        &mut manager,
        &mut controls,
        &mut |event, _, _, _| {
            events.push(event);
            Ok(match event {
                Event::Metric {
                    address: 0x47fbb0,
                    player: 0,
                } => Sum::from_i32(100),
                Event::Metric {
                    address: 0x47fbb0,
                    player: 1,
                } => Sum::from_i32(200),
                Event::Metric { .. } => panic!("wrong dated-rank metric"),
                _ => Sum::from_i32(0),
            })
        },
    )
    .unwrap();
    assert_eq!(
        events,
        [
            Event::Metric {
                address: 0x47fbb0,
                player: 0
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 0
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 1
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 1
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 0
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 1
            },
            Event::Outcome {
                winner: 1,
                reason: 2
            },
            Event::RemovalCommand(0),
            Event::RemovePlayer(0),
        ]
    );
    assert_eq!(state.reached, [0, 1, 0, 0, 0, 0]);
}

#[test]
fn equal_dated_rank_retains_native_sentinel_and_does_not_eliminate() {
    // Retained kind4_date_max_tie/489df0: winner=-1, reason2; no removals.
    // This asserts evaluator output only, not a modern draw/UI policy.
    let mut manager = roster(&[true, true]);
    let mut state = State {
        kind: 4,
        target: 1940.,
        ..State::default()
    };
    let mut controls = controls(2);
    controls.year = 1;
    let mut outcomes = vec![];
    objective_tick::advance(
        &mut state,
        &mut manager,
        &mut controls,
        &mut |event, _, _, _| {
            if let Event::Metric { address, .. } = event {
                assert_eq!(address, 0x47fbb0);
                return Ok(Sum::from_i32(100));
            }
            outcomes.push(event);
            Ok(Sum::from_i32(0))
        },
    )
    .unwrap();
    assert_eq!(
        outcomes,
        [Event::Outcome {
            winner: -1,
            reason: 2
        }]
    );
    assert_eq!(state.reached, [1, 1, 0, 0, 0, 0]);
    assert_eq!(state.excess, [0; 6]);
}

#[test]
fn nonfinite_threshold_targets_preserve_native_comparison_and_ftol() {
    for kind in [3, 5, 6] {
        for target in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut manager = roster(&[true, false]);
            for player in &mut manager.players {
                player.bytes[0x1b4..0x1b8].copy_from_slice(&100i32.to_le_bytes());
            }
            let mut state = State {
                kind,
                target,
                flag: 0,
                reached: [2, 3, 4, 5, 6, 7],
                excess: [20, 30, 40, 50, 60, 70],
                ..State::default()
            };
            let mut metrics = vec![];
            let mut events = vec![];
            objective_tick::advance(
                &mut state,
                &mut manager,
                &mut controls(1),
                &mut |event, _, _, _| {
                    if let Event::Metric { address, player } = event {
                        assert_eq!(address, if kind == 3 { 0x47fca0 } else { 0x47fbb0 });
                        metrics.push(player);
                        Ok(Sum::from_i32(100))
                    } else {
                        events.push(event);
                        Ok(Sum::from_i32(0))
                    }
                },
            )
            .unwrap();
            assert_eq!(
                metrics,
                if kind == 6 { vec![] } else { vec![0, 1] },
                "native threshold evaluation still visits inactive allocated rows"
            );
            if target == f32::NEG_INFINITY {
                // Finite - (-inf) converts through native FTOL's i64 indefinite
                // value. Its low word is zero, leaving these two rows tied.
                assert_eq!(state.reached, [1, 1, 4, 5, 6, 7]);
                assert_eq!(state.excess, [0, 0, 40, 50, 60, 70]);
                assert_eq!(
                    events,
                    [Event::Outcome {
                        winner: -1,
                        reason: 2
                    }]
                );
            } else {
                assert_eq!(state.reached, [2, 3, 4, 5, 6, 7]);
                assert_eq!(state.excess, [20, 30, 40, 50, 60, 70]);
                assert!(
                    events.is_empty(),
                    "NaN and +inf never produce a new outcome"
                );
            }
        }
    }
}

#[test]
fn finite_thresholds_keep_excess_ranking_including_inactive_rows() {
    for kind in [3, 5, 6] {
        let mut manager = roster(&[true, false]);
        for (slot, player) in manager.players.iter_mut().enumerate() {
            player.bytes[0x1b4..0x1b8].copy_from_slice(&(100 + slot as i32).to_le_bytes());
        }
        let mut state = State {
            kind,
            target: 99.5,
            flag: 0,
            ..State::default()
        };
        let mut events = vec![];
        objective_tick::advance(
            &mut state,
            &mut manager,
            &mut controls(1),
            &mut |event, _, _, _| {
                if let Event::Metric { player, .. } = event {
                    Ok(Sum::from_f32(100.25 + player as f32).unwrap())
                } else {
                    events.push(event);
                    Ok(Sum::from_i32(0))
                }
            },
        )
        .unwrap();
        assert_eq!(state.reached, [1, 1, 0, 0, 0, 0]);
        assert_eq!(state.excess, [0, 1, 0, 0, 0, 0]);
        assert_eq!(
            events,
            [
                Event::Outcome {
                    winner: 1,
                    reason: 2
                },
                Event::RemovalCommand(0),
                Event::RemovePlayer(0),
            ]
        );
    }
}
