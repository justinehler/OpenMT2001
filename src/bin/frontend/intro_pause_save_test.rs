use super::*;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = openmt2001::fixtures::scratch("PAUSE_SAVE").join(format!(
            "pause-save-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires original v1.4 assets and scripts for the menu Save/Load route"]
fn menu_cold_load_retires_saved_holds_and_keeps_new_process_holds() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use openmt2001::{pause, process_dispatch::DispatchCustody};
            let scratch = Scratch::new();
            let inputs = camera_producer_live_tests::inputs(scratch.0.clone());
            let camera = fresh_city_camera().0;
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let board = inputs.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let (_, mut source) = inputs
                .fresh_preview(
                    camera_producer_live_tests::setup(&inputs),
                    camera,
                    &mut SnapshotCache::default(),
                )
                .unwrap();
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            for reason in [0, 1, 3, 4] {
                let mut apply = |change: pause::Timers, _: &mut pause::State| {
                    change.apply(&mut source.hooks.timers);
                    Ok(())
                };
                if reason == 4 {
                    source.world.pause.focus_lost(false, &mut apply).unwrap();
                } else {
                    source.world.pause.pause(reason, &mut apply).unwrap();
                }
                let mut options = crate::live_host::Host::new(camera.eye);
                options.set_running(true);
                options
                    .options(&mut source.world.pause, &mut source.hooks.timers, true)
                    .unwrap();
                let live_pause = source.world.pause.clone();
                let live_timers = source.hooks.timers.clone();
                let bytes = source
                    .save_v2_with_options_view(&image, cameras, 1, &options, 0)
                    .unwrap();
                assert_eq!(source.world.pause, live_pause);
                assert_eq!(source.hooks.timers, live_timers);
                let path = crate::save_menu::write_new(&scratch.0, &bytes).unwrap();
                // The exact constructor used by the menu consumes the native load
                // completion followed by record12, in a new Session each time.
                for new_hold in [false, true] {
                    let mut process = DispatchCustody::startup(pause::State::default()).unwrap();
                    if new_hold {
                        process
                            .pause
                            .pause(0, |change, _| {
                                change.apply(&mut process.tasks);
                                Ok(())
                            })
                            .unwrap();
                        process
                            .pause
                            .pause(1, |change, _| {
                                change.apply(&mut process.tasks);
                                Ok(())
                            })
                            .unwrap();
                    }
                    process.frequency.frequency = 17.;
                    for task in &mut process.tasks {
                        task[1..4].copy_from_slice(&[0x12, 0x34, 0x56]);
                        task[12..16].copy_from_slice(&99_f32.to_le_bytes());
                    }
                    let (seed, _) = process.prepare_start(false).unwrap();
                    let (_, mut loaded) = inputs
                        .saved_preview_with_selection_joined(
                            &path,
                            camera,
                            &mut SnapshotCache::default(),
                            None,
                            Some(seed),
                        )
                        .unwrap();
                    assert_eq!(
                        loaded.world.clock, source.world.clock,
                        "native timing words must survive"
                    );
                    assert_eq!(
                        &loaded.world.pause.bytes[..4],
                        if new_hold { &[1, 1, 0, 0] } else { &[0; 4] }
                    );
                    assert_eq!(loaded.hooks.frequency.frequency, 17.);
                    for task in &loaded.hooks.timers {
                        assert_eq!(&task[1..4], &[0x12, 0x34, 0x56]);
                        assert_eq!(&task[12..16], &0_f32.to_le_bytes());
                    }
                    assert_eq!(
                        loaded.world.pause.bytes[4], 0,
                        "saved focus loss must not return"
                    );
                    let mut host = crate::live_host::Host::new(camera.eye);
                    host.set_running(true);
                    let before = loaded.world.clock.timing[0];
                    host.frame(&mut loaded, &assets, 40);
                    assert!(host.failure.is_none(), "{:?}", host.failure);
                    if new_hold {
                        assert_eq!(loaded.world.clock.timing[0], before);
                        for reason in [0, 1] {
                            loaded
                                .world
                                .pause
                                .unpause(reason, |change, _| {
                                    change.apply(&mut loaded.hooks.timers);
                                    Ok(())
                                })
                                .unwrap();
                        }
                        host.frame(&mut loaded, &assets, 40);
                        assert!(host.failure.is_none(), "{:?}", host.failure);
                    }
                    assert!(
                        loaded.world.clock.timing[0] > before,
                        "cold load must advance without saved owners"
                    );
                }
                options
                    .options(&mut source.world.pause, &mut source.hooks.timers, false)
                    .unwrap();
                if reason == 4 {
                    source
                        .world
                        .pause
                        .focus_gained(false, |change, _| {
                            change.apply(&mut source.hooks.timers);
                            Ok(())
                        })
                        .unwrap();
                } else {
                    source
                        .world
                        .pause
                        .unpause(reason, |change, _| {
                            change.apply(&mut source.hooks.timers);
                            Ok(())
                        })
                        .unwrap();
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

// A synchronous CPU renderer boundary keeps this regression independent of a
// graphics device. Simulation, auction activation and save/load are production.
fn auction_presenter() -> (
    mpsc::Sender<crate::auction_bridge::Request>,
    std::thread::JoinHandle<()>,
) {
    let (sender, receiver) = mpsc::channel::<crate::auction_bridge::Request>();
    let worker = std::thread::spawn(move || {
        let mut actors = Vec::new();
        for request in receiver {
            if let crate::auction_bridge::Operation::Prepare { actor_events, .. } =
                request.operation
            {
                actors.push(actor_events);
            }
            request.reply.send(Ok(0)).unwrap();
        }
    });
    (sender, worker)
}

#[test]
#[ignore = "requires original v1.4 assets and scripts for a naturally admitted auction"]
fn menu_loaded_active_auction_still_freezes_normal_city_clock() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let scratch = Scratch::new();
            let inputs = camera_producer_live_tests::inputs(scratch.0.clone());
            let camera = fresh_city_camera().0;
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let board = inputs.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let mut setup = camera_producer_live_tests::setup(&inputs);
            setup.preview = crate::singleplayer::Preview::Scenario(200001);
            let (_, mut source) = inputs
                .fresh_preview(setup, camera, &mut SnapshotCache::default())
                .unwrap();
            let (renderer, renderer_thread) = auction_presenter();
            let mut source_host = crate::live_host::Host::new(camera.eye);
            source_host.auction_renderer = Some(renderer);
            source_host.begin_presentation(&source).unwrap();
            source_host.set_running(true);
            for _ in 0..5000 {
                source_host.frame(&mut source, &assets, 40);
                assert!(source_host.failure.is_none(), "{:?}", source_host.failure);
                for prompt in source_host.take_prompts(&source) {
                    if let Some(dialog) = prompt.dialog {
                        source_host
                            .human_dialog_reply(
                                &mut source,
                                &assets,
                                crate::human_dialog::Reply {
                                    callback: dialog.callback,
                                    context: dialog.context,
                                    button: if dialog.button_count == 2 { 1 } else { 0 },
                                },
                            )
                            .unwrap();
                    } else {
                        source
                            .dispatch_session_event(
                                "InterfacePromptDismiss",
                                prompt.dismissal_id as i32,
                                &assets,
                                true,
                            )
                            .unwrap();
                    }
                }
                if source.world.auction.controller.record[0x7c] != 0 {
                    break;
                }
            }
            assert_ne!(
                source.world.auction.controller.record[0x7c], 0,
                "natural auction must activate"
            );
            // The independent old owner must not be needed to keep the loaded
            // auction's city clock frozen (or survive after that auction closes).
            source
                .world
                .pause
                .pause(1, |change, _| {
                    change.apply(&mut source.hooks.timers);
                    Ok(())
                })
                .unwrap();
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            let bytes = source
                .save_v2_with_auction_view(&image, cameras, 2, source_host.auction_save_state(), 0)
                .unwrap();
            let path = crate::save_menu::write_new(&scratch.0, &bytes).unwrap();
            let (inspection, mut loaded) = inputs
                .saved_preview(&path, camera, &mut SnapshotCache::default())
                .unwrap();
            let continuation = inspection.saved_auction.unwrap();
            let (renderer, loaded_renderer_thread) = auction_presenter();
            let mut host = crate::live_host::Host::new(camera.eye);
            host.auction_renderer = Some(renderer);
            host.begin_presentation(&loaded).unwrap();
            host.set_running(true);
            host.activate_saved_auction(&mut loaded).unwrap();
            continuation.apply(&mut loaded).unwrap();
            host.restore_auction_widgets(continuation.host.as_ref(), &loaded)
                .unwrap();
            assert!(!loaded.world.pause.has_reason());
            assert_eq!(loaded.world.clock.timing[5], 2);
            let before = loaded.world.clock;
            for _ in 0..5 {
                host.frame(&mut loaded, &assets, 40);
                assert!(host.failure.is_none(), "{:?}", host.failure);
            }
            assert!(loaded.world.clock.timing[0] > before.timing[0]);
            assert_eq!(loaded.world.clock.timing[1..4], before.timing[1..4]);
            assert_eq!(loaded.world.clock.calendar, before.calendar);
            drop(host);
            loaded_renderer_thread.join().unwrap();
            drop(source_host);
            renderer_thread.join().unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
