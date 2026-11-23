// Terms continuation from the legally generated 68-tenant V3 snapshot.
use super::*;
use crate::fresh_city::{character_names, load_era_styles, CommandAssets, Inputs, Policy, Session};

fn occupied(session: &Session<'_>, business: usize) -> u16 {
    u16::from_le_bytes(session.world.ledgers.businesses[business][0x10..0x12].try_into().unwrap())
}
fn compare(source: &Session<'_>, resumed: &Session<'_>) {
    assert_eq!(source.world.clock, resumed.world.clock);
    for (index, (a, b)) in source.world.ledgers.businesses.iter().zip(&resumed.world.ledgers.businesses).enumerate() {
        assert_eq!(a, b, "business{index}");
    }
    assert_eq!(source.world.available, resumed.world.available);
    for (index, (a, b)) in source.world.ai_demand.rows.iter().zip(&resumed.world.ai_demand.rows).enumerate() {
        assert_eq!(a, b, "demand row{index}");
    }
    assert_eq!(source.world.ai_demand.ready, resumed.world.ai_demand.ready);
    assert_eq!(source.world.ledgers.residents.active_count, resumed.world.ledgers.residents.active_count);
    for (a, b) in source.world.ledgers.residents.nodes.iter().zip(&resumed.world.ledgers.residents.nodes) {
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert_eq!(a.active_byte(), b.active_byte());
        assert_eq!(a.auxiliary_reference(), b.auxiliary_reference());
    }
    for (a, b) in source.world.ledgers.residents.accounts.iter().zip(&resumed.world.ledgers.residents.accounts) {
        assert_eq!(a.as_bytes(), b.as_bytes());
    }
    for (a, b) in source.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
        assert_eq!(a.bytes, b.bytes);
    }
    assert_eq!(source.world.creation.auxiliary.used(), resumed.world.creation.auxiliary.used());
    for (index, (a, b)) in source.world.creation.auxiliary.slots().iter().zip(resumed.world.creation.auxiliary.slots()).enumerate() {
        assert_eq!(a, b, "resident auxiliary{index}");
    }
    assert_eq!(source.world.creation.initialization_rng.constructor, resumed.world.creation.initialization_rng.constructor);
    assert_eq!(source.world.creation.initialization_rng.movement, resumed.world.creation.initialization_rng.movement);
    assert_eq!(source.world.creation.initialization_rng.objective, resumed.world.creation.initialization_rng.objective);
    assert_eq!(source.road_traffic.admission.random, resumed.road_traffic.admission.random);
    for (a, b) in source.world.categories.iter().zip(&resumed.world.categories) {
        assert_eq!(a[5], b[5]);
        assert_eq!(&a[0x6c..0x8c], &b[0x6c..0x8c]);
    }
    assert_eq!(openmt2001::building_pool::category_chains(&source.world.categories, &source.world.buildings, 0x591318).unwrap(),
        openmt2001::building_pool::category_chains(&resumed.world.categories, &resumed.world.buildings, 0x591318).unwrap());
}

#[test]
#[ignore = "requires original assets and legal_rental_eviction_rehousing_and_next_income V3 provenance"]
fn modern_overdue_terms_resume_real_tenants() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let exe = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts = exe.parent().unwrap().join("scripts");
        let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        let styles = load_era_styles(std::path::Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap();
        let inputs = || Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(), era_styles: styles.clone(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO3", difficulty: 0,
            selected_character: 4, profile_name: b"Rental turnover validation", character_names: &names,
        };
        let policy = || Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 284] };
        let path = std::fs::read_to_string("work/save/legal-rental68-v3-path.txt").unwrap();
        let provenance = std::fs::read(path).unwrap();
        let saved = SaveFile::parse(&provenance).unwrap();
        assert!(matches!(extension::Extension::parse(&saved).unwrap().unwrap().version, 3 | 4));
        let mut source = Session::load_original(inputs(), policy(), &provenance).unwrap();
        source.restore_v2_extension(&saved).unwrap();
        assert_eq!(source.world.clock.calendar[4], 1);
        assert_eq!(source.world.clock.hour(), 7);
        let current = source.roster.manager.players.iter().position(|p| p.reference == source.roster.status.current_player).unwrap();
        let owner = source.roster.manager.players[current].bytes[0x1bc] + 1;
        let business = source.world.ledgers.businesses.iter().position(|b|
            b[0x21c] != 0 && b[10] == owner && word(b, 4) == 42 && u16::from_le_bytes(b[0x10..0x12].try_into().unwrap()) == 68).unwrap();
        let target = crate::business_edit::Target {
            business, building: usize::from(u16::from_le_bytes(source.world.ledgers.businesses[business][8..10].try_into().unwrap())),
        };
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        for _ in 0..600 {
            crate::business_edit::edit(&mut source, &assets, target, crate::business_edit::Action::Rent { increase: true }).unwrap();
        }
        // Use the actual submission producer, with a controlled due-now queue
        // boundary with an already elapsed inner deadline instead of its
        // ordinary UI delay25. No clock/pool edits.
        let mut commands = Vec::new();
        openmt2001::economy::submit_business_terms(&mut source.world.ledgers.businesses, i32::from(owner) - 1,
            |command, _| { commands.push(command); Ok(()) }).unwrap();
        assert_eq!(commands.len(), 1);
        let due = source.world.clock.timing[0];
        assert!(due > 0, "occupied rental fixture must be past the initial tick");
        source.live_commands.restore_queued(Command::Terms(commands.pop().unwrap()),
            openmt2001::scheduler::ClockDomain::Total, due, due - 1, false);
        let bytes = source.save_v2(&image, source.saved_cameras.unwrap(), 1).unwrap();
        let saved = SaveFile::parse(&bytes).unwrap();
        assert_eq!(verified_modern_presentation_queue(&saved).unwrap(), [true]);
        // Removing only the V4 extension restores the original synchronous
        // admission boundary; this does not broaden tenant18 native loading.
        let extension_at = saved.sections.iter().find(|section| section.tag == extension::TAG).unwrap().offset;
        let mut native_only = bytes[..extension_at].to_vec();
        native_only.extend_from_slice(&bytes[bytes.len() - 8..]);
        assert!(Session::load_original(inputs(), policy(), &native_only)
            .err().unwrap().contains("synchronous tenant callbacks"));
        let slot = crate::save_menu::write_new(std::path::Path::new("work/save/terms-roundtrip-profile"), &bytes).unwrap();
        let bytes = std::fs::read(slot).unwrap();
        let saved = SaveFile::parse(&bytes).unwrap();
        let mut resumed = Session::load_original(inputs(), policy(), &bytes).unwrap();
        resumed.restore_v2_extension(&saved).unwrap();
        compare(&source, &resumed);
        assert_eq!(occupied(&resumed, business), 68, "load must not evict tenants early");
        assert_eq!(resumed.live_commands.snapshot().unwrap().len(), 1);
        let mut left = crate::live_host::Host::new([0., 20., 0.]); left.set_running(true);
        let mut right = crate::live_host::Host::new([0., 20., 0.]); right.set_running(true);
        for _ in 0..2 {
            left.tick(&mut source, &assets); right.tick(&mut resumed, &assets);
            assert!(left.failure.is_none(), "{:?}", left.failure);
            assert!(right.failure.is_none(), "{:?}", right.failure);
            compare(&source, &resumed);
        }
        assert_eq!(occupied(&source, business), 0);
        assert!(source.world.available.contains(&business));
        assert!(source.live_commands.snapshot().unwrap().is_empty());
        assert!(resumed.live_commands.snapshot().unwrap().is_empty());
        let start_hour = source.world.clock.hour();
        for _ in 0..2000 {
            left.frame(&mut source, &assets, 40); right.frame(&mut resumed, &assets, 40);
            assert!(left.failure.is_none(), "{:?}", left.failure);
            assert!(right.failure.is_none(), "{:?}", right.failure);
            compare(&source, &resumed);
            if source.world.clock.hour() != start_hour { break; }
        }
        assert_ne!(source.world.clock.hour(), start_hour, "next natural hour not reached");
        println!("V4 overdue terms:68 real tenants retained at load, evicted once, identical demand/accounts/availability and next-hour continuation");
    }).unwrap().join().unwrap();
}
