//! Real loopback auction lifecycle over the live Runtime/queue/scheduler.
use super::*;
use crate::network::{lobby::worker, synchronization::Synchronization};
use std::{
    thread,
    time::{Duration, Instant},
};

fn descriptor(kind: u32, delayed: bool, values: &[(usize, u32)]) -> [u8; 52] {
    let mut d = [0; 52];
    put(&mut d, 4, kind);
    d[16] = u8::from(delayed);
    for &(at, value) in values {
        put(&mut d, at, value);
    }
    d
}
fn auction_event(
    event: live_commands::Event,
    outgoing: &mut Vec<([u8; 52], u32)>,
    prepared: &mut bool,
) -> std::result::Result<u32, String> {
    match event {
        live_commands::Event::AuctionNetworkBarrier => *prepared = true,
        live_commands::Event::AuctionQueue {
            descriptor,
            additional_delay,
        } => outgoing.push((descriptor, additional_delay)),
        live_commands::Event::PlayerMessageRoute {
            route: crate::player_messages::Route::NetworkRecipient { .. },
            packet,
        } => {
            let mut d = [0; 52];
            d[..48].copy_from_slice(&packet);
            outgoing.push((d, 0));
        }
        _ => (), // Presentation only; no scene assets are required by CI.
    }
    Ok(0)
}

impl TestSession {
    fn auction_frame(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        adapter: &mut Adapter,
        sync: &mut Synchronization,
        worker: &worker::Handle,
        acknowledge: bool,
    ) {
        adapter
            .pump(
                worker,
                self.world.clock.timing[0],
                &mut self.queue,
                &mut self.world.city.categories,
                Targets {
                    buildings: &self.world.city.buildings,
                    businesses: &self.world.city.ledgers.businesses,
                },
                &mut self.world.city.pause,
                &mut self.tasks,
            )
            .unwrap();
        self.controls.deadline = adapter.deadline();
        self.controls.network_override = adapter.is_host();
        if acknowledge {
            let mut tasks = self.tasks.clone();
            crate::timers::dispatch(&mut tasks, 40, |_, address, tasks| {
                if address == 0x450410 {
                    sync.pump(
                        worker,
                        crate::network::synchronization::active_humans(
                            &self.roster.manager.players,
                        ),
                        &mut self.world.city.pause,
                        tasks,
                    )?;
                }
                Ok(())
            })
            .unwrap();
            self.tasks = tasks;
        }
        if self.world.city.pause.has_reason() {
            return;
        }
        let mut messages = crate::player_messages::Context {
            arena: crate::player_messages::Arena { next: 0x100000 },
            current_player: self.roster.status.current_player,
            network: [true, adapter.is_host()],
            prior: [0; 48],
        };
        // Retain the allocator across frames as a real Session does.
        messages.arena.next += self
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.auxiliaries.len() as u32 * 0x100)
            .sum::<u32>();
        let mut utilities = economy::Utilities::new([0; 4]);
        let mut deck = crate::event_cards::Deck::new([0; 3]);
        let mut outgoing = Vec::new();
        let mut prepared = false;
        let city = &mut self.world.city;
        scheduler::advance(
            &mut self.world.clock,
            &mut self.controls,
            |event, clock, _| {
                if event == scheduler::Event::Helper(0x4bac90) {
                    crate::network::commands::pause_at_limit(&mut city.pause, &mut self.tasks)?;
                }
                if !matches!(event, scheduler::Event::Helper(0x480ec0 | 0x465230)) {
                    return Ok(0);
                }
                let mut runtime = live_commands::Runtime {
                    city,
                    roster: &mut self.roster,
                    population: &mut self.population,
                    utilities: &mut utilities,
                    board,
                    event_cards: &mut deck,
                    message_context: &mut messages,
                };
                if event == scheduler::Event::Helper(0x465230) {
                    runtime.advance_auction(clock, [true, adapter.is_host()], |e, _| {
                        auction_event(e, &mut outgoing, &mut prepared)
                    })?;
                } else {
                    self.queue
                        .drain(&mut [clock.timing[0], clock.timing[1]], |command| {
                            if let live_commands::Command::Auction { descriptor } = command {
                                let kind = crate::player_manager::word(descriptor, 4);
                                self.applied.push(kind as u8);
                                if matches!(kind, 0x1e..=0x20) {
                                    return runtime.auction_phase(
                                        descriptor,
                                        clock,
                                        [true, adapter.is_host()],
                                        |e, _| auction_event(e, &mut outgoing, &mut prepared),
                                    );
                                }
                            }
                            runtime.apply_with_announcement(
                                command,
                                &mut live_commands::CommandContext {
                                    image,
                                    clock,
                                    accounting_enabled: true,
                                    notifications: false,
                                    tutorial_state: 0,
                                    network_enabled: true,
                                    network_override: adapter.is_host(),
                                    utility_factor: 0.,
                                },
                                &mut TestGeometry,
                                None,
                                |e, _| auction_event(e, &mut outgoing, &mut prepared).map(|_| ()),
                            )
                        })?;
                    if prepared {
                        sync.auction_prepared(&mut runtime.city.pause, &mut self.tasks)?;
                        prepared = false;
                    }
                }
                Ok(0)
            },
        )
        .unwrap();
        for (d, additional) in outgoing {
            adapter
                .submit_auction(
                    worker,
                    d,
                    additional,
                    &mut self.queue,
                    &mut self.world.city.categories,
                    self.world.clock.timing[0],
                )
                .unwrap();
        }
    }
}

#[test]
fn loopback_auction_prepared_barrier_bid_closure_and_city_resume() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    let (host, client, peer) = worker::tests::playing_pair(false);
    let hb = host
        .bind_world(left.world.topology.epoch().serial())
        .unwrap();
    let cb = client
        .bind_world(right.world.topology.epoch().serial())
        .unwrap();
    let peers = vec![Some(0), Some(peer)];
    let mut ha = Adapter::new(hb, true, 0, peers.clone()).unwrap();
    let mut ca = Adapter::new(cb, false, peer, peers.clone()).unwrap();
    let mut hs = Synchronization::new(hb, true, 0, peers.clone()).unwrap();
    let mut cs = Synchronization::new(cb, false, peer, peers).unwrap();
    right.roster.status.current_player = right.roster.manager.players[1].reference;
    for (s, sync) in [(&mut left, &mut hs), (&mut right, &mut cs)] {
        s.world.clock.timing[5] = 1;
        sync.enable(&mut s.tasks).unwrap();
    }
    // Admission and its 120-second announcement both use real authority routing.
    ha.submit_auction(
        &host,
        descriptor(0x1d, false, &[(20, 7), (24, 0), (28, 100)]),
        0,
        &mut left.queue,
        &mut left.world.city.categories,
        100,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    let mut held = 0;
    let mut held_tick = None;
    let mut bid = false;
    let mut passed = false;
    let mut closed = None;
    loop {
        left.auction_frame(&image, &board, &mut ha, &mut hs, &host, true);
        // A live connected peer which has prepared its room but does not pump
        // synchronization cannot manufacture an acknowledgment for the host.
        let hold = hs.barrier.active && cs.barrier.active && held < 100;
        if hold {
            held += 1;
        }
        right.auction_frame(&image, &board, &mut ca, &mut cs, &client, !hold);
        if hold {
            assert!(hs.barrier.active);
            let tick = *held_tick.get_or_insert(left.world.clock.timing[0]);
            assert_eq!(left.world.clock.timing[0], tick);
            assert_eq!(left.world.city.pause.bytes[2], 1);
        }
        if held == 100
            && !hs.barrier.active
            && !cs.barrier.active
            && !bid
            && crate::player_manager::word(&left.world.city.auction.controller.record, 0x74) == 2
            && crate::player_manager::word(&right.world.city.auction.controller.record, 0x74) == 2
        {
            ca.submit_auction(
                &client,
                descriptor(0x1f, false, &[(20, 0), (24, 1), (28, 500)]),
                0,
                &mut right.queue,
                &mut right.world.city.categories,
                right.world.clock.timing[0],
            )
            .unwrap();
            bid = true;
        }
        if bid
            && !passed
            && [&left, &right].iter().all(|s| {
                crate::player_manager::word(&s.world.city.auction.controller.record, 0x6c) == 500
            })
        {
            ha.submit_auction(
                &host,
                descriptor(0x1f, false, &[(20, 1), (24, 0)]),
                0,
                &mut left.queue,
                &mut left.world.city.categories,
                left.world.clock.timing[0],
            )
            .unwrap();
            passed = true;
        }
        if passed
            && left.world.city.categories[7][5] == 2
            && right.world.city.categories[7][5] == 2
            && left.world.city.auction.controller.active_count == 0
            && right.world.city.auction.controller.active_count == 0
        {
            let ticks =
                *closed.get_or_insert((left.world.clock.timing[0], right.world.clock.timing[0]));
            if left.world.clock.timing[0] > ticks.0 + 30
                && right.world.clock.timing[0] > ticks.1 + 30
            {
                break;
            }
        }
        assert!(
            Instant::now() < until,
            "auction stalled: ticks {:?}, held {held}, bid {bid}, passed {passed}, phases {:?}, active {:?}",
            (left.world.clock.timing[0], right.world.clock.timing[0]),
            (
                crate::player_manager::word(&left.world.city.auction.controller.record, 0x74),
                crate::player_manager::word(&right.world.city.auction.controller.record, 0x74)
            ),
            (hs.barrier.active, cs.barrier.active)
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(held, 100);
    assert!(!hs.barrier.active && !cs.barrier.active);
    assert_eq!(left.world.city.pause.bytes[2], 0);
    assert_eq!(right.world.city.pause.bytes[2], 0);
    assert_eq!(left.world.clock.timing[5], 1);
    assert_eq!(right.world.clock.timing[5], 1);
    assert_eq!(
        left.world.city.categories[7],
        right.world.city.categories[7]
    );
    assert_eq!(
        left.world.city.auction.controller.record[..0x78],
        right.world.city.auction.controller.record[..0x78]
    );
    assert_eq!(
        left.world.city.ledgers.players[1].cash,
        right.world.city.ledgers.players[1].cash
    );
    assert_eq!(left.world.city.ledgers.players[1].cash, 9500.);
    for s in [&left, &right] {
        assert_eq!(s.applied.iter().filter(|k| **k == 0x20).count(), 1);
    }
}

#[test]
fn auction_commands_require_authority_or_the_bound_participant_and_keep_native_timing() {
    let (owner, binding, peer) = owner();
    let mut adapter = Adapter::new(binding, true, 0, vec![Some(0), Some(4)]).unwrap();
    let start = Request::auction(0, 0, descriptor(0x1e, true, &[]), 3000);
    let Message::CommandCommit(commit) = adapter
        .request(&owner, start.clone(), 100, Targets::default())
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(commit.due, 3125);
    assert_eq!(
        crate::network::codec::decode(
            &crate::network::codec::encode(&crate::network::Frame {
                session: [7; 16],
                sequence: 0,
                message: Message::CommandCommit(commit.clone()),
            })
            .unwrap()
        )
        .unwrap()
        .message,
        Message::CommandCommit(commit)
    );
    for request in [
        start,
        Request::auction(1, 0, descriptor(0x20, true, &[]), 0),
        Request::auction(
            2,
            1,
            descriptor(0x1f, false, &[(20, 0), (24, 0), (28, 10)]),
            0,
        ),
        Request::auction(3, 2, descriptor(0x1f, false, &[(20, 4), (24, 3)]), 0),
    ] {
        let admitted = ingress(
            &owner,
            binding,
            peer,
            Message::CommandRequest(request.clone()),
            request.serial,
        )
        .unwrap();
        assert_eq!(
            adapter.receive(&owner, admitted, 101, Targets::default()),
            Err(Error::Peer)
        );
    }
    let bid = Request::auction(
        4,
        2,
        descriptor(0x1f, false, &[(20, 0), (24, 1), (28, 10)]),
        0,
    );
    let admitted = ingress(
        &owner,
        binding,
        peer,
        Message::CommandRequest(bid.clone()),
        4,
    )
    .unwrap();
    let Message::CommandCommit(commit) = adapter
        .receive(&owner, admitted, 101, Targets::default())
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        commit.due, 0,
        "flag-zero bids do not acquire a scheduling lead"
    );
    let replay = ingress(
        &owner,
        binding,
        peer,
        Message::CommandRequest(bid.clone()),
        5,
    )
    .unwrap();
    assert_eq!(
        adapter.receive(&owner, replay, 102, Targets::default()),
        Err(Error::Sequence)
    );
    for action in [5, u32::MAX] {
        let bad = Request::auction(5, 2, descriptor(0x1f, false, &[(20, action), (24, 1)]), 0);
        assert_eq!(bad.validate(), Err(Error::Row));
    }
    let mut delayed_bid = bid;
    delayed_bid.payload[16] = 1;
    assert_eq!(delayed_bid.validate(), Err(Error::Row));
    let mut truncated = Request::auction(5, 0, descriptor(0x1e, true, &[]), 0);
    truncated.payload.pop();
    assert_eq!(truncated.validate(), Err(Error::Unsupported));
    let unsold = Request::auction(5, 0, descriptor(0x16, true, &[(20, 100), (24, 7)]), 0);
    assert_eq!(adapter.authorize_auction(0, &unsold), Ok(()));
    assert_eq!(adapter.authorize_auction(4, &unsold), Err(Error::Peer));
}

#[test]
fn offline_auction_room_keeps_local_activation_and_closure_unpaused() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut session = TestSession::new(&image, &board);
    let mut messages = crate::player_messages::Context {
        arena: crate::player_messages::Arena { next: 0x100000 },
        current_player: session.roster.status.current_player,
        network: [false, false],
        prior: [0; 48],
    };
    let mut deck = crate::event_cards::Deck::new([0; 3]);
    let mut utilities = economy::Utilities::new([0; 4]);
    let mut runtime = live_commands::Runtime {
        city: &mut session.world.city,
        roster: &mut session.roster,
        population: &mut session.population,
        utilities: &mut utilities,
        board: &board,
        event_cards: &mut deck,
        message_context: &mut messages,
    };
    let mut outgoing = vec![];
    let mut prepared = false;
    runtime
        .apply_with_announcement(
            &live_commands::Command::Auction {
                descriptor: descriptor(0x1d, false, &[(20, 7), (24, 0), (28, 100)]),
            },
            &mut live_commands::CommandContext {
                image: &image,
                clock: &mut session.world.clock,
                accounting_enabled: true,
                notifications: false,
                tutorial_state: 0,
                network_enabled: false,
                network_override: false,
                utility_factor: 0.,
            },
            &mut TestGeometry,
            None,
            |e, _| auction_event(e, &mut outgoing, &mut prepared).map(|_| ()),
        )
        .unwrap();
    let mut normal_speed = 0;
    runtime
        .auction_phase(
            &descriptor(0x1e, true, &[]),
            &mut session.world.clock,
            [false, false],
            |e, _| {
                if matches!(
                    e,
                    live_commands::Event::AuctionActivation(
                        crate::ai_messages::offers::auction::activation::Call::NormalSpeed
                    )
                ) {
                    normal_speed += 1;
                }
                auction_event(e, &mut outgoing, &mut prepared)
            },
        )
        .unwrap();
    assert_eq!(normal_speed, 1);
    assert!(!prepared);
    assert!(!runtime.city.pause.has_reason());
    for d in [
        descriptor(0x1f, false, &[(20, 0), (24, 1), (28, 500)]),
        descriptor(0x1f, false, &[(20, 1), (24, 0)]),
        descriptor(0x20, true, &[]),
    ] {
        runtime
            .auction_phase(&d, &mut session.world.clock, [false, false], |e, _| {
                auction_event(e, &mut outgoing, &mut prepared)
            })
            .unwrap();
    }
    assert_eq!(runtime.city.auction.controller.active_count, 0);
    assert_eq!(session.world.clock.timing[5], 1);
    assert!(!runtime.city.pause.has_reason());
    assert!(!prepared);
    assert_eq!(
        outgoing
            .iter()
            .filter(|(d, _)| crate::player_manager::word(d, 4) == 0x16)
            .count(),
        1
    );
}

#[test]
fn auction_closure_deferred_by_a_published_deadline_keeps_its_phase_order() {
    let (host, client, peer) = worker::tests::playing_pair(false);
    let binding = host.bind_world(1).unwrap();
    client.bind_world(1).unwrap();
    let mut adapter = Adapter::new(binding, true, 0, vec![Some(0), Some(peer)]).unwrap();
    adapter.closed_tick = Some(100);
    let mut queue = live_commands::Queue::default();
    let mut categories = [[0; 380]; 52];
    adapter
        .submit_auction(
            &host,
            descriptor(0x20, true, &[]),
            0,
            &mut queue,
            &mut categories,
            100,
        )
        .unwrap();
    adapter
        .submit_auction(
            &host,
            descriptor(0x1f, false, &[(20, 4), (24, 5)]),
            0,
            &mut queue,
            &mut categories,
            100,
        )
        .unwrap();
    assert!(adapter.auction_control_pending());
    assert!(queue.snapshot().unwrap().is_empty());
    adapter
        .pump(
            &host,
            101,
            &mut queue,
            &mut categories,
            Targets::default(),
            &mut crate::pause::State::default(),
            &mut [],
        )
        .unwrap();
    assert!(!adapter.auction_control_pending());
    assert_eq!(adapter.next_commit, 2);
    assert_eq!(adapter.next_enqueued, 2);
    let until = Instant::now() + Duration::from_secs(10);
    let mut commits = vec![];
    while commits.len() < 2 {
        if let Some(ingress) = client.poll_gameplay()
            && let Message::CommandCommit(commit) = ingress.message
        {
            commits.push(commit);
        }
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        commits
            .iter()
            .map(|c| (c.order, c.request.serial, c.request.kind, c.due))
            .collect::<Vec<_>>(),
        [(0, 0, 0x20, 126), (1, 1, 0x1f, 0)]
    );
}
