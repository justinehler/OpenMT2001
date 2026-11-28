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
/// A fresh road traffic pool with no vehicles, for the sample's traffic sum.
fn empty_roads(image: &crate::pe::PeImage<'_>) -> crate::traffic::road::runtime::RoadTraffic {
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
    crate::traffic::road::runtime::RoadTraffic::fresh(
        image,
        0,
        [0; 3],
        &mut [0; 284],
        0x10000000,
        &mut Models,
    )
    .unwrap()
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
    applied: Vec<u8>,
    deck: crate::event_cards::Deck,
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
            applied: vec![],
            deck: crate::event_cards::Deck::new([17, 214013, 2531011]),
        }
    }
    fn tick(&mut self, image: &crate::pe::PeImage<'_>, board: &crate::board::Board) {
        self.tick_observing(image, board, |_, _, _, _| Ok(()));
    }
    /// Live residents at the given auxiliary positions (+4, +0xc), bound as
    /// creation binds them: through the resident reset with reference slot + 1.
    /// Auxiliaries are allocated in reverse resident order, as a load can leave
    /// them, so a slot is not the resident's index. Returns the expected
    /// people sum.
    fn add_residents(&mut self, positions: &[[f32; 2]]) -> u32 {
        let city = &mut self.world.city;
        let first = city.creation.auxiliary.used();
        for &[x, y] in positions.iter().rev() {
            let mut auxiliary = [0; 0x11c];
            put(&mut auxiliary, 4, x.to_bits());
            put(&mut auxiliary, 0xc, y.to_bits());
            city.creation.auxiliary.allocate(&auxiliary).unwrap();
        }
        for count in (1..=positions.len()).rev() {
            let residents = &mut city.ledgers.residents;
            let index = residents.allocate_resident_slot().unwrap().unwrap();
            residents.active_count += 1;
            let slot = first + count - 1;
            let auxiliary = city.creation.auxiliary.get_mut(slot).unwrap();
            residents.nodes[index]
                .reset_runtime_state(slot as u32 + 1, auxiliary, &mut 0, 1)
                .unwrap();
            assert_eq!(residents.nodes[index].auxiliary_slot(), Some(slot));
        }
        positions.iter().fold(0u32, |sum, [x, y]| {
            sum.wrapping_add(*x as i32 as u32)
                .wrapping_add(*y as i32 as u32)
        })
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
        self.tick_with(image, board, |event, clock, city, players, _, _| {
            observe(event, clock, city, players)
        });
    }
    /// One retained scheduler pass. `observe` sees each event first, with the
    /// live scheduler controls and retained timer tasks, as a Session does.
    fn tick_with(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        mut observe: impl FnMut(
            scheduler::Event,
            &crate::clock::Clock,
            &mut level_startup::City,
            &[player_manager::Player],
            &mut scheduler::Controls,
            &mut [[u8; 20]],
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
        let mut utilities = economy::Utilities::new([0; 4]);
        let city = &mut self.world.city;
        scheduler::advance(
            &mut self.world.clock,
            &mut self.controls,
            |event, clock, controls| {
                observe(
                    event,
                    clock,
                    city,
                    &self.roster.manager.players,
                    controls,
                    &mut self.tasks,
                )?;
                if event == scheduler::Event::Helper(0x480ec0) {
                    let mut runtime = live_commands::Runtime {
                        city,
                        roster: &mut self.roster,
                        population: &mut self.population,
                        utilities: &mut utilities,
                        board,
                        event_cards: &mut self.deck,
                        message_context: &mut messages,
                    };
                    let mut clocks = [clock.timing[0], clock.timing[1]];
                    self.queue.drain(&mut clocks, |command| {
                        self.applied.push(match command {
                            live_commands::Command::Demolish(_) => 0x10,
                            live_commands::Command::Open(_) => 0x11,
                            live_commands::Command::Convert(_) => 0x12,
                            live_commands::Command::SellBusiness(_) => 0x13,
                            live_commands::Command::Takeover(_) => 0x15,
                            live_commands::Command::Terms(_) => 0x18,
                            live_commands::Command::EventCard(_) => 0x1a,
                            live_commands::Command::PropertyTransfer { .. } => 0x16,
                            _ => 0x0f,
                        });
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
fn authority_grants_only_when_the_published_ceiling_is_reached() {
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let binding = host.bind_world(1).unwrap();
    client.bind_world(1).unwrap();
    let mut authority = Adapter::new(binding, true, 0, vec![Some(0), Some(peer)]).unwrap();
    let mut queue = live_commands::Queue::default();
    let mut categories = [[0; 380]; 52];
    let mut pause = crate::pause::State::default();
    let mut tasks = crate::timers::startup_tasks([0; 20]);
    assert_eq!(authority.deadline(), 25);
    let mut grants = Vec::new();
    for tick in 0..=101 {
        let before = authority.deadline();
        authority
            .pump(
                &host,
                tick,
                &mut queue,
                &mut categories,
                Targets::default(),
                &mut pause,
                &mut tasks,
            )
            .unwrap();
        if authority.deadline() != before {
            grants.push((tick, authority.deadline()));
        }
        assert_eq!(authority.deadline(), (tick / 25 + 1) * 25);
    }
    assert_eq!(grants, [(25, 50), (50, 75), (75, 100), (100, 125)]);
    // Unsigned >= also covers a host that has advanced past the old ceiling;
    // the grant is based on Total, without rounding to a five-tick boundary.
    authority
        .pump(
            &host,
            127,
            &mut queue,
            &mut categories,
            Targets::default(),
            &mut pause,
            &mut tasks,
        )
        .unwrap();
    assert_eq!(authority.deadline(), 152);
    host.cancellation().cancel();
    client.cancellation().cancel();
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
    let Message::CommandCommit(commit) = host
        .receive(&owner, admitted, 100, Targets::default())
        .unwrap()
    else {
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
            crate::network::commands::Targets {
                buildings: &right.world.city.buildings,
                businesses: &right.world.city.ledgers.businesses,
                ..Targets::default()
            },
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
                    // The AI producer admits on the authority (4517f2) before
                    // routing; the roster is borrowed here, so reserve directly.
                    assert!(crate::construction::reserve(
                        &ai,
                        &mut session.world.city.categories
                    )?);
                    adapter.submit(
                        worker,
                        live_commands::Command::Build(ai.clone()),
                        &mut session.queue,
                        &mut session.world.city.categories,
                        crate::network::commands::Targets {
                            buildings: &session.world.city.buildings,
                            businesses: &session.world.city.ledgers.businesses,
                            ..Targets::default()
                        },
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
                crate::network::commands::Targets {
                    buildings: &right.world.city.buildings,
                    businesses: &right.world.city.ledgers.businesses,
                    ..Targets::default()
                },
                100
            )
            .is_err()
    );
    let limit = Instant::now() + Duration::from_secs(60);
    // Wait for this request before publishing tick100's deadline. In live play
    // a request that arrives after that publication correctly waits a tick.
    let ingress = loop {
        if let Some(Gameplay::Frame(ingress)) = host.poll_gameplay() {
            break *ingress;
        }
        assert!(
            Instant::now() < limit,
            "client construction request did not arrive"
        );
        thread::sleep(Duration::from_millis(5));
    };
    let message = authority
        .receive(host.gameplay_owner(), ingress, 100, Targets::default())
        .unwrap();
    let Message::CommandCommit(commit) = &message else {
        panic!()
    };
    authority
        .enqueue_received(
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
                crate::network::commands::Targets {
                    buildings: &left.world.city.buildings,
                    businesses: &left.world.city.ledgers.businesses,
                    ..Targets::default()
                },
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
                crate::network::commands::Targets {
                    buildings: &right.world.city.buildings,
                    businesses: &right.world.city.ledgers.businesses,
                    ..Targets::default()
                },
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

#[path = "ceiling_tests.rs"]
mod ceiling;

fn ready_building(session: &mut TestSession, id: usize, owner: u8) {
    let row = &mut session.world.city.buildings[id];
    put(row, 0, id as u32);
    row[4..9].copy_from_slice(&[0, 0, 1, 1, 2]);
    put(row, 0x14, 4);
    put(row, 0x1c, u32::MAX);
    put(row, 0x24, 0);
    row[0x2d] = owner;
    row[0x3f] = 1;
    put(row, 0x40, 0x617628 + 7 * 380);
}

#[test]
fn event_cards_loopback_apply_once_at_125_with_identical_ledgers_targets_and_rng() {
    use crate::event_cards::{ai, lifecycle, selection};
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let mut bytes = image_bytes();
    put(&mut bytes, 0x134200 + 0x200, 2f32.to_bits());
    // Invented cards: collect from rivals, promotion, a debit, and an AI credit.
    for (card, class, amount, effect) in [(0, 0, 7, 1), (1, 2, 2, 6), (2, 0, 3, 7), (3, 0, 5, 0)] {
        let at = 0x134208 + 0x200 + card * 20;
        put(&mut bytes, at, class);
        put(&mut bytes, at + 8, amount);
        put(&mut bytes, at + 12, effect);
    }
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
    let owners = vec![Some(0), Some(peer), None];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, owners).unwrap();
    // These sessions start at Total100, within an existing window ending125.
    authority.admit_deadline(0, 125).unwrap();
    replica.admit_deadline(0, 125).unwrap();
    left.controls.network_override = true;
    for session in [&mut left, &mut right] {
        let mut player = player_manager::Player {
            reference: 0x3000,
            bytes: session.roster.manager.players[0].bytes.clone(),
            auxiliaries: vec![player_manager::Auxiliary {
                reference: 0x4000,
                bytes: [0; 0x30],
            }],
        };
        player.bytes[0x1bc] = 2;
        put(&mut player.bytes, 0x154, 1);
        put(&mut player.bytes, 0x21c, 0x4000);
        put(&mut player.bytes, 0x1b8, 3);
        let aux = &mut player.auxiliaries[0].bytes;
        for (at, value) in [
            (4, 3),
            (0x14, 0x3000),
            (0x18, 1),
            (0x1c, 17),
            (0x20, 214013),
            (0x24, 2531011),
        ] {
            put(aux, at, value);
        }
        session.roster.manager.players.push(player);
        session
            .roster
            .collect_accounts(&mut session.world.city)
            .unwrap();
        ready_building(session, 0, 1);
        put(&mut session.world.city.buildings[0], 0x1c, 0);
        let business = &mut session.world.city.ledgers.businesses[0];
        business[0xa] = 1;
        business[0x21c] = 1;
        for card in 0..4 {
            session.deck.draw_specific(card, |_, _| Ok(())).unwrap();
        }
    }
    // Host human and remote human use the same ordinary submission path.
    for (adapter, worker, session, cards, player) in [
        (&mut authority, &host, &mut left, vec![0, 1], 0),
        (&mut replica, &client, &mut right, vec![2], 1),
    ] {
        for card in cards {
            adapter
                .submit(
                    worker,
                    live_commands::Command::EventCard(lifecycle::Command {
                        card,
                        player,
                        target: 0,
                    }),
                    &mut session.queue,
                    &mut session.world.city.categories,
                    Targets::default(),
                    100,
                )
                .unwrap();
        }
    }
    // The real AI lifecycle resolves on both peers and resets its registered
    // auxiliary RNG. Only the authority's resulting command enters the stream.
    for (adapter, worker, session) in [
        (&mut authority, &host, &mut left),
        (&mut replica, &client, &mut right),
    ] {
        let city = &mut session.world.city;
        let references: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.reference)
            .collect();
        let binding = ai::Bindings {
            world: selection::World {
                buildings: &city.buildings,
                businesses: &city.ledgers.businesses,
                categories: &city.categories,
                catalog: &city.catalog.info,
                products: &city.catalog.products,
                commodities: &city.commodities.rows,
                utilities: [1; 4],
                hour: 6,
                tick: 0,
                unselected_building: 0,
            },
            targets: lifecycle::Targets {
                buildings: &city.buildings,
                businesses: &city.ledgers.businesses,
                categories: &city.categories,
                players: &references,
                nodes: &[],
            },
            period: 0,
            image: &image,
        };
        let mut commands = Vec::new();
        ai::run_update(
            &mut session.roster.manager.players[2],
            crate::ai_hourly::UpdateCall::Retire { account: 0x4000 },
            &binding,
            |call, _| {
                if let ai::RuntimeCall::Submit(command) = call {
                    commands.push(command);
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(commands.len(), 1);
        for command in commands {
            adapter
                .submit_ai_card(
                    worker,
                    command,
                    &mut session.queue,
                    &mut city.categories,
                    Targets::default(),
                    100,
                )
                .unwrap();
        }
        let ai = &session.roster.manager.players[2];
        let next = 17u32.wrapping_mul(214013).wrapping_add(2531011);
        assert_eq!(player_manager::word(&ai.auxiliaries[0].bytes, 0x1c), next);
        assert_eq!(
            player_manager::word(&ai.auxiliaries[0].bytes, 0),
            20 + ((next >> 16) & 0x7fff) % 9
        );
        assert_eq!(player_manager::word(&ai.bytes, 0x1b8), 0);
    }
    assert!(right.queue.snapshot().unwrap().is_empty());
    assert_eq!(replica.next_request, 1);
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        for (adapter, worker, session) in [
            (&mut authority, &host, &mut left),
            (&mut replica, &client, &mut right),
        ] {
            adapter
                .pump(
                    worker,
                    100,
                    &mut session.queue,
                    &mut session.world.city.categories,
                    Targets::default(),
                    &mut session.world.city.pause,
                    &mut session.tasks,
                )
                .unwrap();
        }
        if left.queue.snapshot().unwrap().len() == 4 && right.queue.snapshot().unwrap().len() == 4 {
            break;
        }
        assert!(
            Instant::now() < until,
            "card commits did not cross loopback"
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!((authority.next_commit, replica.next_enqueued), (4, 4));
    assert_eq!(replica.next_request, 1); // No commit rebroadcast or AI request.
    let initial_rng = left.deck.rng;
    for session in [&mut left, &mut right] {
        let queued = session.queue.snapshot().unwrap();
        let cards: Vec<_> = queued
            .iter()
            .map(|(_, due, inner, _, command)| {
                assert_eq!((*due, *inner), (125, 125));
                let live_commands::Command::EventCard(command) = command else {
                    panic!()
                };
                command.card
            })
            .collect();
        assert_eq!(cards, [0, 1, 3, 2]);
        while session.world.clock.timing[0] < 124 {
            session.tick(&image, &board);
        }
        assert!(session.applied.is_empty());
        assert!(
            session
                .world
                .ledgers
                .players
                .iter()
                .all(|p| p.cash == 10000.)
        );
        assert_eq!(session.world.ledgers.businesses[0][0x74], 0);
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 125);
        assert_eq!(session.applied, [0x1a; 4]);
        assert_eq!(
            session
                .world
                .ledgers
                .players
                .iter()
                .map(|p| p.cash)
                .collect::<Vec<_>>(),
            [10028., 9980., 9996.]
        );
        assert_eq!(session.world.ledgers.businesses[0][0x74], 24);
        assert_eq!(
            player_manager::word(&session.world.ledgers.businesses[0], 0x78),
            2
        );
        assert_eq!(&session.deck.used[..4], &[0; 4]);
        assert_eq!(session.deck.rng, initial_rng); // Application releases cards; it never draws again.
        assert!(session.queue.snapshot().unwrap().is_empty());
    }
    // At the ceiling the client pauses on the same tasks, then admitted kind8
    // releases it after every card commit. No command can execute a second time.
    assert_eq!(right.world.city.pause.bytes[1], 1);
    loop {
        for (adapter, worker, session) in [
            (&mut authority, &host, &mut left),
            (&mut replica, &client, &mut right),
        ] {
            adapter
                .pump(
                    worker,
                    125,
                    &mut session.queue,
                    &mut session.world.city.categories,
                    Targets::default(),
                    &mut session.world.city.pause,
                    &mut session.tasks,
                )
                .unwrap();
            session.controls.deadline = adapter.deadline();
        }
        if replica.deadline() == 150 {
            break;
        }
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(2));
    }
    for session in [&mut left, &mut right] {
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 126);
        assert_eq!(session.applied.len(), 4);
    }
    assert_eq!(left.postings, right.postings);
    for (a, b) in left
        .world
        .ledgers
        .players
        .iter()
        .zip(&right.world.ledgers.players)
    {
        assert_eq!(a.cash, b.cash);
        for event in 0..38 {
            assert_eq!(a.ledger(event), b.ledger(event));
        }
    }
    assert_eq!(left.world.city.categories, right.world.city.categories);
    assert_eq!(left.world.buildings, right.world.buildings);
    assert_eq!(
        left.world.ledgers.businesses,
        right.world.ledgers.businesses
    );
    assert_eq!(
        left.roster.manager.players[2].bytes,
        right.roster.manager.players[2].bytes
    );
    assert_eq!(
        left.roster.manager.players[2].auxiliaries[0].bytes,
        right.roster.manager.players[2].auxiliaries[0].bytes
    );
    assert_eq!(left.deck.rng, right.deck.rng);
    assert_eq!(left.deck.cards, right.deck.cards);
    host.cancellation().cancel();
    client.cancellation().cancel();
}

fn business_family_applies_once_at_125(kind: u8) {
    let mut bytes = image_bytes();
    if kind == 0x12 {
        bytes[0x125b00 + 0x200..0x125b08 + 0x200].copy_from_slice(&20f64.to_le_bytes());
        bytes[0x125b08 + 0x200..0x125b10 + 0x200].copy_from_slice(&3f64.to_le_bytes());
        put(&mut bytes, 0x125034 + 0x200, 1f32.to_bits());
    }
    if kind == 0x13 {
        put(&mut bytes, 0x125c50 + 0x200, 0.5f32.to_bits());
    }
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    let mut right = TestSession::new(&image, &board);
    for session in [&mut left, &mut right] {
        ready_building(session, 0, if kind == 0x11 { 2 } else { 1 });
        session.world.city.categories[7][5] = if kind == 0x15 { 2 } else { 1 };
        // The business owner differs from both the building owner and landlord.
        if kind != 0x11 {
            put(&mut session.world.city.buildings[0], 0x1c, 0);
            let business = &mut session.world.city.ledgers.businesses[0];
            put(business, 0, 0);
            put(business, 4, if kind == 0x12 { 1 } else { 0 });
            business[8..10].copy_from_slice(&0u16.to_le_bytes());
            business[0xa] = if kind == 0x15 { 1 } else { 2 };
            put(business, 0x20c, u32::MAX);
            business[0x21c] = 1;
            session.roster.category_counts[7 * 45 + usize::from(kind == 0x12)] = 1;
            put(&mut session.world.city.buildings[0], 0x38, 500f32.to_bits());
            if kind == 0x12 {
                put(
                    session.world.city.catalog.info.row_mut(0).unwrap(),
                    0x54,
                    10,
                );
            }
        }
    }
    let (owner, binding, peer) = owner();
    let owners = vec![Some(0), Some(4)];
    let mut host = Adapter::new(binding, true, 0, owners.clone()).unwrap();
    let mut client = Adapter::new(binding, false, 4, owners).unwrap();
    let family = match kind {
        0x10 => Request::demolition(
            1,
            2,
            &crate::demolition::Command {
                category: 7,
                building: 0,
                amount: 37.25,
            },
        ),
        0x11 => Request::business_open(
            1,
            &crate::business_open::Command {
                kind: 0,
                building: 0,
                owner: 2,
                stock_argument: 1,
            },
        ),
        0x12 => Request::conversion(
            1,
            &crate::business_conversion::Command {
                kind: 0,
                building: 0,
                owner: 2,
            },
        ),
        0x13 => Request::business_sale(
            1,
            &crate::business_sale::Command {
                business: 0,
                player: 1,
            },
        ),
        0x15 => Request::takeover(1, 2, &crate::business_takeover::Command { building: 0 }),
        0x18 => Request::terms(
            1,
            2,
            &crate::economy::PendingBusinessTerms {
                business_index: 0,
                terms: crate::economy::BusinessTerms {
                    stock_targets: [-1, 2, -3, 4, 5, 6],
                    prices: [1.25, 2.5, 3.75, 4., 5., 6.],
                    rent: 12.5,
                },
            },
        ),
        _ => unreachable!(),
    };
    // Equal-due ordering spans the existing property-transfer family.
    for (sequence, request) in [
        Request::transfer(
            0,
            Transfer {
                owner: 2,
                amount: 100,
                category: 8,
                due: 0,
            },
        ),
        family.clone(),
        Request::transfer(
            2,
            Transfer {
                owner: 2,
                amount: 200,
                category: 9,
                due: 0,
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let admitted = ingress(
            &owner,
            binding,
            peer,
            Message::CommandRequest(request),
            sequence as u64,
        )
        .unwrap();
        let Message::CommandCommit(commit) = host
            .receive(
                &owner,
                admitted,
                100,
                Targets {
                    buildings: &left.world.city.buildings,
                    businesses: &left.world.city.ledgers.businesses,
                    category_owners: left.world.city.categories.map(|row| row[5]),
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(commit.order, sequence as u64);
        assert_eq!(commit.due, 125);
        host.enqueue(
            &commit,
            &mut left.queue,
            &mut left.world.city.categories,
            100,
        )
        .unwrap();
        // A client receiving two ticks later keeps the authority's due unchanged.
        client
            .enqueue(
                &commit,
                &mut right.queue,
                &mut right.world.city.categories,
                102,
            )
            .unwrap();
        assert_eq!(
            client.enqueue(
                &commit,
                &mut right.queue,
                &mut right.world.city.categories,
                102
            ),
            Err(Error::Sequence)
        );
    }
    let replay = ingress(
        &owner,
        binding,
        peer,
        Message::CommandRequest(family.clone()),
        3,
    )
    .unwrap();
    assert_eq!(
        host.receive(
            &owner,
            replay,
            100,
            Targets {
                buildings: &left.world.city.buildings,
                businesses: &left.world.city.ledgers.businesses,
                category_owners: left.world.city.categories.map(|row| row[5])
            }
        ),
        Err(Error::Sequence)
    );
    let mut wrong = family.clone();
    wrong.serial = 3;
    wrong.owner = 1;
    if matches!(kind, 0x11 | 0x12) {
        wrong.payload[8..12].copy_from_slice(&1u32.to_le_bytes());
    }
    if kind == 0x13 {
        wrong.payload[4] = 0;
    }
    let wrong = ingress(&owner, binding, peer, Message::CommandRequest(wrong), 4).unwrap();
    assert_eq!(
        host.receive(
            &owner,
            wrong,
            100,
            Targets {
                buildings: &left.world.city.buildings,
                businesses: &left.world.city.ledgers.businesses,
                category_owners: left.world.city.categories.map(|row| row[5])
            }
        ),
        Err(Error::Peer)
    );
    assert_eq!(host.next_commit, 3);
    for session in [&mut left, &mut right] {
        let before = session.world.city.buildings[0];
        let business_before = session.world.city.ledgers.businesses[0];
        while session.world.clock.timing[0] < 124 {
            session.tick(&image, &board);
        }
        assert_eq!(session.world.city.buildings[0], before);
        assert_eq!(session.world.city.ledgers.businesses[0], business_before);
        assert_eq!(session.world.ledgers.players[1].cash, 10000.);
        assert!(session.applied.is_empty());
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 125);
        assert_eq!(session.applied, [0x16, kind, 0x16]);
        assert_eq!(
            session.world.ledgers.players[0].cash,
            if kind == 0x15 { 10500. } else { 10000. }
        );
        if kind == 0x10 {
            assert_eq!(session.world.ledgers.players[1].cash, 9662.75);
            assert_eq!(player_manager::word(&session.world.buildings[0], 0x14), 0);
            assert_eq!(
                player_manager::word(&session.world.buildings[0], 0x1c),
                u32::MAX
            );
            assert_eq!(session.world.ledgers.businesses[0][0x21c], 0);
            assert_eq!(session.world.pending_world.count, 1);
            assert_eq!(session.world.pending_world.records[0][2], 2);
        } else if kind == 0x11 {
            assert_eq!(session.world.ledgers.players[1].cash, 9700.);
            assert_eq!(player_manager::word(&session.world.buildings[0], 0x1c), 0);
            assert_eq!(session.world.ledgers.businesses[0][0xa], 2);
            assert_eq!(session.world.ledgers.businesses[0][0x21c], 1);
            assert_eq!(session.population.business_counts.age, 100);
        }
        if kind == 0x12 {
            assert_eq!(session.world.ledgers.players[1].cash, 9260.); // 300 transfers + 440 conversion quote
            let linked = player_manager::word(&session.world.buildings[0], 0x1c) as usize;
            assert_eq!(
                player_manager::word(&session.world.ledgers.businesses[linked], 4),
                0
            );
            assert_eq!(session.world.ledgers.businesses[linked][0xa], 2);
            assert_eq!(session.population.business_counts.age, 100);
        } else if kind == 0x13 {
            assert_eq!(session.world.ledgers.players[1].cash, 9950.); // 250 sale proceeds - 300 transfers
            assert_eq!(session.world.ledgers.businesses[0][0xa], 0);
            assert_eq!(session.world.buildings[0][0x2d], 0);
            assert_eq!(session.population.business_counts.age, 100);
        } else if kind == 0x15 {
            assert_eq!(session.world.ledgers.players[1].cash, 9200.);
            assert_eq!(session.world.ledgers.businesses[0][0xa], 2);
            assert_eq!(session.world.buildings[0][0x2d], 2);
            assert_eq!(session.population.business_counts.age, 100);
        } else if kind == 0x18 {
            assert_eq!(session.world.ledgers.players[1].cash, 9700.);
            let row = &session.world.ledgers.businesses[0];
            let terms = family.terms_payload().unwrap().terms;
            for i in 0..6 {
                assert_eq!(
                    &row[0x1b8 + i * 2..0x1ba + i * 2],
                    &terms.stock_targets[i].to_le_bytes()
                );
                assert_eq!(
                    &row[0x1c4 + i * 4..0x1c8 + i * 4],
                    &terms.prices[i].to_le_bytes()
                );
            }
            assert_eq!(player_manager::word(row, 0x1dc), 12.5f32.to_bits());
            assert_eq!(session.population.business_counts.age, 0);
        }
        let business_after = session.world.city.ledgers.businesses.clone();
        let after = session.world.city.buildings[0];
        let cash = session.world.ledgers.players[1].cash;
        session.controls.deadline = 150;
        session.world.city.pause.bytes[1] = 0;
        session.tick(&image, &board);
        assert_eq!(session.world.city.buildings[0], after);
        assert_eq!(session.world.ledgers.players[1].cash, cash);
        assert_eq!(session.applied.len(), 3);
        assert_eq!(session.world.city.ledgers.businesses, business_after);
        assert!(session.queue.snapshot().unwrap().is_empty());
    }
    let expired = Commit {
        order: 3,
        due: 125,
        request: family.clone(),
    };
    assert_eq!(
        host.enqueue(
            &expired,
            &mut left.queue,
            &mut left.world.city.categories,
            126
        ),
        Err(Error::Sequence)
    );
    assert_eq!(
        client.enqueue(
            &expired,
            &mut right.queue,
            &mut right.world.city.categories,
            126
        ),
        Err(Error::Sequence)
    );
    let mut stale = family;
    if kind != 0x18 {
        stale.serial = 3;
    }
    let stale = ingress(&owner, binding, peer, Message::CommandRequest(stale), 5).unwrap();
    assert!(
        host.receive(
            &owner,
            stale,
            126,
            Targets {
                buildings: &left.world.city.buildings,
                businesses: &left.world.city.ledgers.businesses,
                category_owners: left.world.city.categories.map(|row| row[5])
            }
        )
        .is_err()
    );
    assert_eq!(host.next_commit, 3);
    assert_eq!(left.world.buildings, right.world.buildings);
    assert_eq!(
        left.world.ledgers.businesses,
        right.world.ledgers.businesses
    );
}

#[test]
fn demolition_runtime_charges_linked_owner_and_starts_removal_once_at_125() {
    business_family_applies_once_at_125(0x10);
}

#[test]
fn business_open_runtime_allocates_and_attaches_encoded_owner_once_at_125() {
    business_family_applies_once_at_125(0x11);
}

#[test]
fn conversion_runtime_charges_business_owner_and_opens_new_kind_once_at_125() {
    business_family_applies_once_at_125(0x12);
}
#[test]
fn sale_runtime_pays_business_owner_and_relinquishes_once_at_125() {
    business_family_applies_once_at_125(0x13);
}
#[test]
fn takeover_runtime_charges_category_landlord_and_pays_seller_once_at_125() {
    business_family_applies_once_at_125(0x15);
}
#[test]
fn terms_runtime_applies_signed_stock_prices_and_rent_once_at_125() {
    business_family_applies_once_at_125(0x18);
}

#[test]
fn loopback_authority_completes_remote_human_building_without_client_rebroadcast() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
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
    let mut completions = Vec::new();
    for (session, is_host) in [(&mut left, true), (&mut right, false)] {
        ready_building(session, 0, 2);
        session
            .world
            .city
            .pending_world
            .construct(7, &mut session.world.city.buildings[0], 0, 2)
            .unwrap();
        // The final completion stage has finished its visual countdown.
        session.world.city.pending_world.records[0][3] = 1;
        session.world.city.pending_world.records[0][4] = u32::MAX;
        live_commands::PendingRuntime {
            city: &mut session.world.city,
            roster: &session.roster,
            commands: &mut session.queue,
            selected: &mut [u32::MAX; 2],
        }
        .advance_with_submission(
            99,
            &mut crate::pending_world::Controls {
                network_enabled: true,
                network_override: is_host,
                effect_model_reference: 0,
            },
            &mut TestGeometry,
            |_, _| Ok(0),
            |command, _| {
                assert!(is_host, "client generated a completion request");
                completions.push(command);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(session.world.pending_world.count, 0);
        assert_eq!(player_manager::word(&session.world.buildings[0], 0x14), 4);
        assert!(session.queue.snapshot().unwrap().is_empty());
    }
    assert_eq!(completions.len(), 1);
    let live_commands::Command::Open(command) = &completions[0] else {
        panic!()
    };
    assert_eq!(command.owner, 2);
    // Ordinary local requests cannot impersonate the remote human.
    assert!(
        authority
            .submit(
                &host,
                completions[0].clone(),
                &mut left.queue,
                &mut left.world.city.categories,
                Targets {
                    buildings: &left.world.city.buildings,
                    businesses: &left.world.city.ledgers.businesses,
                    ..Targets::default()
                },
                99
            )
            .is_err()
    );
    assert!(
        replica
            .submit_completion(
                &client,
                command,
                &mut right.queue,
                &mut right.world.city.categories,
                Targets {
                    buildings: &right.world.city.buildings,
                    businesses: &right.world.city.ledgers.businesses,
                    ..Targets::default()
                },
                99
            )
            .is_err()
    );
    // A closed authority window defers submission and stamps the next Total+25.
    authority.closed_tick = Some(99);
    authority
        .submit_completion(
            &host,
            command,
            &mut left.queue,
            &mut left.world.city.categories,
            Targets {
                buildings: &left.world.city.buildings,
                businesses: &left.world.city.ledgers.businesses,
                ..Targets::default()
            },
            99,
        )
        .unwrap();
    assert!(left.queue.snapshot().unwrap().is_empty());
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        for (adapter, worker, session) in [
            (&mut authority, &host, &mut left),
            (&mut replica, &client, &mut right),
        ] {
            adapter
                .pump(
                    worker,
                    100,
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
        }
        if left.queue.snapshot().unwrap().len() == 1 && right.queue.snapshot().unwrap().len() == 1 {
            break;
        }
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(5));
    }
    for session in [&mut left, &mut right] {
        assert_eq!(session.queue.snapshot().unwrap()[0].1, 125);
        while session.world.clock.timing[0] < 124 {
            session.tick(&image, &board);
        }
        assert_eq!(session.world.ledgers.businesses[0][0x21c], 0);
        session.tick(&image, &board);
        assert_eq!(session.world.ledgers.businesses[0][0xa], 2);
        assert_eq!(session.world.ledgers.businesses[0][0x21c], 1);
        assert_eq!(session.applied, [0x11]);
    }
    assert!(
        host.poll_gameplay().is_none(),
        "client rebroadcast a received commit"
    );
    assert_eq!(
        left.world.ledgers.businesses,
        right.world.ledgers.businesses
    );
}

#[path = "auction_tests.rs"]
mod auction_tests;

#[path = "player_message_tests.rs"]
mod player_message_tests;

#[path = "offer_tests.rs"]
mod offer_tests;
#[path = "peer_loss_tests.rs"]
mod peer_loss_tests;

#[test]
fn lifecycle_loopback_submits_remote_human_and_host_ai_without_rebroadcast() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let bytes = image_bytes();
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
    let owners = vec![Some(0), Some(peer), None];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, owners).unwrap();
    authority.admit_deadline(0, 125).unwrap();
    for session in [&mut left, &mut right] {
        session.world.city.categories[7][5] = 2;
        session.world.city.categories[8][5] = 3;
        for id in 0..8 {
            ready_building(session, id, 1);
            let building = &mut session.world.city.buildings[id];
            put(building, 0x1c, id as u32);
            put(
                building,
                0x40,
                0x617628 + if id < 4 { 7 * 380 } else { 8 * 380 },
            );
            let business = &mut session.world.city.ledgers.businesses[id];
            business[8..10].copy_from_slice(&(id as u16).to_le_bytes());
            business[0xa] = if id % 4 == 2 {
                1
            } else if id < 4 {
                2
            } else {
                3
            };
            business[0x21c] = 1;
        }
    }
    for (gameplay_owner, adapter, worker, session) in [
        (2, &mut replica, &client, &mut right),
        (3, &mut authority, &host, &mut left),
    ] {
        for (offset, request) in lifecycle_requests(gameplay_owner).into_iter().enumerate() {
            let id = offset as u32 + if gameplay_owner == 2 { 0 } else { 4 };
            let command = match request.kind {
                0x12 => live_commands::Command::Convert(crate::business_conversion::Command {
                    kind: 4,
                    building: id,
                    owner: u32::from(gameplay_owner),
                }),
                0x13 => live_commands::Command::SellBusiness(crate::business_sale::Command {
                    business: id,
                    player: gameplay_owner - 1,
                }),
                0x15 => live_commands::Command::Takeover(crate::business_takeover::Command {
                    building: id,
                }),
                0x18 => {
                    let mut terms = request.terms_payload().unwrap();
                    terms.business_index = id;
                    live_commands::Command::Terms(terms)
                }
                _ => unreachable!(),
            };
            let targets = Targets::new(
                &session.world.city.buildings,
                &session.world.city.ledgers.businesses,
                &session.world.city.categories,
            );
            adapter
                .submit(
                    worker,
                    command,
                    &mut session.queue,
                    &mut session.world.city.categories,
                    targets,
                    101,
                )
                .unwrap();
        }
    }
    assert!(right.queue.snapshot().unwrap().is_empty());
    assert_eq!(left.queue.snapshot().unwrap().len(), 4);
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        for (adapter, worker, session) in [
            (&mut authority, &host, &mut left),
            (&mut replica, &client, &mut right),
        ] {
            let targets = Targets::new(
                &session.world.city.buildings,
                &session.world.city.ledgers.businesses,
                &session.world.city.categories,
            );
            adapter
                .pump(
                    worker,
                    101,
                    &mut session.queue,
                    &mut session.world.city.categories,
                    targets,
                    &mut session.world.city.pause,
                    &mut session.tasks,
                )
                .unwrap();
        }
        if right.queue.snapshot().unwrap().len() == 8 {
            break;
        }
        assert!(
            Instant::now() < until,
            "lifecycle commits did not cross loopback"
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(authority.next_commit, 8);
    assert_eq!(authority.next_enqueued, 8);
    assert_eq!(replica.next_enqueued, 8);
    for session in [&left, &right] {
        let snapshot = session.queue.snapshot().unwrap();
        assert_eq!(snapshot.len(), 8);
        assert!(snapshot.iter().all(|(_, due, _, _, _)| *due == 126));
    }
    // Client admission cannot create a request or advance its request serial.
    assert_eq!(replica.next_request, 4);
    assert_eq!(replica.next_commit, 8);
    assert_eq!(authority.requests[peer as usize], Some(3));
    assert_eq!(authority.next_request, 4);
    host.cancellation().cancel();
    client.cancellation().cancel();
}

/// 0x451720 kind 15 on the authority: two computer rows build on the same
/// rectangle in one tick. The first is reserved, committed and applied on both
/// peers; the second is dropped with sender success and its owner receives
/// inbox message 0x30. A client's request for a taken rectangle is dropped
/// without failing either peer, and a construction admitted after the tick's
/// deadline is published commits on the next tick without a second check.
#[test]
fn authority_drops_constructions_on_a_reserved_rectangle_like_the_original() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let bytes = image_bytes();
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
    // Owner 1 is the host human, owner 2 the client, owners 3 and 4 computers.
    let owners = vec![Some(0), Some(peer), None, None];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, owners).unwrap();
    // Tick 100 stays open until the client's requests have crossed.
    authority.admit_deadline(0, 125).unwrap();
    for session in [&mut left, &mut right] {
        for slot in [2, 3] {
            let mut player = player_manager::Player {
                reference: 0x1000 + u32::from(slot) * 0x1000,
                bytes: session.roster.manager.players[0].bytes.clone(),
                auxiliaries: vec![],
            };
            player.bytes[0x1bc] = slot;
            put(&mut player.bytes, 0x154, 1);
            session.roster.manager.players.push(player);
        }
        session
            .roster
            .collect_accounts(&mut session.world.city)
            .unwrap();
    }
    let mut arena = crate::player_messages::Arena { next: 0x20000000 };
    // The authority's AI producer: admit (4517f2), then route if admitted.
    let mut build = |authority: &mut Adapter,
                     session: &mut TestSession,
                     command: &crate::construction::Command,
                     tick: u32| {
        let admitted = crate::construction::admit(
            command,
            &mut session.world.city.categories,
            &mut session.roster.manager.players,
            &mut arena,
            0,
            crate::player_messages::Routing {
                current_player: session.roster.status.current_player,
                network: [true, true],
                prior: [0; 0x30],
            },
            |route, _| Err(format!("computer notice left the authority: {route:?}")),
        )
        .unwrap();
        if admitted {
            let targets = Targets::new(
                &session.world.city.buildings,
                &session.world.city.ledgers.businesses,
                &session.world.city.categories,
            );
            authority
                .submit(
                    &host,
                    live_commands::Command::Build(command.clone()),
                    &mut session.queue,
                    &mut session.world.city.categories,
                    targets,
                    tick,
                )
                .unwrap();
        }
        admitted
    };
    let first = construction(3);
    let mut second = construction(4);
    second.rectangle = [1, 0, 2, 1];
    second.business = 6;
    assert!(build(&mut authority, &mut left, &first, 100));
    assert!(!build(&mut authority, &mut left, &second, 100));
    let notice = &left.roster.manager.players[3].auxiliaries;
    assert_eq!(notice.len(), 1);
    let word = |at: usize| u32::from_le_bytes(notice[0].bytes[at..at + 4].try_into().unwrap());
    assert_eq!((word(4), word(0x18), word(0x1c)), (0x30, 7, 6));
    assert_eq!(notice[0].bytes[0x15], 3);
    assert!(left.roster.manager.players[2].auxiliaries.is_empty());
    assert_eq!(left.queue.snapshot().unwrap().len(), 1);

    // The client asks for the taken rectangle, then for a free one.
    let mut taken = construction(2);
    taken.rectangle = [0, 1, 0, 1];
    let mut free = construction(2);
    free.category = 8;
    for command in [&taken, &free] {
        let targets = Targets::new(
            &right.world.city.buildings,
            &right.world.city.ledgers.businesses,
            &right.world.city.categories,
        );
        replica
            .submit(
                &client,
                live_commands::Command::Build(command.clone()),
                &mut right.queue,
                &mut right.world.city.categories,
                targets,
                100,
            )
            .unwrap();
    }
    let limit = Instant::now() + Duration::from_secs(60);
    let pump = |authority: &mut Adapter,
                replica: &mut Adapter,
                left: &mut TestSession,
                right: &mut TestSession,
                tick: u32,
                expected: usize| {
        loop {
            for (adapter, worker, session) in [
                (&mut *authority, &host, &mut *left),
                (&mut *replica, &client, &mut *right),
            ] {
                let targets = Targets::new(
                    &session.world.city.buildings,
                    &session.world.city.ledgers.businesses,
                    &session.world.city.categories,
                );
                adapter
                    .pump(
                        worker,
                        tick,
                        &mut session.queue,
                        &mut session.world.city.categories,
                        targets,
                        &mut session.world.city.pause,
                        &mut session.tasks,
                    )
                    .unwrap();
            }
            if left.queue.snapshot().unwrap().len() == expected
                && right.queue.snapshot().unwrap().len() == expected
            {
                break;
            }
            assert!(Instant::now() < limit, "construction commits did not cross");
            thread::sleep(Duration::from_millis(5));
        }
    };
    // The free request is processed after the taken one, so both arrived.
    pump(&mut authority, &mut replica, &mut left, &mut right, 100, 2);

    // Tick 100's deadline is published: a computer construction admitted now
    // keeps its reservation and commits at tick 101 without being rechecked.
    authority.closed_tick = Some(100);
    let mut late = construction(4);
    late.category = 9;
    assert!(build(&mut authority, &mut left, &late, 100));
    assert_eq!(left.queue.snapshot().unwrap().len(), 2);
    assert_eq!(left.world.categories[9][0x9c], 2);
    pump(&mut authority, &mut replica, &mut left, &mut right, 101, 3);

    for session in [&left, &right] {
        let queued: Vec<_> = session
            .queue
            .snapshot()
            .unwrap()
            .into_iter()
            .map(|(_, due, _, _, command)| {
                let live_commands::Command::Build(build) = command else {
                    panic!("only constructions were committed")
                };
                (due, build.owner, build.category)
            })
            .collect();
        assert_eq!(queued, [(125, 3, 7), (125, 2, 8), (126, 4, 9)]);
        // Only the first construction reserved category 7: cells (0,0),(0,1),
        // (1,0),(1,1) with its height; the rejected (2,0),(2,1) stay free.
        let occupancy = &session.world.categories[7][0x9c..0xcd];
        assert_eq!(
            [occupancy[0], occupancy[1], occupancy[7], occupancy[8]],
            [2; 4]
        );
        assert_eq!([occupancy[14], occupancy[15]], [0; 2]);
    }
    assert_eq!(left.world.city.categories, right.world.city.categories);
    for session in [&mut left, &mut right] {
        while session.world.clock.timing[0] < 125 {
            session.tick(&image, &board);
        }
        session.controls.deadline = 150;
        session.world.city.pause.bytes[1] = 0;
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], 126);
        assert_eq!(
            session
                .world
                .buildings
                .iter()
                .filter(|row| row[0x3f] != 0)
                .count(),
            3
        );
        assert!(session.queue.snapshot().unwrap().is_empty());
    }
    assert_eq!(left.world.buildings, right.world.buildings);
    assert_eq!(left.world.city.categories, right.world.city.categories);
    for slot in 0..4 {
        assert_eq!(
            left.world.ledgers.players[slot].cash,
            right.world.ledgers.players[slot].cash
        );
    }
    host.cancellation().cancel();
    client.cancellation().cancel();
}
