//! Connected tick-ceiling windows over the real loopback worker. Both peers run
//! the retained scheduler; its ceiling events go through `reach_ceiling`, the
//! same call a live Session makes, and nothing grants a window on the side.
//! Each 200th tick both peers take the state sample the way a Session does, and
//! the client's samples reach the authority's comparison.
use super::*;
use crate::network::synchronization::Synchronization;
use std::{
    thread,
    time::{Duration, Instant},
};

/// One game hour is 625 category-clock ticks; a day is 24 of them.
const DAY: u32 = 24 * 625;

fn linked_pair_runs_in_step_to(end: u32) {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    let roads = empty_roads(&image);
    // Live residents bound the way creation binds them (#453).
    for session in [&mut left, &mut right] {
        session.add_residents(&[[40.5, -3.25], [1.9, 2.1], [-17.75, 600.]]);
    }
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let hb = host
        .bind_world(left.world.topology.epoch().serial())
        .unwrap();
    let cb = client
        .bind_world(right.world.topology.epoch().serial())
        .unwrap();
    let owners = vec![Some(0), Some(peer)];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, owners.clone()).unwrap();
    let mut host_sync = Synchronization::new(hb, true, 0, owners.clone()).unwrap();
    let mut client_sync = Synchronization::new(cb, false, peer, owners).unwrap();
    let mut sampled = [Vec::new(), Vec::new()];
    // A new connected city: Total 0 and the initial ceiling 25 (4b9d40).
    for (session, adapter) in [(&mut left, &authority), (&mut right, &replica)] {
        session.world.clock.timing[0] = 0;
        session.controls.deadline = adapter.deadline();
        session.controls.network_override = adapter.is_host();
    }
    // Property transfers requested by each peer in turn, some exactly at a
    // ceiling tick, each committed by the authority at its own Total+25.
    let mut requests = [
        (300u32, 1usize, 2u8),
        (1000, 0, 1),
        (2000, 1, 2),
        (3125, 0, 1),
    ]
    .into_iter()
    .peekable();
    let mut client_ceiling_pauses = 0;
    let limit = Instant::now() + Duration::from_secs(120);
    while left.world.clock.timing[0] < end || right.world.clock.timing[0] < end {
        let client_tick = right.world.clock.timing[0];
        for (index, session, adapter, sync, worker) in [
            (0, &mut left, &mut authority, &mut host_sync, &host),
            (1, &mut right, &mut replica, &mut client_sync, &client),
        ] {
            let tick = session.world.clock.timing[0];
            if adapter.is_host() {
                // The authority granted its window when it reached the
                // ceiling, so a frame never starts at an exhausted window.
                assert!(tick < adapter.deadline(), "authority stuck at {tick}");
            }
            adapter
                .pump(
                    worker,
                    tick,
                    &mut session.queue,
                    &mut session.world.city.categories,
                    Targets {
                        buildings: &session.world.city.buildings,
                        businesses: &session.world.city.ledgers.businesses,
                        ..Targets::default()
                    },
                    &mut session.world.city.pause,
                    &mut session.tasks,
                )
                .unwrap();
            // Commands first, then synchronization, as the Session's 0x450410.
            sync.pump(
                worker,
                [true, true, false, false, false, false],
                &mut session.world.city.pause,
                &mut session.tasks,
            )
            .unwrap();
            session.controls.deadline = adapter.deadline();
            if let Some(&(at, by, owner)) = requests.peek()
                && (at, by) == (tick, index)
                && !session.world.pause.has_reason()
            {
                requests.next();
                adapter
                    .submit(
                        worker,
                        live_commands::Command::PropertyTransfer {
                            category: 7,
                            owner,
                            amount: 500,
                        },
                        &mut session.queue,
                        &mut session.world.city.categories,
                        Targets::default(),
                        tick,
                    )
                    .unwrap();
            }
            // Frame pacing only: the authority never waits for a client, but
            // a real host advances at wall-clock rate, not unboundedly.
            let ahead = index == 0 && tick >= end.min(client_tick + 100);
            if tick >= end || ahead {
                continue;
            }
            session.tick_with(
                &image,
                &board,
                |event, clock, city, players, controls, tasks| {
                    if event == scheduler::Event::Helper(0x481340) {
                        sync.capture(worker, clock.timing[0], city, &roads, players)?;
                        sampled[index].push(clock.timing[0]);
                        return Ok(());
                    }
                    if adapter.reach_ceiling(
                        worker,
                        event,
                        clock.timing[0],
                        controls,
                        &mut city.pause,
                        tasks,
                    )? {
                        assert_eq!(clock.timing[0] % 25, 0);
                        if index == 1 {
                            assert_eq!(city.pause.bytes[1], 1);
                            client_ceiling_pauses += 1;
                        } else {
                            assert_eq!(controls.deadline, clock.timing[0] + 25);
                        }
                    }
                    Ok(())
                },
            );
        }
        assert!(
            Instant::now() < limit,
            "host {}, client {}, deadlines {} / {}",
            left.world.clock.timing[0],
            right.world.clock.timing[0],
            authority.deadline(),
            replica.deadline()
        );
        thread::sleep(Duration::from_micros(200));
    }
    // The authority grants its next window on reaching `end`, so that last
    // grant can still be in flight when both peers stop.
    while replica.deadline() != authority.deadline() {
        replica
            .pump(
                &client,
                end,
                &mut right.queue,
                &mut right.world.city.categories,
                Targets::default(),
                &mut right.world.city.pause,
                &mut right.tasks,
            )
            .unwrap();
        assert!(Instant::now() < limit, "final grant never arrived");
        thread::sleep(Duration::from_micros(200));
    }
    assert_eq!(requests.next(), None);
    assert_eq!(left.world.clock, right.world.clock);
    assert_eq!(left.world.clock.timing[0], end);
    assert!(client_ceiling_pauses > 0);
    // The same four commits applied once on both peers at the same ticks.
    assert_eq!(left.applied, [0x16; 4]);
    assert_eq!(left.applied, right.applied);
    assert_eq!(left.postings, right.postings);
    for (a, b) in left
        .world
        .ledgers
        .players
        .iter()
        .zip(&right.world.ledgers.players)
    {
        assert_eq!(a.cash.to_bits(), b.cash.to_bits());
    }
    assert_eq!(left.world.city.categories, right.world.city.categories);
    assert_eq!(
        left.world.city.ledgers.businesses,
        right.world.city.ledgers.businesses
    );
    assert_eq!(authority.deadline(), replica.deadline());
    // Every sample point on both peers, and the authority found no difference
    // in any client sample it compared.
    let expected: Vec<u32> = (200..=end).step_by(200).collect();
    assert_eq!(sampled, [expected.clone(), expected.clone()]);
    assert_eq!(host_sync.samples.status, Default::default());
    for &tick in expected.iter().rev().take(10) {
        let (a, b) = (
            host_sync.samples.retained(tick).unwrap(),
            client_sync.samples.retained(tick).unwrap(),
        );
        assert_eq!((a.sender, b.sender), (0, 1));
        assert_eq!(
            (a.sums, a.round, a.precision),
            (b.sums, b.round, b.precision)
        );
        assert_eq!(a.sums[0], 40 - 3 + 1 + 2 - 17 + 600);
    }
    host.cancellation().cancel();
    client.cancellation().cancel();
}

#[test]
fn connected_peers_cross_every_ceiling_for_a_game_hour() {
    linked_pair_runs_in_step_to(625 * 6);
}

#[test]
#[ignore = "long soak: one connected game day of tick-ceiling windows"]
fn connected_peers_cross_every_ceiling_for_a_game_day() {
    linked_pair_runs_in_step_to(DAY);
}
