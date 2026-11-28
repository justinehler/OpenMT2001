//! Actual production spawn/constructor/mailbox/OS join with supplied private
//! inputs. CPU only: no App window, GPU, decoder or original execution.
use super::*;
use camera_producer::{Controls, Initial, Producer};
use process_frontend::{Owner, Phase, ProcessFrontend, Provider};
use std::time::Duration;
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let root = openmt2001::fixtures::scratch("TEST_SCRATCH");
        assert!(root.is_absolute());
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "pf-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn inputs(root: PathBuf) -> (Arc<SceneInputs>, Owner) {
    let mut inputs = camera_producer_live_tests::inputs(root);
    let (process, mut activity) =
        crate::live_host::process_runtime::startup(&inputs.executable, "").unwrap();
    Arc::get_mut(&mut inputs).unwrap().process_runtime = Some(process.clone());
    let owner = ProcessFrontend::new(
        activity.take_frontend_startup().unwrap(),
        process,
        winit::window::WindowId::from(7),
        crate::settings_menu::Levels::default(),
    )
    .unwrap();
    owner.borrow_mut().observe(false).unwrap();
    (inputs, owner)
}
struct Running {
    route: focus::Route,
    owner: Owner,
    worker: Option<camera_producer::WorkerExit>,
    loaded: Option<LoadedInspection>,
    sender: Option<mpsc::Sender<FreshRequest>>,
    producer: Option<Producer>,
}
impl Running {
    fn start(inputs: Arc<SceneInputs>, owner: Owner, start: WorkerStart) -> Self {
        Self::start_with_route(inputs, owner, start, focus::Route::default())
    }
    fn start_with_route(
        inputs: Arc<SceneInputs>,
        owner: Owner,
        start: WorkerStart,
        route: focus::Route,
    ) -> Self {
        Self::start_after_menu_delay(inputs, owner, start, route, Duration::ZERO)
    }
    fn start_after_menu_delay(
        inputs: Arc<SceneInputs>,
        owner: Owner,
        start: WorkerStart,
        route: focus::Route,
        delay: Duration,
    ) -> Self {
        // Production Intro retains this ONE Route across every World. Its
        //sequence must continue with the SAME retained ProcessFrontend.
        let sent_before = route.sent();
        let (camera, controls) = fresh_city_camera();
        let (tx, rx) = mpsc::channel();
        let mut custody = owner
            .borrow_mut()
            .take_custody(!matches!(&start, WorkerStart::Saved(..)))
            .unwrap();
        // The process clock was last sampled `delay` ago, as after menus.
        let now = custody.epoch.elapsed().as_millis() as u32;
        custody
            .frame
            .synchronize(now.wrapping_sub(delay.as_millis() as u32));
        let worker = camera_producer::spawn_session_joined(
            "pf-connected",
            inputs,
            start,
            camera,
            tx,
            None,
            true,
            Some(custody),
        )
        .unwrap();
        let mut run = Self {
            route,
            owner,
            worker: Some(worker),
            loaded: None,
            sender: None,
            producer: None,
        };
        let mut loaded = rx.recv_timeout(Duration::from_secs(45)).unwrap().unwrap();
        run.sender = loaded.fresh_session.clone();
        let initial = Initial::from_loaded(
            &loaded,
            Controls {
                camera,
                controls,
                inertia: openmt2001::camera::BlockCameraInertia::new(camera.eye, camera.target),
                saved: false,
                building: false,
                building_angle: 0.,
                selected: None,
            },
        )
        .unwrap();
        let mut parked = Some(camera_producer::new_process_owner().unwrap());
        run.producer = Some(
            Producer::attach(
                &mut parked,
                loaded.camera_publisher.take().unwrap(),
                &initial,
            )
            .unwrap(),
        );
        run.loaded = Some(loaded);
        // Loading history must not become a second event replay.
        run.owner.borrow_mut().observe(false).unwrap();
        run.owner.borrow_mut().observe(true).unwrap();
        if !run.route.enabled() {
            run.route
                .install(Box::new(Provider(run.owner.clone())))
                .unwrap();
        }
        run.route.observe(true).unwrap();
        let (reply, ready) = mpsc::channel();
        run.send(FreshRequest::UiCameraReady { reply });
        run.route
            .attach(run.sender.as_ref().unwrap().clone())
            .unwrap();
        run.owner.borrow_mut().note_sent(run.route.sent());
        ready
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
            .unwrap();
        run.route.wait_prefix();
        run.route.service_one().unwrap();
        let _ = run.clock();
        run.route.poll().unwrap();
        assert_eq!(
            run.route.sent(),
            sent_before + 1,
            "exactly one current snapshot per cold attach"
        );
        assert_eq!(
            run.route.completed(),
            sent_before + 1,
            "that exact operation reached its terminal receipt"
        );
        run
    }
    fn send(&self, request: FreshRequest) {
        self.sender.as_ref().unwrap().send(request).unwrap();
    }
    fn clock(&self) -> crate::time_panel::ClockSample {
        let (reply, rx) = mpsc::channel();
        self.send(FreshRequest::LogicTick(reply));
        rx.recv_timeout(Duration::from_secs(15)).unwrap()
    }
    fn finish(&mut self) -> Result<crate::process_dispatch::DispatchCustody, String> {
        self.route.detach();
        if let Some(producer) = self.producer.take() {
            producer.retire().1?;
        }
        self.sender = None;
        self.loaded = None;
        let worker = self.worker.as_mut().ok_or("worker already joined")?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(result) = worker.poll() {
                result?;
                let custody = worker.take_custody().ok_or("joined without custody")?;
                self.worker = None;
                return Ok(custody);
            }
            assert!(Instant::now() < deadline, "actual join did not finish");
            std::thread::yield_now();
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.finish();
        }
    }
}
#[test]
#[ignore = "private supplied inputs and owned disk scratch; actual joined CPU worker"]
fn all_three_start_routes_full_join_and_warm_settings_keep_process_ownership() {
    use openmt2001::skirmish::roster::{Kind, Participant};
    let scratch = Scratch::new();
    let (inputs, owner) = inputs(scratch.0.join("profiles"));
    let setup = camera_producer_live_tests::setup(&inputs);
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let catalog =
        crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap()).unwrap();
    let skirmish = crate::menu::skirmish_route::Request {
        catalog,
        save_root: scratch.0.join("skirmish"),
        launch: crate::fresh_city::SkirmishLaunch {
            level_id: 300001,
            directory: "net1".into(),
            map: "empty.lua".into(),
            objective_kind: 6,
            target: 3500.,
            difficulty: 1,
            participants: vec![
                Participant {
                    kind: Kind::Human,
                    token: Some(4),
                    name: b"PF human".to_vec(),
                    ready: true,
                    difficulty: 2,
                },
                Participant {
                    kind: Kind::Ai,
                    token: Some(1),
                    name: b"PF AI".to_vec(),
                    ready: true,
                    difficulty: 0,
                },
            ],
        },
    };
    let save = openmt2001::fixtures::path("SAVE_ONE");
    let mut route = Some(focus::Route::default());
    for start in [
        WorkerStart::Fresh(setup),
        WorkerStart::Saved(save, Some(scratch.0.join("selected"))),
        WorkerStart::Skirmish(skirmish),
    ] {
        let audio = owner.borrow().audio.clone();
        let mut run =
            Running::start_with_route(inputs.clone(), owner.clone(), start, route.take().unwrap());
        // Space/Run remains Host.running; it cannot stand in for reason3.
        run.send(FreshRequest::Run(false));
        let _ = run.clock();
        let (reply, status) = mpsc::channel();
        run.send(FreshRequest::Status {
            camera: fresh_city_camera().0.eye,
            reply,
        });
        assert!(
            !status
                .recv_timeout(Duration::from_secs(15))
                .unwrap()
                .running
        );
        run.owner.borrow_mut().observe(false).unwrap();
        run.route.observe(false).unwrap();
        run.owner.borrow_mut().note_sent(run.route.sent());
        run.route.wait_prefix();
        run.route.service_one().unwrap();
        let _ = run.clock();
        run.route.poll().unwrap();
        owner
            .borrow_mut()
            .commit_levels(crate::settings_menu::Levels {
                music: 25,
                effects: 25,
            });
        assert!(audio.music_paused());
        let generation = run.route.attachment_id();
        let (reply, rx) = mpsc::channel();
        run.send(FreshRequest::Inspect {
            camera: fresh_city_camera().0,
            reply,
        });
        drop(rx.recv_timeout(Duration::from_secs(45)).unwrap().unwrap());
        assert_eq!(run.route.attachment_id(), generation);
        assert!(audio.same_owner(&owner.borrow().audio));
        assert_eq!(owner.borrow().phase, Phase::World);
        let custody = run.finish().unwrap();
        assert_eq!(custody.tasks.len(), 4);
        assert!(camera_producer::startup_ready(&run.worker, &None));
        owner.borrow_mut().return_custody(custody).unwrap();
        assert_eq!(owner.borrow().phase, Phase::Menu);
        assert!(audio.same_owner(&owner.borrow().audio));
        route = Some(std::mem::take(&mut run.route));
    }
}
#[test]
#[ignore = "private supplied inputs and owned disk scratch; actual cold record12 and joined CPU worker"]
fn cold_record12_supersedes_menu_numeric_image_but_keeps_input_wrapper_and_audio() {
    let scratch = Scratch::new();
    let (inputs, owner) = inputs(scratch.0.join("profiles"));
    let (camera, _) = fresh_city_camera();
    let mut cache = SnapshotCache::default();
    let (_, mut session) = inputs
        .fresh_preview(
            camera_producer_live_tests::setup(&inputs),
            camera,
            &mut cache,
        )
        .unwrap();
    let expected = [2, 1, 0, 0, 0, 0xa5, 0x5a, 0xee, 1];
    session.world.pause.bytes = expected;
    for task in &mut session.hooks.timers {
        task[0] = 0;
        task[1..4].copy_from_slice(&[0x71, 0x82, 0x93]);
    }
    session.hooks.frequency.frequency = 17.;
    session.hooks.frequency.calendar_timer = Some(1);
    session.hooks.frequency.logic_timer = Some(2);
    let rows = session.hooks.timers.clone();
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    // This connected save fixture must use the actual process camera image,
    //not the isolated pose encoder's deliberate all-zero kind prior.
    let (publisher, _reader) =
        crate::save_menu::camera_source::channel(session.world.topology.epoch());
    let mut parked = Some(camera_producer::new_process_owner().unwrap());
    let (_, controls) = fresh_city_camera();
    let initial = Initial::prepare(
        None,
        Some(0),
        Controls {
            camera,
            controls,
            inertia: openmt2001::camera::BlockCameraInertia::new(camera.eye, camera.target),
            saved: false,
            building: false,
            building_angle: 0.,
            selected: None,
        },
    )
    .unwrap();
    let producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
    let cameras = producer.capture().into_records();
    assert_eq!(
        u32::from_le_bytes(cameras[0][..4].try_into().unwrap()),
        6,
        "actual process initialization preserves supported native camera kind6"
    );
    let bytes = session.save_v2(&image, cameras, 1).unwrap();
    producer.retire().1.unwrap();
    drop(session);
    let path = scratch.0.join("different-image.sav");
    fs::write(&path, bytes).unwrap();
    let audio = owner.borrow().audio.clone();
    let native_before = owner.borrow().input.cells().clone();
    let mut run = Running::start(
        inputs,
        owner.clone(),
        WorkerStart::Saved(path, Some(scratch.0.join("selected"))),
    );
    assert_eq!(
        owner.borrow().input.cells().mouse_current,
        native_before.mouse_current
    );
    assert_eq!(
        owner.borrow().input.cells().keyboard_current,
        native_before.keyboard_current
    );
    assert_eq!(owner.borrow().input.cells().input_enabled, 1);
    assert!(audio.same_owner(&owner.borrow().audio));
    assert!(!audio.music_paused());
    let (reply, probe) = mpsc::channel();
    run.send(FreshRequest::DispatchProbe(reply));
    assert_eq!(
        probe.recv_timeout(Duration::from_secs(15)).unwrap().pause,
        expected
    );
    let custody = run.finish().unwrap();
    assert_eq!(custody.pause.bytes, [1, 0, 0, 0, 0, 0xa5, 0x5a, 0xee, 1]);
    assert_eq!(custody.tasks, rows);
    assert_eq!(
        (
            custody.frequency.frequency,
            custody.frequency.calendar_timer,
            custody.frequency.logic_timer
        ),
        (17., Some(1), Some(2))
    );
    owner.borrow_mut().return_custody(custody).unwrap();
}

#[test]
#[ignore = "requires supplied original assets; joined city switches after Options"]
fn options_quit_then_load_and_start_in_one_process_advances_clock() {
    let scratch = Scratch::new();
    let (inputs, owner) = inputs(scratch.0.join("profiles"));
    let mut save = None;
    let mut route = focus::Route::default();
    for city in 0..3 {
        let start = if city == 1 {
            WorkerStart::Saved(save.take().unwrap(), None)
        } else {
            WorkerStart::Fresh(camera_producer_live_tests::setup(&inputs))
        };
        let started = Instant::now();
        let mut run = Running::start_after_menu_delay(
            inputs.clone(),
            owner.clone(),
            start,
            route,
            Duration::from_secs(180),
        );
        let (reply, probe) = mpsc::channel();
        run.send(FreshRequest::DispatchProbe(reply));
        let probe = probe.recv_timeout(Duration::from_secs(15)).unwrap();
        assert_eq!(
            &probe.pause[..4],
            &[0; 4],
            "no holds from the previous city"
        );
        let before = run.clock();
        let loaded_tick = run.loaded.as_ref().unwrap().logic_tick.unwrap();
        let real_time_ticks = (started.elapsed().as_millis() * 25).div_ceil(1000) as u32;
        assert!(
            before.ticks - loaded_tick <= real_time_ticks + 1,
            "menu interval leaked into successor city: {loaded_tick} -> {}",
            before.ticks
        );
        std::thread::sleep(Duration::from_millis(120));
        assert!(
            run.clock().ticks > before.ticks,
            "successor city clock must advance"
        );
        if city == 0 {
            run.route.toggle_player_pause().unwrap();
            owner.borrow_mut().note_sent(run.route.sent());
            run.route.wait_prefix();
            run.route.service_one().unwrap();
            let (reply, probe) = mpsc::channel();
            run.send(FreshRequest::DispatchProbe(reply));
            assert_eq!(
                probe.recv_timeout(Duration::from_secs(15)).unwrap().pause[3],
                1
            );
            run.route.poll().unwrap();
        }
        let (reply, opened) = mpsc::channel();
        run.send(FreshRequest::Options { open: true, reply });
        opened
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
            .unwrap();
        let (reply, probe) = mpsc::channel();
        run.send(FreshRequest::DispatchProbe(reply));
        assert_eq!(
            probe.recv_timeout(Duration::from_secs(15)).unwrap().pause[0],
            1
        );
        if city == 0 {
            let (reply, saved) = mpsc::channel();
            run.send(FreshRequest::SaveSelected {
                target: crate::save_menu::Target::NewName("city-switch".into()),
                capture: Some(run.producer.as_ref().unwrap().capture()),
                camera: fresh_city_camera().0,
                game_mode: 1,
                interface_mode: 0,
                reply,
            });
            let path = saved
                .recv_timeout(Duration::from_secs(15))
                .unwrap()
                .unwrap();
            // Keep native records, which do not store pause reasons. A modern
            // record12 would replace inherited bytes and mask the leaked hold.
            let bytes = fs::read(&path).unwrap();
            let parsed = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let extension = parsed
                .sections
                .iter()
                .find(|section| section.tag == openmt2001::save::extension::TAG)
                .unwrap();
            let mut native = bytes[..extension.offset].to_vec();
            native.extend_from_slice(&u32::MAX.to_le_bytes());
            native.extend_from_slice(&parsed.terminator_padding);
            fs::write(&path, native).unwrap();
            save = Some(path);
        }
        // Quit drops this generation without the Continue/close Options path.
        let custody = run.finish().unwrap();
        assert_eq!(&custody.pause.bytes[..4], &[1, 0, 0, 0]);
        owner.borrow_mut().return_custody(custody).unwrap();
        route = std::mem::take(&mut run.route);
    }
}
#[test]
#[ignore = "private supplied inputs and owned disk scratch; partial construction has no replacement custody"]
fn joined_constructor_failure_cannot_return_defaults_or_enable_successor() {
    let scratch = Scratch::new();
    let (inputs, owner) = inputs(scratch.0.join("profiles"));
    let custody = owner.borrow_mut().take_custody(false).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut worker = camera_producer::spawn_session_joined(
        "pf-failed-constructor",
        inputs,
        WorkerStart::Saved(scratch.0.join("absent.sav"), None),
        fresh_city_camera().0,
        tx,
        None,
        true,
        Some(custody),
    )
    .unwrap();
    assert!(rx.recv_timeout(Duration::from_secs(15)).unwrap().is_err());
    let deadline = Instant::now() + Duration::from_secs(30);
    let error = loop {
        if let Some(result) = worker.poll() {
            break result.unwrap_err();
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert!(worker.take_custody().is_none());
    owner.borrow_mut().fail(error);
    assert_eq!(owner.borrow().phase, Phase::Failed);
    assert!(owner.borrow_mut().take_custody(true).is_err());
}

#[test]
#[ignore = "private supplied inputs and owned disk scratch; cancellation and full OS join"]
fn joined_cancelled_loading_returns_actual_custody_after_full_join() {
    let scratch = Scratch::new();
    let (inputs, owner) = inputs(scratch.0.join("profiles"));
    let setup = camera_producer_live_tests::setup(&inputs);
    let custody = owner.borrow_mut().take_custody(true).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut worker = camera_producer::spawn_session_joined(
        "pf-cancelled-loading",
        inputs,
        WorkerStart::Fresh(setup),
        fresh_city_camera().0,
        tx,
        None,
        true,
        Some(custody),
    )
    .unwrap();
    drop(rx);
    assert_eq!(owner.borrow().phase, Phase::Loading);
    assert!(owner.borrow_mut().take_custody(true).is_err());
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(result) = worker.poll() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let custody = worker
        .take_custody()
        .expect("same actual dispatch parts after cancellation");
    assert_eq!(custody.tasks.len(), 4);
    owner.borrow_mut().return_custody(custody).unwrap();
    assert_eq!(owner.borrow().phase, Phase::Menu);
}

#[test]
#[ignore = "private supplied original save; actual Ready/current-focus/logic and named load completion"]
fn original_save_releases_one_startup_hold_and_advances_after_ready_focus() {
    let scratch = Scratch::new();
    let (inputs, owner) = inputs(scratch.0.join("profiles"));
    let save = openmt2001::fixtures::path("SAVE_ONE");
    let started = Instant::now();
    let mut run = Running::start_after_menu_delay(
        inputs.clone(),
        owner.clone(),
        WorkerStart::Saved(save.clone(), None),
        focus::Route::default(),
        Duration::from_secs(180),
    );
    let (reply, status) = mpsc::channel();
    run.send(FreshRequest::Status {
        camera: fresh_city_camera().0.eye,
        reply,
    });
    let status = status.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(
        status.running && !status.script_paused && status.failure.is_none(),
        "native saved load kept the startup hold: {status:?}"
    );
    let (reply, probe) = mpsc::channel();
    run.send(FreshRequest::DispatchProbe(reply));
    let probe = probe.recv_timeout(Duration::from_secs(15)).unwrap();
    assert_eq!(&probe.pause[..4], &[0; 4]);
    assert_eq!(probe.pause[4], 0);
    let logic = probe
        .tasks
        .iter()
        .find(|task| u32::from_le_bytes(task[16..20].try_into().unwrap()) == 0x481680)
        .unwrap();
    assert_eq!(
        logic[0], 1,
        "actual inherited logic row remains disabled after native saved completion"
    );
    let before = run.clock();
    let saved_tick = run.loaded.as_ref().unwrap().logic_tick.unwrap();
    let real_time_ticks = (started.elapsed().as_millis() * 25).div_ceil(1000) as u32;
    assert!(
        before.ticks - saved_tick <= real_time_ticks + 1,
        "frontend interval advanced the loaded city: {saved_tick} -> {}",
        before.ticks
    );
    std::thread::sleep(Duration::from_millis(120));
    let after = run.clock();
    assert!(
        after.ticks > before.ticks,
        "Ready + current Gain must permit actual normal logic"
    );
    let custody = run.finish().unwrap();
    owner.borrow_mut().return_custody(custody).unwrap();
    // Distinct native-load case: ONE unpause, no holder/padding normalization.
    let mut custody = owner.borrow_mut().take_custody(false).unwrap();
    custody.pause.bytes = [2, 1, 0, 0xff, 0xfe, 0xa5, 0x5a, 0x92, 1];
    custody.frequency.frequency = 17.;
    custody.frequency.calendar_timer = None;
    custody.frequency.logic_timer = None;
    for task in &mut custody.tasks {
        task[1..4].copy_from_slice(&[0x12, 0x34, 0x56]);
        task[12..16].copy_from_slice(&99f32.to_le_bytes());
    }
    let (seed, _timing) = custody.prepare_start(false).unwrap();
    let (_, session) = inputs
        .saved_preview_with_selection_joined(
            &save,
            fresh_city_camera().0,
            &mut SnapshotCache::default(),
            None,
            Some(seed),
        )
        .unwrap();
    assert_eq!(
        session.world.pause.bytes,
        [1, 1, 0, 0xff, 0xfe, 0xa5, 0x5a, 0x92, 1]
    );
    assert_eq!(session.hooks.frequency.frequency, 17.);
    for task in &session.hooks.timers {
        assert_eq!(&task[1..4], &[0x12, 0x34, 0x56]);
        assert_eq!(&task[12..16], &0f32.to_le_bytes());
    }
}
