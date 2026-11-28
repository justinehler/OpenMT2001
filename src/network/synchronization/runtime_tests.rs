//! Connected scheduler and sample regression with synthetic live engine state.
use super::*;

#[test]
fn loopback_live_scheduler_crosses_200_and_keeps_running_after_desync_to_1000() {
    use crate::network::synchronization::Synchronization;
    use std::{
        thread,
        time::{Duration, Instant},
    };
    struct Models;
    impl crate::traffic::road::runtime::VehicleModels for Models {
        fn bind(&mut self, _: u32, _: &mut [u8; 284]) -> std::result::Result<(), String> {
            Ok(())
        }
        fn extent(
            &self,
            _: &[u8; 284],
        ) -> std::result::Result<crate::traffic::road::ModelExtent, String> {
            unreachable!()
        }
    }
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    let mut roads = crate::traffic::road::runtime::RoadTraffic::fresh(
        &image,
        0,
        [0; 3],
        &mut [0; 284],
        0x10000000,
        &mut Models,
    )
    .unwrap();
    // Nonempty suppliers: a resident auxiliary reached through its runtime
    // reference and one live road actor on a simple synthetic front path.
    use crate::traffic::road::construction::{NODE_BASE, SEGMENT_BASE};
    roads.records[0][0x14] = 1;
    put(&mut roads.records[0], 0xb4, SEGMENT_BASE);
    put(&mut roads.records[0], 0x2c, 0.5f32.to_bits());
    for session in [&mut left, &mut right] {
        let city = &mut session.world.city;
        let mut auxiliary = [0; 0x11c];
        put(&mut auxiliary, 4, 12.9f32.to_bits());
        put(&mut auxiliary, 0xc, (-2.9f32).to_bits());
        assert_eq!(city.creation.auxiliary.allocate(&auxiliary), Some(0));
        city.ledgers.residents.active_count = 1;
        city.ledgers.residents.nodes[0] =
            resident::Resident::from_runtime_bytes([0; 0x14a], 1, 0x6899a0);
        let graph = &mut city.road_network;
        graph.segments[0][0x22] = 1;
        put(&mut graph.segments[0], 0x5c, 1f32.to_bits());
        put(&mut graph.segments[0], 0x60, NODE_BASE);
        put(&mut graph.segments[0], 0x64, NODE_BASE + 44);
        for (axis, endpoint) in [2f32, 4., 6.].into_iter().enumerate() {
            put(&mut graph.nodes[0], 0x20 + axis * 4, endpoint.to_bits());
            put(&mut graph.nodes[1], 0x20 + axis * 4, 0f32.to_bits());
        }
    }
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let hb = host
        .bind_world(left.world.topology.epoch().serial())
        .unwrap();
    let cb = client
        .bind_world(right.world.topology.epoch().serial())
        .unwrap();
    let owners = vec![Some(0), Some(peer)];
    let mut hs = Synchronization::new(hb, true, 0, owners.clone()).unwrap();
    let mut cs = Synchronization::new(cb, false, peer, owners.clone()).unwrap();
    let mut ha = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut ca = Adapter::new(cb, false, peer, owners).unwrap();
    let mut sampled = [Vec::new(), Vec::new()];
    for (session, sync) in [(&mut left, &mut hs), (&mut right, &mut cs)] {
        session.world.clock.timing[0] = 199;
        session.queue.restore_queued(
            live_commands::Command::PropertyTransfer {
                category: 7,
                owner: 2,
                amount: 500,
            },
            scheduler::ClockDomain::Total,
            200,
            200,
            false,
        );
        sync.enable(&mut session.tasks).unwrap();
        sync.barrier
            .open(&mut session.world.pause, &mut session.tasks)
            .unwrap();
    }
    // The fixture starts at 199, so publish its preceding real authority
    // window first. A brand-new adapter's tick-25 window is already expired.
    ha.pump(
        &host,
        195,
        &mut left.queue,
        &mut left.world.city.categories,
        crate::network::commands::Targets {
            buildings: &left.world.city.buildings,
            businesses: &left.world.city.ledgers.businesses,
            ..crate::network::commands::Targets::default()
        },
        &mut left.world.city.pause,
        &mut left.tasks,
    )
    .unwrap();
    // Real retained task dispatch, real loopback worker/transport, real live
    // queue/pause/account storage and scheduler sample event. No synthetic ACK.
    let until = Instant::now() + Duration::from_secs(15);
    while left.world.clock.timing[0] < 1000
        || right.world.clock.timing[0] < 1000
        || hs.samples.status.mismatch_count == 0
    {
        for (index, session, sync, adapter, worker) in [
            (0, &mut left, &mut hs, &mut ha, &host),
            (1, &mut right, &mut cs, &mut ca, &client),
        ] {
            adapter
                .pump(
                    worker,
                    session.world.clock.timing[0],
                    &mut session.queue,
                    &mut session.world.city.categories,
                    crate::network::commands::Targets {
                        buildings: &session.world.city.buildings,
                        businesses: &session.world.city.ledgers.businesses,
                        ..crate::network::commands::Targets::default()
                    },
                    &mut session.world.city.pause,
                    &mut session.tasks,
                )
                .unwrap();
            session.controls.deadline = adapter.deadline();
            let mut tasks = session.tasks.clone();
            crate::timers::dispatch(&mut tasks, 40, |_, address, tasks| {
                session.tasks.clone_from(tasks);
                match address {
                    0x450410 => {
                        sync.pump(
                            worker,
                            [true, true, false, false, false, false],
                            &mut session.world.city.pause,
                            &mut session.tasks,
                        )?;
                    }
                    0x481680 if session.world.clock.timing[0] < 1000 => {
                        session.tick_observing(&image, &board, |event, clock, city, players| {
                            if event == scheduler::Event::Helper(0x481340) {
                                // Deliberate mismatch comes from an actual engine
                                // supplier, not a manufactured wire checksum.
                                if index == 1 && clock.timing[0] == 400 {
                                    city.ledgers.players[0].cash += 1.;
                                }
                                let before = city.ledgers.players[0].cash;
                                sync.capture(worker, clock.timing[0], city, &roads, players)?;
                                assert_eq!(city.ledgers.players[0].cash, before);
                                sampled[index].push(clock.timing[0]);
                            }
                            Ok(())
                        });
                    }
                    _ => (),
                }
                tasks.clone_from(&session.tasks);
                Ok(())
            })
            .unwrap();
            session.tasks = tasks;
        }
        assert!(
            Instant::now() < until,
            "left {}, right {}, diagnostics {:?}",
            left.world.clock.timing[0],
            right.world.clock.timing[0],
            hs.samples.status
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        sampled,
        [
            vec![200, 400, 600, 800, 1000],
            vec![200, 400, 600, 800, 1000]
        ]
    );
    assert_eq!(left.world.clock.timing[0], 1000);
    assert_eq!(right.world.clock.timing[0], 1000);
    assert_eq!(hs.samples.retained(200).unwrap().sums[0], 10);
    assert_eq!(
        hs.samples.retained(200).unwrap().sums[1],
        1f32.to_bits()
            .wrapping_add(2f32.to_bits())
            .wrapping_add(3f32.to_bits())
    );
    assert_eq!(hs.samples.retained(200).unwrap().sums[4], 20_000);
    assert_eq!(cs.samples.retained(200).unwrap().sums[4], 20_000);
    assert_eq!(left.world.ledgers.players[1].cash, 9500.);
    assert_eq!(right.world.ledgers.players[1].cash, 9500.);
    assert!(!left.world.pause.has_reason());
    assert!(!right.world.pause.has_reason());
    assert!(
        hs.samples
            .status
            .last_mismatches
            .iter()
            .any(|m| m.field == crate::network::synchronization::Field::Players)
    );
}
