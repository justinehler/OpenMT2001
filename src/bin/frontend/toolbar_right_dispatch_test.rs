// Ignored Linux/X11 integration fixture: actual Intro mouse dispatcher, actual
// supplied assets and real snapshot construction. No policy-helper substitute.
use super::*;
use std::path::Path;
use winit::{
    event::MouseButton, event_loop::EventLoop, platform::x11::EventLoopBuilderExtX11,
    window::Window,
};

fn point_at_info(intro: &mut Intro) {
    let b = intro.inspection.as_ref().unwrap().toolbar_bounds[0];
    // Real inspection cursor conversion: physical1280x960 -> internal1280x960,
    // then the ordinary toolbar consumes logical640x480.
    intro.inspection_cursor(
        [f64::from(b[0] + b[2]), f64::from(b[1] + b[3])],
        [1280, 960],
    );
}
fn click_info(intro: &mut Intro) {
    point_at_info(intro);
    intro.inspection_mouse(MouseButton::Left, true);
    intro.inspection_mouse(MouseButton::Left, false);
}
fn assert_collapsed(intro: &Intro) {
    assert!(
        !intro.inspection.as_ref().unwrap().details_visible,
        "right held or canceled gesture must not reveal the collapsed body"
    );
}
fn assert_prompt(intro: &Intro) {
    assert!(
        intro
            .inspection
            .as_ref()
            .unwrap()
            .prompts
            .request()
            .is_some(),
        "fixture must continue through the real prompt early-return route"
    );
}
/// Pass an actual decoded supplied-save snapshot through Intro::draw's existing
/// construction/retention path. No background worker is started by this fixture.
fn install_snapshot(intro: &mut Intro, presenter: &Presenter, save: &Path, refresh: bool) {
    let inputs = intro.assets.inputs.clone();
    let (camera, _) = fresh_city_camera();
    let (loaded, session) = inputs
        .saved_preview(save, camera, &mut SnapshotCache::default())
        .unwrap();
    assert!(
        loaded.fresh_session.is_none(),
        "fixture must not spawn/retain a simulation worker"
    );
    let mut host = crate::live_host::Host::new(camera.eye);
    host.begin_presentation(&session).unwrap();
    let mut status = host.status(&session);
    // Explicit controlled tutorial-modal setup, not a claimed naturally emitted
    // tutorial prompt. The scene, roster, report and resources are supplied data.
    status.player_panels = [true; 2];
    status.stats = None;
    status.card = None;
    status.auction = None;
    status.auction_notice = None;
    status.event_list_visible = false;
    status.prompts.clear();
    status.tutorial_session = true;
    status.tutorial_controls.all_disabled = true;
    status.tutorial_enabled_controls = vec![9];
    drop(host);
    drop(session);
    let (tx, rx) = mpsc::channel();
    tx.send(Ok(loaded)).unwrap();
    intro.menu.as_mut().unwrap().load.visible = true;
    intro.loading = Some(rx);
    intro.refreshing_fresh = refresh;
    intro.draw(presenter).unwrap(); // actual constructor and snapshot transfer
    let view = intro
        .inspection
        .as_mut()
        .expect("actual Intro must install decoded snapshot");
    if !refresh {
        let category = view
            .block_information
            .as_ref()
            .unwrap()
            .categories
            .iter()
            .enumerate()
            .find(|(category, row)| {
                row.is_some()
                    && intro
                        .assets
                        .selection
                        .blocks
                        .iter()
                        .any(|b| b.id == *category && b.kind != 0)
            })
            .map(|(category, _)| category)
            .expect("supplied ordinary property category");
        view.selected = inspection_hit(&intro.assets.selection.blocks, category, None);
        view.utility_report.select(Some(category));
        view.strategic = false;
        view.details_visible = false;
        view.mouse_buttons = [false; 2];
        view.live_status = Some(status);
        view.prompts = crate::prompt::State::default();
        let dialog = crate::human_dialog::Dialog {
            title: 1934,
            body: 1935,
            buttons: [1437, 0],
            button_count: 1,
            timeout: false,
            ..Default::default()
        };
        view.prompts.submit(crate::prompt::Request {
            script: None,
            dialog: Some(dialog),
            title: 1934,
            body: 1935,
            dismissal_id: 0,
            card: None,
        });
    }
}

/// Requires an external owned X11 display (e.g. root's supervised Xvfb), GPU,
/// supplied v1.4 input environment and an existing owned disk scratch parent.
/// It is ignored so the normal test suite never creates windows/loads a save.
#[test]
#[ignore = "requires X11/GPU, supplied original assets/SAVE_ONE, ffmpeg and OPENMT2001_TOOLBAR_RIGHT_TEST_ROOT; actual dispatcher integration"]
#[allow(deprecated)]
fn prompt_swallowed_right_edges_block_whitelisted_collapsed_info_dispatch() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let root = openmt2001::fixtures::scratch("TOOLBAR_RIGHT_TEST_ROOT");
            assert!(
                root.is_dir(),
                "supply an existing owned disk scratch parent"
            );
            let scratch = root.join(format!(
                "toolbar-right-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&scratch).unwrap();
            struct Cleanup(PathBuf);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
            let cleanup = Cleanup(scratch.clone());
            let original = openmt2001::fixtures::path("SAVE_ONE");
            let save_bytes = fs::read(&original).unwrap();
            let save = scratch.join("fixture.sav");
            fs::write(&save, &save_bytes).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let params = openmt2001::fixtures::path("PARAMETERS");
            let directory = openmt2001::fixtures::path("ARCHIVE_DIR");
            let mut builder = EventLoop::builder();
            builder.with_x11().with_any_thread(true);
            let event_loop = builder.build().unwrap();
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_visible(false)
                            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 960)),
                    )
                    .unwrap(),
            );
            let presenter = pollster::block_on(Presenter::new(window)).unwrap();
            let assets = Assets::load(Paths {
                compatibility: crate::compatibility::Mode::Modern,
                installer: params.parent().unwrap().parent().unwrap().to_path_buf(),
                ffmpeg: PathBuf::from(
                    std::env::var("OPENMT2001_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
                ),
                scene: directory.parent().unwrap().to_path_buf(),
                executable: exe.clone(),
                map: openmt2001::fixtures::path("MAP"),
                quality: Some("low".into()),
                language: exe
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
                profile_root: scratch.join("profiles"),
                save_root: scratch.join("saves"),
            })
            .unwrap();
            let mut intro = Intro::new(&presenter, assets).unwrap();
            intro.open_menu();
            install_snapshot(&mut intro, &presenter, &save, false);
            point_at_info(&mut intro);
            assert_prompt(&intro);
            assert_collapsed(&intro);
            assert_eq!(
                intro.inspection_cursor_kind(),
                Some(3),
                "fixture Info must be tutorial-whitelisted, outside prompt and enabled"
            );
            let view = intro.inspection.as_ref().unwrap();
            let menu = intro.menu.as_ref().unwrap();
            assert!(
                !menu.prompt_assets().input_occupied(
                    view.pointer.unwrap().map(|p| p as f32 * 0.5),
                    view.live_status
                        .as_ref()
                        .unwrap()
                        .tutorial_interaction
                        .prompt_offset()
                )
            );

            // The actual prompt dispatcher swallows this Right press. In v1 its
            // late camera assignment is never reached, so the next Left click wins.
            intro.inspection_mouse(MouseButton::Right, true);
            assert_prompt(&intro);
            assert!(
                !intro.inspection.as_ref().unwrap().mouse_buttons[1],
                "test must exercise swallowed Right, not the late camera branch"
            );
            click_info(&mut intro);
            assert_collapsed(&intro);
            assert_prompt(&intro);
            assert_ne!(intro.inspection_cursor_kind(), Some(3));
            intro.draw(&presenter).unwrap(); // actual tooltip/dwell consumer
            assert!(intro.inspection.as_ref().unwrap().tooltip_state.is_none());

            // Rebuild through actual snapshot retention while physical Right is
            // still held and the retained camera flag is false.
            install_snapshot(&mut intro, &presenter, &save, true);
            assert!(intro.inspection.as_ref().unwrap().toolbar_right_held);
            assert!(!intro.inspection.as_ref().unwrap().mouse_buttons[1]);
            click_info(&mut intro);
            assert_collapsed(&intro);
            assert_prompt(&intro);

            // Release is also swallowed by the prompt, but must release toolbar
            // admission. Actual dispatcher Info then reveals normally.
            intro.inspection_mouse(MouseButton::Right, false);
            assert_prompt(&intro);
            assert!(!intro.inspection.as_ref().unwrap().toolbar_right_held);
            assert_eq!(intro.inspection_cursor_kind(), Some(3));
            click_info(&mut intro);
            assert!(intro.inspection.as_ref().unwrap().details_visible);
            for _ in 0..26 {
                intro.draw(&presenter).unwrap();
            }
            assert_eq!(
                intro
                    .inspection
                    .as_ref()
                    .unwrap()
                    .tooltip_state
                    .map(|s| s.0),
                Some(29)
            );

            // A partially captured Left gesture cannot survive a Right state change.
            intro.inspection.as_mut().unwrap().details_visible = false;
            point_at_info(&mut intro);
            intro.inspection_mouse(MouseButton::Left, true);
            assert!(intro.inspection.as_ref().unwrap().toolbar_input.captured());
            intro.inspection_mouse(MouseButton::Right, true);
            assert!(!intro.inspection.as_ref().unwrap().toolbar_input.captured());
            intro.inspection_mouse(MouseButton::Right, false);
            intro.inspection_mouse(MouseButton::Left, false);
            assert_collapsed(&intro);

            // Focus loss clears the physical sample and cancels captures; a later
            // ordinary Left click is allowed without requiring a missing OS release.
            intro.inspection_mouse(MouseButton::Right, true);
            intro.inspection_unfocus();
            assert!(!intro.inspection.as_ref().unwrap().toolbar_right_held);
            assert!(!intro.inspection.as_ref().unwrap().toolbar_input.captured());
            click_info(&mut intro);
            assert!(intro.inspection.as_ref().unwrap().details_visible);
            assert_prompt(&intro);
            assert_eq!(fs::read(&original).unwrap(), save_bytes);
            drop(intro);
            drop(presenter);
            drop(event_loop); // no worker sender was installed
            drop(cleanup);
            assert!(!scratch.exists());
        })
        .unwrap()
        .join()
        .unwrap();
}

// Direct Intro handler/production callback coverage; App routing is source-joined
// separately. Reuses the supplied snapshot constructor above, without a worker.
#[test]
#[ignore = "requires X11/GPU, supplied original assets/SAVE_ONE, ffmpeg and OPENMT2001_TOOLBAR_RIGHT_TEST_ROOT; direct Intro arrow regression"]
#[allow(deprecated)]
fn modal_arrow_releases_retire_held_state_and_preserve_admitted_taps() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let root = openmt2001::fixtures::scratch("TOOLBAR_RIGHT_TEST_ROOT");
            assert!(
                root.is_dir(),
                "supply an existing owned disk scratch parent"
            );
            let scratch = root.join(format!(
                "camera-arrow-release-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&scratch).unwrap();
            struct Cleanup(PathBuf);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
            let cleanup = Cleanup(scratch.clone());
            let original = openmt2001::fixtures::path("SAVE_ONE");
            let save_bytes = fs::read(&original).unwrap();
            let save = scratch.join("fixture.sav");
            fs::write(&save, &save_bytes).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let params = openmt2001::fixtures::path("PARAMETERS");
            let directory = openmt2001::fixtures::path("ARCHIVE_DIR");
            let mut builder = EventLoop::builder();
            builder.with_x11().with_any_thread(true);
            let event_loop = builder.build().unwrap();
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_visible(false)
                            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 960)),
                    )
                    .unwrap(),
            );
            let presenter = pollster::block_on(Presenter::new(window)).unwrap();
            let assets = Assets::load(Paths {
                compatibility: crate::compatibility::Mode::Modern,
                installer: params.parent().unwrap().parent().unwrap().to_path_buf(),
                ffmpeg: PathBuf::from(
                    std::env::var("OPENMT2001_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
                ),
                scene: directory.parent().unwrap().to_path_buf(),
                executable: exe.clone(),
                map: openmt2001::fixtures::path("MAP"),
                quality: Some("low".into()),
                language: exe
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
                profile_root: scratch.join("profiles"),
                save_root: scratch.join("saves"),
            })
            .unwrap();
            let mut intro = Intro::new(&presenter, assets).unwrap();
            intro.open_menu();
            install_snapshot(&mut intro, &presenter, &save, false);
            use winit::keyboard::NamedKey;
            // The shared fixture deliberately starts disabled and prompted.
            // Establish ordinary admitted input, rather than seed held bits.
            {
                let view = intro.inspection.as_mut().unwrap();
                view.prompts = crate::prompt::State::default();
                view.stats_requested = false;
                view.result = None;
                view.result_pending = None;
                view.game_options.open = false;
                view.save_dialog.visible = false;
                view.planning = None;
                view.planning_pending = None;
                view.adjustment = None;
                view.auction.scene = None;
                view.strategic = false;
                view.building_camera = false;
                let status = view.live_status.as_mut().unwrap();
                status.stats = None;
                status.result_request = None;
                status.tutorial_controls.all_disabled = false;
                status.tutorial_controls.camera_disabled = false;
                assert!(!status.tutorial_controls.all_disabled);
                assert!(!status.tutorial_controls.camera_disabled);
                assert!(view.prompts.request().is_none());
                assert!(view.saved_camera, "supplied snapshot uses the existing saved helper");
                assert_eq!(view.keys, [false; 4]);
                assert_eq!(view.taps, [false; 4]);
            }
            // Force an actual production callback due without sleeping or
            // asserting on interpolated/pixel pose equality.
            fn due_callback(intro: &mut Intro) {
                let due = {
                    let view = intro.inspection.as_mut().unwrap();
                    let due = (view.started.elapsed().as_secs_f64() * 50.).floor() as usize;
                    view.tick = due;
                    due
                };
                intro.inspection_update().unwrap();
                assert!(intro.inspection.as_ref().unwrap().tick > due);
            }
            for (index, key) in [NamedKey::ArrowRight, NamedKey::ArrowLeft,
                NamedKey::ArrowDown, NamedKey::ArrowUp].into_iter().enumerate() {
                intro.inspection_key(key, true).unwrap();
                assert!(intro.inspection.as_ref().unwrap().keys[index]);
                assert!(intro.inspection.as_ref().unwrap().taps[index]);
                due_callback(&mut intro);
                assert_eq!(intro.inspection.as_ref().unwrap().taps, [false; 4],
                    "initial admitted tap must be consumed before modal release");

                // This is the actual unconditional Stats-request key gate,
                // controlled explicitly; no naturally opened Stats claim.
                intro.inspection.as_mut().unwrap().stats_requested = true;
                intro.inspection_key(key, false).unwrap();
                let view = intro.inspection.as_ref().unwrap();
                assert!(view.stats_requested);
                assert_eq!(view.keys, [false; 4]);
                assert_eq!(view.taps, [false; 4]);
                intro.inspection_key(key, true).unwrap();
                let view = intro.inspection.as_ref().unwrap();
                assert!(view.stats_requested, "blocked press must not dismiss gate");
                assert_eq!(view.keys, [false; 4]);
                assert_eq!(view.taps, [false; 4]);
                let center = view.controls.center;
                due_callback(&mut intro);
                due_callback(&mut intro);
                assert_eq!(intro.inspection.as_ref().unwrap().controls.center, center,
                    "released held input must not apply new control translation");
                intro.inspection.as_mut().unwrap().stats_requested = false;
                due_callback(&mut intro);
                assert_eq!(intro.inspection.as_ref().unwrap().controls.center, center);

                // A quick admitted tap survives its release until production
                // consumption, then cannot repeat on a subsequent callback.
                intro.inspection_key(key, true).unwrap();
                intro.inspection_key(key, false).unwrap();
                let view = intro.inspection.as_ref().unwrap();
                assert_eq!(view.keys, [false; 4]);
                assert!(view.taps[index]);
                assert_eq!(view.taps.iter().filter(|&&tap| tap).count(), 1);
                let center = view.controls.center;
                due_callback(&mut intro);
                let view = intro.inspection.as_ref().unwrap();
                assert_eq!(view.taps, [false; 4]);
                assert_ne!(view.controls.center, center, "actual callback must consume the tap");
                let after = view.controls.center;
                due_callback(&mut intro);
                assert_eq!(intro.inspection.as_ref().unwrap().controls.center, after,
                    "consumed tap must not translate again");
            }
            intro.inspection_key(NamedKey::ArrowRight, true).unwrap();
            assert!(intro.inspection.as_ref().unwrap().keys[0]);
            assert!(intro.inspection.as_ref().unwrap().taps[0]);
            intro.inspection_unfocus();
            assert_eq!(intro.inspection.as_ref().unwrap().keys, [false; 4]);
            assert_eq!(intro.inspection.as_ref().unwrap().taps, [false; 4]);
            // Feed the same focus-admitted physical path as App. These view
            // shortcuts must share the existing compass/parent-block controls.
            use winit::keyboard::{KeyCode, PhysicalKey};
            fn sample(intro: &mut Intro, code: KeyCode, down: bool, repeat: bool) {
                intro.camera_physical_sample(process_frontend::PhysicalSample::Key {
                    key: PhysicalKey::Code(code), down, repeat,
                }).unwrap();
            }
            for code in [KeyCode::Tab, KeyCode::Home] {
                let view = intro.inspection.as_mut().unwrap();
                view.live_status.as_mut().unwrap().tutorial_controls.camera_disabled = true;
                let before = camera_producer::Controls::read(view);
                sample(&mut intro, code, true, false);
                let view = intro.inspection.as_mut().unwrap();
                assert_eq!(view.camera.eye, before.camera.eye);
                assert!(!view.strategic);
                view.live_status.as_mut().unwrap().tutorial_controls.camera_disabled = false;
                sample(&mut intro, code, true, true);
                assert!(!intro.inspection.as_ref().unwrap().strategic);
                sample(&mut intro, code, false, false);
            }
            let before = camera_producer::Controls::read(intro.inspection.as_ref().unwrap());
            sample(&mut intro, KeyCode::Tab, true, false);
            assert!(intro.inspection.as_ref().unwrap().strategic);
            sample(&mut intro, KeyCode::Tab, false, false);
            sample(&mut intro, KeyCode::Tab, true, false);
            sample(&mut intro, KeyCode::Tab, false, false);
            let view = intro.inspection.as_ref().unwrap();
            assert!(!view.strategic);
            assert_eq!(view.camera.eye, before.camera.eye);
            assert_eq!(view.camera.target, before.camera.target);
            assert_eq!(view.controls.center, before.controls.center);
            assert_eq!(view.controls.angle, before.controls.angle);
            assert_eq!(view.controls.height, before.controls.height);
            assert_eq!(view.controls.radius, before.controls.radius);
            assert_eq!(view.selected, before.selected);
            let hit = intro.inspection.as_ref().unwrap().scene.inspection_buildings
                .iter().enumerate().find_map(|(index, building)|
                    inspection_hit(&intro.assets.selection.blocks, building.category, Some(index)))
                .expect("supplied snapshot has a building with a parent block");
            intro.inspection.as_mut().unwrap().selected = Some(hit);
            intro.inspection.as_mut().unwrap().stats_requested = true;
            sample(&mut intro, KeyCode::Home, true, false);
            assert_eq!(intro.inspection.as_ref().unwrap().selected, Some(hit));
            intro.inspection.as_mut().unwrap().stats_requested = false;
            sample(&mut intro, KeyCode::Home, true, true);
            assert_eq!(intro.inspection.as_ref().unwrap().selected, Some(hit));
            sample(&mut intro, KeyCode::Home, false, false);
            sample(&mut intro, KeyCode::Home, true, false);
            sample(&mut intro, KeyCode::Home, false, false);
            let view = intro.inspection.as_ref().unwrap();
            assert_eq!(view.selected, Some(crate::selection::Hit { category: hit.category, building: None }));
            assert_eq!(view.controls.center, intro.assets.selection.center(hit.category));
            assert_eq!(view.controls.radius, 80.);
            assert_eq!(view.controls.height, 120.);
            assert!(!view.building_camera);
            assert_eq!(fs::read(&original).unwrap(), save_bytes);
            drop(intro);
            drop(presenter);
            drop(event_loop);
            drop(cleanup);
            assert!(!scratch.exists());
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Same fixture as above, with a stand-in Session worker that owns the real
/// Options pause transition (`Host::options`). It drives Escape and the
/// clock's Options button over an open prompt through the production
/// keyboard, mouse and dialog-key paths (#416).
#[test]
#[ignore = "requires X11/GPU, supplied original assets/SAVE_ONE, ffmpeg and OPENMT2001_TOOLBAR_RIGHT_TEST_ROOT; actual dispatcher integration"]
#[allow(deprecated)]
fn escape_and_options_button_open_options_over_a_prompt_and_continue_returns_to_it() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let root = openmt2001::fixtures::scratch("TOOLBAR_RIGHT_TEST_ROOT");
            assert!(root.is_dir(), "supply an existing owned disk scratch parent");
            let scratch = root.join(format!(
                "options-over-prompt-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&scratch).unwrap();
            struct Cleanup(PathBuf);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
            let cleanup = Cleanup(scratch.clone());
            let save = scratch.join("fixture.sav");
            fs::write(&save, fs::read(openmt2001::fixtures::path("SAVE_ONE")).unwrap()).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let params = openmt2001::fixtures::path("PARAMETERS");
            let directory = openmt2001::fixtures::path("ARCHIVE_DIR");
            let mut builder = EventLoop::builder();
            builder.with_x11().with_any_thread(true);
            let event_loop = builder.build().unwrap();
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_visible(false)
                            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 960)),
                    )
                    .unwrap(),
            );
            let presenter = pollster::block_on(Presenter::new(window)).unwrap();
            let assets = Assets::load(Paths {
                compatibility: crate::compatibility::Mode::Modern,
                installer: params.parent().unwrap().parent().unwrap().to_path_buf(),
                ffmpeg: PathBuf::from(
                    std::env::var("OPENMT2001_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
                ),
                scene: directory.parent().unwrap().to_path_buf(),
                executable: exe.clone(),
                map: openmt2001::fixtures::path("MAP"),
                quality: Some("low".into()),
                language: exe.parent().unwrap().parent().unwrap().join("US/language/usa"),
                profile_root: scratch.join("profiles"),
                save_root: scratch.join("saves"),
            })
            .unwrap();
            let mut intro = Intro::new(&presenter, assets).unwrap();
            intro.open_menu();
            install_snapshot(&mut intro, &presenter, &save, false);

            // Stand-in Session worker: the real Host owns the Options hold.
            let (tx, rx) = mpsc::channel::<FreshRequest>();
            let (pause_tx, pauses) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let mut host = crate::live_host::Host::new([0.; 3]);
                host.set_running(true);
                let mut pause = openmt2001::pause::State::default();
                let mut timers = openmt2001::timers::startup_tasks([0; 20]);
                while let Ok(request) = rx.recv() {
                    if let FreshRequest::Options { open, reply } = request {
                        reply.send(host.options(&mut pause, &mut timers, open)).unwrap();
                        pause_tx.send((pause.clone(), host.running)).unwrap();
                    }
                }
            });
            let unpaused = (openmt2001::pause::State::default(), true);
            let pause_after = |pauses: &mpsc::Receiver<(openmt2001::pause::State, bool)>| {
                pauses.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
            };
            {
                let view = intro.inspection.as_mut().unwrap();
                view._fresh_session = Some(tx.clone());
                let status = view.live_status.as_mut().unwrap();
                status.script_paused = false;
                assert!(status.tutorial_controls.all_disabled);
                assert!(view.prompts.blocks_controls());
                assert!(!view.options_visible());
            }
            let prompt = intro.inspection.as_ref().unwrap().prompts.request().unwrap();
            let escape = |intro: &mut Intro, pressed: bool| {
                let admitted = intro.admit_city_key(NamedKey::Escape, pressed, false, false);
                intro.inspection_keyboard(NamedKey::Escape, pressed, admitted).unwrap();
            };
            use process_frontend::PhysicalSample;
            use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};
            let enter = |intro: &mut Intro, down: bool| {
                intro.dialog_physical_sample(PhysicalSample::Key {
                    key: PhysicalKey::Code(KeyCode::Enter),
                    down,
                    repeat: false,
                });
                intro.dialog_ui_event1().unwrap();
            };
            // Arm the prompt's Return shortcut (its first update only arms it).
            intro.dialog_ui_event1().unwrap();

            // Escape over the prompt opens Options and holds reason 0 once.
            escape(&mut intro, true);
            let view = intro.inspection.as_ref().unwrap();
            assert!(view.options_visible(), "Escape opens Options over the prompt");
            assert_eq!(view.prompts.request(), Some(prompt.clone()));
            let (held, running) = pause_after(&pauses);
            assert_eq!(&held.bytes[..4], &[1, 0, 0, 0]);
            assert!(!running);
            // A held Escape repeats nothing; Return under Options leaves the prompt.
            escape(&mut intro, true);
            enter(&mut intro, true);
            enter(&mut intro, false);
            let view = intro.inspection.as_mut().unwrap();
            assert!(view.options_visible());
            assert_eq!(view.prompts.request(), Some(prompt.clone()));
            assert!(view.prompts.take_dialog_replies().is_empty());
            assert!(pauses.try_recv().is_err(), "held Escape acts once");
            escape(&mut intro, false);

            // Continue returns to the prompt with the pause state restored.
            let continue_button = intro.assets.game_options.bounds()[0];
            let click = |intro: &mut Intro, logical: [f32; 2]| {
                intro.inspection_cursor(
                    [f64::from(logical[0] * 2.), f64::from(logical[1] * 2.)],
                    [1280, 960],
                );
                intro.inspection_mouse(MouseButton::Left, true);
                intro.inspection_mouse(MouseButton::Left, false);
            };
            let centre = |b: [f32; 4]| [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5];
            click(&mut intro, centre(continue_button));
            let view = intro.inspection.as_ref().unwrap();
            assert!(!view.options_visible(), "Continue closes Options");
            assert_eq!(view.prompts.request(), Some(prompt.clone()));
            assert_eq!(pause_after(&pauses), unpaused);

            // The clock's Options button also opens Options over the prompt,
            // with the tutorial's controls disabled.
            assert!(
                intro.inspection.as_ref().unwrap().live_status.as_ref().unwrap().time_display.visible,
                "fixture shows the clock"
            );
            let options_point = internal_pointer([880., 26.], [960, 720])
                .unwrap()
                .map(|v| v as f32 * 0.5);
            click(&mut intro, options_point);
            let view = intro.inspection.as_ref().unwrap();
            assert!(view.options_visible(), "the Options button opens Options");
            assert_eq!(view.prompts.request(), Some(prompt.clone()));
            assert_eq!(&pause_after(&pauses).0.bytes[..4], &[1, 0, 0, 0]);
            click(&mut intro, centre(continue_button));
            assert!(!intro.inspection.as_ref().unwrap().options_visible());
            assert_eq!(pause_after(&pauses), unpaused);

            // With Options gone, the prompt's Return answers it again.
            enter(&mut intro, true);
            enter(&mut intro, false);
            let view = intro.inspection.as_mut().unwrap();
            assert_eq!(view.prompts.take_dialog_replies().len(), 1);

            // A held pause reason (0x8b22d8) stops Escape over the prompt.
            view.prompts = crate::prompt::State::default();
            view.prompts.submit(prompt.clone());
            view.live_status.as_mut().unwrap().script_paused = true;
            escape(&mut intro, true);
            escape(&mut intro, false);
            assert!(!intro.inspection.as_ref().unwrap().options_visible());
            assert!(pauses.try_recv().is_err());

            intro.inspection.as_mut().unwrap()._fresh_session = None;
            drop(tx);
            worker.join().unwrap();
            drop(intro);
            drop(presenter);
            drop(event_loop);
            drop(cleanup);
            assert!(!scratch.exists());
        })
        .unwrap()
        .join()
        .unwrap();
}
