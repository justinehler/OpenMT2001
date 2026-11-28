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
struct NoGeometry;
impl crate::building_pool::GeometryBinding for NoGeometry {
    fn prepare(&mut self, _: usize, _: &mut [u8; 916]) -> std::result::Result<(), String> {
        panic!("transfer requested geometry")
    }
    fn model_height(&mut self, _: u32) -> std::result::Result<f32, String> {
        panic!("transfer requested model")
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
                            &mut NoGeometry,
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
    host.enqueue(commit, &mut left.queue, 100).unwrap();
    client.enqueue(commit, &mut right.queue, 100).unwrap();
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
    assert_eq!(left.world.categories, right.world.categories);
    assert_eq!(
        left.world.ledgers.players[1].cash,
        right.world.ledgers.players[1].cash
    );
}
