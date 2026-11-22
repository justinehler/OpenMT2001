// Legal original Scenario2 acquisition, then actual saved-worker Rights requests.
use super::*;

fn rights_acquisition_services(
    host: &mut crate::live_host::Host,
    inputs: Arc<SceneInputs>,
) -> (
    std::thread::JoinHandle<()>,
    Arc<std::sync::atomic::AtomicBool>,
) {
    let (graphics_tx, graphics_rx) = mpsc::channel::<crate::auction_bridge::Request>();
    let (sound_tx, sound_rx) = mpsc::channel::<crate::gameplay_sound::Request>();
    let (auction_tx, auction_rx) = mpsc::channel::<crate::live_host::auction_audio::Request>();
    let (era_tx, era_rx) = mpsc::channel::<crate::era_audio::Request>();
    let (transport_tx, transport_rx) = mpsc::channel::<crate::transport_audio::Request>();
    host.auction_renderer = Some(graphics_tx);
    host.gameplay_sound = Some(sound_tx);
    host.auction_audio = Some(auction_tx);
    host.era_audio = Some(era_tx);
    host.transport_audio = Some(transport_tx);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let service_done = done.clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    let service = std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(move || {
        let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        let mut view = crate::auction_bridge::View::default();
        let mut sound = crate::gameplay_sound::Player::new(
            crate::gameplay_sound::Assets::load(&image, &inputs.installer).unwrap());
        let mut auction = crate::live_host::auction_audio::Player::new(
            crate::live_host::auction_audio::Assets::load(&image, &inputs.installer).unwrap());
        let ffmpeg = std::env::var_os("OPENMT2001_FFMPEG").map(PathBuf::from).unwrap_or_else(|| {
            if cfg!(windows) { "work/python/Lib/site-packages/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe".into() }
            else { "ffmpeg".into() }
        });
        let mut era = crate::era_audio::Player::new(
            crate::era_audio::Assets::load(&image, &inputs.installer, &ffmpeg).unwrap());
        let mut transport = crate::transport_audio::Player::new(
            crate::transport_audio::Assets::load(&image, &inputs.installer).unwrap());
        ready_tx.send(()).unwrap();
        while !service_done.load(std::sync::atomic::Ordering::Acquire) {
            while let Ok(request) = graphics_rx.try_recv() {
                let result = view.present(request.operation, &renderer,
                    (&inputs.executable, &inputs.directory, &inputs.archive), 0);
                let _ = request.reply.send(result);
            }
            while let Ok(request) = sound_rx.try_recv() {
                let _ = request.reply.send(sound.apply(request.operation));
            }
            while let Ok(request) = auction_rx.try_recv() {
                let _ = request.reply.send(auction.apply(request.operation));
            }
            while let Ok(request) = era_rx.try_recv() {
                let _ = request.reply.send(era.apply(request.operation));
            }
            while let Ok(request) = transport_rx.try_recv() { transport.apply(request.operation); }
            era.poll().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }).unwrap();
    ready_rx
        .recv_timeout(std::time::Duration::from_secs(60))
        .unwrap();
    (service, done)
}

fn rights_acquisition_prompts(
    host: &mut crate::live_host::Host,
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
) {
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
        } else if prompt.dismissal_id != 0 {
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

#[test]
#[ignore = "requires original Scenario2/assets and the retained saved worker"]
fn saved_worker_rights_input_and_native_delay() {
    let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
    let executable = read("OPENMT2001_EXE");
    let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
    let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
    let language = exe_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let inputs = Arc::new(SceneInputs {
            compatibility: crate::compatibility::Mode::StrictV14,
        style: test_board_style(&executable),
        time_runtime: Default::default(),
        profile_root: PathBuf::from("work/frontend/rights-worker-profile"),
        character_names: crate::fresh_city::character_names(
            &openmt2001::pe::PeImage::parse(&executable).unwrap(),
            &language,
        )
        .unwrap(),
        installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
        language,
        maps: PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap())
            .parent()
            .unwrap()
            .to_path_buf(),
        executable,
        build_file: read("OPENMT2001_BUILD_FILE"),
        directory: read("OPENMT2001_ARCHIVE_DIR"),
        archive: read("OPENMT2001_ARCHIVE_BIN"),
        suncols: read("OPENMT2001_SUNCOLS"),
    });
    let index =
        openmt2001::archive::ArchiveIndex::parse(&inputs.directory, inputs.archive.len() as u64)
            .unwrap();
    let camera = crate::navigation::Navigation::load(&index, &inputs.archive, &inputs.executable)
        .unwrap()
        .camera;
    let prepare_inputs = inputs.clone();
    let (path, category, controlled) = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let image = openmt2001::pe::PeImage::parse(&prepare_inputs.executable).unwrap();
            let catalog =
                crate::campaign_store::load_catalog(&image, prepare_inputs.maps.parent().unwrap())
                    .unwrap();
            let setup = crate::singleplayer::Setup {
                preview: crate::singleplayer::Preview::Scenario(200002),
                difficulty: crate::singleplayer::Difficulty::Easy,
                // Match the authored Scenario2 HUMAN_PLAYER/HAT registration;
                // Shoe2 is also an authored AI and is not this acquisition recipe.
                character_index: 4,
                profile_name: b"Rights worker".to_vec(),
                profile_path: crate::campaign_store::profile_path(
                    &prepare_inputs.profile_root,
                    b"Rights worker",
                ),
                catalog,
            };
            let (_, mut session) = prepare_inputs
                .fresh_preview(setup, camera, &mut SnapshotCache::default())
                .unwrap();
            //94 is outside the saved category prefix: original startup style
            //must survive both the initial bind and the saved-worker reload.
            assert_eq!(u32::from_le_bytes(session.world.categories[23][0x94..0x98]
                .try_into().unwrap()), 0xfff9f400);
            let board = prepare_inputs.styled_board(&image).unwrap();
            let file_bytes = prepare_inputs.build_file.clone();
            let file = openmt2001::build_file::BuildFile::parse(&file_bytes).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let mut host = crate::live_host::Host::new(camera.eye);
            let (service, done) = rights_acquisition_services(&mut host, prepare_inputs.clone());
            host.running = true;
            // Allow the ordinary first-day scheduler to establish the original
            // auction participants. An immediate startup offer is legally
            // acquired uncontested, so it cannot exercise a human bid button.
            for _ in 0..12000 {
                if session.world.clock.timing[0] >= 6875 { break; }
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "{:?}", host.failure);
                rights_acquisition_prompts(&mut host, &mut session, &assets);
            }
            assert!(session.world.clock.timing[0] >= 6875,
                "authored first-day scheduler stalled before the observed offer time");
            // Authored Scenario2 initialpref enables category0 for leasing.
            // Original Scenario3 deliberately leaves Pacific unavailable.
            let category = 0;
            assert_eq!(session.world.categories[category][5], 0);
            let inspection = host
                .inspect_property_offer(
                    &mut session,
                    &assets,
                    category as u32,
                    openmt2001::property_offer::Panel::default(),
                    true,
                )
                .unwrap();
            assert_ne!(inspection.panel.action_visible, 0);
            let submitted = host
                .property_offer(&mut session, &assets, category as u32, inspection.panel)
                .unwrap();
            assert!(
                submitted.submitted,
                "original legal minimum Lease offer rejected"
            );
            host.running = true;
            let mut bids = 0;
            for _ in 0..12000 {
                // The same 40ms scheduler input as the accepted original auction
                // continuation fixture; no clock/resource/ownership mutation.
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "{:?}", host.failure);
                rights_acquisition_prompts(&mut host, &mut session, &assets);
                if host
                    .auction_save_state()
                    .is_some_and(|s| s.visible && s.controls.buttons_enabled & 1 != 0)
                {
                    host.auction_button(&mut session, &assets, 0).unwrap();
                    bids += 1;
                }
                if session.world.categories[category][5] != 0 {
                    break;
                }
            }
            // Seed0 can legally win the authored minimum offer uncontested.
            // Press the actual increment control if offered, without requiring
            // an opponent to bid merely to create that additional fixture seam.
            assert_eq!(
                session.world.categories[category][5], 1,
                "legal human offer failed to acquire category0 after {bids} bids"
            );
            host.running = false;
            let auction_state = host.auction_save_state();
            drop(host);
            done.store(true, std::sync::atomic::Ordering::Release);
            service.join().unwrap();
            let controlled = !session
                .inspect_property_rights(category as u32, 0)
                .unwrap()
                .activate_visible;
            if controlled {
                // Only the pre-save closed-rights state is controlled. Original
                // ownership, cash, resources, RNG and both clocks are unchanged.
                assert_eq!(session.world.categories[category][5], 1);
                assert_eq!(session.world.categories[category][0x6d], 0);
                session.world.categories[category][0x78..0x7c].copy_from_slice(&0u32.to_le_bytes());
            }
            assert!(
                session
                    .inspect_property_rights(category as u32, 0)
                    .unwrap()
                    .activate_visible
            );
            let cameras = crate::save_menu::cameras(session.saved_cameras, camera).unwrap();
            let bytes = session.save_v2_with_auction(&image, cameras, 1, auction_state).unwrap();
            let path = crate::save_menu::write_new(&prepare_inputs.profile_root, &bytes).unwrap();
            (path, category, controlled)
        })
        .unwrap()
        .join()
        .unwrap();
    let immutable = fs::read(&path).unwrap();
    let worker_inputs = inputs.clone();
    let worker_path = path.clone();
    let (reply, result) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || worker_inputs.saved_worker(worker_path, None, camera, reply))
        .unwrap();
    let loaded = result
        .recv_timeout(std::time::Duration::from_secs(60))
        .unwrap()
        .unwrap();
    loaded
        .fresh_session
        .as_ref()
        .unwrap()
        .send(FreshRequest::Run(false))
        .unwrap();
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
    let mut auction_room = crate::auction_bridge::View::default();
    let mut sound = crate::gameplay_sound::Player::new(
        crate::gameplay_sound::Assets::load(&image, &inputs.installer).unwrap(),
    );
    let mut auction = crate::live_host::auction_audio::Player::new(
        crate::live_host::auction_audio::Assets::load(&image, &inputs.installer).unwrap(),
    );
    let ffmpeg = std::env::var_os("OPENMT2001_FFMPEG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "work/python/Lib/site-packages/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe"
                    .into()
            } else {
                "ffmpeg".into()
            }
        });
    let mut era = crate::era_audio::Player::new(
        crate::era_audio::Assets::load(&image, &inputs.installer, &ffmpeg).unwrap(),
    );
    let mut transport = crate::transport_audio::Player::new(
        crate::transport_audio::Assets::load(&image, &inputs.installer).unwrap(),
    );
    let mut pump = || {
        while let Ok(r) = loaded.gameplay_sound_requests.as_ref().unwrap().try_recv() {
            let _ = r.reply.send(sound.apply(r.operation));
        }
        while let Ok(r) = loaded.auction_audio_requests.as_ref().unwrap().try_recv() {
            let _ = r.reply.send(auction.apply(r.operation));
        }
        while let Ok(r) = loaded.era_audio_requests.as_ref().unwrap().try_recv() {
            let _ = r.reply.send(era.apply(r.operation));
        }
        while let Ok(r) = loaded.transport_audio_requests.as_ref().unwrap().try_recv() {
            transport.apply(r.operation);
        }
        era.poll().unwrap();
        while let Ok(request) = loaded.auction_requests.as_ref().unwrap().try_recv() {
            let response = auction_room.present(
                request.operation,
                &renderer,
                (&inputs.executable, &inputs.directory, &inputs.archive),
                0,
            );
            request.reply.send(response).unwrap();
        }
    };
    macro_rules! answer {
        ($receiver:expr) => {{
            let deadline = Instant::now() + std::time::Duration::from_secs(30);
            loop {
                pump();
                match $receiver.recv_timeout(std::time::Duration::from_millis(5)) {
                    Ok(value) => break value,
                    Err(mpsc::RecvTimeoutError::Timeout) => assert!(
                        Instant::now() < deadline,
                        "saved Rights worker reply timeout"
                    ),
                    Err(error) => panic!("saved Rights worker disconnected: {error}"),
                }
            }
        }};
    }
    let sender = loaded.fresh_session.as_ref().unwrap();
    sender.send(FreshRequest::Run(false)).unwrap();
    let report = loaded.block_information.as_ref().unwrap();
    assert_eq!(
        report.title_colors[23], 0xfff9f400,
        "actual saved-worker report must retain the original Ventnor style"
    );
    let off = fs::read(inputs.language.join("tycoon.off")).unwrap();
    let data = fs::read(inputs.language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = off
        .chunks_exact(2)
        .map(|b| {
            let end = at + u16::from_le_bytes(b.try_into().unwrap()) as usize;
            let v = &data[at..end];
            at = end;
            v
        })
        .collect::<Vec<_>>();
    let assets = crate::locator::Assets::load(
        &index,
        &inputs.archive,
        &inputs.installer,
        &strings,
        &inputs.executable,
    )
    .unwrap();
    let mut view = crate::utility_report::View::default();
    view.select(Some(category));
    let pointer = |view: &mut crate::utility_report::View, control| {
        (0..320)
            .find_map(|y| {
                (0..196).find_map(|x| {
                    let p = [x * 2, y * 2];
                    assets.property_pointer(view, Some(p), report, category);
                    (view.property_control_hover == Some(control)).then_some(p)
                })
            })
            .expect("original Rights widget has an actual input hitbox")
    };
    let p = pointer(&mut view, 11);
    assets.property_pointer(&mut view, Some(p), report, category);
    view.mouse(true);
    view.mouse(false);
    assert_eq!(view.take_tutorial_control(), Some(11));
    let request = view.take_rights_request().unwrap();
    let (reply, result) = mpsc::channel();
    sender
        .send(FreshRequest::PropertyRights {
            category: request.category,
            panel_mode: request.panel_mode,
            activate: request.activate,
            reply,
        })
        .unwrap();
    view.apply_rights_inspection(category as u32, answer!(result));
    assert_eq!(view.property_rights_state().unwrap().message, Some(1202));
    sender.send(FreshRequest::Run(true)).unwrap();
    let p = pointer(&mut view, 102);
    assets.property_pointer(&mut view, Some(p), report, category);
    view.mouse(true);
    view.mouse(false);
    let request = view.take_rights_request().unwrap();
    assert!(request.activate);
    let (reply, result) = mpsc::channel();
    sender
        .send(FreshRequest::PropertyRights {
            category: request.category,
            panel_mode: request.panel_mode,
            activate: request.activate,
            reply,
        })
        .unwrap();
    view.apply_rights_inspection(category as u32, answer!(result));
    assert_eq!(view.property_rights_state().unwrap().message, Some(1406));
    assert!(!view.property_rights_state().unwrap().activate_visible);
    let (reply, result) = mpsc::channel();
    sender
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::NewName(format!("rights-{}",
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())),
            camera,
            game_mode: 1,
            interface_mode: 0,
            reply,
        })
        .unwrap();
    let pending_path = answer!(result).unwrap();
    let pending_bytes = fs::read(pending_path).unwrap();
    let pending = openmt2001::save::SaveFile::parse(&pending_bytes).unwrap();
    let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let due = pending
        .scheduled()
        .unwrap()
        .iter()
        .find_map(|s| {
            s.packet
                .filter(|p| word(p, 4) == 0x19 && word(p, 20) == category as u32)
                .map(|p| word(p, 12))
        })
        .expect("native delayed category command19 persisted");
    assert!(pending.clock().unwrap().timing[0] < due);
    assert!(due - pending.clock().unwrap().timing[0] <= 25);
    let deadline = Instant::now() + std::time::Duration::from_secs(15);
    let ready_bytes = loop {
        std::thread::sleep(std::time::Duration::from_millis(25));
        let (reply, result) = mpsc::channel();
        sender
            .send(FreshRequest::SaveSelected {
                target: crate::save_menu::Target::NewName(format!("rights-{}",
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())),
                camera,
                game_mode: 1,
                interface_mode: 0,
                reply,
            })
            .unwrap();
        let snapshot = fs::read(answer!(result).unwrap()).unwrap();
        let save = openmt2001::save::SaveFile::parse(&snapshot).unwrap();
        if save.category_ownership_state(category).unwrap() == 1 {
            break snapshot;
        }
        assert!(
            save.clock().unwrap().timing[0] < due,
            "Rights failed to admit at native deadline"
        );
        assert!(
            Instant::now() < deadline,
            "Rights worker did not admit command19"
        );
    };
    sender.send(FreshRequest::Run(false)).unwrap();
    let ready = openmt2001::save::SaveFile::parse(&ready_bytes).unwrap();
    assert!(ready.clock().unwrap().timing[0] >= due);
    assert_eq!(ready.category_countdowns(category).unwrap()[1], 24);
    let expiry = ready
        .scheduled()
        .unwrap()
        .iter()
        .find(|s| word(s.header, 0) == 1 && word(s.header, 12) == category as u32)
        .map(|s| word(s.header, 8))
        .unwrap();
    let before = pending.clock().unwrap().timing;
    let after = ready.clock().unwrap().timing;
    assert_eq!(after[1].wrapping_sub(before[1]), after[0].wrapping_sub(before[0]),
        "Rights admission window stays in the original normal clock mode");
    //480ec0 consumes after the total clock increment and before the normal
    //clock increment in the same native scheduler iteration.
    assert_eq!(
        expiry,
        before[1].wrapping_add(due - before[0] - 1).wrapping_add(15000),
        "native normal-clock exclusive deadline"
    );
    assert!(expiry > after[1]);
    let (reply, result) = mpsc::channel();
    sender
        .send(FreshRequest::PropertyRights {
            category: category as u32,
            panel_mode: 0,
            activate: false,
            reply,
        })
        .unwrap();
    view.apply_rights_inspection(category as u32, answer!(result));
    assert_eq!(view.take_rights_open(), Some(category));
    let (reply, result) = mpsc::channel();
    sender
        .send(FreshRequest::Planning { category, reply })
        .unwrap();
    assert_eq!(
        answer!(result).unwrap().entry(),
        crate::build_panel::Entry::Open
    );
    let (reply, result) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let status = answer!(result);
    assert!(status.failure.is_none(), "{:?}", status.failure);
    assert_eq!(
        fs::read(path).unwrap(),
        immutable,
        "generated input save remained immutable"
    );
    println!(
        "Saved Rights worker: originalScenario2 legal acquisition; controlledClosed={controlled}; command19 due{due}, exclusive expiry{expiry}; actual GUI inspect/activate and Planning transition accepted"
    );
    drop(loaded);
    worker.join().unwrap();
}

#[test]
#[ignore = "requires original v1.4 Tutorial5 scripts and city assets"]
fn original_tutorial_five_starts_with_disabled_building_rights() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let executable_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let language = executable_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let inputs = SceneInputs {
            compatibility: crate::compatibility::Mode::StrictV14,
            style: test_board_style(&executable),
            time_runtime: Default::default(),
            profile_root: PathBuf::from("work/frontend/tutorial-disabled-rights-profile"),
            character_names: crate::fresh_city::character_names(&image, &language).unwrap(),
            installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
            language,
            maps: PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap())
                .parent().unwrap().to_path_buf(),
            executable,
            build_file: read("OPENMT2001_BUILD_FILE"),
            directory: read("OPENMT2001_ARCHIVE_DIR"),
            archive: read("OPENMT2001_ARCHIVE_BIN"),
            suncols: read("OPENMT2001_SUNCOLS"),
        };
        let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
        let catalog = crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap()).unwrap();
        let (camera, _) = fresh_city_camera();
        let (id, symbol) = (100004, "PACIFIC_AVENUE");
        let setup = crate::singleplayer::Setup {
            preview: crate::singleplayer::Preview::Scenario(id),
            difficulty: crate::singleplayer::Difficulty::Easy,
            character_index: 0,
            profile_name: b"Rights startup diagnostic".to_vec(),
            profile_path: crate::campaign_store::profile_path(
                &inputs.profile_root, b"Rights startup diagnostic"),
            catalog: catalog.clone(),
        };
        let (_, mut session) = inputs.fresh_preview(setup, camera, &mut SnapshotCache::default()).unwrap();
        let category = session.scripts.runtime.number(symbol).unwrap().unwrap() as usize;
        let row = &session.world.categories[category];
        let current = session.roster.manager.players.iter()
            .find(|player| player.reference == session.roster.status.current_player)
            .unwrap().bytes[0x1bc].wrapping_add(1);
        let rights = u32::from_le_bytes(row[0x78..0x7c].try_into().unwrap());
        println!("Tutorial{id} target{category} owner{} current{current} rights{rights} pending{}",
            row[5], row[0x6d]);
        assert_eq!((row[5], current, rights, row[0x6d]), (1, 1, 0, 0),
            "owned-category fresh loop must clear scripted startup rights");
        let context = crate::build_panel::Context::from_session(&session, category).unwrap();
        assert_eq!(context.entry(), crate::build_panel::Entry::Message {
            text: 1202, show_rights_action: true,
        });
        // Prompt2's "Building Rights button" is Strategic View's information
        // toggle 26, not the property panel's Activate Rights child 91.
        assert_eq!(image.f32_at(0x4dbddf).unwrap(), 26.);
        assert_eq!(image.f32_at(0x4dc19c).unwrap(), 91.);
        assert!(crate::scenarios::live_tutorial(id),
            "Tutorial5 is available with the modern repair; these strict inputs retain the original defect");
        // Tutorial8 has its own paired strict/modern acceptance. These reads
        // retain the authored preferences as evidence without changing inputs.
        let scripts = inputs.maps.parent().unwrap();
        let eighth_pref = fs::read(scripts.join("TUTORIAL8/initialpref.lua")).unwrap();
        assert!(eighth_pref.windows(b"Block[KENTUCKY_AVENUE].owner = 1".len())
            .any(|part| part == b"Block[KENTUCKY_AVENUE].owner = 1"));
        assert!(eighth_pref.windows(b"Block[KENTUCKY_AVENUE].buildingrights = 2".len())
            .any(|part| part == b"Block[KENTUCKY_AVENUE].buildingrights = 2"));
        for name in ["TUTORIAL5", "TUTORIAL8"] {
            let hub = fs::read(scripts.join(name).join("hub.lua")).unwrap();
            assert!(!hub.windows(b"TUT_BUILD_RIGHTS_BUTTON".len())
                .any(|part| part == b"TUT_BUILD_RIGHTS_BUTTON"),
                "{name} unexpectedly enables the original rights child");
            if name == "TUTORIAL5" {
                assert!(hub.windows(b"Interface.EnableControl(TUT_AERIAL_INFOBUTTON_3)".len())
                    .any(|part| part == b"Interface.EnableControl(TUT_AERIAL_INFOBUTTON_3)"));
            } else {
                assert!(hub.windows(b"Interface.EnableControlAndChildren(TUT_BUILD_PANEL)".len())
                    .any(|part| part == b"Interface.EnableControlAndChildren(TUT_BUILD_PANEL)"));
            }
        }
    }).unwrap().join().unwrap();
}
