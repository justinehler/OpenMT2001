//! Original-asset natural opponent continuation; no synthetic pool mutations.
use super::*;

#[test]
#[ignore = "requires original v1.4 game assets and audited scripts"]
fn natural_ai_session_continues_after_host_save() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        use crate::fresh_city::{CommandAssets, Inputs, Policy};
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let default = std::path::PathBuf::from(std::env::var("OPENMT2001_DEFAULT_SETTINGS").unwrap());
        let scripts = default.parent().unwrap().parent().unwrap();
        let styles = crate::fresh_city::load_era_styles(std::path::Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap();
        let names = std::array::from_fn(|_| Vec::new());
        let inputs = || Inputs {
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(), era_styles: styles.clone(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO1",
            difficulty: 0, selected_character: 4, profile_name: b"AI save fixture", character_names: &names,
        };
        let policy = || Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 284] };
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        let camera = openmt2001::flyby::CameraSample { eye: [100., 120., 100.], target: [0.; 3], up: [0., 1., 0.], vertical_fov: 0.7 };
        let cameras = crate::save_menu::cameras(None, camera).unwrap();
        let mut session = Session::initialize(inputs(), policy()).unwrap();
        assert_eq!(session.roster.manager.players.len(), 4, "original Scenario1 must include three live opponents");
        let mut host = crate::live_host::Host::new(camera.eye);
        host.running = true;
        while session.world.clock.timing[0] < 400 {
            host.tick(&mut session, &assets);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            for prompt in host.take_prompts(&session) {
                session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32, &assets, true).unwrap();
            }
        }
        let mut auction_admission = None;
        let bytes = session.save_v2(&image, cameras, 1).unwrap();
        let path = crate::save_menu::write_new(std::path::Path::new("work/save/v2-ai-roundtrip-profile"), &bytes).unwrap();
        let loaded = std::fs::read(path).unwrap();
        let save = SaveFile::parse(&loaded).unwrap();
        let mut resumed = Session::load_original(inputs(), policy(), &loaded).unwrap();
        for (slot, (saved, rebuilt)) in save.players().unwrap().iter().zip(&resumed.roster.manager.players).enumerate() {
            let changed: Vec<_> = saved.bytes.iter().zip(&rebuilt.bytes[4..444]).enumerate().filter_map(|(at, (a, b))| (a != b).then_some(at + 4)).collect();
            println!("Native load player {slot} derived prefix changes: {changed:x?}");
        }
        resumed.restore_v2_extension(&save).unwrap();
        let mut resumed_host = crate::live_host::Host::new(camera.eye);
        resumed_host.running = true;
        while session.world.clock.timing[0] < 1600 {
            host.tick(&mut session, &assets);
            resumed_host.tick(&mut resumed, &assets);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            assert!(resumed_host.failure.is_none(), "{:?}", resumed_host.failure);
            let tick = session.world.clock.timing[0];
            assert_eq!(session.world.clock, resumed.world.clock, "clock at tick {tick}");
            let left = host.take_prompts(&session);
            let right = resumed_host.take_prompts(&resumed);
            assert_eq!(left, right, "prompt callback deadline at tick {tick}");
            for prompt in left {
                session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32, &assets, true).unwrap();
                resumed.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32, &assets, true).unwrap();
            }
            assert_eq!(session.world.ledgers.businesses, resumed.world.ledgers.businesses, "business accounting at tick {tick}");
            assert_eq!(session.world.pending_world.to_bytes(), resumed.world.pending_world.to_bytes(), "construction queue at tick {tick}");
            assert_eq!(session.roster.profiles, resumed.roster.profiles, "live AI profiles at tick {tick}");
            for (slot, (left, right)) in session.world.categories.iter().zip(&resumed.world.categories).enumerate() {
                // Native writer fills12..108 with save-stream membership IDs;
                // loaded renderer geometry and list handles are reconstructed.
                assert_eq!(&left[..12], &right[..12], "category {slot} owner/membership at tick {tick}");
                assert_eq!(&left[108..140], &right[108..140], "category {slot} permission/economy at tick {tick}");
            }
            let chains = |session: &Session<'_>| openmt2001::building_pool::category_chains(&session.world.categories, &session.world.buildings, 0x591318).unwrap();
            assert_eq!(chains(&session), chains(&resumed), "ordered category building traversal at tick {tick}");
            assert_eq!(session.world.auction.controller.reference, resumed.world.auction.controller.reference, "auction admission at tick {tick}");
            assert_eq!(session.world.auction.controller.record, resumed.world.auction.controller.record, "auction controller at tick {tick}");
            assert_eq!(session.world.auction.controller.active_count, resumed.world.auction.controller.active_count);
            let presentation = |session: &Session<'_>| {
                let p = &session.world.auction.presentation;
                (p.ambient_handle, p.visible, p.selected, p.announcement, p.countdown, p.countdown_sound, p.countdown_last)
            };
            assert_eq!(presentation(&session), presentation(&resumed), "auction presentation at tick {tick}");
            let callbacks = |session: &mut Session<'_>| session.scripts.events.pending().iter().map(|event| {
                (event.remaining, event.argument, session.scripts.runtime.event_reference_identity(event.reference).unwrap())
            }).collect::<Vec<_>>();
            assert_eq!(callbacks(&mut session), callbacks(&mut resumed), "named pending callbacks at tick {tick}");
            if session.world.auction.controller.reference != 0 && auction_admission.is_none() { auction_admission = Some(tick); }
            for (left, right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                assert_eq!(left.bytes, right.bytes, "player allocation at tick {tick}");
            }
            for (left, right) in session.world.ledgers.residents.nodes.iter().zip(&resumed.world.ledgers.residents.nodes) {
                assert!(left.as_bytes() == right.as_bytes(), "resident allocation at tick {tick}");
            }
            // Examine portable runtime sections at regular checkpoints. These
            // are continued-versus-reloaded live state, not a native fidelity oracle.
            if tick.is_multiple_of(100) && session.world.auction.controller.reference == 0 {
                let left = session.save_v2(&image, cameras, 1).unwrap();
                let right = resumed.save_v2(&image, cameras, 1).unwrap();
                let left_save = SaveFile::parse(&left).unwrap();
                let right_save = SaveFile::parse(&right).unwrap();
                let left_extension = extension::Extension::parse(&left_save).unwrap().unwrap();
                let right_extension = extension::Extension::parse(&right_save).unwrap().unwrap();
                for section in [2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13] {
                    assert_eq!(left_extension.required(section).unwrap(), right_extension.required(section).unwrap(), "runtime section {section} at tick {tick}");
                }
            }
        }
        assert!(auction_admission.is_some(), "natural original opponents made no auction decision");
        println!("V2 natural Scenario1 save: three opponents, save tick400, identical live player/resident/accounting/AI/queue/RNG continuation to tick1600; natural auction admitted at {auction_admission:?}, final calendar {:?}", session.world.clock.calendar);
    }).unwrap().join().unwrap();
}
