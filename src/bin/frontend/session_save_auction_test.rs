//! Original-asset natural opponent continuation; no synthetic pool mutations.
use super::*;

#[test]
#[ignore = "requires original v1.4 game assets and audited scripts"]
fn natural_auction_countdown_and_bidding_resume() {
    natural_auction_resume(0);
}
#[test]
#[ignore = "requires original v1.4 game assets and audited scripts"]
fn strategic_auction_countdown_and_bidding_resume() {
    natural_auction_resume(3);
}
fn natural_auction_resume(prior_mode: u32) {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(move || {
        use crate::fresh_city::{CommandAssets, Inputs, Policy};
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = std::sync::Arc::new(read("OPENMT2001_EXE"));
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let directory = std::sync::Arc::new(read("OPENMT2001_ARCHIVE_DIR"));
        let archive = std::sync::Arc::new(read("OPENMT2001_ARCHIVE_BIN"));
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let default = std::path::PathBuf::from(std::env::var("OPENMT2001_DEFAULT_SETTINGS").unwrap());
        let scripts = default.parent().unwrap().parent().unwrap();
        let styles = crate::fresh_city::load_era_styles(std::path::Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap();
        let executable_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let names = crate::fresh_city::character_names(&image,
            &executable_path.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        assert!(names.iter().all(|name| !name.is_empty()), "supplied character names must be populated");
        let inputs = || Inputs {
            tree_locators: None,
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
        for player in &session.roster.manager.players {
            if word(&player.bytes, 0x154) == 1 {
                let name = &names[word(&player.bytes, 0x150) as usize];
                assert_eq!(&player.bytes[0x158..0x158+name.len()], name, "native AI name field must retain supplied language name");
            }
        }
        let mut host = crate::live_host::Host::new(camera.eye);
        host.begin_presentation(&session).unwrap();
        let (source_renderer, source_thread) = bridge(exe.clone(), directory.clone(), archive.clone(), "source", prior_mode);
        host.auction_renderer = Some(source_renderer); host.running = true;
        if prior_mode == 3 {
            let bytes = session.save_v2_with_auction_view(&image, cameras, 1, host.auction_save_state(), 3).unwrap();
            let path = crate::save_menu::write_new(std::path::Path::new("work/save/v2-strategic-profile"), &bytes).unwrap();
            let save = SaveFile::parse(&bytes).unwrap();
            assert_eq!(save.game_mode().unwrap(), 1);
            assert!(save.auction().unwrap().is_none());
            let ext = extension::Extension::parse(&save).unwrap().unwrap();
            assert_eq!(ext.version, 4); assert!(ext.records.contains_key(&15)); assert!(ext.records.contains_key(&16)); assert!(!ext.records.contains_key(&14));
            let mut restored = Session::load_original(inputs(), policy(), &bytes).unwrap();
            let metadata = restored.restore_v2_extension(&save).unwrap().unwrap();
            assert_eq!(metadata.interface_mode, Some(3)); assert!(metadata.auction.is_none());
            assert_eq!(restored.saved_cameras, Some(cameras));
            let mut restored_host = crate::live_host::Host::new(camera.eye); restored_host.running = true;
            for _ in 0..100 {
                frame(&mut session, &mut host, &assets); frame(&mut restored, &mut restored_host, &assets);
                compare(&session, &mut host, &restored, &mut restored_host);
            }
            std::fs::write("work/save/normal-strategic-view-path.txt", path.to_string_lossy().as_bytes()).unwrap();
        }
        for _ in 0..3000 {
            frame(&mut session, &mut host, &assets);
            if session.world.auction.controller.reference != 0 && session.world.auction.presentation.countdown < 700 { break; }
        }
        assert_ne!(session.world.auction.controller.reference, 0, "natural auction never admitted");
        assert!(session.world.auction.presentation.countdown > 0, "announcement phase not observed");
        let root = std::path::Path::new("work/save/v2-auction-profile");
        let countdown_bytes = session.save_v2_with_auction_view(&image, cameras, 1, host.auction_save_state(), prior_mode).unwrap();
        let countdown_path = crate::save_menu::write_new(root, &countdown_bytes).unwrap();
        let countdown_bytes = std::fs::read(&countdown_path).unwrap();
        let countdown_save = SaveFile::parse(&countdown_bytes).unwrap();
        let mut resumed = Session::load_original(inputs(), policy(), &countdown_bytes).unwrap();
        let metadata = resumed.restore_v2_extension(&countdown_save).unwrap().unwrap();
        let continuation = metadata.auction.unwrap(); assert_eq!(continuation.game_mode, 1); assert_eq!(continuation.prior_mode, prior_mode);
        let mut resumed_host = crate::live_host::Host::new(camera.eye);
        let (reloaded_renderer, reloaded_thread) = bridge(exe.clone(), directory.clone(), archive.clone(), "reloaded", prior_mode);
        resumed_host.auction_renderer = Some(reloaded_renderer); resumed_host.running = true;
        resumed_host.activate_saved_auction(&mut resumed).unwrap();
        continuation.apply(&mut resumed).unwrap();
        resumed_host.restore_auction_widgets(continuation.host.as_ref(), &resumed).unwrap();
        for _ in 0..3000 {
            frame(&mut session, &mut host, &assets); frame(&mut resumed, &mut resumed_host, &assets);
            compare(&session, &mut host, &resumed, &mut resumed_host);
            let state = host.auction_save_state();
            if state.as_ref().is_some_and(|s| s.visible && s.controls.timer_visible && s.controls.timer.remaining < 790 && s.controls.buttons_enabled & 1 != 0) {
                #[cfg(feature = "capture")]
                if std::env::var_os("OPENMT2001_TIME_AUCTION_CAPTURE").is_some() {
                    crate::time_panel::capture_auction_status(&host.status(&session),"fresh");
                }
                let actor = crate::auction_panel::Info::from_session(&session).unwrap().actor;
                assert_ne!(actor[0], 0, "active auction bidder must have a visible supplied name");
                let bytes = session.save_v2_with_auction_view(&image, cameras, 2, state, prior_mode).unwrap();
                let path = crate::save_menu::write_new(root, &bytes).unwrap(); let active_path = path.clone();
                // Release the first loaded scene after its verified countdown activation.
                drop(resumed_host); reloaded_thread.join().unwrap();
                let bytes = std::fs::read(path).unwrap(); let save = SaveFile::parse(&bytes).unwrap();
                resumed = Session::load_original(inputs(), policy(), &bytes).unwrap();
                let continuation = resumed.restore_v2_extension(&save).unwrap().unwrap().auction.unwrap();
                assert_eq!(continuation.game_mode, 2); assert_eq!(continuation.prior_mode, prior_mode);
                resumed_host = crate::live_host::Host::new(camera.eye);
                resumed_host.begin_presentation(&resumed).unwrap();
                let (renderer, thread) = bridge(exe.clone(), directory.clone(), archive.clone(), "active-reloaded", prior_mode);
                resumed_host.auction_renderer = Some(renderer); resumed_host.running = true;
                resumed_host.activate_saved_auction(&mut resumed).unwrap();
                continuation.apply(&mut resumed).unwrap();
                resumed_host.restore_auction_widgets(continuation.host.as_ref(), &resumed).unwrap();
                #[cfg(feature = "capture")]
                if std::env::var_os("OPENMT2001_TIME_AUCTION_CAPTURE").is_some() {
                    crate::time_panel::capture_auction_status(&resumed_host.status(&resumed),"loaded");
                }
                compare(&session, &mut host, &resumed, &mut resumed_host);
                host.auction_button(&mut session, &assets, 0).unwrap();
                resumed_host.auction_button(&mut resumed, &assets, 0).unwrap();
                compare(&session, &mut host, &resumed, &mut resumed_host);
                let category = word(&session.world.auction.controller.record, 4) as usize;
                let mut closed = false;
                for _ in 0..6000 {
                    frame(&mut session, &mut host, &assets); frame(&mut resumed, &mut resumed_host, &assets);
                    compare(&session, &mut host, &resumed, &mut resumed_host);
                    if session.world.auction.controller.active_count == 0 && session.world.auction.controller.record[0x7c] == 0 && session.world.auction.controller.record[0x7d] == 0 { closed = true; break; }
                    if host.auction_save_state().is_some_and(|s| s.controls.buttons_enabled & (1 << 3) != 0) {
                        host.auction_button(&mut session, &assets, 3).unwrap(); resumed_host.auction_button(&mut resumed, &assets, 3).unwrap();
                    }
                }
                assert!(closed, "natural auction failed to settle");
                // Save before the delayed ownership transfer consumes its exact
                // admission deadline; native format deliberately has no auction.
                let dormant_bytes = session.save_v2_with_auction_view(&image, cameras, 1, host.auction_save_state(), prior_mode).unwrap();
                let dormant_path = crate::save_menu::write_new(root, &dormant_bytes).unwrap();
                let dormant_save = SaveFile::parse(&dormant_bytes).unwrap();
                assert!(dormant_save.auction().unwrap().is_none());
                resumed = Session::load_original(inputs(), policy(), &dormant_bytes).unwrap();
                let dormant = resumed.restore_v2_extension(&dormant_save).unwrap().unwrap().auction.unwrap();
                resumed_host.activate_saved_auction(&mut resumed).unwrap();
                dormant.apply(&mut resumed).unwrap();
                resumed_host.restore_auction_widgets(dormant.host.as_ref(), &resumed).unwrap();
                compare(&session, &mut host, &resumed, &mut resumed_host);
                for _ in 0..100 { frame(&mut session, &mut host, &assets); frame(&mut resumed, &mut resumed_host, &assets); compare(&session, &mut host, &resumed, &mut resumed_host); }
                assert_ne!(session.world.categories[category][5], 0, "settled winning bid must transfer ownership");
                println!("Normal auction host continuation: countdown {:?}, active {:?}, closed tick{}, owner{}", countdown_path, active_path, session.world.clock.timing[0], session.world.categories[category][5]);
                std::fs::create_dir_all("work/save").unwrap();
                std::fs::write(format!("work/save/auction-continuation-paths-{prior_mode}.txt"), format!("{}\n{}\n{}\n", countdown_path.display(), active_path.display(), dormant_path.display())).unwrap();
                drop(resumed_host); thread.join().unwrap();
                drop(host); source_thread.join().unwrap();
                return;
            }
        }
        panic!("natural auction never reached a live local bidding timer");
    }).unwrap().join().unwrap();
}
fn bridge(
    exe: std::sync::Arc<Vec<u8>>,
    directory: std::sync::Arc<Vec<u8>>,
    archive: std::sync::Arc<Vec<u8>>,
    label: &'static str,
    prior_mode: u32,
) -> (
    std::sync::mpsc::Sender<crate::auction_bridge::Request>,
    std::thread::JoinHandle<()>,
) {
    let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
    let thread = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view = crate::auction_bridge::View::default();
            for request in receiver {
                let activate = matches!(
                    request.operation,
                    crate::auction_bridge::Operation::Activate
                );
                let result = view.present(
                    request.operation,
                    &renderer,
                    (&exe, &directory, &archive),
                    prior_mode,
                );
                if activate && result.is_ok() {
                    crate::capture_test::capture_standalone(
                        &renderer,
                        &view.scene.as_ref().unwrap().0,
                        &format!("work/save/auction-{label}-{prior_mode}.png"),
                    );
                }
                request.reply.send(result).unwrap();
            }
        })
        .unwrap();
    (sender, thread)
}
fn frame(
    session: &mut Session<'_>,
    host: &mut crate::live_host::Host,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
) {
    host.frame(session, assets, 40);
    assert!(host.failure.is_none(), "{:?}", host.failure);
    for prompt in host.take_prompts(session) {
        if let Some(dialog) = prompt.dialog {
            host.human_dialog_reply(
                session,
                assets,
                crate::human_dialog::Reply {
                    callback: dialog.callback,
                    context: dialog.context,
                    button: if dialog.button_count == 2 { 1 } else { 0 },
                },
            )
            .unwrap();
        } else {
            session
                .dispatch_session_event(
                    "InterfacePromptDismiss",
                    prompt.dismissal_id as i32,
                    assets,
                    true,
                )
                .unwrap();
        }
    }
}
fn compare(
    source: &Session<'_>,
    host: &mut crate::live_host::Host,
    resumed: &Session<'_>,
    reloaded_host: &mut crate::live_host::Host,
) {
    let tick = source.world.clock.timing[0];
    assert_eq!(source.world.clock, resumed.world.clock, "clock tick{tick}");
    assert!(
        source.world.ledgers.businesses == resumed.world.ledgers.businesses,
        "businesses tick{tick}"
    );
    for (a, b) in source
        .roster
        .manager
        .players
        .iter()
        .zip(&resumed.roster.manager.players)
    {
        assert!(a.bytes == b.bytes, "player tick{tick}");
    }
    for (a, b) in source
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .zip(&resumed.world.ledgers.residents.nodes)
    {
        assert!(a.as_bytes() == b.as_bytes(), "resident tick{tick}");
    }
    let account_rows = |s: &Session<'_>| {
        s.world
            .ledgers
            .players
            .iter()
            .map(|account| {
                let mut bytes = vec![0; 0x1e50];
                account.write_runtime(&mut bytes).unwrap();
                bytes
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        account_rows(source),
        account_rows(resumed),
        "typed account/cash ledgers tick{tick}"
    );
    let auction_rows = |s: &Session<'_>| {
        let a = &s.world.auction;
        (
            a.controller.active_count,
            a.participant_ui,
            a.bid_ui,
            a.opening,
        )
    };
    assert_eq!(
        auction_rows(source),
        auction_rows(resumed),
        "auction state/UI rows tick{tick}"
    );
    assert_eq!(
        source.roster.profiles, resumed.roster.profiles,
        "AI profiles tick{tick}"
    );
    assert_eq!(
        source.world.auction.controller.record, resumed.world.auction.controller.record,
        "controller tick{tick}"
    );
    assert_eq!(
        source.world.auction.controller.reference,
        resumed.world.auction.controller.reference
    );
    let p = |s: &Session<'_>| {
        let p = &s.world.auction.presentation;
        (
            p.visible,
            p.selected,
            p.announcement,
            p.countdown,
            p.countdown_last,
        )
    };
    assert_eq!(p(source), p(resumed), "presentation tick{tick}");
    for (a, b) in source
        .world
        .categories
        .iter()
        .zip(&resumed.world.categories)
    {
        assert_eq!(&a[..12], &b[..12]);
        assert_eq!(&a[108..140], &b[108..140]);
    }
    let chains = |s: &Session<'_>| {
        openmt2001::building_pool::category_chains(
            &s.world.categories,
            &s.world.buildings,
            0x591318,
        )
        .unwrap()
    };
    assert_eq!(
        chains(source),
        chains(resumed),
        "ordered category membership tick{tick}"
    );
    let queues = |s: &Session<'_>| {
        s.live_commands
            .snapshot()
            .unwrap()
            .into_iter()
            .map(|(domain, outer, admission, direct, command)| {
                let domain = match domain {
                    openmt2001::scheduler::ClockDomain::Total => 0,
                    openmt2001::scheduler::ClockDomain::Normal => 1,
                };
                let bytes = match command {
                    Command::CategoryReady { category } => words([0xf000, *category]),
                    Command::EventCard(c) => words([0xf001, c.card, c.player, c.target as u32]),
                    Command::Eliminate { slot } => words([0xf002, *slot as u32]),
                    Command::OutcomeNotice { player, reason } => {
                        words([0xf003, *player as u32, *reason])
                    }
                    other => packet(other, admission).unwrap(),
                };
                (domain, outer, admission, direct, bytes)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        queues(source),
        queues(resumed),
        "live deadlines/order tick{tick}"
    );
    let a = host.status(source);
    let b = reloaded_host.status(resumed);
    assert_eq!(a.auction, b.auction, "auction widgets tick{tick}");
    assert_eq!(
        a.auction_seconds, b.auction_seconds,
        "announcement text tick{tick}"
    );
}
