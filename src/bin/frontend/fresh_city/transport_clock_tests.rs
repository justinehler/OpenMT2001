//! Transport timing on the original clocks: aircraft fly on the global tick
//! 822d40, which runs in every scheduler mode (481680), while trains and ships
//! step only on normal city ticks (44ec50). Drawing transport (44f0b0) never
//! advances it, however often the frontend asks.
use super::*;
use openmt2001::fixtures;

fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}

/// Aircraft locator rows start at 685cc8, one 16-byte row per variant (44e000).
fn aircraft_locator(row: &[u8]) -> u32 {
    0x685cc8 + word(row, 8) * 16
}

fn with_session(body: impl FnOnce(&mut Session<'_>, &CommandAssets<'_, '_>) + Send) {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let read = |key| fs::read(fixtures::path(key)).unwrap();
                let executable = read("EXE");
                let directory = read("ARCHIVE_DIR");
                let archive = read("ARCHIVE_BIN");
                let build = read("BUILD_FILE");
                let parameters = read("PARAMETERS");
                let timetable = read("TIMETABLE");
                let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
                let board = openmt2001::board::Board::from_v14(&image).unwrap();
                let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
                let index =
                    openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
                        .unwrap();
                let default_path = fixtures::path("DEFAULT_SETTINGS");
                let scripts = default_path.parent().unwrap().parent().unwrap();
                let names = character_names(&image, &fixtures::path("LANGUAGE")).unwrap();
                let parameter_path = fixtures::path("PARAMETERS");
                let inputs = Inputs {
                    tree_locators: None,
                    image: &image,
                    board: &board,
                    file: &file,
                    people: openmt2001::people::PeopleModels::load(&image, &index, &archive)
                        .unwrap(),
                    vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                        &image, &index, &archive,
                    )
                    .unwrap(),
                    transport_models: openmt2001::traffic::models::Models::from_original(
                        &image, &index, &archive,
                    )
                    .unwrap(),
                    road_routes: read("ROAD_ROUTE_TABLE"),
                    scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(),
                    era_styles: load_era_styles(parameter_path.parent().unwrap()).unwrap(),
                    timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario: "SCENARIO3",
                    difficulty: 0,
                    selected_character: 4,
                    profile_name: b"Transport Clock",
                    character_names: &names,
                };
                let policy = Policy {
                    seed: 0x12345678,
                    auxiliary_prior_bytes: [0; 0x11c],
                };
                let mut session = Session::initialize(inputs, policy).unwrap();
                let assets = CommandAssets {
                    image: &image,
                    board: &board,
                    file: &file,
                };
                body(&mut session, &assets);
            })
            .unwrap()
            .join()
            .unwrap();
    });
}

/// Put the clock one normal tick before 10:00, when 44f2a0 creates train
/// route 5 and an unconditional aircraft. The global tick is moved well ahead
/// of the normal tick, as earlier auctions and other modes leave it.
fn just_before_ten(session: &mut Session<'_>) {
    let clock = &mut session.world.clock;
    clock.calendar[0] = 9;
    clock.timing[1] = (clock.timing[1] / 625 + 1) * 625 - 1;
    clock.timing[0] = clock.timing[1].wrapping_add(10_000);
    clock.timing[5] = 1;
}

fn iterate(session: &mut Session<'_>, assets: &CommandAssets<'_, '_>, count: u32) {
    let mut policy = super::tests::simulation_test_policy();
    for _ in 0..count {
        // Scenario tips pause the city; dismiss them as a player would.
        for _ in 0..8 {
            if !session.world.pause.has_reason() {
                break;
            }
            session
                .dispatch_session_event("InterfacePromptDismiss", 0, assets, false)
                .unwrap();
        }
        assert!(
            !session.world.pause.has_reason(),
            "a prompt kept the city paused"
        );
        let before = session.world.clock.timing[0];
        session
            .advance_session(assets, &mut policy, |_, _| Ok(0))
            .unwrap();
        assert_eq!(session.world.clock.timing[0], before.wrapping_add(1));
    }
}

fn render(session: &mut Session<'_>) -> Vec<[u8; 284]> {
    let mut submitted = Vec::new();
    session
        .advance_transport_frame(|auxiliary| {
            submitted.push(*auxiliary);
            Ok(())
        })
        .unwrap();
    submitted
}

fn active_aircraft(session: &Session<'_>) -> Vec<usize> {
    (0..8)
        .filter(|&i| word(&session.transport.aircraft[i], 0) != 0)
        .collect()
}

/// The descriptor translation an aircraft submission carries (44e000 copies
/// the locator transform's translation to descriptor +4).
fn translation(auxiliary: &[u8; 284]) -> [u32; 3] {
    std::array::from_fn(|axis| word(auxiliary, 4 + axis * 4))
}

#[test]
#[ignore = "requires original assets"]
fn aircraft_keep_flying_through_auction_mode_while_trains_wait() {
    with_session(|session, assets| {
        just_before_ten(session);
        iterate(session, assets, 1);
        assert_eq!(session.world.clock.hour(), 10);
        let aircraft = active_aircraft(session);
        assert_eq!(aircraft.len(), 1, "10:00 creates one aircraft");
        let slot = aircraft[0];
        // 44df70: start = global tick - 1, stamped during this iteration.
        let start = word(&session.transport.aircraft[slot], 16);
        assert_eq!(start, session.world.clock.timing[0].wrapping_sub(1));
        let locator = aircraft_locator(&session.transport.aircraft[slot]);
        let frames = session.transport_models.mesh(locator).unwrap().frames as u32;
        let expected = |session: &Session<'_>, elapsed: u32| {
            let matrix = session
                .transport_models
                .locator_matrix(locator, elapsed)
                .unwrap();
            std::array::from_fn::<_, 3, _>(|axis| matrix[9 + axis].to_bits())
        };
        let first = render(session);
        assert!(first.iter().any(|a| translation(a) == expected(session, 1)));

        let trains = session.transport.trains;
        assert!(
            trains.iter().any(|row| word(row, 0) != 0),
            "10:00 creates a train"
        );
        let ship_frame = session.transport.environment.frame;
        let normal = session.world.clock.timing[1];
        let global = session.world.clock.timing[0];

        // Auction mode: 481680 still increments the global tick each
        // iteration, but skips the city branch with 44ec50 and the normal tick.
        let auction_iterations = 100;
        assert!(auction_iterations + 1 < frames);
        session.world.clock.timing[5] = 2;
        iterate(session, assets, auction_iterations);
        assert_eq!(session.world.clock.timing[0], global + auction_iterations);
        assert_eq!(session.world.clock.timing[1], normal);
        assert_eq!(session.transport.trains, trains, "trains wait in auctions");
        assert_eq!(session.transport.environment.frame, ship_frame);

        session.world.clock.timing[5] = 1;
        let after = render(session);
        assert!(
            after
                .iter()
                .any(|a| translation(a) == expected(session, 1 + auction_iterations)),
            "the aircraft is drawn {auction_iterations} frames further along"
        );
        assert!(
            !after.iter().any(|a| translation(a) == expected(session, 1)),
            "the aircraft did not stay where the auction found it"
        );

        // A long enough auction outlasts the flight: the next city render
        // expires the aircraft without drawing it (44e000).
        session.world.clock.timing[5] = 2;
        iterate(session, assets, frames - 1 - auction_iterations);
        session.world.clock.timing[5] = 1;
        let elapsed = session.world.clock.timing[0].wrapping_sub(start);
        assert_eq!(elapsed, frames);
        let expired = render(session);
        assert_eq!(word(&session.transport.aircraft[slot], 0), 0);
        assert!(
            !expired
                .iter()
                .any(|a| translation(a) == expected(session, frames))
        );

        // Back in the city, trains move again on normal ticks.
        iterate(session, assets, 1);
        assert_eq!(session.world.clock.timing[1], normal + 1);
        assert_ne!(
            session.transport.trains, trains,
            "trains move on city ticks"
        );
    });
}

#[test]
#[ignore = "requires original assets"]
fn transport_state_does_not_depend_on_how_often_frames_are_drawn() {
    // One session drawn once per tick and one drawn five times per tick must
    // hold the same transport state and draw the same poses after every tick.
    let mut runs = Vec::new();
    for draws_per_tick in [1, 5] {
        let mut history = Vec::new();
        with_session(|session, assets| {
            just_before_ten(session);
            for _ in 0..250 {
                iterate(session, assets, 1);
                let mut last = Vec::new();
                for _ in 0..draws_per_tick {
                    last = render(session);
                }
                let state = openmt2001::session_save::transport::encode_parts(
                    &session.world.road_network,
                    &session.road_traffic,
                    &session.transport,
                )
                .unwrap();
                history.push((session.world.clock.timing, state, last));
            }
            assert!(!active_aircraft(session).is_empty());
            assert!(session.transport.trains.iter().any(|row| word(row, 0) != 0));
        });
        runs.push(history);
    }
    assert_eq!(runs[0].len(), runs[1].len());
    for (tick, (once, often)) in runs[0].iter().zip(&runs[1]).enumerate() {
        assert!(once == often, "transport differs after tick {tick}");
    }
}
