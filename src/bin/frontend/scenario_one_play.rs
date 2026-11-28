//! Original Scenario1 date/rank objective after the real Scenario12 Win.
use super::scenario_twelve_play::{export, wealth};
use super::{
    CommandAssets, Inputs, Path, Policy, Session, character_names, finish_original_win, fs,
    load_era_styles, submit_business_shape, validate_natural_owned_shoppers, word,
};

fn date(session: &Session<'_>) -> u32 {
    1930 + session.world.clock.calendar[2] * 10 + session.world.clock.calendar[1]
}

/// Preserve the proven five-shop strategy's legal demand-ranked location choice.
pub(super) fn build_ranked(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    business: u32,
    prior: &[i32],
) -> Option<i32> {
    let mut categories = (0..52usize).collect::<Vec<_>>();
    categories.sort_by_key(|&c| std::cmp::Reverse(word(&session.world.categories[c], 0x88)));
    for category in categories {
        if prior.contains(&(category as i32)) {
            continue;
        }
        let excluded = (0..52i32)
            .filter(|&c| c != category as i32)
            .collect::<Vec<_>>();
        if let Some(built) = submit_business_shape(session, assets, business, &excluded, None) {
            return Some(built);
        }
    }
    None
}

#[test]
#[ignore = "requires original assets and the real Scenario12 Win profile"]
fn legal_scenario_one_date_rank_win_and_profile_reload() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let build = read("OPENMT2001_BUILD_FILE");
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe = openmt2001::fixtures::path("EXE");
        let scripts = exe.parent().unwrap().join("scripts");
        let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        let profile = crate::campaign_store::checkpoint_profile(&image, &scripts, 200001);
        let before_profile = fs::read(&profile).unwrap();
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
        let level = &catalog.levels[catalog.find(200001).unwrap()];
        assert!(level.unlocked()); assert_eq!(level.attempts(), 0);
        assert!(!catalog.levels[catalog.find(200006).unwrap()].unlocked());
        for id in [200003, 200004, 200005, 200002, 200012] {
            let previous = &catalog.levels[catalog.find(id).unwrap()];
            assert_eq!(previous.attempts(), 1); assert!(previous.completed()[0]);
        }
        let mut session = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO1", difficulty: 0,
            selected_character: 4, profile_name: b"Scenario3 Legal Sales", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }, catalog, profile.clone()).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        assert_eq!((session.world.objective.kind, session.world.objective.target), (4, 1945.));
        assert_eq!(session.world.ledgers.players[0].cash, 8000.);
        assert_eq!(session.roster.manager.players.len(), 4);
        assert_eq!(wealth(&session, &image).to_f32(), 8000.);
        let mut host = crate::live_host::Host::new([0., 20., 0.]);
        host.set_running(true);
        let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
        host.auction_renderer = Some(sender);
        let presenter = std::thread::spawn(move || {
            let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
            let dir = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
            let bin = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view = crate::auction_bridge::View::default(); let mut counts = [0; 3];
            while let Ok(request) = receiver.recv() {
                let index = match &request.operation {
                    crate::auction_bridge::Operation::Prepare { .. } => 0,
                    crate::auction_bridge::Operation::Activate => 1,
                    crate::auction_bridge::Operation::Restore => 2,
                };
                let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                if result.is_ok() { counts[index] += 1; }
                request.reply.send(result).unwrap();
            }
            (counts, view.scene.is_some())
        });
        let plan = [20, 21, 11, 13, 26];
        let mut built = Vec::new(); let mut owned_shopper = false;
        let mut previous_hour = u32::MAX; let mut result_dialog = None;
        for frame in 0..180000 {
            if frame % 625 == 0 && built.len() < plan.len()
                && let Some(category) = build_ranked(&mut session, &assets, plan[built.len()], &built) {
                built.push(category);
            }
            if !owned_shopper && frame % 125 == 0 {
                owned_shopper = validate_natural_owned_shoppers(&session, &image);
            }
            host.frame(&mut session, &assets, 40);
            assert!(host.failure.is_none(), "actual Scenario1 Host boundary: {:?}", host.failure);
            let status = host.status(&session);
            if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                host.auction_button(&mut session, &assets, 3).unwrap();
            }
            for prompt in status.prompts {
                if let Some(dialog) = prompt.dialog {
                    if dialog.callback == 0x48b490 { result_dialog = Some(dialog); break; }
                    host.human_dialog_reply(&mut session, &assets,
                        crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context,
                            button: u32::from(dialog.button_count == 2) }).unwrap();
                } else if prompt.dismissal_id != 0 {
                    session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                        &assets, false).unwrap();
                }
            }
            if previous_hour != session.world.clock.hour() {
                previous_hour = session.world.clock.hour();
                crate::session_validation::assert_live_links(&session);
                println!("SCENARIO1 date{} day{} hour{previous_hour} wealth{} cash{} legalshops{} reached{:?}",
                    date(&session), session.world.clock.calendar[4], wealth(&session, &image).to_f32(),
                    session.world.ledgers.players[0].cash, built.len(), session.world.objective.reached);
                if previous_hour == 23 {
                    export(&session, &image, &format!("SCENARIO1-DAY{}", session.world.clock.calendar[4]));
                }
            }
            if result_dialog.is_some() { break; }
        }
        export(&session, &image, "SCENARIO1");
        let dialog = result_dialog.expect("bounded legal Scenario1 did not reach an original result");
        assert_eq!(dialog.context, 0, "original dated wealth rank was won by an opponent");
        assert!(date(&session) >= 1945, "survival result does not establish the original date objective");
        assert_eq!(session.world.objective.reached[0], 1, "original dated rank must select the human");
        assert_eq!((session.world.objective.kind, session.world.objective.target), (4, 1945.));
        assert_eq!(built.len(), 5); assert!(owned_shopper);
        assert_eq!(fs::read(&profile).unwrap(), before_profile);
        host.human_dialog_reply(&mut session, &assets,
            crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context, button: 0 }).unwrap();
        finish_original_win(&mut session, &assets, &mut host, &profile, &before_profile);
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
        for id in [200003, 200004, 200005, 200002, 200012, 200001] {
            let level = &catalog.levels[catalog.find(id).unwrap()];
            assert_eq!(level.attempts(), 1); assert!(level.completed()[0]);
        }
        assert!(catalog.levels[catalog.find(200006).unwrap()].unlocked());
        drop(host); let (counts, active) = presenter.join().unwrap();
        assert!(!active); assert_eq!(counts[0], counts[1]); assert_eq!(counts[1], counts[2]);
        println!("Natural Scenario1 dated wealth Win once, Scenario6 unlocked; auctions{counts:?}");
    }).unwrap().join().unwrap();
}
