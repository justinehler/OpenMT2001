//! Objective regressions through process custody, Lua and the live 50-tick caller.
use super::*;
use openmt2001::{fixtures, live_commands::Command};

fn set_objective(session: &mut Session<'_>, assets: &CommandAssets<'_, '_>, source: &str) {
    session
        .scripts
        .runtime
        .execute(
            "objective test handler",
            format!("function CATCHEVENT_ObjectiveTest() {source} end").as_bytes(),
            &mut |_, _| panic!("defining a handler must not dispatch bindings"),
        )
        .unwrap();
    session
        .dispatch_session_event("ObjectiveTest", 0, assets, false)
        .unwrap();
}

fn periodic_tick(session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
    assert!(!session.world.pause.has_reason());
    // The production scheduler calls 0x4896e0 at the next multiple of 50.
    session.world.clock.timing[0] = (session.world.clock.timing[0] / 50 + 1) * 50 - 1;
    let before = session.world.clock.timing[0];
    let mut policy = super::tests::simulation_test_policy();
    session
        .advance_session(assets, &mut policy, |_, _| Ok(0))
        .unwrap();
    assert_eq!(session.world.clock.timing[0], before + 1);
}

fn outcomes(session: &Session<'_>) -> Vec<(i32, u32)> {
    session
        .live_commands
        .snapshot()
        .unwrap()
        .iter()
        .filter_map(|row| {
            if let Command::OutcomeNotice { player, reason } = row.4 {
                Some((*player, *reason))
            } else {
                None
            }
        })
        .collect()
}

#[test]
#[ignore = "requires original assets; two-city startup and periodic live objective regression"]
fn objective_city_replacement_and_nonfinite_live_targets() {
    // Native save reconstruction uses large fixed pools, as in the existing
    // cold reload tests; keep their larger test-thread stack here too.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(exercise_objective_city_replacement_and_nonfinite_live_targets)
        .unwrap()
        .join()
        .unwrap();
}

fn exercise_objective_city_replacement_and_nonfinite_live_targets() {
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
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let default_path = fixtures::path("DEFAULT_SETTINGS");
    let scripts = default_path.parent().unwrap().parent().unwrap();
    let names = character_names(&image, &fixtures::path("LANGUAGE")).unwrap();
    let parameter_path = fixtures::path("PARAMETERS");
    let inputs = || Inputs {
        tree_locators: None,
        image: &image,
        board: &board,
        file: &file,
        people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
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
        scenario: "NET1",
        difficulty: 1,
        selected_character: 4,
        profile_name: b"Objective Test",
        character_names: &names,
    };
    let policy = || Policy {
        seed: 0x12345678,
        auxiliary_prior_bytes: [0; 0x11c],
    };
    let launch = || {
        use openmt2001::skirmish::roster::{Kind, Participant};
        SkirmishLaunch {
            level_id: 300001,
            directory: "net1".into(),
            map: "empty.lua".into(),
            objective_kind: 6,
            target: 3500.,
            difficulty: 1,
            participants: vec![
                Participant {
                    kind: Kind::Human,
                    token: Some(4),
                    name: b"Objective Test".to_vec(),
                    ready: true,
                    difficulty: 2,
                },
                Participant {
                    kind: Kind::Ai,
                    token: Some(1),
                    name: b"Second Player".to_vec(),
                    ready: true,
                    difficulty: 0,
                },
            ],
        }
    };
    let assets = CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    let custody = crate::process_dispatch::DispatchCustody::startup(Default::default()).unwrap();
    let (seed, timing) = custody.prepare_start(true).unwrap();
    let mut first = Session::initialize_joined(
        inputs(),
        policy(),
        None,
        None,
        false,
        None,
        crate::compatibility::Mode::StrictV14,
        Some(launch()),
        None,
        Some(seed),
    )
    .unwrap();
    assert_eq!(
        first.world.objective.excess, [0; 6],
        "clean process starts with zeros"
    );

    // Both exceptional targets go through the actual Game.SetObjective binding
    // and Session::advance_session's periodic helper for all threshold kinds.
    for kind in [3, 5, 6] {
        for target in ["1e300", "0/0"] {
            set_objective(
                &mut first,
                &assets,
                &format!("Game.SetObjective({kind},{target},0)"),
            );
            assert_eq!(first.world.objective.flag, 0);
            assert!(if target == "1e300" {
                first.world.objective.target == f32::INFINITY
            } else {
                first.world.objective.target.is_nan()
            });
            first.world.objective.reached = [7, 6, 5, 4, 3, 2];
            first.world.objective.excess = [11, -12, 13, -14, 15, -16];
            first.world.clock.timing[0] = (first.world.clock.timing[0] / 50 + 1) * 50 - 1;
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.running = true;
            host.tick(&mut first, &assets);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            assert!(
                first.world.clock.timing[0].is_multiple_of(50),
                "Host ran the periodic evaluator"
            );
            assert_eq!(first.world.objective.reached, [7, 6, 5, 4, 3, 2]);
            assert_eq!(first.world.objective.excess, [11, -12, 13, -14, 15, -16]);
            assert!(outcomes(&first).is_empty());
            let before = first.world.clock.timing[0];
            host.tick(&mut first, &assets);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            assert!(
                first.world.clock.timing[0] > before,
                "live work continues after evaluation"
            );
        }
    }

    // Store unequal excess in city one via a real finite sales evaluation.
    first.world.objective.reached = [0; 6];
    first.world.objective.excess = [0, 0, 17, -3, i32::MIN, i32::MAX];
    first.world.ledgers.players[0].retail_counters[1] = 100;
    first.world.ledgers.players[1].retail_counters[1] = 200;
    set_objective(
        &mut first,
        &assets,
        "Game.SetObjective(6,50,0); Game.SetObjective(6,50)",
    );
    assert_eq!(
        first.world.objective.flag, 0,
        "omitted third argument preserves the byte"
    );
    periodic_tick(&mut first, &assets);
    assert_eq!(outcomes(&first), [(1, 2)]);
    let excess = [50, 150, 17, -3, i32::MIN, i32::MAX];
    assert_eq!(first.world.objective.excess, excess);
    first.world.objective.cash_statuses = [9; 6];
    first.world.objective.reserved = [0xa5; 15];
    let custody = crate::process_dispatch::custody_from_session(&mut first, timing).unwrap();
    drop(first);
    let (seed, timing) = custody.prepare_start(true).unwrap();
    let mut second = Session::initialize_joined(
        inputs(),
        policy(),
        None,
        None,
        false,
        None,
        crate::compatibility::Mode::StrictV14,
        Some(launch()),
        None,
        Some(seed),
    )
    .unwrap();
    assert_eq!(second.world.objective.excess, excess);
    assert_eq!(second.world.objective.reserved, [0xa5; 15]);
    assert_eq!(second.world.objective.cash_statuses, [0; 6]);
    assert_eq!(second.world.objective.reached, [0; 6]);
    assert_eq!(
        second.world.objective.flag, 1,
        "fresh reset precedes NET1's omitted flag setter"
    );

    // Empty map has no owned property, so cash supplies two distinct values
    // in the same date-objective integer bucket. The old excess breaks the tie.
    second.world.ledgers.players[0].cash = 100.25;
    second.world.ledgers.players[1].cash = 100.75;
    set_objective(&mut second, &assets, "Game.SetObjective(4,1930)");
    // Snapshot before queuing result presentation; native saves do not encode
    // pending outcome notices. Loading must restore this objective verbatim.
    let cameras = crate::save_menu::cameras(
        None,
        openmt2001::flyby::CameraSample {
            eye: [100., 120., 100.],
            target: [0., 0., 0.],
            up: [0., 1., 0.],
            vertical_fov: 0.7,
        },
    )
    .unwrap();
    let saved_objective = second.world.objective.to_bytes();
    let saved = second.save_v2(&image, cameras, 1).unwrap();
    periodic_tick(&mut second, &assets);
    assert_eq!(&second.world.objective.reached[..2], &[1, 1]);
    assert_eq!(second.world.objective.excess, excess);
    assert_eq!(
        outcomes(&second),
        [(1, 2)],
        "native command9 winner/reason identity"
    );

    // The departing city's newer values must not override the saved controller.
    second.world.objective.excess = [-777; 6];
    let custody = crate::process_dispatch::custody_from_session(&mut second, timing).unwrap();
    drop(second);
    let (seed, _) = custody.prepare_start(false).unwrap();
    let loaded = Session::load_original_joined(
        inputs(),
        policy(),
        &saved,
        None,
        None,
        None,
        None,
        None,
        Some(seed),
    )
    .unwrap();
    assert_eq!(loaded.world.objective.to_bytes(), saved_objective);
}
