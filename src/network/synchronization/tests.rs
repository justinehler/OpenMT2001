use super::*;
use crate::network::{Frame, Owner, codec};

fn sample(tick: u32) -> Sample {
    Sample {
        sender: 1,
        tick,
        sums: [10, 20, 30, 40, 50],
        round: 0,
        precision: 3,
        available: 0x1f,
    }
}
fn owner() -> (Owner, SessionBinding, crate::network::Connection) {
    let owner = Owner::new().unwrap();
    let attempt = owner.begin().unwrap();
    owner.opened(attempt, [7; 16]).unwrap();
    let peer = owner.adopt(attempt, 4, false).unwrap();
    owner.begin_start(attempt).unwrap();
    let binding = owner.bind_world(attempt, 42).unwrap();
    (owner, binding, peer)
}
#[test]
fn retains_one_group_zero_task_and_reenables_without_resetting_accumulator() {
    let (_, binding, _) = owner();
    let mut sync = Synchronization::new(binding, true, 0, vec![Some(0), Some(4)]).unwrap();
    let mut tasks = crate::timers::startup_tasks([0; 20]);
    let id = sync.enable(&mut tasks).unwrap();
    assert_eq!(id, 4);
    assert_eq!(&tasks[id][4..8], &[0; 4]);
    assert_eq!(
        f32::from_le_bytes(tasks[id][8..12].try_into().unwrap()),
        200.
    );
    tasks[id][12..16].copy_from_slice(&123f32.to_le_bytes());
    tasks[id][0] = 0;
    let before = tasks[id];
    assert_eq!(sync.enable(&mut tasks).unwrap(), id);
    assert_eq!(tasks.len(), 5);
    assert_eq!(&tasks[id][1..], &before[1..]);
    let mut pause = crate::pause::State::default();
    sync.barrier.open(&mut pause, &mut tasks).unwrap();
    assert_eq!(tasks[id][0], 1);
    assert_eq!(tasks[3][0], 0);
}
#[test]
fn barrier_waits_for_real_ack_excludes_inactive_and_uses_send_result() {
    let mut barrier = Barrier::default();
    let mut pause = crate::pause::State::default();
    let mut tasks = crate::timers::startup_tasks([0; 20]);
    barrier.open(&mut pause, &mut tasks).unwrap();
    let mut calls = Vec::new();
    for _ in 0..5 {
        assert!(
            !barrier
                .pump(
                    true,
                    0,
                    [true, true, false, false, false, false],
                    &mut pause,
                    &mut tasks,
                    |target, c| {
                        calls.push((target, c));
                        Ok(true)
                    }
                )
                .unwrap()
        );
    }
    assert_eq!(calls, [(Some(1), 0)]);
    assert!(barrier.active);
    assert_eq!(pause.bytes[2], 1);
    barrier.receive(true, 1, 0, &mut pause, &mut tasks).unwrap();
    for _ in 0..4 {
        barrier
            .pump(
                true,
                0,
                [true, true, false, false, false, false],
                &mut pause,
                &mut tasks,
                |_, _| panic!(),
            )
            .unwrap();
    }
    assert!(
        barrier
            .pump(
                true,
                0,
                [true, true, false, false, false, false],
                &mut pause,
                &mut tasks,
                |target, c| {
                    assert_eq!((target, c), (None, 1));
                    Ok(false)
                }
            )
            .unwrap()
    );
    assert!(!barrier.active);
    assert!(!pause.has_reason());
    barrier.open(&mut pause, &mut tasks).unwrap();
    for _ in 0..5 {
        barrier
            .pump(
                true,
                0,
                [true, true, false, false, false, false],
                &mut pause,
                &mut tasks,
                |_, _| Ok(false),
            )
            .unwrap();
    }
    assert!(!barrier.active); // Original failed-forward semantics.
    assert_eq!(barrier.counter, 15); // open did not reset phase.
}
#[test]
fn signed_wrapping_counter_client_failure_and_request_rearm() {
    let mut barrier = Barrier {
        counter: i32::MAX,
        ..Default::default()
    };
    let mut pause = crate::pause::State::default();
    let mut tasks = vec![];
    barrier.open(&mut pause, &mut tasks).unwrap();
    barrier
        .receive(false, 0, 0, &mut pause, &mut tasks)
        .unwrap();
    let mut count = 0;
    for _ in 0..4 {
        barrier
            .pump(false, 1, [false; 6], &mut pause, &mut tasks, |target, c| {
                assert_eq!((target, c), (None, 0));
                count += 1;
                Ok(false)
            })
            .unwrap();
    }
    assert_eq!(barrier.counter, i32::MIN + 3);
    assert_eq!(count, 1);
    for _ in 0..5 {
        barrier
            .pump(
                false,
                1,
                [false; 6],
                &mut pause,
                &mut tasks,
                |_, _| panic!(),
            )
            .unwrap();
    }
    barrier
        .receive(false, 0, 0, &mut pause, &mut tasks)
        .unwrap();
    for _ in 0..5 {
        barrier
            .pump(false, 1, [false; 6], &mut pause, &mut tasks, |_, _| {
                count += 1;
                Ok(true)
            })
            .unwrap();
    }
    assert_eq!(count, 2);
    assert!(
        barrier
            .receive(false, 0, 255, &mut pause, &mut tasks)
            .unwrap()
    );
    assert!(!pause.has_reason());
}
#[test]
fn ring_reuse_matching_ticks_individual_diagnostics_and_placeholders() {
    let mut samples = sample::Samples::default();
    for tick in (200..=2400).step_by(200) {
        samples.retain(sample(tick));
    }
    assert!(samples.retained(200).is_none());
    assert!(samples.retained(400).is_none());
    assert!(samples.retained(600).is_some());
    samples.compare(Sample {
        sums: [0; 5],
        ..sample(200)
    });
    assert_eq!(samples.status.mismatch_count, 0);
    let mut remote = sample(2400);
    remote.sums = [0; 5];
    remote.round = 2;
    remote.precision = 2;
    samples.compare(remote);
    assert_eq!(samples.status.mismatch_count, 7);
    assert_eq!(samples.status.last_mismatches.len(), 7);
    remote.available = 0;
    samples.compare(remote);
    assert_eq!(samples.status.mismatch_count, 9);
    samples.retain(sample(2600));
    assert_eq!(samples.retained(2600), Some(sample(2600)));
}
#[test]
fn different_ticks_are_ignored_until_a_matching_sample_arrives() {
    let mut samples = sample::Samples::default();
    samples.compare(Sample {
        sums: [0; 5],
        ..sample(200)
    });
    assert_eq!(samples.status.mismatch_count, 0);
    samples.retain(sample(200));
    assert_eq!(samples.status.mismatch_count, 0);
    samples.compare(Sample {
        sums: [0; 5],
        ..sample(200)
    });
    assert_eq!(samples.status.mismatch_count, 5);
}
#[test]
fn codec_round_trip_and_bad_sender_rejected_against_adopted_connection() {
    let (owner, binding, connection) = owner();
    let mut sync = Synchronization::new(binding, true, 0, vec![Some(0), Some(4)]).unwrap();
    let mut pause = crate::pause::State::default();
    let mut tasks = vec![];
    sync.barrier.open(&mut pause, &mut tasks).unwrap();
    for (sequence, message) in [
        Message::Barrier {
            sender: 0,
            complete: 0,
        },
        Message::StateSample(Sample {
            sender: 0,
            ..sample(200)
        }),
    ]
    .into_iter()
    .enumerate()
    {
        let frame = Frame {
            session: [7; 16],
            sequence: sequence as u64,
            message,
        };
        let bytes = codec::encode(&frame).unwrap();
        assert_eq!(codec::decode(&bytes).unwrap(), frame);
        let message = owner.receive(connection, Some(binding), &bytes).unwrap();
        assert_eq!(
            sync.receive(
                &owner,
                Ingress {
                    binding,
                    connection,
                    message
                },
                &mut pause,
                &mut tasks
            ),
            Err(Error::Peer.to_string())
        );
    }
    assert_eq!(sync.barrier.acknowledgments, [false; 6]);
    assert!(
        codec::encode(&Frame {
            session: [7; 16],
            sequence: 3,
            message: Message::StateSample(Sample {
                sender: 6,
                ..sample(200)
            })
        })
        .is_err()
    );
}
#[test]
fn inactive_buildings_businesses_and_players_are_excluded_and_cash_is_signed() {
    let mut buildings = vec![[0; 0x394]; 2];
    for row in &mut buildings {
        row[0xc..0x10].copy_from_slice(&12.9f32.to_le_bytes());
        row[0x10..0x14].copy_from_slice(&(-2.9f32).to_le_bytes());
        row[0x1c..0x20].copy_from_slice(&7u32.to_le_bytes());
        row[0x24..0x28].copy_from_slice(&3u32.to_le_bytes());
    }
    buildings[0][0x3f] = 1;
    assert_eq!(sample::buildings(&buildings), 20);
    let mut businesses = vec![[0; 0x220]; 2];
    for row in &mut businesses {
        row[8..10].copy_from_slice(&(-1i16).to_le_bytes());
        row[0xa] = 255;
        row[0x1c..0x20].copy_from_slice(&10.9f32.to_le_bytes());
        row[0x1c4..0x1c8].copy_from_slice(&(-2.9f32).to_le_bytes());
    }
    businesses[0][0x21c] = 1;
    assert_eq!(
        sample::businesses(&businesses, &buildings, &[[0; 0x17c]; 52]).unwrap(),
        6
    );
    let mut a = crate::economy::PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap();
    a.cash = 16_777_216.;
    a.retail_counters[0] = -16_777_217;
    a.housing_counter = 0;
    let mut b = a.clone();
    b.cash = -0.5;
    assert_eq!(sample::players([(true, &a), (true, &b)]), 16_777_215);
    assert_eq!(sample::players([(true, &a), (false, &b)]), 16_777_216);
    a.cash = -1.;
    b.cash = 0.5;
    assert_eq!(sample::players([(true, &a), (true, &b)]), 0);
}

#[test]
fn barrier_membership_tracks_live_humans_not_the_startup_roster() {
    let player = |active, kind: u32| {
        let mut bytes = vec![0; 0x224];
        bytes[0x10] = active;
        bytes[0x154..0x158].copy_from_slice(&kind.to_le_bytes());
        crate::player_manager::Player {
            reference: 1,
            bytes,
            auxiliaries: vec![],
        }
    };
    let mut players = vec![player(1, 0), player(0, 0), player(1, 1), player(1, 2)];
    assert_eq!(
        active_humans(&players),
        [true, false, false, false, false, false]
    );
    players[0].bytes[0x10] = 0;
    players[1].bytes[0x10] = 1;
    assert_eq!(
        active_humans(&players),
        [false, true, false, false, false, false]
    );
}

#[cfg(feature = "lua4")]
#[test]
fn admitted_loss_is_observed_before_barrier_work() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let binding = host.bind_world(1).unwrap();
    client.bind_world(2).unwrap();
    let mut synchronization =
        Synchronization::new(binding, true, 0, vec![Some(0), Some(peer)]).unwrap();
    let mut pause = crate::pause::State::default();
    let mut tasks = crate::timers::startup_tasks([0; 20]);
    synchronization.enable(&mut tasks).unwrap();
    synchronization
        .barrier
        .open(&mut pause, &mut tasks)
        .unwrap();
    drop(client);
    let until = Instant::now() + Duration::from_secs(3);
    while host.validate_binding(binding).is_ok() {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(5));
    }
    let before = synchronization.barrier.counter;
    assert!(
        synchronization
            .pump(
                &host,
                [true, true, false, false, false, false],
                &mut pause,
                &mut tasks,
            )
            .is_err()
    );
    assert_eq!(synchronization.barrier.counter, before);
    assert!(synchronization.barrier.active);
    assert_eq!(pause.bytes[2], 1);
}
#[test]
fn comparisons_record_every_received_sample_in_order() {
    let mut samples = sample::Samples::default();
    samples.retain(sample(200));
    samples.compare(sample(200));
    let mut differing = sample(200);
    differing.sums[0] = 11;
    samples.compare(differing);
    // No retained host sample for 400: recorded, nothing compared.
    samples.compare(sample(400));
    // An invalid tick is not a comparison at all.
    samples.compare(sample(201));
    let all: Vec<_> = samples.comparisons.since(0).collect();
    assert_eq!(samples.comparisons.count, 3);
    assert_eq!(
        all.iter().map(|c| (c.tick, c.local)).collect::<Vec<_>>(),
        [(200, true), (200, true), (400, false)]
    );
    assert!(all[0].differences.is_empty());
    assert_eq!(all[1].differences.len(), 1);
    assert_eq!(all[1].differences[0].field, Field::People);
    assert_eq!(samples.comparisons.since(2).count(), 1);
    // The window is bounded; the count keeps numbering.
    for _ in 0..100 {
        samples.compare(sample(200));
    }
    assert_eq!(samples.comparisons.count, 103);
    assert_eq!(samples.comparisons.since(0).count(), 64);
    assert_eq!(samples.comparisons.since(100).count(), 3);
}
