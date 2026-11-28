//! Connected tick-ceiling windows over the real loopback worker. Both peers run
//! the retained scheduler; its ceiling events go through `reach_ceiling`, the
//! same call a live Session makes, and nothing grants a window on the side.
use super::*;
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
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let hb = host
        .bind_world(left.world.topology.epoch().serial())
        .unwrap();
    let cb = client
        .bind_world(right.world.topology.epoch().serial())
        .unwrap();
    let owners = vec![Some(0), Some(peer)];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, owners).unwrap();
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
        for (index, session, adapter, worker) in [
            (0, &mut left, &mut authority, &host),
            (1, &mut right, &mut replica, &client),
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
            session.tick_with(&image, &board, |event, clock, city, _, controls, tasks| {
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
            });
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
