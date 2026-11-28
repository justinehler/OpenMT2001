// Real saved worker handshake: rebuilt renderer first, continuation, widgets,
// then another production Save request. Each run builds its own natural saves.
use super::*;

#[test]
#[ignore = "requires original v1.4 assets/scripts and an offscreen graphics device"]
fn saved_auction_worker_rebuilds_and_saves() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let executable = read("OPENMT2001_EXE");
    let scratch = openmt2001::fixtures::scratch("AUCTION_WORKER").join(format!(
        "worker-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let selected = crate::save_menu::selected_profile_root(&scratch, b"Auction worker");
    let inputs = Arc::new(SceneInputs {
            process_runtime: None, //legacy fixture, not a process-owner assertion
            compatibility: crate::compatibility::Mode::StrictV14,
        time_runtime: Default::default(),
        style: test_board_style(&executable),
        profile_root: scratch.clone(),
        character_names: crate::fresh_city::character_names(
            &openmt2001::pe::PeImage::parse(&executable).unwrap(),
            &exe_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa"),
        )
        .unwrap(),
        installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
        language: exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa"),
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
    let camera = CameraSample {
        eye: [100., 120., 100.],
        target: [0.; 3],
        up: [0., 1., 0.],
        vertical_fov: 0.7,
    };
    for prior in [0, 3] {
        let paths = crate::session_save_host_tests::auction_test::auction_continuation_saves(prior);
        let source_root = paths[0].parent().unwrap().parent().unwrap().to_path_buf();
        for (phase, path) in paths.into_iter().enumerate() {
            let expected_inputs = inputs.clone();
            let expected_path = path.clone();
            let expected = std::thread::Builder::new()
                .stack_size(32 * 1024 * 1024)
                .spawn(move || {
                    let (loaded, session) = expected_inputs
                        .saved_preview(&expected_path, camera, &mut SnapshotCache::default())
                        .unwrap();
                    assert_eq!(loaded.saved_prior_mode, Some(prior));
                    assert_eq!(loaded.saved_viewpoint.unwrap().0.eye, camera.eye);
                    // A restored announcement reopens with empty seconds (41d700);
                    // 41d900 writes the next whole-second boundary (#266).
                    let announcing = session.saved_logic_mode != Some(2)
                        && session.world.auction.controller.record[0x7d] != 0;
                    (loaded.saved_auction, announcing)
                })
                .unwrap()
                .join()
                .unwrap();
            let (expected, announcing) = expected;
            if let Some(expected) = &expected {
                assert_eq!(expected.prior_mode, prior);
            }
            let mode = expected.as_ref().map_or(1, |expected| expected.game_mode);
            let expected_host = expected.and_then(|expected| expected.host);
            let (reply, loaded_result) = mpsc::channel();
            let worker_inputs = inputs.clone();
            let worker_selected = selected.clone();
            // Legacy saved-session coverage; this does not activate camera-owner review.
            let worker = std::thread::Builder::new()
                .stack_size(32 * 1024 * 1024)
                .spawn(move || worker_inputs.session_worker(WorkerStart::Saved(path, Some(worker_selected)), camera, reply, None))
                .unwrap();
            let mut loaded = loaded_result
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap()
                .unwrap();
            assert_eq!(loaded.saved_prior_mode, Some(prior));
            let renderer_requests = loaded.auction_requests.take().unwrap();
            let render_inputs = inputs.clone();
            let (panel_sender, panel_receiver) =
                mpsc::channel::<(openmt2001::auction_panel::Auction, mpsc::Sender<()>)>();
            let render_thread = std::thread::Builder::new()
                .stack_size(32 * 1024 * 1024)
                .spawn(move || {
                    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                    if cfg!(windows) {
                        descriptor.backends = wgpu::Backends::DX12;
                    }
                    let renderer =
                        pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                    let mut view = crate::auction_bridge::View::default();
                    loop {
                        if let Ok((auction, done)) = panel_receiver.try_recv() {
                            let index = openmt2001::archive::ArchiveIndex::parse(
                                &render_inputs.directory,
                                render_inputs.archive.len() as u64,
                            )
                            .unwrap();
                            let offsets =
                                fs::read(render_inputs.language.join("tycoon.off")).unwrap();
                            let bytes =
                                fs::read(render_inputs.language.join("tycoon.bin")).unwrap();
                            let mut at = 0;
                            let strings = offsets
                                .chunks_exact(2)
                                .map(|n| {
                                    let end =
                                        at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
                                    let text = &bytes[at..end];
                                    at = end;
                                    text
                                })
                                .collect::<Vec<_>>();
                            let assets = crate::auction_panel::Assets::load(
                                &index,
                                &render_inputs.archive,
                                &render_inputs.installer,
                                &strings,
                                &render_inputs.executable,
                            )
                            .unwrap();
                            let mut panel_view = crate::auction_panel::View::default();
                            panel_view
                                .prepare(&assets, &renderer, Some(&auction), None)
                                .unwrap();
                            let panel = panel_view.prepared.as_ref().unwrap();
                            crate::capture_test::capture_overlay(
                                &renderer,
                                &view.scene.as_ref().unwrap().0,
                                panel,
                                &format!("save/auction-worker-ui-{prior}-{phase}.png"),
                            );
                            done.send(()).unwrap();
                        }
                        let request = match renderer_requests
                            .recv_timeout(std::time::Duration::from_millis(10))
                        {
                            Ok(request) => request,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        let activate = matches!(
                            request.operation,
                            crate::auction_bridge::Operation::Activate
                        );
                        let result = view.present(
                            request.operation,
                            &renderer,
                            (
                                &render_inputs.executable,
                                &render_inputs.directory,
                                &render_inputs.archive,
                            ),
                            prior,
                        );
                        if activate && result.is_ok() {
                            crate::capture_test::capture_standalone(
                                &renderer,
                                &view.scene.as_ref().unwrap().0,
                                &format!("save/auction-worker-{prior}-{phase}.png"),
                            );
                        }
                        request.reply.send(result).unwrap();
                    }
                })
                .unwrap();
            // These UI channel owners verify handles are rebuilt. They do not
            // claim audible output; original sound decoding has its own gate.
            let sound = loaded.gameplay_sound_requests.take().unwrap();
            let sound_thread = std::thread::spawn(move || {
                let mut next = 1000;
                for request in sound {
                    next += 1;
                    let _ = request.reply.send(Ok(next));
                }
            });
            let audio = loaded.auction_audio_requests.take().unwrap();
            let audio_thread = std::thread::spawn(move || {
                let mut next = 2000;
                for request in audio {
                    next += 1;
                    let _ = request.reply.send(Ok(next));
                }
            });
            let era = loaded.era_audio_requests.take().unwrap();
            let era_thread = std::thread::spawn(move || {
                for request in era {
                    let _ = request.reply.send(Ok(3000));
                }
            });
            let transport = loaded.transport_audio_requests.take().unwrap();
            let transport_thread = std::thread::spawn(move || for _ in transport {});
            let sender = loaded.fresh_session.as_ref().unwrap();
            let (reply, status_result) = mpsc::channel();
            sender
                .send(FreshRequest::Status {
                    camera: camera.eye,
                    reply,
                })
                .unwrap();
            let status = status_result
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap();
            assert!(status.failure.is_none(), "{:?}", status.failure);
            assert_eq!(
                status.auction_seconds,
                if announcing {
                    None
                } else {
                    expected_host
                        .as_ref()
                        .and_then(|host| host.announcement_seconds)
                }
            );
            if mode == 2 {
                let expected_host = expected_host.as_ref().unwrap();
                let auction = status.auction.as_ref().unwrap();
                assert_eq!(auction.visible, expected_host.visible);
                let mut controls = auction.controls.clone();
                controls.timer.sound = 0;
                assert_eq!(controls, expected_host.controls);
                assert_ne!(
                    auction.info.as_ref().unwrap().actor[0],
                    0,
                    "real worker must retain named auction bidder"
                );
                if mode == 2 {
                    let (done, completed) = mpsc::channel();
                    panel_sender.send((auction.clone(), done)).unwrap();
                    completed
                        .recv_timeout(std::time::Duration::from_secs(60))
                        .unwrap();
                }
            } else {
                assert!(status.auction.is_none());
            }
            let (reply, saved_result) = mpsc::channel();
            sender
                .send(FreshRequest::SaveSelected { capture: None,
                    target: crate::save_menu::Target::NewName(format!("auction-{}-{}", mode,
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())),
                    camera,
                    game_mode: mode,
                    interface_mode: prior,
                    reply,
                })
                .unwrap();
            let saved = saved_result
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap()
                .unwrap();
            assert!(
                saved.starts_with(fs::canonicalize(selected.join("saves")).unwrap())
            );
            let bytes = fs::read(&saved).unwrap();
            let parsed = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            assert_eq!(parsed.clock().unwrap().timing[5], mode);
            assert_eq!(parsed.session_option_word().unwrap(), 0);
            assert_eq!(parsed.auction().unwrap().is_some(), phase < 2);
            if phase == 3 {
                let ext = openmt2001::save::extension::Extension::parse(&parsed)
                    .unwrap()
                    .unwrap();
                assert_eq!(ext.version, 4);
                assert_eq!(ext.required(15).unwrap(), &3u32.to_le_bytes());
                assert!(!ext.records.contains_key(&14));
            }
            println!(
                "Auction worker prior{prior} phase{phase}: {saved:?}, exact controls and native presence"
            );
            drop(loaded);
            worker.join().unwrap();
            render_thread.join().unwrap();
            sound_thread.join().unwrap();
            audio_thread.join().unwrap();
            era_thread.join().unwrap();
            transport_thread.join().unwrap();
        }
        fs::remove_dir_all(source_root).unwrap();
    }
    fs::remove_dir_all(scratch).unwrap();
}
