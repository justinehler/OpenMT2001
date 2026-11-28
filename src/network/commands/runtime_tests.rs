//! Synthetic live-world consumer, independent of any supplied game data.
use super::*;
use crate::{
    economy, level_runtime, level_startup, live_commands, player_manager, resident, scheduler,
};

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn image_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 0x200000 + 0x200];
    bytes[..2].copy_from_slice(b"MZ");
    put(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut bytes, 0xb4, 0x400000);
    put(&mut bytes, 0xb8 + 12, 0);
    put(&mut bytes, 0xb8 + 16, 0x200000);
    put(&mut bytes, 0xb8 + 20, 0x200);
    // One real map intersection, no buildings/residents; all prices/constants
    // are synthetic zeroes. No original tables or executable bytes are used.
    for y in 1..4 {
        for x in 1..3 {
            bytes[0x1366b8 + 0x200 + y * 113 + x] = b'+';
        }
    }
    bytes
}
struct TestGeometry;
impl crate::building_pool::GeometryBinding for TestGeometry {
    fn prepare(&mut self, _: usize, record: &mut [u8; 916]) -> std::result::Result<(), String> {
        // No presentation resources or entrances; retain the real building record.
        record[0x4c..0x50].fill(0);
        Ok(())
    }
    fn model_height(&mut self, _: u32) -> std::result::Result<f32, String> {
        Ok(20.)
    }
}
struct TestSession {
    world: level_startup::World,
    roster: level_startup::Roster,
    population: level_runtime::Population,
    queue: live_commands::Queue,
    tasks: Vec<[u8; 20]>,
    controls: scheduler::Controls,
    postings: Vec<economy::AccountPosting>,
}
impl TestSession {
    fn new(image: &crate::pe::PeImage<'_>, board: &crate::board::Board) -> Self {
        let timetable = [
            "ADULT",
            "UNHOUSED",
            "SENIOR_CITIZENS",
            "CHILDREN_SCHOOL",
            "TEENAGER_UNI",
            "TOURIST",
        ]
        .map(|category| format!("Add_Daily_Objective 0 {category}\n"))
        .concat();
        let configuration =
            resident::Configuration::before_defaults("", &timetable, image).unwrap();
        let creation = economy::ResidentCreationState {
            auxiliary: resident::AuxiliaryPool::for_residents(),
            initialization_rng: resident::InitializationRng {
                constructor: 0,
                movement: 0,
                objective: 0,
            },
            identity_rng: 0,
            preference_rng: 0,
        };
        let mut world = level_startup::World::fresh(image, board, configuration, creation).unwrap();
        let players = (0..2)
            .map(|slot| {
                let mut bytes = vec![0; 0x224];
                bytes[0x10] = 1;
                bytes[0x1bc] = slot;
                put(&mut bytes, 4, 10000f32.to_bits());
                player_manager::Player {
                    reference: 0x1000 + u32::from(slot) * 0x1000,
                    bytes,
                    auxiliaries: vec![],
                }
            })
            .collect();
        let mut roster = level_startup::Roster {
            manager: player_manager::Manager {
                bytes: [0; 40],
                players,
            },
            profiles: vec![],
            ai_ownership: Default::default(),
            category_counts: [0; 52 * 45],
            liquidation_targets: Default::default(),
            proposal_identity: 0,
            scripts: crate::ai_script::AiScript::new().unwrap(),
            status: Default::default(),
        };
        roster.collect_accounts(&mut world.city).unwrap();
        roster.status.current_player = roster.manager.players[0].reference;
        world.clock.timing[0] = 100;
        world.clock.timing[5] = 0;
        let population = level_runtime::Population {
            shopping: crate::shopping::Topology::from_runtime(
                &world.buildings,
                &world.categories,
                0x591318,
                board,
            )
            .unwrap(),
            housing: crate::housing::Topology::from_runtime(
                &world.buildings,
                &world.categories,
                0x591318,
            )
            .unwrap(),
            settings: crate::shopping::Settings::from_runtime(
                &world.catalog,
                &world.commodities,
                &world.configuration,
            )
            .unwrap(),
            housing_settings: crate::housing::Settings::from_parameters("", image).unwrap(),
            routes: world.category_routes(board),
            bindings: vec![],
            animations: Default::default(),
            animation_settings: resident::ResidentAnimationSettings::from_image(image).unwrap(),
            properties: economy::PropertyIncome {
                owners: [0; 4],
                totals: [0.; 4],
            },
            market: crate::market::Cache {
                dirty: true,
                prices: [0.; 32],
                rent: 0.,
            },
            business_counts: crate::player_metrics::BusinessCounts {
                age: 0,
                values: [0; 6],
            },
        };
        Self {
            world,
            roster,
            population,
            queue: Default::default(),
            tasks: crate::timers::startup_tasks([0; 20]),
            controls: scheduler::Controls {
                iterations: 1,
                abort: false,
                alternate_script_active: false,
                network_enabled: true,
                network_override: false,
                deadline: 125,
                before_six_sound: 0,
                gate_32: false,
                gate_32_offset: false,
            },
            postings: vec![],
        }
    }
    fn tick(&mut self, image: &crate::pe::PeImage<'_>, board: &crate::board::Board) {
        self.tick_observing(image, board, |_, _, _, _| Ok(()));
    }
    fn tick_observing(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        mut observe: impl FnMut(
            scheduler::Event,
            &crate::clock::Clock,
            &mut level_startup::City,
            &[player_manager::Player],
        ) -> std::result::Result<(), String>,
    ) {
        if self.world.pause.has_reason() {
            return;
        }
        let mut messages = crate::player_messages::Context {
            arena: crate::player_messages::Arena { next: 0x10000 },
            current_player: self.roster.status.current_player,
            network: [true, false],
            prior: [0; 48],
        };
        let mut deck = crate::event_cards::Deck::new([0, 0, 0]);
        let mut utilities = economy::Utilities::new([0; 4]);
        let city = &mut self.world.city;
        scheduler::advance(
            &mut self.world.clock,
            &mut self.controls,
            |event, clock, _| {
                observe(event, clock, city, &self.roster.manager.players)?;
                if event == scheduler::Event::Helper(0x480ec0) {
                    let mut runtime = live_commands::Runtime {
                        city,
                        roster: &mut self.roster,
                        population: &mut self.population,
                        utilities: &mut utilities,
                        board,
                        event_cards: &mut deck,
                        message_context: &mut messages,
                    };
                    let mut clocks = [clock.timing[0], clock.timing[1]];
                    self.queue.drain(&mut clocks, |command| {
                        runtime.apply(
                            command,
                            &mut live_commands::CommandContext {
                                image,
                                clock,
                                accounting_enabled: true,
                                notifications: false,
                                tutorial_state: 0,
                                network_enabled: true,
                                network_override: false,
                                utility_factor: 0.,
                            },
                            &mut TestGeometry,
                            |event| {
                                if let live_commands::Event::AccountPosted(posting) = event {
                                    self.postings.push(posting);
                                }
                                Ok(())
                            },
                        )
                    })?;
                }
                if event == scheduler::Event::Helper(0x4bac90) {
                    crate::network::commands::pause_at_limit(&mut city.pause, &mut self.tasks)?;
                }
                Ok(0)
            },
        )
        .unwrap();
    }
}
#[test]
fn two_live_runtimes_apply_exact_property_and_cash_once_at_125_and_pause_same_tasks() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    let (owner, binding, peer) = owner();
    let mut host = Adapter::new(binding, true, 0, vec![Some(0), Some(4)]).unwrap();
    let mut client = Adapter::new(binding, false, 4, vec![Some(0), Some(4)]).unwrap();
    let request = Request::transfer(
        0,
        Transfer {
            owner: 2,
            ..transfer()
        },
    );
    let admitted = ingress(&owner, binding, peer, Message::CommandRequest(request), 0).unwrap();
    let Message::CommandCommit(commit) = host.receive(&owner, admitted, 100).unwrap() else {
        panic!()
    };
    host.enqueue(
        &commit,
        &mut left.queue,
        &mut left.world.city.categories,
        100,
    )
    .unwrap();
    client
        .enqueue(
            &commit,
            &mut right.queue,
            &mut right.world.city.categories,
            100,
        )
        .unwrap();
    let before_tasks = right.tasks.clone();
    for session in [&mut left, &mut right] {
        while session.world.clock.timing[0] < 124 {
            session.tick(&image, &board);
        }
        assert_eq!(session.world.categories[7][5], 0);
        assert_eq!(session.world.ledgers.players[1].cash, 10000.);
        assert!(session.postings.is_empty());
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 125);
        assert_eq!(session.world.categories[7][5], 2);
        assert_eq!(session.world.ledgers.players[1].cash, 9500.);
        assert_eq!(session.world.ledgers.players[1].owned_categories[7], 1);
        assert_eq!(session.postings.len(), 1);
        assert!(session.queue.snapshot().unwrap().is_empty());
        assert_eq!(session.world.pause.bytes[1], 1);
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 125);
        assert_eq!(session.postings.len(), 1);
    }
    for (old, paused) in before_tasks.iter().zip(&right.tasks) {
        assert_eq!(&old[1..], &paused[1..]);
        if u32::from_le_bytes(old[4..8].try_into().unwrap()) & 2 != 0 {
            assert_eq!(paused[0], 0);
        }
    }
    assert_eq!(left.world.city.categories, right.world.categories);
    assert_eq!(
        left.world.ledgers.players[1].cash,
        right.world.ledgers.players[1].cash
    );
}

#[test]
fn loopback_client_and_host_ai_construction_apply_once_at_125_with_original_quotes() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let mut bytes = image_bytes();
    // Synthetic vtable for the periodic player dispatcher.
    put(&mut bytes, 0x308, 0x466ca0);
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(true);
    let hb = host
        .bind_world(left.world.topology.epoch().serial())
        .unwrap();
    let cb = client
        .bind_world(right.world.topology.epoch().serial())
        .unwrap();
    // Compact owner1 is the host human, owner2 the client, owner3 the AI.
    let owners = vec![Some(0), Some(peer), None];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, owners).unwrap();
    // Add a real third account for the compact AI owner, on both runtimes.
    for session in [&mut left, &mut right] {
        let mut player = player_manager::Player {
            reference: 0x3000,
            bytes: session.roster.manager.players[0].bytes.clone(),
            auxiliaries: vec![],
        };
        player.bytes[0x1bc] = 2;
        put(&mut player.bytes, 0x154, 1);
        session.roster.manager.players.push(player);
        session
            .roster
            .collect_accounts(&mut session.world.city)
            .unwrap();
    }
    let command = construction(2);
    replica
        .submit(
            &client,
            live_commands::Command::Build(command.clone()),
            &mut right.queue,
            &mut right.world.city.categories,
            100,
        )
        .unwrap();
    assert!(right.queue.snapshot().unwrap().is_empty());
    assert_eq!(right.world.categories[7][0x9c], 0);
    let mut ai = construction(3);
    ai.category = 8;
    ai.components = [200., 30., 4., 1.];
    for session in [&mut left, &mut right] {
        for player in &mut session.roster.manager.players {
            put(&mut player.bytes, 0, 0x400100);
        }
    }
    for (session, adapter, worker, expected) in [
        (&mut left, &mut authority, &host, 1),
        (&mut right, &mut replica, &client, 0),
    ] {
        let mut controls = crate::player_tick::Controls {
            network_enabled: true,
            network_override: false,
            current_player: session.roster.status.current_player,
        };
        adapter.configure_players(&mut controls);
        let mut decisions = 0;
        crate::player_tick::advance(
            &mut session.roster.manager,
            &mut controls,
            &image,
            |event, _, _| {
                if matches!(
                    event,
                    crate::player_tick::Event::Controller {
                        player: 0x3000,
                        target: 0x466ca0
                    }
                ) {
                    decisions += 1;
                    adapter.submit(
                        worker,
                        live_commands::Command::Build(ai.clone()),
                        &mut session.queue,
                        &mut session.world.city.categories,
                        100,
                    )?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(decisions, expected);
    }
    // The replica cannot submit the same AI owner through its local path.
    assert!(
        replica
            .submit(
                &client,
                live_commands::Command::Build(ai.clone()),
                &mut right.queue,
                &mut right.world.city.categories,
                100
            )
            .is_err()
    );
    let limit = Instant::now() + Duration::from_secs(60);
    // Wait for this request before publishing tick100's deadline. In live play
    // a request that arrives after that publication correctly waits a tick.
    let ingress = loop {
        if let Some(ingress) = host.poll_gameplay() {
            break ingress;
        }
        assert!(
            Instant::now() < limit,
            "client construction request did not arrive"
        );
        thread::sleep(Duration::from_millis(5));
    };
    let message = authority
        .receive(host.gameplay_owner(), ingress, 100)
        .unwrap();
    let Message::CommandCommit(commit) = &message else {
        panic!()
    };
    authority
        .enqueue(
            commit,
            &mut left.queue,
            &mut left.world.city.categories,
            100,
        )
        .unwrap();
    host.send_gameplay(hb, None, message).unwrap();
    loop {
        authority
            .pump(
                &host,
                100,
                &mut left.queue,
                &mut left.world.city.categories,
                &mut left.world.city.pause,
                &mut left.tasks,
            )
            .unwrap();
        replica
            .pump(
                &client,
                100,
                &mut right.queue,
                &mut right.world.city.categories,
                &mut right.world.city.pause,
                &mut right.tasks,
            )
            .unwrap();
        if left.queue.snapshot().unwrap().len() == 2 && right.queue.snapshot().unwrap().len() == 2 {
            break;
        }
        assert!(
            Instant::now() < limit,
            "construction commits did not reach both peers"
        );
        thread::sleep(Duration::from_millis(5));
    }
    for session in [&mut left, &mut right] {
        let queued = session.queue.snapshot().unwrap();
        assert_eq!(queued.len(), 2);
        for ((_, due, inner, _, queued), expected) in queued.iter().zip([&ai, &command]) {
            assert_eq!((*due, *inner), (125, 125));
            let live_commands::Command::Build(build) = queued else {
                panic!()
            };
            assert_eq!(build.packet(), expected.packet());
        }
        assert_eq!(session.world.categories[7][0x9c], 2);
        assert_eq!(session.world.categories[8][0x9c], 2);
        // Deliberately change each receiver's current quote scratch. Due
        // application must debit the transmitted quote, never these prices.
        session.world.city.construction_components = [9999.; 4];
        while session.world.clock.timing[0] < 124 {
            session.tick(&image, &board);
        }
        assert_eq!(
            session
                .world
                .buildings
                .iter()
                .filter(|row| row[0x3f] != 0)
                .count(),
            0
        );
        assert_eq!(session.world.ledgers.players[1].cash, 10000.);
        assert_eq!(session.world.ledgers.players[2].cash, 10000.);
        assert!(session.postings.is_empty());
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 125);
        assert_eq!(
            session
                .world
                .buildings
                .iter()
                .filter(|row| row[0x3f] != 0)
                .count(),
            2
        );
        assert_eq!(session.world.pending_world.count, 2);
        assert_eq!(session.world.ledgers.players[1].cash, 9865.);
        assert_eq!(session.world.ledgers.players[2].cash, 9765.);
        assert_eq!(session.postings.len(), 8);
        assert!(session.queue.snapshot().unwrap().is_empty());
        session.controls.deadline = 150;
        session.world.city.pause.bytes[1] = 0;
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 126);
        assert_eq!(session.postings.len(), 8);
        assert_eq!(session.world.pending_world.count, 2);
    }
    assert_eq!(left.world.buildings, right.world.buildings);
    assert_eq!(left.world.city.categories, right.world.categories);
    assert_eq!(left.world.pending_world, right.world.pending_world);
}

#[test]
fn connected_hour_boundary_keeps_shared_accounts_and_rng_with_ai_on_both_peers() {
    use crate::{clock, hourly, player_manager::word};
    let mut bytes = image_bytes();
    put(&mut bytes, 0x308, 0x466ca0);
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    let (_, binding, _) = owner();
    let owners = vec![Some(0), Some(4), None];
    let host = Adapter::new(binding, true, 0, owners.clone()).unwrap();
    let client = Adapter::new(binding, false, 4, owners).unwrap();
    let resident_rngs = |session: &TestSession| {
        let rng = &session.world.creation;
        [
            rng.initialization_rng.constructor,
            rng.initialization_rng.movement,
            rng.initialization_rng.objective,
            rng.identity_rng,
            rng.preference_rng,
        ]
    };
    for (session, current) in [(&mut left, 0), (&mut right, 1)] {
        session.roster.manager.players.push(player_manager::Player {
            reference: 0x3000,
            bytes: vec![0; 0x1e50],
            auxiliaries: vec![],
        });
        for (slot, player) in session.roster.manager.players.iter_mut().enumerate() {
            put(&mut player.bytes, 0, 0x400100);
            put(&mut player.bytes, 4, 10000f32.to_bits());
            player.bytes[0x10] = 1;
            player.bytes[0x1bc] = slot as u8;
            put(&mut player.bytes, 0x154, u32::from(slot == 2));
            let reference = player.reference + 0x800;
            put(&mut player.bytes, 0x21c, reference);
            let mut auxiliary = player_manager::Auxiliary {
                reference,
                bytes: [0; 0x30],
            };
            put(&mut auxiliary.bytes, 0, 10);
            put(&mut auxiliary.bytes, 0x14, player.reference);
            put(&mut auxiliary.bytes, 0x1c, 123);
            put(&mut auxiliary.bytes, 0x20, 0x343fd);
            put(&mut auxiliary.bytes, 0x24, 0x269ec3);
            player.auxiliaries.push(auxiliary);
        }
        let ai = &mut session.roster.manager.players[2];
        put(&mut ai.bytes, 0x224, 456);
        put(&mut ai.bytes, 0x228, 0x343fd);
        put(&mut ai.bytes, 0x22c, 0x269ec3);
        put(&mut ai.bytes, 0x13e0, 1);
        put(&mut ai.bytes, 0x143c, 2);
        put(&mut ai.bytes, 0x1e14, 1);
        session
            .roster
            .collect_accounts(&mut session.world.city)
            .unwrap();
        session.roster.status.current_player = session.roster.manager.players[current].reference;
        // Typed account changes precede the hour; raw records are deliberately
        // stale until the real 470b80 supplier publishes them on each peer.
        for account in &mut session.world.city.ledgers.players {
            account.post(-17.25, 0xc).unwrap();
            account.owned_categories[7] = 1;
            account.retail_counters = [123, 6];
            account.housing_counter = 50;
            account.replenishment_modifier = 2;
        }
        session.world.city.creation.initialization_rng.constructor = 11;
        session.world.city.creation.initialization_rng.movement = 22;
        session.world.city.creation.initialization_rng.objective = 33;
        session.world.city.creation.identity_rng = 44;
        session.world.city.creation.preference_rng = 55;
    }
    let before_rng = resident_rngs(&left);
    let profiles = [[0; 0x650]; 10];
    for (session, adapter, expected_decisions) in [(&mut left, &host, 1), (&mut right, &client, 0)]
    {
        let mut controls = crate::player_tick::Controls {
            network_enabled: false,
            network_override: false,
            current_player: session.roster.status.current_player,
        };
        adapter.configure_players(&mut controls);
        let mut decisions = 0;
        crate::player_tick::advance(
            &mut session.roster.manager,
            &mut controls,
            &image,
            |event, manager, _| {
                if matches!(
                    event,
                    crate::player_tick::Event::Controller { player: 0x3000, .. }
                ) {
                    // Original47d5f0 consumes only this AI player's embedded RNG.
                    manager.players[2].initial_count(&profiles, 100)?;
                    decisions += 1;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(decisions, expected_decisions);
        let mut hourly = hourly::Controls {
            suppressed: false,
            world_mode: 1,
            before_six_sound: 0,
            network_enabled: false,
            network_override: false,
        };
        adapter.configure_hourly(&mut hourly);
        let mut calendar = clock::Controls {
            years_per_day: 1,
            suppress_rollover_query: false,
            rollover_events_enabled: true,
            autosave_disabled: true,
        };
        let mut passes = 0;
        let mut ai_receiver_ticks = 0;
        session
            .world
            .clock
            .try_advance_hour(&mut calendar, |event, clock, _| {
                if event == clock::Event::Hourly {
                    hourly::advance_world(
                        clock,
                        &mut hourly,
                        &mut session.world.city,
                        |event, clock, _, city| {
                            match event {
                                hourly::Event::CurrentPlayerUpdate => {
                                    assert!(
                                        session
                                            .roster
                                            .advance_hourly_account(city, None)?
                                            .is_none()
                                    );
                                    // A synchronous account callback can change the typed
                                    // ledger after its publication. The later AI helper
                                    // must publish again, including on the client.
                                    city.ledgers.players[0].post(-2.5, 7)?;
                                }
                                hourly::Event::PlayerUpdate(slot) => {
                                    ai_receiver_ticks += 1;
                                    assert!(
                                        session
                                            .roster
                                            .advance_hourly_account(city, Some(slot))?
                                            .is_none()
                                    );
                                }
                                hourly::Event::Helper(0x470b80) => {
                                    session.roster.advance_hourly_ai(
                                        city,
                                        &image,
                                        clock.hour(),
                                        |_, _, _, _| {
                                            panic!("inactive event auxiliary produced a decision")
                                        },
                                    )?;
                                    passes += 1;
                                }
                                _ => (),
                            }
                            Ok(false)
                        },
                    )?;
                }
                Ok::<_, String>(false)
            })
            .unwrap();
        assert_eq!(session.world.clock.hour(), 10);
        assert_eq!(passes, 1); // Clients must run the complete470b80 pass too.
        assert_eq!(ai_receiver_ticks, expected_decisions);
        assert_eq!(resident_rngs(session), before_rng);
        let ai = &session.roster.manager.players[2];
        assert_eq!(word(&ai.bytes, 0x13e0), 2);
        assert_eq!(word(&ai.bytes, 0x143c), 1);
        assert_eq!(word(&ai.bytes, 0x1e14), 0);
        for (slot, (player, account)) in session
            .roster
            .manager
            .players
            .iter()
            .zip(&session.world.ledgers.players)
            .enumerate()
        {
            let mut expected = player.bytes.clone();
            account.write_runtime(&mut expected).unwrap();
            assert_eq!(&player.bytes[4..0x1b8], &expected[4..0x1b8]);
            assert_eq!(account.cash, if slot == 0 { 9980.25 } else { 9982.75 });
            assert_eq!(account.owned_categories[7], 1);
            assert_eq!(account.retail_counters, [123, 6]);
            assert_eq!(account.housing_counter, 50);
        }
    }
    for (host, client) in left
        .roster
        .manager
        .players
        .iter()
        .zip(&right.roster.manager.players)
    {
        // Exclude each human's local auxiliary/UI and the AI's private runtime.
        assert_eq!(&host.bytes[4..0x1b8], &client.bytes[4..0x1b8]);
    }
    assert_ne!(
        word(&left.roster.manager.players[2].bytes, 0x224),
        word(&right.roster.manager.players[2].bytes, 0x224)
    );
    assert_eq!(resident_rngs(&left), resident_rngs(&right));
    assert_eq!(left.world.categories, right.world.categories);
    assert_eq!(left.world.buildings, right.world.buildings);
    assert_eq!(
        left.world.objective.cash_statuses,
        right.world.objective.cash_statuses
    );
}

#[path = "../synchronization/runtime_tests.rs"]
mod synchronization;
