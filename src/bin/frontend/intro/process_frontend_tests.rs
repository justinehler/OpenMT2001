//! CPU tests of the production owner/prefix and physical admission. Output
//! mixers are retained but never attached to an OS audio device.
use super::*;
use winit::keyboard::{KeyCode, PhysicalKey};
fn owner() -> Owner {
    let (process, receipt) = crate::live_host::process_runtime::test_frontend_startup();
    ProcessFrontend::new(
        receipt,
        process,
        WindowId::from(7),
        crate::settings_menu::Levels {
            music: 63,
            effects: 83,
        },
    )
    .unwrap()
}
#[test]
fn ordinary_birth_and_menu_focus_preserve_native_cells_and_saved_audio() {
    let owner = owner();
    let mut p = owner.borrow_mut();
    assert!(
        p.activation.guards().active
            && p.activation.guards().sound_enabled
            && p.activation.guards().music_enabled
    );
    assert!(p.activation.birth().is_err());
    let cells = p.input.cells().clone();
    let characters = p.input.characters().clone();
    p.observe(true).unwrap(); // Wrapper suspended0: no unconditional Resume.
    assert_eq!(
        (p.audio.music_level(), p.audio.factor()),
        (63, crate::music_gain::normalized_percentage(83))
    );
    p.observe(false).unwrap();
    assert_eq!(p.wrapper.suspended_byte(), 1);
    assert!(p.audio.music_paused());
    assert_eq!(p.audio.music_level(), 0);
    let mut expected = cells.clone();
    expected.input_enabled = 0;
    expected.focus_transition = 1; // preceding actual Gain writes1; Loss retains it.
    assert_eq!(p.input.cells().focus_transition, 1);
    assert_eq!(p.input.cells(), &expected);
    assert_eq!(p.input.characters(), &characters);
    p.commit_levels(crate::settings_menu::Levels {
        music: 25,
        effects: 25,
    });
    assert!(p.audio.music_paused());
    p.observe(true).unwrap();
    assert_eq!(p.wrapper.suspended_byte(), 0);
    assert_eq!(p.audio.music_level(), 63);
    assert_eq!(
        p.audio.factor(),
        crate::music_gain::normalized_percentage(83)
    );
    expected.input_enabled = 1;
    expected.focus_transition = 1;
    assert_eq!(p.input.cells(), &expected);
    assert_eq!(p.input.characters(), &characters);
    assert_eq!(p.custody.as_ref().unwrap().tasks.len(), 4);
    assert_eq!(
        p.custody.as_ref().unwrap().pause.bytes[0],
        1,
        "startup hold survives focus"
    );
}
#[test]
fn menu_signed_reason3_uses_same_wrapper_and_all_nine_bytes() {
    for reason in [-128i8, -1, 0, 1, 2, 127] {
        let owner = owner();
        let mut p = owner.borrow_mut();
        p.custody.as_mut().unwrap().pause.bytes = [0, 0, 0, reason as u8, 0, 0xa5, 0x5a, 0x92, 0];
        p.observe(false).unwrap();
        p.observe(true).unwrap();
        assert_eq!(p.wrapper.suspended_byte(), u8::from(reason > 0));
        assert_eq!(p.custody.as_ref().unwrap().pause.bytes[3], reason as u8);
        assert_eq!(
            &p.custody.as_ref().unwrap().pause.bytes[5..8],
            &[0xa5, 0x5a, 0x92]
        );
    }
}
#[test]
fn actual_receiver_reread_reports_partial_mouse_prefix_without_retained_stores() {
    for replace in [false, true] {
        let owner = owner();
        let mut p = owner.borrow_mut();
        let cells = p.input.cells().clone();
        let receiver = p.receiver.as_ref().unwrap().clone();
        p.after_mouse = Some(Box::new(move |p| {
            p.receiver = replace.then(|| {
                Rc::new(RefCell::new(Receiver {
                    identity: 2,
                    window: WindowId::from(7),
                    admitted: [false; 2],
                }))
            });
        }));
        assert!(p.observe(true).is_err());
        assert_eq!(receiver.borrow().admitted, [true, false]);
        assert_eq!(p.input.cells(), &cells);
        assert_eq!(p.wrapper.suspended_byte(), 0);
        assert_eq!(p.phase, Phase::Failed);
        assert!(p.take_custody(true).is_err());
    }
}
#[test]
fn physical_suffix_preserves_fences_and_foreground_admission_and_cancels_overflow() {
    let owner = owner();
    let mut p = owner.borrow_mut();
    let window = WindowId::from(7);
    p.observe(true).unwrap();
    let key = PhysicalSample::Key {
        key: PhysicalKey::Code(KeyCode::ArrowRight),
        down: true,
        repeat: false,
    };
    assert!(p.sample(WindowId::from(8), key, 0, 0).is_err());
    p.sample(window, key, 1, 0).unwrap();
    assert!(p.take_samples().is_empty());
    p.complete_physical(1);
    assert!(matches!(
        &p.take_samples()[..],
        [PhysicalSample::Key { down: true, .. }]
    ));
    p.sample(window, key, 2, 1).unwrap();
    p.observe(false).unwrap();
    p.complete_physical(2);
    assert!(
        p.take_samples().is_empty(),
        "loss admission rejects the physical suffix"
    );
    for _ in 0..256 {
        p.sample(window, key, 3, 2).unwrap();
    }
    assert!(p.sample(window, key, 3, 2).is_err());
    assert_eq!(p.phase, Phase::Failed);
    assert_eq!(p.cancelled_samples, 256);
    assert!(p.take_samples().is_empty());
}
#[test]
fn move_only_cut_loading_and_join_return_reconcile_only_newest_observation() {
    let owner = owner();
    let mut p = owner.borrow_mut();
    p.observe(false).unwrap();
    let input = p.input.cells().clone();
    let audio = p.audio.clone();
    let mut custody = p.take_custody(true).unwrap();
    custody.pause.bytes = [0, 3, 4, 5, 6, 0x12, 0x34, 0x56, 1];
    custody.tasks[0][1..4].copy_from_slice(&[0xa1, 0xb2, 0xc3]);
    let tasks = custody.tasks.clone();
    p.observe(true).unwrap();
    p.observe(false).unwrap();
    p.observe(true).unwrap();
    assert_eq!(
        p.input.cells(),
        &input,
        "Loading only caches OS observation"
    );
    p.return_custody(custody).unwrap();
    assert!(audio.same_owner(&p.audio));
    assert_eq!(
        &p.custody.as_ref().unwrap().pause.bytes[1..],
        &[3, 4, 5, 5, 0x12, 0x34, 0x56, 1]
    );
    assert_eq!(p.custody.as_ref().unwrap().tasks, tasks);
    assert_eq!(p.input.cells().input_enabled, 1);
    assert_eq!(
        p.wrapper.suspended_byte(),
        1,
        "positive real reason3 prevents Resume"
    );
    let custody = p.take_custody(false).unwrap();
    let observed = p.observation;
    p.return_custody(custody).unwrap();
    assert_eq!(p.observation, observed);
    assert_eq!(
        p.custody.as_ref().unwrap().pause.bytes[4],
        5,
        "no extra return operation when focus unchanged"
    );
}
#[test]
fn menu_pcm_uses_current_suspended_state_and_old_generation_never_appends() {
    let owner = owner();
    let mut p = owner.borrow_mut();
    p.observe(true).unwrap();
    let token = p.menu_token().unwrap();
    p.observe(false).unwrap();
    p.commit_levels(crate::settings_menu::Levels {
        music: 25,
        effects: 25,
    });
    let (mixer, mut source) = rodio::mixer::mixer(2, 44100);
    p.attach_menu(&mixer, token.clone(), vec![1.; 64]).unwrap();
    assert!(p.menu_music.as_ref().unwrap().is_paused());
    assert_eq!(
        p.menu_music.as_ref().unwrap().volume(),
        crate::music_gain::amplitude(25)
    );
    assert_eq!(source.next(), Some(0.));
    p.stop_menu().unwrap();
    assert!(!token.is_current());
    assert!(p.attach_menu(&mixer, token, vec![1.; 64]).is_err());
    p.observe(true).unwrap();
    assert!(p.menu_music.is_none());
}
#[test]
fn dispatch_fresh_reset_preserves_padding_frequency_and_timer_envelope() {
    let mut custody = DispatchCustody::startup(openmt2001::pause::State { bytes: [0; 9] }).unwrap();
    custody.pause.bytes = [2, 0, 0, 3, 4, 5, 6, 7, 1];
    custody.frequency.frequency = 17.;
    custody.frequency.calendar_timer = Some(1);
    custody.frequency.logic_timer = Some(2);
    custody.frame.statistics_since = 999;
    custody.frame.rate = 23.;
    custody.frame.frames = 7;
    custody.frame.pending_seconds = 0.125;
    custody.frame.pending_groups = 0xa7;
    for task in &mut custody.tasks {
        task[1..4].copy_from_slice(&[3, 4, 5]);
        task[12..16].copy_from_slice(&99f32.to_le_bytes());
    }
    let (mut seed, mut timing) = custody.prepare_start(true).unwrap();
    seed.fresh_reset().unwrap();
    assert_eq!(seed.pause.bytes, [0, 0, 0, 3, 4, 5, 6, 7, 1]);
    assert_eq!(
        (
            seed.frequency.frequency,
            seed.frequency.calendar_timer,
            seed.frequency.logic_timer
        ),
        (17., Some(1), Some(2))
    );
    for task in seed.tasks {
        assert_eq!(&task[1..4], &[3, 4, 5]);
        assert_eq!(&task[12..16], &0f32.to_le_bytes());
    }
    timing.frame.synchronize(123);
    assert_eq!(
        (
            timing.frame.previous,
            timing.frame.current,
            timing.frame.statistics_since,
            timing.frame.rate,
            timing.frame.frames,
            timing.frame.pending_seconds,
            timing.frame.pending_groups
        ),
        (123, 123, 999, 23., 7, 0.125, 0xa7)
    );
}

#[test]
fn first_admitted_resume_uses_constructor_saved_defaults_independent_of_preferences() {
    let owner = owner();
    let mut p = owner.borrow_mut();
    p.wrapper = native::Frontend::from_process_byte(1);
    p.observe(true).unwrap();
    assert_eq!((p.audio.factor(), p.audio.music_level()), (1., 75));
    assert_eq!(p.wrapper.suspended_byte(), 0);
}

#[test]
fn gain_down_loss_before_receipt_never_reestablishes_app_camera_or_desktop_holds() {
    use crate::intro::{FreshRequest, focus};
    let owner = owner();
    owner.borrow_mut().observe(true).unwrap();
    let custody = owner.borrow_mut().take_custody(false).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut route = focus::Route::default();
    route.install(Box::new(Provider(owner.clone()))).unwrap();
    route.observe(true).unwrap();
    route.attach(tx).unwrap();
    owner.borrow_mut().note_sent(route.sent());
    let window = WindowId::from(7);
    for sample in [
        PhysicalSample::Key {
            key: PhysicalKey::Code(KeyCode::ArrowRight),
            down: true,
            repeat: false,
        },
        PhysicalSample::Key {
            key: PhysicalKey::Code(KeyCode::Enter),
            down: true,
            repeat: false,
        },
        PhysicalSample::Button {
            button: MouseButton::Right,
            down: true,
        },
        PhysicalSample::Button {
            button: MouseButton::Left,
            down: true,
        },
    ] {
        owner
            .borrow_mut()
            .sample(window, sample, route.sent(), route.completed())
            .unwrap();
    }
    owner.borrow_mut().observe(false).unwrap();
    route.observe(false).unwrap();
    owner.borrow_mut().note_sent(route.sent());
    owner
        .borrow_mut()
        .sample(
            window,
            PhysicalSample::Unfocus,
            route.sent(),
            route.completed(),
        )
        .unwrap();
    let mut camera = super::super::camera_live::PhysicalInput::default();
    let mut desktop = crate::menu::desktop_quit::Keys::default();
    let mut left = crate::app::ParticipantLeft::default();
    let consume = |samples: Vec<PhysicalSample>,
                   camera: &mut super::super::camera_live::PhysicalInput,
                   desktop: &mut crate::menu::desktop_quit::Keys,
                   left: &mut crate::app::ParticipantLeft| {
        for sample in samples {
            project_camera_sample(camera, None, sample);
            crate::app::apply_desktop_sample(desktop, left, sample);
        }
    };
    consume(
        owner.borrow_mut().take_samples(),
        &mut camera,
        &mut desktop,
        &mut left,
    );
    let mut pause = custody.pause;
    let mut tasks = custody.tasks;
    // Real broker/Host prefix receipts arrive only AFTER loss cleanup ran.
    for _ in 0..2 {
        let event = match rx.recv().unwrap() {
            FreshRequest::Focus(event) => event,
            _ => panic!("focus FIFO"),
        };
        let worker = std::thread::spawn(move || {
            let mut host = crate::live_host::Host::new([0.; 3]);
            event.apply(&mut host, &mut pause, &mut tasks).unwrap();
            (pause, tasks)
        });
        route.wait_prefix();
        route.service_one().unwrap();
        (pause, tasks) = worker.join().unwrap();
        route.poll().unwrap();
        owner.borrow_mut().complete_physical(route.completed());
        consume(
            owner.borrow_mut().take_samples(),
            &mut camera,
            &mut desktop,
            &mut left,
        );
        let (keys, buttons, delta) = camera.snapshot().unwrap();
        assert_eq!(keys, [false; 4]);
        assert_eq!(buttons, [false; 2]);
        assert_eq!(delta, [0; 2]);
        assert!(!desktop.right());
    }
    assert_eq!(owner.borrow().cancelled_samples, 4);
    assert!(
        crate::app::apply_desktop_sample(
            &mut desktop,
            &mut left,
            PhysicalSample::Button {
                button: MouseButton::Left,
                down: true
            }
        ),
        "left history was cleaned before any later press"
    );
    route.detach();
}
