use super::*;

#[test]
#[ignore = "requires original scripts and assets"]
fn selected_income_survives_between_tick_and_hourly_script_edits() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
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
    let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
    let scripts = default.parent().unwrap().parent().unwrap();
    let exe = openmt2001::fixtures::path("EXE");
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let names = character_names(&image, &language).unwrap();
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
        era_styles: load_era_styles(openmt2001::fixtures::path("PARAMETERS").parent().unwrap())
            .unwrap(),
        timetable: std::str::from_utf8(&timetable).unwrap(),
        scenario: "SCENARIO3",
        difficulty: 0,
        selected_character: 4,
        profile_name: b"Income test",
        character_names: &names,
    };
    let assets = CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    for hourly_edit in [false, true] {
        let mut session = Session::initialize(
            inputs(),
            Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 0x11c],
            },
        )
        .unwrap();
        let selected = session.world.settings.selected_income();
        assert_eq!((selected.multiplier, selected.daily_freight), (1., 800.));
        assert_eq!(session.population.properties.totals, [800.; 4]);
        let edit = "Tweaks.railutilincomeEASY=2; Tweaks.dailyfreightEASY=1600;";
        if hourly_edit {
            session
                .scripts
                .runtime
                .execute(
                    "income callback",
                    format!("function CATCHEVENT_HourlyUpdate(a) {edit} end;").as_bytes(),
                    &mut |_, _| Err("declaration called host".into()),
                )
                .unwrap();
        } else {
            let mut vm = openmt2001::lua4::Runtime::new().unwrap();
            let bindings = openmt2001::startup_settings::Bindings::register(&mut vm, 600).unwrap();
            vm.execute("between tick edit", edit.as_bytes(), &mut |id, args| {
                bindings.handle(id, args, &mut session.world.settings)
            })
            .unwrap();
        }
        // A controlled one-owner railroad makes its payout ledger exact.
        for category in [2, 35, 39, 42, 4, 17, 43, 44] {
            session.world.categories[category][5] = 0;
        }
        session.world.categories[2][5] = 1;
        session.world.categories[4][5] = 1;
        let mut policy = super::tests::simulation_test_policy();
        session.world.pause.bytes = [0; 9];
        for (reapply, freight, property_payout) in [
            (false, 800., 200.),
            (false, 800., 200.),
            (true, 1600., 800.),
        ] {
            if reapply {
                session.world.settings.apply_difficulty(session.difficulty);
            }
            session.world.settings.selected_income().reset_income(
                &mut session.hooks.utilities,
                &mut session.population.properties,
            );
            session.hooks.utilities.totals = [400., 0., 0., 0.];
            session.world.clock.calendar[0] = 5;
            session.world.clock.timing[1] = 624;
            session.world.clock.timing[2] = 624;
            session.world.clock.timing[5] = 1;
            let before = session.world.ledgers.players[0].ledger(29).unwrap();
            session
                .advance_session(&assets, &mut policy, |_, _| Ok(0))
                .unwrap();
            assert_eq!(session.world.clock.hour(), 6);
            assert_eq!(session.world.settings.rail_income(0), Some(2.));
            assert_eq!(session.world.settings.daily_freight(0), Some(1600.));
            assert_eq!(
                session.world.ledgers.players[0].ledger(29).unwrap() - before,
                property_payout
            );
            assert_eq!(session.population.properties.totals, [freight; 4]);
        }
        // Both fresh and cold-load bootstrap run this same phase: select from
        // DEFAULT, then let level Lua edit the configuration without publishing.
        struct NoGeometry;
        impl openmt2001::building_pool::GeometryBinding for NoGeometry {
            fn prepare(&mut self, _: usize, _: &mut [u8; 916]) -> Result<(), String> {
                Err("income startup must not build geometry".into())
            }
            fn model_height(&mut self, _: u32) -> Result<f32, String> {
                Err("income startup must not query geometry".into())
            }
        }
        let context = session
            .world
            .script_context(
                &image,
                &session.resident_models,
                level_startup::HostContext {
                    difficulty: 0,
                    notifications: false,
                    accounting_enabled: false,
                    auxiliary_template: [0; 0x11c],
                    category_base: 0x617628,
                    building_base: 0x591318,
                },
            )
            .unwrap();
        level_startup::Scripts::new(&image).unwrap().initialize(
            level_startup::Sources {
                default_hub: b"Tweaks.railutilincomeEASY=1; Tweaks.dailyfreightEASY=800;",
                scenario_name: "income startup",
                scenario_hub: b"Tweaks.railutilincomeEASY=2; Tweaks.dailyfreightEASY=1600; function EVENTINITIALISE_Level(a) end;",
            },
            &mut session.world.script_state(&[None; 6]), &context, &mut NoGeometry, &mut session.hooks,
        ).unwrap();
        assert_eq!(session.world.settings.rail_income(0), Some(2.));
        let selected = session.world.settings.selected_income();
        assert_eq!((selected.multiplier, selected.daily_freight), (1., 800.));
    }
}
