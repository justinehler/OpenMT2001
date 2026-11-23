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
            let root = PathBuf::from(std::env::var("OPENMT2001_TOOLBAR_RIGHT_TEST_ROOT").unwrap());
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
            let original = PathBuf::from(std::env::var("OPENMT2001_SAVE_ONE").unwrap());
            let save_bytes = fs::read(&original).unwrap();
            let save = scratch.join("fixture.sav");
            fs::write(&save, &save_bytes).unwrap();
            let exe = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let params = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let directory = PathBuf::from(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap());
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
                map: PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap()),
                quality: "low".into(),
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
