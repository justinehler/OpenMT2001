use super::*;

#[test]
#[ignore = "requires original Starting Out assets and offscreen GPU"]
fn menu_profile_identity_reaches_live_hud_with_and_without_locator() {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
        let executable_path = openmt2001::fixtures::path("EXE");
        let language = executable_path.parent().unwrap().parent().unwrap()
            .join("US/language/usa");
        let inputs = SceneInputs {
            process_runtime: None, //legacy fixture, not a process-owner assertion
            compatibility: crate::compatibility::Mode::StrictV14,
            style: test_board_style(&executable), time_runtime: Default::default(),
            profile_root: std::env::temp_dir().join("omt-v2-gui-regression"),
            character_names: crate::fresh_city::character_names(&image, &language).unwrap(),
            installer, language,
            maps: openmt2001::fixtures::path("MAP")
                .parent().unwrap().to_path_buf(),
            build_file: read("OPENMT2001_BUILD_FILE"), executable,
            directory: read("OPENMT2001_ARCHIVE_DIR"),
            archive: read("OPENMT2001_ARCHIVE_BIN"), suncols: read("OPENMT2001_SUNCOLS"),
        };
        let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&inputs.directory,
            inputs.archive.len() as u64).unwrap();
        let off = fs::read(inputs.language.join("tycoon.off")).unwrap();
        let bin = fs::read(inputs.language.join("tycoon.bin")).unwrap();
        let mut cursor = 0;
        let strings = off.chunks_exact(2).map(|n| {
            let end = cursor + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
            let row = &bin[cursor..end]; cursor = end; row
        }).collect::<Vec<_>>();
        let localized = crate::tooltip::localized_strings(&inputs.installer, &strings).unwrap();
        let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut catalog = crate::campaign_store::load_catalog(&image,
            inputs.maps.parent().unwrap()).unwrap();
        let path = crate::campaign_store::profile_path(&inputs.profile_root, b"Justin");
        crate::campaign_store::load_progress(&mut catalog, &path).unwrap();
        let mut single = crate::singleplayer::SinglePlayer::new(&index, &inputs.archive,
            &inputs.installer, &strings, &inputs.executable).unwrap();
        single.open(false, "Justin", catalog.clone(), path.clone());
        single.activate(0); // selected unlocked campaign scenario -> character panel
        single.activate(0); // actual character-screen Start producer
        let setup = single.request.take().expect("menu Start must publish Setup");
        assert!(matches!(setup.preview, crate::singleplayer::Preview::Scenario(200003)));
        assert_eq!(setup.profile_name, b"Justin");
        // The tutorial producer must preserve the same runtime name encoding.
        single.open_tutorial("Justin", catalog, path);
        single.activate(0);
        assert_eq!(single.request.take().unwrap().profile_name, b"Justin");
        let (camera, _) = fresh_city_camera();
        // Decode actual native MOV operands, rather than our own camera fixture.
        for (address, expected) in [(0x4b9e65, camera.eye[0]), (0x4b9e6f, camera.eye[1]),
            (0x4b9e79, camera.eye[2]), (0x4b9e83, camera.target[0]),
            (0x4b9e8d, camera.target[1]), (0x4b9e97, camera.target[2])] {
            assert_eq!(u32::from_le_bytes(image.at(address,4).unwrap().try_into().unwrap()),
                expected.to_bits(), "native startup operand {address:x}");
        }
        let (loaded, session) = inputs.fresh_preview(setup, camera,
            &mut SnapshotCache::default()).unwrap();
        let mut host = crate::live_host::Host::new(camera.eye);
        host.begin_presentation(&session).unwrap();
        host.set_running(true);
        let status = host.status(&session);
        assert!(status.failure.is_none(), "{:?}", status.failure);
        let financial = status.financial.as_ref().unwrap().clone();
        let locator_assets = crate::locator::Assets::load(&index, &inputs.archive,
            &inputs.installer, &strings, &inputs.executable).unwrap();
        let mut locator = Some(locator_assets.runtime_state(
            loaded.runtime_locator.as_ref().unwrap()).unwrap());
        let time_assets = crate::time_panel::Assets::load(&index, &inputs.archive,
            &inputs.installer, &inputs.language, &inputs.executable).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        for present in [true, false] {
            if !present { locator = None; }
            update_locator_financial(&mut locator, &status).unwrap();
            assert_eq!(status.financial.as_ref(), Some(&financial));
            if let Some(locator) = &locator {
                assert_eq!(locator.financial_records(), financial.records);
            }
            let mut time = crate::time_panel::View::default();
            time.update(&time_assets, &renderer, &status).unwrap();
            assert_eq!(time.displayed_name(), Some("Justin"));
        }
        crate::capture_test::capture(&renderer, None, &loaded.scene,
            "work/frontend/fresh-native-startup-overview.png");
    }).unwrap().join().unwrap();
}
#[test]
#[ignore = "requires original v1.4 tutorial assets and connected city worker"]
fn proven_tutorial_menu_rows_launch_retained_city_worker() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
            let executable_path = openmt2001::fixtures::path("EXE");
            let language = executable_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa");
            let inputs = Arc::new(SceneInputs {
            process_runtime: None, //legacy fixture, not a process-owner assertion
            compatibility: crate::compatibility::Mode::default(),
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: PathBuf::from(format!(
                    "work/frontend/tutorial-menu-worker-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                        .unwrap().as_nanos()
                )),
                character_names: crate::fresh_city::character_names(&image, &language).unwrap(),
                installer,
                language,
                maps: openmt2001::fixtures::path("MAP")
                    .parent()
                    .unwrap()
                    .to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"),
                executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            });
            let index = openmt2001::archive::ArchiveIndex::parse(
                &inputs.directory,
                inputs.archive.len() as u64,
            )
            .unwrap();
            let offsets = fs::read(inputs.language.join("tycoon.off")).unwrap();
            let text = fs::read(inputs.language.join("tycoon.bin")).unwrap();
            let mut at = 0;
            let strings = offsets
                .chunks_exact(2)
                .map(|length| {
                    let end = at + usize::from(u16::from_le_bytes(length.try_into().unwrap()));
                    let value = &text[at..end];
                    at = end;
                    value
                })
                .collect::<Vec<_>>();
            assert_eq!(at, text.len());
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let mut menu = crate::singleplayer::SinglePlayer::new(
                &index,
                &inputs.archive,
                &inputs.installer,
                &strings,
                &inputs.executable,
            )
            .unwrap();
            let mut catalog =
                crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap()).unwrap();
            let profile =
                crate::campaign_store::profile_path(&inputs.profile_root, b"Tutorial menu worker");
            crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
            let (camera, _) = fresh_city_camera();
            for id in [100004, 100005, 100006, 100007, 100008, 100009, 100010, 100011, 100012] {
                assert!(crate::scenarios::live_tutorial(id));
                crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
                menu.open_tutorial("Tutorial menu worker", catalog.clone(), profile.clone());
                menu.activate(2 + (id - 100000) as usize);
                menu.activate(0);
                let setup = menu
                    .request
                    .take()
                    .expect("proven tutorial menu must publish Setup");
                assert!(
                    matches!(setup.preview, crate::singleplayer::Preview::Scenario(actual)
                if actual == id)
                );
                let (tx, rx) = mpsc::channel();
                let worker_inputs = inputs.clone();
                let worker = std::thread::Builder::new()
                    .name(format!("tutorial-{id}-worker"))
                    .stack_size(32 * 1024 * 1024)
                    .spawn(move || worker_inputs.preview_worker(setup, camera, tx))
                    .unwrap();
                let loaded = rx
                    .recv_timeout(std::time::Duration::from_secs(90))
                    .unwrap()
                    .unwrap();
                assert!(
                    loaded.fresh_session.is_some(),
                    "tutorial {id} fell through to a static, noninteractive preview"
                );
                assert!(
                    !loaded.scene.draws.is_empty(),
                    "tutorial {id} lacks its live city scene"
                );
                let (reply, status) = mpsc::channel();
                loaded
                    .fresh_session
                    .as_ref()
                    .unwrap()
                    .send(FreshRequest::Status {
                        camera: camera.eye,
                        reply,
                    })
                    .unwrap();
                let status = status
                    .recv_timeout(std::time::Duration::from_secs(30))
                    .unwrap();
                assert!(
                    status.failure.is_none(),
                    "tutorial {id} worker: {:?}",
                    status.failure
                );
                assert!(
                    status.tutorial_session,
                    "tutorial {id} worker lost tutorial mode"
                );
                if matches!(id, 100010 | 100011) {
                    assert!(!catalog.levels[catalog.find(id).unwrap()].completed()[0],
                        "Tutorial{id} End was recorded before its controls");
                    let sender = loaded.fresh_session.as_ref().unwrap();
                    let read_status = || {
                        let (reply, receiver) = mpsc::channel();
                        sender.send(FreshRequest::Status { camera: camera.eye, reply }).unwrap();
                        receiver.recv_timeout(std::time::Duration::from_secs(30)).unwrap()
                    };
                    let mut state = status;
                    for _ in 0..100 {
                        if !state.prompts.is_empty() { break; }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        state = read_status();
                        assert!(state.failure.is_none(), "Tutorial{id} startup: {:?}", state.failure);
                    }
                    assert_eq!(state.prompts.first().map(|prompt| prompt.dismissal_id), Some(2));
                    sender.send(FreshRequest::DismissPrompt(2, None)).unwrap();
                    state = read_status();
                    let selected = (0..52).find(|&category_id| state.tutorial_interaction.callback(
                        crate::tutorial_two_interaction::Pick::BlockEntry { category_id }) == Some(1))
                        .expect("first original utility/railroad block-entry gate");
                    sender.send(FreshRequest::TutorialPick(
                        crate::tutorial_two_interaction::Pick::BlockEntry { category_id: selected })).unwrap();
                    state = read_status();
                    assert_eq!(state.prompts.first().map(|prompt| prompt.dismissal_id), Some(4));
                    sender.send(FreshRequest::DismissPrompt(4, None)).unwrap();
                    state = read_status();
                    assert_eq!(state.tutorial_enabled_controls.len(), 1);
                    let information = state.tutorial_enabled_controls[0];
                    assert_ne!(information, 103);
                    sender.send(FreshRequest::TutorialControl(information)).unwrap();
                    state = read_status();
                    for dismissal in if id == 100010 { &[6][..] } else { &[6, 7, 8, 9][..] } {
                        assert_eq!(state.prompts.first().map(|prompt| prompt.dismissal_id), Some(*dismissal));
                        sender.send(FreshRequest::DismissPrompt(*dismissal, None)).unwrap();
                        state = read_status();
                    }
                    for step in 0..3 {
                        let selected = (0..52).find(|&category_id| state.tutorial_interaction.callback(
                            crate::tutorial_two_interaction::Pick::BlockEntry { category_id }) == Some(1))
                            .expect("subsequent original utility/railroad block-entry gate");
                        // After the first stage each lesson's nested dismiss catch
                        // (0x4db6b0 -> 0x43bed0 -> 0x429d50, #370) disables world
                        // picks, so those stages continue through Navigator 103.
                        if (id == 100010 && step != 1) || (id == 100011 && step >= 1) {
                            if step > 0 {
                                assert!(state.tutorial_controls.picking_disabled);
                            }
                            assert!(state.tutorial_enabled_controls.contains(&103));
                            sender.send(FreshRequest::TutorialControl(103)).unwrap();
                        } else {
                            sender.send(FreshRequest::TutorialPick(
                                crate::tutorial_two_interaction::Pick::BlockEntry { category_id: selected })).unwrap();
                        }
                        state = read_status();
                        if id == 100010 && step == 0 {
                            assert_eq!(state.prompts.first().map(|prompt| prompt.dismissal_id), Some(8));
                            sender.send(FreshRequest::DismissPrompt(8, None)).unwrap();
                            state = read_status();
                        }
                    }
                    let final_dismissal = if id == 100010 { 11 } else { 13 };
                    assert_eq!(state.prompts.first().map(|prompt| prompt.dismissal_id), Some(final_dismissal));
                    sender.send(FreshRequest::DismissPrompt(final_dismissal, None)).unwrap();
                    let mut completed = false;
                    for _ in 0..100 {
                        state = read_status();
                        assert!(state.failure.is_none(), "Tutorial{id} retained worker: {:?}", state.failure);
                        if state.tutorial_completed { completed = true; break; }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    assert!(completed, "Tutorial{id} normal menu/worker requests did not reach Game.End");
                    let recorded = fs::read(&profile).expect("tutorial End must persist the V2 profile");
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    assert_eq!(fs::read(&profile).unwrap(), recorded,
                        "Tutorial{id} completion was recorded more than once");
                    let mut reopened = crate::campaign_store::load_catalog(&image,
                        inputs.maps.parent().unwrap()).unwrap();
                    crate::campaign_store::load_progress(&mut reopened, &profile).unwrap();
                    let level = &reopened.levels[reopened.find(id).unwrap()];
                    assert_eq!(level.completed(), [true, false, false]);
                    assert_eq!(level.attempts(), 0);
                    if id == 100011 {
                        assert!(reopened.levels[reopened.find(100010).unwrap()].completed()[0],
                            "Tutorial11 profile save lost Tutorial10 completion");
                    }
                }
                drop(loaded);
                worker.join().unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
