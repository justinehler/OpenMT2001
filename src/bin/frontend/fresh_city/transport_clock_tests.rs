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

/// Scheduler boundaries for a test run: the auction room scene and camera
/// are prepared as the host would, and every other host call is a no-op.
fn boundary(event: tick::Event<'_>, _: &openmt2001::clock::Clock) -> Result<u32, String> {
    use openmt2001::{ai_messages::offers::auction, live_commands::Event};
    Ok(match event {
        tick::Event::Command(Event::AuctionScenePrepare)
        | tick::Event::Command(Event::AuctionActivation(auction::activation::Call::Camera)) => 1,
        _ => 0,
    })
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
            .advance_session(assets, &mut policy, boundary)
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
    use openmt2001::{ai_messages::offers::auction, live_commands};
    with_session(|session, assets| {
        // Keep the opponents out of strategy so only the auction runs.
        for player in &mut session.roster.manager.players {
            if word(&player.bytes, 0x154) == 1 {
                player.bytes[0x1403] = 1;
            }
        }
        // A real auction: the 0x1d announcement is admitted 25 ticks after
        // submission, and its 0x1e start, 3025 ticks later, enters mode 2.
        let category = (0..52)
            .find(|&i| session.world.categories[i][5] == 0)
            .unwrap();
        let initiator = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let mut descriptor = [0; 52];
        for (at, value) in [
            (4, 0x1d),
            (20, category as u32),
            (24, initiator as u32),
            (28, 100),
        ] {
            descriptor[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        descriptor[16] = 1;
        let admitted_at = session.world.clock.timing[0] + 25;
        session
            .submit_command(
                live_commands::Command::Auction { descriptor },
                assets,
                |_| Err("auction admission ran early".into()),
            )
            .unwrap();
        while session.world.clock.timing[0] < admitted_at {
            iterate(session, assets, 1);
        }
        assert_eq!(session.world.auction.controller.active_count, 1);

        // The last city tick before the auction is 10:00, which creates an
        // aircraft and a train (44f2a0).
        let auction_start = admitted_at + 3025;
        let clock = &mut session.world.clock;
        clock.calendar[0] = 9;
        clock.timing[1] = (clock.timing[1] / 625 + 1) * 625 - 1;
        clock.timing[0] = auction_start - 2;
        iterate(session, assets, 1);
        assert_eq!(session.world.clock.timing[5], 1);
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
        let before = render(session);
        assert!(
            before
                .iter()
                .any(|a| translation(a) == expected(session, 1))
        );
        let trains = session.transport.trains;
        assert!(
            trains.iter().any(|row| word(row, 0) != 0),
            "10:00 creates a train"
        );
        let ship_frame = session.transport.environment.frame;
        let normal = session.world.clock.timing[1];
        let global = session.world.clock.timing[0];

        // Auction mode: 481680 still increments the global tick each
        // iteration but skips the city branch, 44ec50 and the normal tick.
        iterate(session, assets, 1);
        assert_eq!(session.world.clock.timing[5], 2, "the auction started");
        iterate(session, assets, 99);
        assert_eq!(session.world.clock.timing[5], 2);
        // Every participant withdraws; the closure returns the city to mode 1.
        for _ in 0..6 {
            if session.world.clock.timing[5] != 2
                || word(&session.world.auction.controller.record, 0x60) != u32::MAX
            {
                break;
            }
            let participant = word(&session.world.auction.controller.record, 0x58);
            session
                .apply_auction_action(
                    auction::Command {
                        action: 2,
                        participant,
                        amount: 0,
                    },
                    assets,
                    |_| Ok(0),
                )
                .unwrap();
        }
        // Commands run in 480ec0 before the mode dispatch, so the iteration
        // whose closure restores mode 1 already runs the city branch.
        let mut closing = 0;
        loop {
            iterate(session, assets, 1);
            if session.world.clock.timing[5] != 2 {
                break;
            }
            assert_eq!(session.world.clock.timing[1], normal, "no city ticks ran");
            assert_eq!(session.transport.trains, trains, "trains wait in auctions");
            assert_eq!(session.transport.environment.frame, ship_frame);
            closing += 1;
            assert!(closing < 400, "the auction did not close");
        }
        assert_eq!(session.world.clock.timing[1], normal + 1);
        assert_ne!(
            session.transport.trains, trains,
            "trains move on city ticks"
        );
        let auction_iterations = session.world.clock.timing[0] - global - 1;
        assert!(auction_iterations >= 100);
        assert!(
            auction_iterations + 2 < frames,
            "the flight outlasts the auction"
        );

        // Back in the city, the aircraft is drawn as many frames further on
        // as the auction ran scheduler iterations (plus the closing tick).
        let elapsed = session.world.clock.timing[0].wrapping_sub(start);
        assert_eq!(elapsed, auction_iterations + 2);
        let after = render(session);
        assert!(
            after
                .iter()
                .any(|a| translation(a) == expected(session, elapsed)),
            "the aircraft is drawn {auction_iterations} frames further along"
        );
        assert!(
            !after.iter().any(|a| translation(a) == expected(session, 1)),
            "the aircraft did not stay where the auction found it"
        );

        // Once the global tick reaches the locator frame count, the next
        // render expires the aircraft without drawing it (44e000).
        let remaining = frames - session.world.clock.timing[0].wrapping_sub(start);
        session.world.clock.timing[0] += remaining;
        let expired = render(session);
        assert_eq!(word(&session.transport.aircraft[slot], 0), 0);
        assert!(
            !expired
                .iter()
                .any(|a| translation(a) == expected(session, frames))
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
