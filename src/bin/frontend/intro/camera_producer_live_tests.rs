//! Supplied-data CPU witnesses. This does not execute a window, GPU, decoder,
//! original program, or claim native numeric equality for the legacy projection.
use super::*;
use camera_producer::{Controls, Initial, Producer};
use openmt2001::camera::bank::Capture;
use std::{thread::JoinHandle, time::Duration};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = openmt2001::fixtures::scratch("TEST_SCRATCH");
        assert!(parent.is_absolute());
        fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!(
            "camera-producer-{}-{}",
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
        fs::remove_dir_all(&self.0).unwrap();
    }
}
pub(super) fn inputs(root: PathBuf) -> Arc<SceneInputs> {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let map = openmt2001::fixtures::path("MAP");
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let exe = openmt2001::fixtures::path("EXE");
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let style = crate::block_sidewalk_labels::load_style(&image, &installer).unwrap();
    let names = crate::fresh_city::character_names(&image, &language).unwrap();
    Arc::new(SceneInputs {
        process_runtime: None,
        compatibility: crate::compatibility::Mode::StrictV14,
        style,
        time_runtime: Default::default(),
        profile_root: root,
        character_names: names,
        installer,
        language,
        maps: map.parent().unwrap().to_path_buf(),
        build_file: read("OPENMT2001_BUILD_FILE"),
        executable,
        directory: read("OPENMT2001_ARCHIVE_DIR"),
        archive: read("OPENMT2001_ARCHIVE_BIN"),
        suncols: read("OPENMT2001_SUNCOLS"),
    })
}
pub(super) fn setup(inputs: &SceneInputs) -> crate::singleplayer::Setup {
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    crate::singleplayer::Setup {
        preview: crate::singleplayer::Preview::Scenario(200003),
        difficulty: crate::singleplayer::Difficulty::Easy,
        character_index: 2,
        profile_name: b"Camera producer".to_vec(),
        catalog: crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap())
            .unwrap(),
        profile_path: crate::campaign_store::profile_path(&inputs.profile_root, b"Camera producer"),
    }
}
fn fresh_controls() -> Controls {
    let (camera, controls) = fresh_city_camera();
    Controls {
        camera,
        controls,
        inertia: openmt2001::camera::BlockCameraInertia::new(camera.eye, camera.target),
        saved: false,
        building: false,
        building_angle: 0.,
        selected: None,
    }
}
fn cold_pair() -> Capture {
    let mut rows = [[0xa7; 196], [0x5b; 196]];
    for (index, kind, eye, target) in [
        (0, 4u32, [-220f32, 1050., -30.], [-300., 5., -30.]),
        (1, 12u32, [100., 120., 0.], [20., 5., 0.]),
    ] {
        rows[index][..4].copy_from_slice(&kind.to_le_bytes());
        for (at, values) in [(0x4c, eye), (0x58, target), (0x64, [0., 1., 0.])] {
            for (axis, value) in values.into_iter().enumerate() {
                rows[index][at + 4 * axis..at + 4 * axis + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    Capture::from_records(rows)
}
fn read_pair(path: &std::path::Path) -> Capture {
    let bytes = fs::read(path).unwrap();
    let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
    let [p, s] = save.cameras().unwrap();
    Capture::from_records([*p, *s])
}
struct Worker {
    sender: Option<mpsc::Sender<FreshRequest>>,
    loaded: Option<LoadedInspection>,
    thread: Option<JoinHandle<()>>,
    producer: Option<Producer>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(p) = self.producer.take() {
            let _ = p.retire();
        }
        self.sender.take();
        self.loaded.take();
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
#[test]
#[ignore = "private supplied assets/scripts, CPU actual worker, explicit disk scratch"]
fn worker_source_attachment_manual_save_warm_refresh_and_retirement() {
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let (camera, _) = fresh_city_camera();
    let source = Arc::clone(&inputs);
    let launch = setup(&inputs);
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            source.session_worker_with_sources(
                WorkerStart::Fresh(launch),
                camera,
                tx,
                None,
                false,
                true,
            )
        })
        .unwrap();
    let mut loaded = rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    let sender = loaded.fresh_session.as_ref().unwrap().clone();
    let publisher = loaded
        .camera_publisher
        .take()
        .expect("production source path publishes its unique writer");
    let mut run = Worker {
        sender: Some(sender),
        loaded: Some(loaded),
        thread: Some(thread),
        producer: None,
    };
    let mut parked = Some(camera_producer::new_process_owner().unwrap());
    let initial = Initial::prepare(None, Some(0), fresh_controls()).unwrap();
    run.producer = Some(Producer::attach(&mut parked, publisher, &initial).unwrap());
    assert!(parked.is_none());
    let (reply, ready) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::UiCameraReady { reply })
        .unwrap();
    ready
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let (reply, opened) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Options { open: true, reply })
        .unwrap();
    opened
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    // Commit actual last UI pose. The intentionally poisoned request sample
    // proves SaveSelected consumes the completed source instead of its cache.
    let moved = CameraSample {
        eye: [100., 120., 0.],
        target: [20., 5., 0.],
        up: [0., 1., 0.],
        vertical_fov: 53.,
    };
    run.producer.as_mut().unwrap().commit(moved, 0).unwrap();
    let expected = run.producer.as_ref().unwrap().capture();
    let (reply, saved) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::NewName("Producer".into()),
            camera: CameraSample {
                eye: [f32::NAN; 3],
                ..camera
            },
            capture: None,
            game_mode: 1,
            interface_mode: 3,
            reply,
        })
        .unwrap();
    let path = saved
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    assert_eq!(read_pair(&path), expected);
    let (reply, refreshed) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Inspect {
            camera: moved,
            reply,
        })
        .unwrap();
    let snapshot = refreshed
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    assert!(
        snapshot.camera_publisher.is_none(),
        "warm snapshot cannot allocate another producer"
    );
    drop(snapshot);
    let mut next = None;
    camera_producer::transfer(&mut run.producer, &mut next).unwrap();
    assert!(run.producer.is_none());
    assert_eq!(next.as_ref().unwrap().capture(), expected);
    run.producer = next;
    let (owner, result) = run.producer.take().unwrap().retire();
    result.unwrap();
    parked = Some(owner);
    assert_eq!(parked.as_ref().unwrap().capture(), expected);
    let prior = fs::read(&path).unwrap();
    let (reply, rejected) = mpsc::channel();
    let _ = run
        .sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::ExistingSlot("Producer.sav".into()),
            camera: moved,
            capture: None,
            game_mode: 1,
            interface_mode: 0,
            reply,
        });
    assert!(
        matches!(
            rejected.recv_timeout(Duration::from_secs(15)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ),
        "UI World retirement closes old admissions, not merely its camera source"
    );
    assert_eq!(fs::read(path).unwrap(), prior);
    drop(run);
}
#[test]
#[ignore = "private supplied assets/scripts, CPU actual worker, explicit disk scratch"]
fn worker_rejects_ready_without_completed_publication_before_any_following_request() {
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let (camera, _) = fresh_city_camera();
    let launch = setup(&inputs);
    let (tx, rx) = mpsc::channel();
    let source = Arc::clone(&inputs);
    let thread = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            source.session_worker_with_sources(
                WorkerStart::Fresh(launch),
                camera,
                tx,
                None,
                false,
                true,
            )
        })
        .unwrap();
    let loaded = rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    let sender = loaded.fresh_session.as_ref().unwrap().clone();
    let mut run = Worker {
        sender: Some(sender),
        loaded: Some(loaded),
        thread: Some(thread),
        producer: None,
    };
    let (reply, ready) = mpsc::channel();
    let (after, blocked) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::UiCameraReady { reply })
        .unwrap();
    let _ = run
        .sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::LogicTick(after));
    assert!(
        ready
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
            .is_err()
    );
    assert!(matches!(
        blocked.recv_timeout(Duration::from_secs(15)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    run.loaded.as_mut().unwrap().camera_publisher.take();
    drop(run);
}
#[test]
#[ignore = "private supplied assets/scripts; controlled real Session midnight, CPU, explicit disk scratch"]
fn completed_cold_return_producer_feeds_real_manual_and_autosave_and_new_world_epoch() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let scratch = Scratch::new();
            let inputs = inputs(scratch.0.join("profiles"));
            let initial = fresh_controls();
            let (loaded, mut session) = inputs
                .fresh_preview(
                    setup(&inputs),
                    initial.camera,
                    &mut SnapshotCache::default(),
                )
                .unwrap();
            drop(loaded);
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let board = inputs.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let epoch = session.world.topology.epoch();
            let (publisher, reader) = crate::save_menu::camera_source::channel(epoch);
            let mut parked = Some(camera_producer::new_process_owner().unwrap());
            let cold = cold_pair();
            let setup = Initial::prepare(Some(cold), Some(3), initial).unwrap();
            let mut producer = Producer::attach(&mut parked, publisher, &setup).unwrap();
            let strategic = reader.read(epoch).unwrap();
            assert_eq!(strategic.capture(), cold);
            assert_eq!(strategic.interface_mode(), 3);
            let next = producer
                .navigate(setup.active, None, setup.active.camera)
                .unwrap();
            assert!(!next.strategic);
            let mut returned = next.active;
            producer.commit(returned.camera, 0).unwrap();
            let expected = Capture::from_records([cold.records()[1]; 2]);
            assert_eq!(producer.capture(), expected);
            let pose = mode12_input_tick_with_height(
                &mut returned.controls,
                true,
                [false; 4],
                [0; 2],
                50,
                false,
                false,
            )
            .unwrap();
            returned.inertia.tick(pose.0, pose.1);
            let (eye, target) = returned.inertia.sample(0.5).unwrap();
            assert_eq!((eye, target), ([100., 120., 0.], [20., 5., 0.]));
            producer
                .commit(
                    CameraSample {
                        eye,
                        target,
                        ..returned.camera
                    },
                    0,
                )
                .unwrap();
            assert_eq!(reader.read(epoch).unwrap().capture(), expected);
            let mut host = crate::live_host::Host::new(eye);
            host.install_ui_camera_source(&session, reader.clone())
                .unwrap();
            host.set_running(true);
            host.policy.hourly.suppressed = true;
            let root = crate::save_menu::selected_profile_root(&scratch.0, b"Selected");
            host.autosave_profile_root = Some(root.clone());
            session.world.clock.calendar = [23, 0, 0, 23, 0];
            session.world.clock.timing = [1000, 624, 624, 14999, 0, 1];
            session.world.settings.tweaks[4..8].copy_from_slice(&1u32.to_le_bytes());
            assert!(
                !session.world.pause.has_reason(),
                "fixture may not silently clear original script holds"
            );
            host.tick(&mut session, &assets);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            assert_eq!(session.world.clock.calendar, [0, 1, 0, 24, 1]);
            let automatic = match &host.autosave_status {
                Some(crate::save_menu::AutosaveStatus::Saved(path)) => path.clone(),
                other => panic!("{other:?}"),
            };
            assert_eq!(read_pair(&automatic), expected);
            host.options(&mut session.world.pause, &mut session.hooks.timers, true)
                .unwrap();
            let pause = session.world.pause.clone();
            let timers = session.hooks.timers.clone();
            let manual = host
                .save_selected_from_ui_snapshot(
                    &mut session,
                    &image,
                    &root,
                    &crate::save_menu::Target::NewName("Returned".into()),
                    1,
                )
                .unwrap();
            assert_eq!(read_pair(&manual), expected);
            assert_eq!(session.world.pause, pause);
            assert_eq!(session.hooks.timers, timers);
            let previous = fs::read(&automatic).unwrap();
            let (owner, result) = producer.retire();
            result.unwrap();
            assert!(reader.read(epoch).is_err());
            parked = Some(owner);
            assert!(
                host.save_selected_from_ui_snapshot(
                    &mut session,
                    &image,
                    &root,
                    &crate::save_menu::Target::ExistingSlot("Returned.sav".into()),
                    1
                )
                .is_err()
            );
            host.options(&mut session.world.pause, &mut session.hooks.timers, false)
                .unwrap();
            session.world.clock.calendar = [23, 0, 0, 23, 0];
            session.world.clock.timing = [1000, 624, 624, 14999, 0, 1];
            host.tick(&mut session, &assets);
            assert!(matches!(
                host.autosave_status,
                Some(crate::save_menu::AutosaveStatus::Failed {
                    error: crate::save_menu::AutosaveFailure::UnsupportedState(_),
                    ..
                })
            ));
            assert_eq!(fs::read(automatic).unwrap(), previous);
            let (next_loaded, next_session) = inputs
                .fresh_preview(
                    setup_for_next(&inputs),
                    initial.camera,
                    &mut SnapshotCache::default(),
                )
                .unwrap();
            drop(next_loaded);
            let next_epoch = next_session.world.topology.epoch();
            assert_ne!(next_epoch.serial(), epoch.serial());
            assert!(reader.read(next_epoch).is_err());
            let (publisher, next_reader) = crate::save_menu::camera_source::channel(next_epoch);
            let fresh = Initial::prepare(None, Some(0), initial).unwrap();
            let next = Producer::attach(&mut parked, publisher, &fresh).unwrap();
            assert_eq!(
                next.capture().records()[1],
                expected.records()[1],
                "new World preserves process return history"
            );
            assert_eq!(next_reader.read(next_epoch).unwrap().interface_mode(), 0);
            assert!(
                host.install_ui_camera_source(&session, next_reader)
                    .is_err(),
                "old host cannot adopt new World source"
            );
            let _ = next.retire();
        })
        .unwrap()
        .join()
        .unwrap();
}
fn setup_for_next(inputs: &SceneInputs) -> crate::singleplayer::Setup {
    setup(inputs)
}

fn selection_assets(inputs: &SceneInputs) -> crate::selection::Assets {
    let index =
        openmt2001::archive::ArchiveIndex::parse(&inputs.directory, inputs.archive.len() as u64)
            .unwrap();
    crate::selection::Assets::load(&inputs.executable, &index, &inputs.archive).unwrap()
}
fn linked_hit(
    loaded: &LoadedInspection,
    assets: &crate::selection::Assets,
) -> crate::selection::Hit {
    let (index, b) = loaded
        .scene
        .inspection_buildings
        .iter()
        .enumerate()
        .find(|(_, b)| b.business().is_some() || b.initialized_apartment.is_some())
        .expect("real linked building fixture");
    inspection_hit(&assets.blocks, b.category, Some(index)).unwrap()
}
/// Obtain the real transport payload; Session borrow ends before the wait.
fn admitted_payload(
    session: &crate::fresh_city::Session<'_>,
    turn: camera_producer::Turn,
) -> (
    camera_producer::Payload,
    JoinHandle<Result<Vec<camera_producer::Post>, String>>,
) {
    let admitted = camera_producer::admit(session, turn);
    let (reply, answer) = mpsc::channel();
    let waiter = std::thread::spawn(move || camera_producer::deliver(turn, admitted, reply));
    (
        answer
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
            .unwrap(),
        waiter,
    )
}
fn complete_local(
    session: &crate::fresh_city::Session<'_>,
    producer: &mut Producer,
    state: &mut camera_producer::View,
    event: camera_producer::Event,
    assets: &crate::selection::Assets,
) -> Result<(), String> {
    let turn = camera_producer::Turn {
        epoch: session.world.topology.epoch(),
        building: None,
    };
    let (payload, waiter) = admitted_payload(session, turn);
    let mut ui_committed = false;
    let result =
        camera_producer::complete_event(producer, state, turn, event, payload, assets, |_| {
            ui_committed = true
        });
    let receipt = waiter.join().unwrap();
    assert_eq!(receipt.is_ok(), result.is_ok());
    assert_eq!(ui_committed, result.is_ok());
    result
}
#[test]
#[ignore = "supplied original data, CPU shared UI events/actual Session, explicit scratch"]
fn shared_ui_scene_locator_adjustment_hyperlink_tutorial_and_navigator_keep_saves_and_compass() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use camera_producer::{ActorOrigin, BlockOrigin, Event, Resolved, Turn, View};
            let scratch = Scratch::new();
            let inputs = inputs(scratch.0.join("profiles"));
            let fresh = fresh_controls();
            let (mut loaded, mut session) = inputs
                .fresh_preview(setup(&inputs), fresh.camera, &mut SnapshotCache::default())
                .unwrap();
            let assets = selection_assets(&inputs);
            let hit = linked_hit(&loaded, &assets);
            // Poison every former Scene numeric operand. The actual UI identity
            // helper still produces a valid key; admitted live words remain finite.
            let display = &mut loaded.scene.inspection_buildings[hit.building.unwrap()];
            display.translation = [f32::NAN; 3];
            display.camera_entrance = Some([f32::NAN; 3]);
            display.kind = u8::MAX;
            let expected = camera_producer::identity_for_hit(&loaded.scene, &assets, hit).unwrap();
            assert!(expected.business_slot.is_some());
            let epoch = session.world.topology.epoch();
            let (publisher, reader) = crate::save_menu::camera_source::channel(epoch);
            let initial = Initial::from_loaded(&loaded, fresh).unwrap();
            let mut parked = Some(camera_producer::new_process_owner().unwrap());
            let mut producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
            let mut state = View {
                active: initial.active,
                strategic: initial.strategic(),
                normal_return: initial.normal_return,
            };
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let mut host = crate::live_host::Host::new(fresh.camera.eye);
            host.install_ui_camera_source(&session, reader.clone())
                .unwrap();
            host.options(&mut session.world.pause, &mut session.hooks.timers, true)
                .unwrap();
            let root = crate::save_menu::selected_profile_root(&scratch.0, b"UI paths");
            for (index, origin) in [
                ActorOrigin::Scene,
                ActorOrigin::Locator,
                ActorOrigin::Adjustment,
                ActorOrigin::Hyperlink,
                ActorOrigin::Tutorial,
            ]
            .into_iter()
            .enumerate()
            {
                let before = producer.capture();
                let turn = Turn {
                    epoch,
                    building: Some(expected),
                };
                let (payload, waiter) = admitted_payload(&session, turn);
                let live = match payload.input {
                    Resolved::Actor(value) => value,
                    _ => panic!("linked actor did not resolve"),
                };
                assert_eq!(
                    (live.business_slot, live.building_slot, live.category_id),
                    (
                        expected.business_slot.unwrap(),
                        expected.building_slot,
                        expected.category_id
                    )
                );
                let event = Event::Actor {
                    selected: hit,
                    auto_rotate: true,
                    origin,
                };
                let mut ui_commit = false;
                camera_producer::complete_event(
                    &mut producer,
                    &mut state,
                    turn,
                    event,
                    payload,
                    &assets,
                    |next| {
                        assert!(next.active.building);
                        assert!(!next.strategic);
                        ui_commit = true;
                    },
                )
                .unwrap();
                assert!(ui_commit);
                waiter.join().unwrap().unwrap();
                assert_eq!(producer.kind(), 13);
                assert!(producer.error().is_none());
                assert_eq!(producer.capture().records()[1], before.records()[1]);
                assert!(state.active.controls.center.into_iter().all(f32::is_finite));
                assert_eq!(reader.read(epoch).unwrap().interface_mode(), 0);
                let saved = host
                    .save_selected_from_ui_snapshot(
                        &mut session,
                        &image,
                        &root,
                        &crate::save_menu::Target::NewName(format!("Actor{index}")),
                        1,
                    )
                    .unwrap();
                assert_eq!(read_pair(&saved), producer.capture());
                let strategic = CameraSample {
                    eye: [-220., 1050., -30.1],
                    target: [-220., 0., -30.],
                    up: [0., 1., 0.],
                    vertical_fov: 45.,
                };
                complete_local(
                    &session,
                    &mut producer,
                    &mut state,
                    Event::Compass { strategic },
                    &assets,
                )
                .unwrap();
                assert_eq!(producer.kind(), 4);
                assert!(state.strategic);
                let saved = host
                    .save_selected_from_ui_snapshot(
                        &mut session,
                        &image,
                        &root,
                        &crate::save_menu::Target::NewName(format!("Strategic{index}")),
                        1,
                    )
                    .unwrap();
                assert_eq!(read_pair(&saved), producer.capture());
                let before = producer.capture();
                let yaw = before.records()[0][0x7c..0x80].to_vec();
                complete_local(
                    &session,
                    &mut producer,
                    &mut state,
                    Event::Block {
                        category: expected.category_id,
                        selected: crate::selection::Hit {
                            category: hit.category,
                            building: None,
                        },
                        origin: BlockOrigin::Navigator,
                    },
                    &assets,
                )
                .unwrap();
                assert_eq!(producer.kind(), 12);
                assert!(!state.strategic);
                assert_eq!(state.active.selected.unwrap().building, None);
                assert_eq!(&producer.capture().records()[0][0x7c..0x80], yaw);
                assert_eq!(producer.capture().records()[1], before.records()[1]);
                let pose = mode12_input_tick_with_height(
                    &mut state.active.controls,
                    state.active.saved,
                    [false; 4],
                    [0; 2],
                    50,
                    false,
                    false,
                )
                .unwrap();
                state.active.inertia.tick(pose.0, pose.1);
                let (eye, target) = state.active.inertia.sample(0.5).unwrap();
                state.active.camera.eye = eye;
                state.active.camera.target = target;
                producer.commit(state.active.camera, 0).unwrap();
                assert!(producer.error().is_none());
                let saved = host
                    .save_selected_from_ui_snapshot(
                        &mut session,
                        &image,
                        &root,
                        &crate::save_menu::Target::NewName(format!("Navigator{index}")),
                        1,
                    )
                    .unwrap();
                assert_eq!(read_pair(&saved), producer.capture());
            }
            let _ = producer.retire();
        })
        .unwrap()
        .join()
        .unwrap();
}

fn start_source_worker(
    inputs: Arc<SceneInputs>,
    parked: &mut Option<camera_live::owner::Owner>,
    require_focus: bool,
) -> Worker {
    let launch = setup(&inputs);
    let (camera, _) = fresh_city_camera();
    let (tx, rx) = mpsc::channel();
    let source = Arc::clone(&inputs);
    let thread = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            source.session_worker_with_sources(
                WorkerStart::Fresh(launch),
                camera,
                tx,
                None,
                require_focus,
                true,
            )
        })
        .unwrap();
    let mut loaded = rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    let initial = Initial::from_loaded(&loaded, fresh_controls()).unwrap();
    let producer =
        Producer::attach(parked, loaded.camera_publisher.take().unwrap(), &initial).unwrap();
    let sender = loaded.fresh_session.as_ref().unwrap().clone();
    let run = Worker {
        sender: Some(sender),
        loaded: Some(loaded),
        thread: Some(thread),
        producer: Some(producer),
    };
    let (reply, ready) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::UiCameraReady { reply })
        .unwrap();
    ready
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    run
}
fn worker_state(run: &Worker) -> camera_producer::View {
    let fresh = fresh_controls();
    camera_producer::View {
        active: fresh,
        strategic: false,
        normal_return: None,
    }
}
struct OrderedEffects {
    active: Option<focus::AttachmentId>,
    committed: std::rc::Rc<std::cell::Cell<bool>>,
    seen: std::rc::Rc<std::cell::RefCell<Vec<bool>>>,
}
impl focus::Effects for OrderedEffects {
    fn activate(&mut self, id: focus::AttachmentId) -> Result<(), String> {
        assert!(self.active.is_none());
        self.active = Some(id);
        Ok(())
    }
    fn retire(&mut self, id: focus::AttachmentId) -> Result<(), String> {
        assert_eq!(self.active, Some(id));
        self.active = None;
        Ok(())
    }
    fn before_pause(
        &mut self,
        id: focus::AttachmentId,
        focused: bool,
        _: i8,
    ) -> Result<focus::Prefix, String> {
        assert_eq!(self.active, Some(id));
        if !focused {
            assert!(
                self.committed.get(),
                "later focus cannot overtake selected operation"
            );
        }
        self.seen.borrow_mut().push(focused);
        Ok(focus::Prefix::Complete(Some(
            openmt2001::native_input::focus::AudioOutcome::Applied,
        )))
    }
}
#[test]
#[ignore = "supplied data, actual CPU worker selection/Ready/focus/frame/save FIFO, explicit scratch"]
fn actual_worker_pending_selection_precedes_focus_due_frame_and_mode13_save() {
    use camera_producer::{ActorOrigin, Event, Turn};
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let assets = selection_assets(&inputs);
    let mut parked = Some(camera_producer::new_process_owner().unwrap());
    let mut run = start_source_worker(Arc::clone(&inputs), &mut parked, true);
    let hit = linked_hit(run.loaded.as_ref().unwrap(), &assets);
    let key = camera_producer::identity_for_hit(&run.loaded.as_ref().unwrap().scene, &assets, hit)
        .unwrap();
    let epoch = run.loaded.as_ref().unwrap().camera_epoch.unwrap();
    let committed = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut route = focus::Route::default();
    route
        .install(Box::new(OrderedEffects {
            active: None,
            committed: std::rc::Rc::clone(&committed),
            seen: std::rc::Rc::clone(&seen),
        }))
        .unwrap();
    route.observe(true).unwrap();
    route.attach(run.sender.as_ref().unwrap().clone()).unwrap();
    route.wait_prefix();
    route.service_one().unwrap();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Run(false))
        .unwrap();
    let (reply, clock) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::LogicTick(reply))
        .unwrap();
    let baseline = clock.recv_timeout(Duration::from_secs(15)).unwrap();
    let mut queue = std::collections::VecDeque::new();
    let turn = Turn {
        epoch,
        building: Some(key),
    };
    let event = Event::Actor {
        selected: hit,
        auto_rotate: true,
        origin: ActorOrigin::Scene,
    };
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Run(true))
        .unwrap();
    camera_producer::enqueue(
        &mut queue,
        run.sender.as_ref().unwrap(),
        turn,
        event,
        None,
        false,
    )
    .unwrap();
    let pending = queue.pop_front().unwrap();
    let payload = pending
        .reply
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    route.observe(false).unwrap();
    let (reply, opened) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Options { open: true, reply })
        .unwrap();
    let (reply, saved) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::NewName("ActorFIFO".into()),
            camera: CameraSample {
                eye: [f32::NAN; 3],
                ..fresh_controls().camera
            },
            capture: None,
            game_mode: 1,
            interface_mode: 3,
            reply,
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(45));
    assert!(
        !route.service_one().unwrap(),
        "focus not admitted while selected turn awaits UI"
    );
    assert!(matches!(opened.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(matches!(saved.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let mut state = worker_state(&run);
    camera_producer::complete_event(
        run.producer.as_mut().unwrap(),
        &mut state,
        turn,
        event,
        payload,
        &assets,
        |view| {
            assert!(view.active.building);
            committed.set(true);
        },
    )
    .unwrap();
    route.wait_prefix();
    route.service_one().unwrap();
    opened
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let path = saved
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    assert_eq!(read_pair(&path), run.producer.as_ref().unwrap().capture());
    assert_eq!(run.producer.as_ref().unwrap().kind(), 13);
    let bytes = fs::read(path).unwrap();
    assert_eq!(
        openmt2001::save::SaveFile::parse(&bytes)
            .unwrap()
            .clock()
            .unwrap()
            .timing[0],
        baseline.ticks,
        "received selected turn and later loss precede already-due logic frame/save"
    );
    assert_eq!(&*seen.borrow(), &[true, false]);
    route.detach();
    drop(run);
}
#[test]
#[ignore = "supplied data, actual CPU worker stale/error/cancel/retire/new World protocol, explicit scratch"]
fn actual_worker_selection_stale_preflight_cancel_retire_and_new_world_preserve_source() {
    use camera_producer::{ActorOrigin, Event, Turn};
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let assets = selection_assets(&inputs);
    let mut parked = Some(camera_producer::new_process_owner().unwrap());
    let mut run = start_source_worker(Arc::clone(&inputs), &mut parked, false);
    let hit = linked_hit(run.loaded.as_ref().unwrap(), &assets);
    let key = camera_producer::identity_for_hit(&run.loaded.as_ref().unwrap().scene, &assets, hit)
        .unwrap();
    let epoch = run.loaded.as_ref().unwrap().camera_epoch.unwrap();
    let (reply, opened) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Options { open: true, reply })
        .unwrap();
    opened
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let prior = run.producer.as_ref().unwrap().capture();
    let event = Event::Actor {
        selected: hit,
        auto_rotate: true,
        origin: ActorOrigin::Locator,
    };
    let mut queue = std::collections::VecDeque::new();
    let mut stale = key;
    stale.category_id = stale.category_id.wrapping_add(1);
    camera_producer::enqueue(
        &mut queue,
        run.sender.as_ref().unwrap(),
        Turn {
            epoch,
            building: Some(stale),
        },
        event,
        None,
        false,
    )
    .unwrap();
    let pending = queue.pop_front().unwrap();
    assert!(
        pending
            .reply
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
            .is_err()
    );
    assert_eq!(run.producer.as_ref().unwrap().capture(), prior);
    assert!(run.producer.as_ref().unwrap().error().is_none());
    let turn = Turn {
        epoch,
        building: Some(key),
    };
    camera_producer::enqueue(
        &mut queue,
        run.sender.as_ref().unwrap(),
        turn,
        event,
        None,
        false,
    )
    .unwrap();
    let pending = queue.pop_front().unwrap();
    let payload = pending
        .reply
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let bad = Event::Actor {
        selected: crate::selection::Hit {
            category: usize::MAX,
            building: hit.building,
        },
        auto_rotate: true,
        origin: ActorOrigin::Locator,
    };
    let mut state = worker_state(&run);
    assert!(
        camera_producer::complete_event(
            run.producer.as_mut().unwrap(),
            &mut state,
            turn,
            bad,
            payload,
            &assets,
            |_| panic!("failed UI preflight must not commit")
        )
        .is_err()
    );
    assert_eq!(run.producer.as_ref().unwrap().capture(), prior);
    assert!(run.producer.as_ref().unwrap().error().is_none());
    // Same cancellation function used before actual refresh Inspect. An already
    // admitted payload is dropped through its Receiver; receipt wakes once.
    camera_producer::enqueue(
        &mut queue,
        run.sender.as_ref().unwrap(),
        turn,
        event,
        None,
        false,
    )
    .unwrap();
    let pending = queue.front().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    // Wait for admission via the real message, then put the payload back in an
    // owned receiver so the production cancellation drop is the terminal edge.
    let payload = pending
        .reply
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap()
        .unwrap();
    let (tx, rx) = mpsc::channel();
    tx.send(Ok(payload)).unwrap();
    drop(tx);
    queue.front_mut().unwrap().reply = rx;
    camera_producer::cancel_pending(&mut queue);
    assert!(queue.is_empty());
    let (reply, refreshed) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::Inspect {
            camera: state.active.camera,
            reply,
        })
        .unwrap();
    drop(
        refreshed
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap(),
    );
    let (reply, saved) = mpsc::channel();
    run.sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::NewName("Retained".into()),
            camera: state.active.camera,
            capture: None,
            game_mode: 1,
            interface_mode: 0,
            reply,
        })
        .unwrap();
    let path = saved
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    assert_eq!(read_pair(&path), prior);
    let before = fs::read(&path).unwrap();
    camera_producer::enqueue(
        &mut queue,
        run.sender.as_ref().unwrap(),
        turn,
        event,
        None,
        false,
    )
    .unwrap();
    let pending = queue.front().unwrap();
    let payload = pending
        .reply
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let (tx, rx) = mpsc::channel();
    tx.send(Ok(payload)).unwrap();
    drop(tx);
    queue.front_mut().unwrap().reply = rx;
    let (owner, result) = run.producer.take().unwrap().retire();
    result.unwrap();
    parked = Some(owner);
    camera_producer::cancel_pending(&mut queue);
    let (reply, rejected) = mpsc::channel();
    let _ = run
        .sender
        .as_ref()
        .unwrap()
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::ExistingSlot("Retained.sav".into()),
            camera: state.active.camera,
            capture: None,
            game_mode: 1,
            interface_mode: 0,
            reply,
        });
    assert!(matches!(
        rejected.recv_timeout(Duration::from_secs(15)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert_eq!(fs::read(path).unwrap(), before);
    drop(run);
    let mut next = start_source_worker(Arc::clone(&inputs), &mut parked, false);
    let next_epoch = next.loaded.as_ref().unwrap().camera_epoch.unwrap();
    assert_ne!(next_epoch, epoch);
    camera_producer::enqueue(
        &mut queue,
        next.sender.as_ref().unwrap(),
        turn,
        event,
        None,
        false,
    )
    .unwrap();
    let pending = queue.pop_front().unwrap();
    assert!(
        pending
            .reply
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
            .is_err()
    );
    assert!(next.producer.as_ref().unwrap().error().is_none());
    drop(next);
}

#[test]
#[ignore = "supplied data, actual saved-file loader/adoption + common UI events + manual saves, CPU"]
fn actual_cold_adoption_absence_explicit_city_view_raw4_and_mismatch_first_return_next_tick_save() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use camera_producer::{Attachment, Event, View};
            let scratch = Scratch::new();
            let inputs = inputs(scratch.0.join("profiles"));
            let fresh = fresh_controls();
            let assets = selection_assets(&inputs);
            let (loaded, mut original) = inputs
                .fresh_preview(setup(&inputs), fresh.camera, &mut SnapshotCache::default())
                .unwrap();
            drop(loaded);
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let pair = cold_pair();
            let modern = original
                .save_v2_with_auction_view(&image, pair.into_records(), 1, None, 3)
                .unwrap();
            let parsed = openmt2001::save::SaveFile::parse(&modern).unwrap();
            let at = parsed
                .sections
                .iter()
                .find(|s| s.tag == openmt2001::save::extension::TAG)
                .unwrap()
                .offset;
            let mut legacy = modern[..at].to_vec();
            legacy.extend_from_slice(&u32::MAX.to_le_bytes());
            legacy.extend_from_slice(&parsed.terminator_padding);
            assert!(
                openmt2001::save::extension::Extension::parse(
                    &openmt2001::save::SaveFile::parse(&legacy).unwrap()
                )
                .unwrap()
                .is_none()
            );
            let city = original
                .save_v2_with_auction_view(&image, pair.into_records(), 1, None, 0)
                .unwrap();
            let root = crate::save_menu::selected_profile_root(&scratch.0, b"Cold provenance");
            for (index, bytes, metadata, attachment, fov) in [
                (0, legacy, None, Attachment::LegacyRawKind4, 45.),
                (1, city, Some(0), Attachment::Explicit(0), 53.),
                (2, modern, Some(3), Attachment::Explicit(3), 45.),
            ] {
                let path = scratch.0.join(format!("cold{index}.sav"));
                fs::write(&path, bytes).unwrap();
                let (loaded, mut restored) = inputs
                    .saved_preview(&path, fresh.camera, &mut SnapshotCache::default())
                    .unwrap();
                assert_eq!(loaded.saved_prior_mode, metadata);
                // EXACT helper called by actual Intro adoption; no unwrap_or(0).
                let initial = Initial::from_loaded(&loaded, fresh).unwrap();
                assert_eq!(initial.provenance, attachment);
                assert_eq!(initial.metadata, metadata);
                assert_eq!(initial.active.camera.vertical_fov, fov);
                let epoch = restored.world.topology.epoch();
                let (publisher, reader) = crate::save_menu::camera_source::channel(epoch);
                let mut parked = Some(camera_producer::new_process_owner().unwrap());
                let mut producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
                assert_eq!(producer.capture(), pair);
                assert_eq!(producer.attachment(), attachment);
                assert_eq!(producer.input_metadata(), metadata);
                let mut view = View {
                    active: initial.active,
                    strategic: initial.strategic(),
                    normal_return: initial.normal_return,
                };
                assert!(
                    !producer.legacy_callback_enabled(view.strategic),
                    "raw4 never becomes a mode12 callback merely because UI mode is0"
                );
                let mut host = crate::live_host::Host::new(initial.active.camera.eye);
                host.install_ui_camera_source(&restored, reader.clone())
                    .unwrap();
                host.options(&mut restored.world.pause, &mut restored.hooks.timers, true)
                    .unwrap();
                let active = host
                    .save_selected_from_ui_snapshot(
                        &mut restored,
                        &image,
                        &root,
                        &crate::save_menu::Target::NewName(format!("Active{index}")),
                        1,
                    )
                    .unwrap();
                assert_eq!(read_pair(&active), pair);
                let (active_loaded, active_session) = inputs
                    .saved_preview(&active, fresh.camera, &mut SnapshotCache::default())
                    .unwrap();
                assert_eq!(
                    active_loaded.saved_prior_mode,
                    Some(if metadata == Some(0) { 0 } else { 3 })
                );
                drop(active_loaded);
                drop(active_session);
                complete_local(
                    &restored,
                    &mut producer,
                    &mut view,
                    Event::Compass {
                        strategic: initial.active.camera,
                    },
                    &assets,
                )
                .unwrap();
                assert!(!view.strategic);
                assert_eq!(producer.capture().into_records(), [pair.records()[1]; 2]);
                assert!(producer.legacy_callback_enabled(false));
                let pose = mode12_input_tick_with_height(
                    &mut view.active.controls,
                    view.active.saved,
                    [false; 4],
                    [0; 2],
                    50,
                    false,
                    false,
                )
                .unwrap();
                assert_eq!(pose, ([100., 120., 0.], [20., 5., 0.]));
                view.active.inertia.tick(pose.0, pose.1);
                let (eye, target) = view.active.inertia.sample(0.5).unwrap();
                assert_eq!((eye, target), ([100., 120., 0.], [20., 5., 0.]));
                view.active.camera.eye = eye;
                view.active.camera.target = target;
                producer.commit(view.active.camera, 0).unwrap();
                let returned = host
                    .save_selected_from_ui_snapshot(
                        &mut restored,
                        &image,
                        &root,
                        &crate::save_menu::Target::NewName(format!("Returned{index}")),
                        1,
                    )
                    .unwrap();
                assert_eq!(read_pair(&returned).into_records(), [pair.records()[1]; 2]);
                assert_eq!(producer.input_metadata(), metadata);
                let _ = producer.retire();
            }
            let normal = Capture::from_records([pair.records()[1]; 2]);
            let bad = original
                .save_v2_with_auction_view(&image, normal.into_records(), 1, None, 3)
                .unwrap();
            let path = scratch.0.join("mismatch.sav");
            fs::write(&path, bad).unwrap();
            let (loaded, restored) = inputs
                .saved_preview(&path, fresh.camera, &mut SnapshotCache::default())
                .unwrap();
            assert_eq!(loaded.saved_prior_mode, Some(3));
            assert!(Initial::from_loaded(&loaded, fresh).is_err());
            assert_eq!(loaded.imported_cameras, Some(normal));
            drop(restored);
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
#[ignore = "supplied data, actual common choose-view/tutorial/navigator and source consumers, CPU"]
fn shared_plan_standalone_city_view_save_compass_tutorial2_order_and_unknown_return_are_recoverable()
 {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use camera_producer::{BlockOrigin, Event, View};
            let scratch = Scratch::new();
            let inputs = inputs(scratch.0.join("profiles"));
            let fresh = fresh_controls();
            let assets = selection_assets(&inputs);
            let (loaded, mut session) = inputs
                .fresh_preview(setup(&inputs), fresh.camera, &mut SnapshotCache::default())
                .unwrap();
            let hit = linked_hit(&loaded, &assets);
            let category = assets.blocks[hit.category].id as u32;
            let cold = cold_pair();
            let initial = Initial::prepare(Some(cold), Some(0), fresh).unwrap();
            let epoch = session.world.topology.epoch();
            let (publisher, reader) = crate::save_menu::camera_source::channel(epoch);
            let mut parked = Some(camera_producer::new_process_owner().unwrap());
            let mut producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
            let mut state = View {
                active: initial.active,
                strategic: false,
                normal_return: initial.normal_return,
            };
            let mut host = crate::live_host::Host::new(state.active.camera.eye);
            host.install_ui_camera_source(&session, reader.clone())
                .unwrap();
            host.options(&mut session.world.pause, &mut session.hooks.timers, true)
                .unwrap();
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let root = crate::save_menu::selected_profile_root(&scratch.0, b"Choose view");
            let strategic = CameraSample {
                eye: [-220., 1050., -30.1],
                target: [-220., 0., -30.],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            };
            let secondary = producer.capture().records()[1];
            complete_local(
                &session,
                &mut producer,
                &mut state,
                Event::ChooseView { mode: 3, strategic },
                &assets,
            )
            .unwrap();
            assert!(state.strategic);
            assert_eq!(producer.kind(), 4);
            assert_eq!(
                producer.capture().records()[1],
                secondary,
                "PlanView never copies primary to secondary"
            );
            let before = producer.capture();
            complete_local(
                &session,
                &mut producer,
                &mut state,
                Event::ChooseView { mode: 0, strategic },
                &assets,
            )
            .unwrap();
            assert!(!state.strategic);
            assert_eq!(producer.kind(), 4);
            assert_eq!(
                producer.capture(),
                before,
                "CityView0 has no raw setter/copy"
            );
            assert_eq!(reader.read(epoch).unwrap().interface_mode(), 0);
            let saved = host
                .save_selected_from_ui_snapshot(
                    &mut session,
                    &image,
                    &root,
                    &crate::save_menu::Target::NewName("StandaloneCity".into()),
                    1,
                )
                .unwrap();
            assert_eq!(read_pair(&saved), before);
            complete_local(
                &session,
                &mut producer,
                &mut state,
                Event::Compass { strategic },
                &assets,
            )
            .unwrap();
            assert_eq!(producer.capture().into_records(), [secondary; 2]);
            assert!(!state.strategic);
            // TUTORIAL2 prompt2 calls CityView then SetMode12; retain each real
            // operation, including the observable savable standalone CityView cut.
            complete_local(
                &session,
                &mut producer,
                &mut state,
                Event::ChooseView { mode: 3, strategic },
                &assets,
            )
            .unwrap();
            complete_local(
                &session,
                &mut producer,
                &mut state,
                Event::ChooseView { mode: 0, strategic },
                &assets,
            )
            .unwrap();
            let prior = producer.capture();
            complete_local(
                &session,
                &mut producer,
                &mut state,
                Event::Block {
                    category,
                    selected: crate::selection::Hit {
                        category: hit.category,
                        building: None,
                    },
                    origin: BlockOrigin::Tutorial,
                },
                &assets,
            )
            .unwrap();
            assert_eq!(producer.kind(), 12);
            assert!(!state.strategic);
            assert_eq!(producer.capture().records()[1], prior.records()[1]);
            assert_eq!(
                &producer.capture().records()[0][0x7c..0x80],
                &prior.records()[0][0x7c..0x80]
            );
            let _ = producer.retire();
            host.options(&mut session.world.pause, &mut session.hooks.timers, false)
                .unwrap();
            drop(host);
            let (opaque_loaded, mut opaque_session) = inputs
                .fresh_preview(setup(&inputs), fresh.camera, &mut SnapshotCache::default())
                .unwrap();
            drop(opaque_loaded);
            let opaque_epoch = opaque_session.world.topology.epoch();
            assert_ne!(opaque_epoch, epoch);
            // Unsupported opaque S fails before mutation but P remains savable;
            // no permanent failure latch or stale-source fallback is introduced.
            let mut rows = cold.into_records();
            rows[1][..4].copy_from_slice(&0xfeedbeefu32.to_le_bytes());
            let opaque = Capture::from_records(rows);
            let initial = Initial::prepare(Some(opaque), Some(3), fresh).unwrap();
            let (publisher, reader) = crate::save_menu::camera_source::channel(opaque_epoch);
            let mut parked = Some(camera_producer::new_process_owner().unwrap());
            let mut producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
            let mut state = View {
                active: initial.active,
                strategic: true,
                normal_return: None,
            };
            assert!(
                complete_local(
                    &opaque_session,
                    &mut producer,
                    &mut state,
                    Event::Compass { strategic },
                    &assets
                )
                .is_err()
            );
            assert_eq!(producer.capture(), opaque);
            assert_eq!(reader.read(opaque_epoch).unwrap().capture(), opaque);
            assert!(producer.error().is_none());
            let mut second_host = crate::live_host::Host::new(state.active.camera.eye);
            second_host
                .install_ui_camera_source(&opaque_session, reader)
                .unwrap();
            second_host
                .options(
                    &mut opaque_session.world.pause,
                    &mut opaque_session.hooks.timers,
                    true,
                )
                .unwrap();
            let saved = second_host
                .save_selected_from_ui_snapshot(
                    &mut opaque_session,
                    &image,
                    &root,
                    &crate::save_menu::Target::NewName("OpaqueStillSavable".into()),
                    1,
                )
                .unwrap();
            assert_eq!(read_pair(&saved), opaque);
            let _ = producer.retire();
        })
        .unwrap()
        .join()
        .unwrap();
}

fn wait_terminal(worker: &mut camera_producer::WorkerExit) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(result) = worker.poll() {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "terminal worker completion must be explicit"
        );
        std::thread::yield_now();
    }
}
#[test]
#[ignore = "supplied data, controlled real tutorial Host/post-actions/shared picking + joined startup fence, CPU"]
fn shared_tutorial_posts_retirement_waits_for_joined_cleanup_before_actual_new_world_reset() {
    use camera_producer::{BlockOrigin, Event, Post, Turn, View};
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let process = crate::live_host::process_runtime::test_fixture();
    let process_worker = process.clone();
    let source = Arc::clone(&inputs);
    let failures = camera_producer::ExitFailures::default();
    let worker_failures = Arc::clone(&failures);
    let (ready, source_ready) = mpsc::channel();
    let (operations, requests) =
        mpsc::channel::<(Turn, mpsc::Sender<Result<camera_producer::Payload, String>>)>();
    let (delayed, delay_seen) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (observed, observations) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let mut launch = setup(&source);
            launch.preview = crate::singleplayer::Preview::Scenario(100001);
            let (loaded, mut session) = source
                .fresh_preview(
                    launch,
                    fresh_controls().camera,
                    &mut SnapshotCache::default(),
                )
                .unwrap();
            drop(loaded);
            let _lease = camera_producer::OfflineLease {
                process: Some(process_worker.clone()),
                failures: Arc::clone(&worker_failures),
            };
            let image = openmt2001::pe::PeImage::parse(&source.executable).unwrap();
            let board = source.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&source.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let mut host = crate::live_host::Host::new(fresh_controls().camera.eye);
            // Real tutorial startup on the zero-elapsed CPU boundary. Process media
            // remains unjoined: no fabricated media provider is installed/executed.
            host.begin_presentation(&session).unwrap();
            host.frame(&mut session, &assets, 0);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            host.process_runtime = Some(process_worker.clone());
            for prompt in [2, 3, 4] {
                session
                    .dispatch_session_event("InterfacePromptDismiss", prompt, &assets, true)
                    .unwrap();
                let _ = host.status(&session);
            }
            let status = host.status(&session);
            assert!(status.tutorial_enabled_controls.contains(&73));
            ready.send(session.world.topology.epoch()).unwrap();
            for index in 0..2 {
                let (turn, reply) = requests.recv().unwrap();
                let admitted = camera_producer::admit(&session, turn);
                let posts = camera_producer::deliver(turn, admitted, reply).unwrap();
                if index == 1 {
                    delayed.send(()).unwrap();
                    released.recv().unwrap();
                }
                camera_producer::run_posts(
                    &mut host,
                    &mut session,
                    &assets,
                    posts,
                    &worker_failures,
                )
                .unwrap();
                observed
                    .send((
                        index,
                        process_worker.picking_projection().unwrap(),
                        host.status(&session).tutorial_message,
                    ))
                    .unwrap();
            }
            // No queued requests/frames run after the retired committed prefix.
        })
        .unwrap();
    let mut lifecycle = Some(camera_producer::supervise(
        worker,
        failures,
        "camera-real-posts",
    ));
    let epoch = source_ready.recv_timeout(Duration::from_secs(30)).unwrap();
    let assets = selection_assets(&inputs);
    let raw = Capture::from_records([cold_pair().records()[1]; 2]);
    let initial = Initial::prepare(Some(raw), Some(0), fresh_controls()).unwrap();
    let (publisher, _reader) = crate::save_menu::camera_source::channel(epoch);
    let mut parked = Some(camera_producer::new_process_owner().unwrap());
    let mut producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
    let mut view = View {
        active: initial.active,
        strategic: false,
        normal_return: initial.normal_return,
    };
    let strategic = CameraSample {
        eye: [-220., 1050., -30.1],
        target: [-220., 0., -30.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let turn = Turn {
        epoch,
        building: None,
    };
    let (reply, answer) = mpsc::channel();
    operations.send((turn, reply)).unwrap();
    let payload = answer
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    camera_producer::complete_with_posts(
        &mut producer,
        &mut view,
        turn,
        Event::Compass { strategic },
        payload,
        &assets,
        vec![Post::TutorialControl(73)],
        |_| {},
    )
    .unwrap();
    let (index, picking, message) = observations.recv_timeout(Duration::from_secs(15)).unwrap();
    assert_eq!(index, 0);
    assert_eq!((picking.common, picking.block), (1, 1));
    let category = picking.block_id as u32;
    assert!(message.is_some());
    let selected = crate::selection::Hit {
        category: assets
            .blocks
            .iter()
            .position(|b| b.id as u32 == category)
            .unwrap(),
        building: None,
    };
    let (reply, answer) = mpsc::channel();
    operations.send((turn, reply)).unwrap();
    let payload = answer
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    camera_producer::complete_with_posts(
        &mut producer,
        &mut view,
        turn,
        Event::Block {
            category,
            selected,
            origin: BlockOrigin::StrategicDoubleClick,
        },
        payload,
        &assets,
        vec![Post::TutorialPick(
            crate::tutorial_two_interaction::Pick::BlockEntry {
                category_id: category,
            },
        )],
        |_| {},
    )
    .unwrap();
    delay_seen.recv_timeout(Duration::from_secs(15)).unwrap();
    let (owner, result) = producer.retire();
    result.unwrap();
    parked = Some(owner);
    drop(operations);
    // Same predicate used BEFORE all three actual menu request.take/spawns.
    let mut pending = Some(WorkerStart::Fresh(setup(&inputs)));
    assert!(!camera_producer::startup_ready(&lifecycle, &None));
    if camera_producer::startup_ready(&lifecycle, &None) {
        let _ = pending.take();
        panic!("new reset overtook admitted old post callback");
    }
    assert!(pending.is_some());
    assert!(lifecycle.as_mut().unwrap().poll().is_none());
    assert_eq!(
        process.picking_projection().unwrap().block,
        1,
        "new World reset has not run while old callback is delayed"
    );
    release.send(()).unwrap();
    let (index, picking, _) = observations.recv_timeout(Duration::from_secs(15)).unwrap();
    assert_eq!(index, 1);
    assert_eq!(
        picking.common, 0,
        "actual Picked3D continuation disabled picking"
    );
    wait_terminal(lifecycle.as_mut().unwrap()).unwrap();
    lifecycle = None;
    assert!(camera_producer::startup_ready(&lifecycle, &None));
    assert!(
        matches!(
            observations.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ),
        "no old callback can replay after terminal join"
    );
    // Actual next production worker/reset, with the same shared process Handle.
    let mut next_inputs = self::inputs(scratch.0.join("next-profiles"));
    Arc::get_mut(&mut next_inputs).unwrap().process_runtime = Some(process.clone());
    let (tx, rx) = mpsc::channel();
    let mut next = camera_producer::spawn_session(
        "camera-next-world",
        next_inputs,
        pending.take().unwrap(),
        fresh_controls().camera,
        tx,
        None,
        false,
    )
    .unwrap();
    let loaded = rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    assert_ne!(loaded.camera_epoch.unwrap(), epoch);
    let after = process.picking_projection().unwrap();
    assert_eq!(
        (
            after.common,
            after.camera,
            after.block,
            after.block_id,
            after.building,
            after.building_id
        ),
        (1, 1, 0, -1, 0, -1)
    );
    drop(loaded);
    drop(rx);
    wait_terminal(&mut next).unwrap();
    assert_eq!(
        process.picking_projection().unwrap(),
        after,
        "old picking/cleanup cannot overwrite new World after joined drain"
    );
    assert!(parked.is_some());
}

#[test]
#[ignore = "supplied data, real Lua post callback error/partial commit and joined terminal, CPU"]
fn real_post_callback_error_keeps_committed_camera_and_reports_failed_terminal_without_retry() {
    use camera_producer::{Event, Post, Turn, View};
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let failures = camera_producer::ExitFailures::default();
    let errors = Arc::clone(&failures);
    let (ready, source_ready) = mpsc::channel();
    let (request, requested) =
        mpsc::channel::<mpsc::Sender<Result<camera_producer::Payload, String>>>();
    let (observed, observation) = mpsc::channel();
    let source = Arc::clone(&inputs);
    let worker=std::thread::Builder::new().stack_size(32*1024*1024).spawn(move||{
        let mut launch=setup(&source);launch.preview=crate::singleplayer::Preview::Scenario(100001);let(loaded,mut session)=source.fresh_preview(launch,fresh_controls().camera,&mut SnapshotCache::default()).unwrap();drop(loaded);
        let image=openmt2001::pe::PeImage::parse(&source.executable).unwrap();let board=source.styled_board(&image).unwrap();let file=openmt2001::build_file::BuildFile::parse(&source.build_file).unwrap();let assets=crate::fresh_city::CommandAssets{image:&image,board:&board,file:&file};
        let mut host=crate::live_host::Host::new(fresh_controls().camera.eye);host.begin_presentation(&session).unwrap();host.frame(&mut session,&assets,0);assert!(host.failure.is_none());
        for prompt in [2,3,4]{session.dispatch_session_event("InterfacePromptDismiss",prompt,&assets,true).unwrap();let _=host.status(&session);}
        assert!(host.status(&session).tutorial_enabled_controls.contains(&73));
        // Maintained failure fixture inside this owned VM, not preserved Lua.
        session.scripts.runtime.execute("camera post error fixture",b"camera_post_count=0; function CATCHEVENT_TutorialEvent(a) camera_post_count=camera_post_count+1; error('camera post failed') end",&mut|_,_|Err("unexpected fixture registration call".into())).unwrap();
        let epoch=session.world.topology.epoch();ready.send(epoch).unwrap();let turn=Turn{epoch,building:None};let reply=requested.recv().unwrap();let admitted=camera_producer::admit(&session,turn);let posts=camera_producer::deliver(turn,admitted,reply).unwrap();
        let failed=camera_producer::run_posts(&mut host,&mut session,&assets,posts,&errors).unwrap_err();let count=session.scripts.runtime.number("camera_post_count").unwrap();observed.send((failed,count,host.failure.is_some())).unwrap();
    }).unwrap();
    let mut terminal = camera_producer::supervise(worker, failures, "camera-post-failure");
    let epoch = source_ready.recv_timeout(Duration::from_secs(30)).unwrap();
    let assets = selection_assets(&inputs);
    let raw = cold_pair();
    let initial = Initial::prepare(Some(raw), Some(0), fresh_controls()).unwrap();
    let (publisher, reader) = crate::save_menu::camera_source::channel(epoch);
    let mut parked = Some(camera_producer::new_process_owner().unwrap());
    let mut producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
    let mut view = View {
        active: initial.active,
        strategic: false,
        normal_return: initial.normal_return,
    };
    let (reply, answer) = mpsc::channel();
    request.send(reply).unwrap();
    let payload = answer
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let strategic = CameraSample {
        eye: [-220., 1050., -30.1],
        target: [-220., 0., -30.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    camera_producer::complete_with_posts(
        &mut producer,
        &mut view,
        Turn {
            epoch,
            building: None,
        },
        Event::ChooseView { mode: 3, strategic },
        payload,
        &assets,
        vec![Post::TutorialControl(73)],
        |_| {},
    )
    .unwrap();
    let committed = producer.capture();
    assert_eq!(reader.read(epoch).unwrap().capture(), committed);
    let (error, count, failed) = observation.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(error.contains("Camera committed"));
    assert_eq!(count, Some(1.));
    assert!(failed);
    assert_eq!(
        producer.capture(),
        committed,
        "callback failure cannot fictitiously roll back published camera"
    );
    let outcome = wait_terminal(&mut terminal);
    assert!(outcome.is_err());
    assert!(!camera_producer::startup_ready(&None, &outcome.err()));
    let _ = producer.retire();
}
#[test]
#[ignore = "supplied data, actual cancelled loading worker including teardown/join before replacement, CPU"]
fn cancelled_inflight_loading_retains_start_intent_until_actual_terminal_join() {
    let scratch = Scratch::new();
    let inputs = inputs(scratch.0.join("profiles"));
    let (tx, rx) = mpsc::channel();
    let mut slot = Some(
        camera_producer::spawn_session(
            "camera-cancelled-loading",
            Arc::clone(&inputs),
            WorkerStart::Fresh(setup(&inputs)),
            fresh_controls().camera,
            tx,
            None,
            false,
        )
        .unwrap(),
    );
    drop(rx);
    let mut pending = Some(setup(&inputs));
    assert!(!camera_producer::startup_ready(&slot, &None));
    if camera_producer::startup_ready(&slot, &None) {
        let _ = pending.take();
        panic!("cancelled loading is still a live old worker");
    }
    assert!(pending.is_some());
    wait_terminal(slot.as_mut().unwrap()).unwrap();
    slot = None;
    assert!(camera_producer::startup_ready(&slot, &None));
    assert!(pending.take().is_some());
}

/// Real authored Tutorial2 prompt2 commands, collected through Host::status.
/// Reissuing the original prompt supplies the later suffix as a controlled
/// scheduling fixture, not a claimed natural prompt progression.
fn refresh_authored_statuses(
    inputs: &SceneInputs,
) -> (crate::live_host::Status, crate::live_host::Status) {
    let mut launch = setup(inputs);
    launch.preview = crate::singleplayer::Preview::Scenario(100001);
    let (loaded, mut session) = inputs
        .fresh_preview(
            launch,
            fresh_controls().camera,
            &mut SnapshotCache::default(),
        )
        .unwrap();
    drop(loaded);
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let board = inputs.styled_board(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
    let assets = crate::fresh_city::CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    let mut host = crate::live_host::Host::new(fresh_controls().camera.eye);
    host.begin_presentation(&session).unwrap();
    host.frame(&mut session, &assets, 0);
    assert!(host.failure.is_none(), "{:?}", host.failure);
    let base = host.status(&session).tutorial_camera_events.len();
    assert_eq!(
        base, 0,
        "zero elapsed does not execute the delayed initial camera events"
    );
    session
        .dispatch_session_event("InterfacePromptDismiss", 2, &assets, true)
        .unwrap();
    let first = host.status(&session);
    assert!(first.failure.is_none(), "{:?}", first.failure);
    assert_eq!(first.tutorial_camera_events.len(), 2);
    assert_eq!(
        first.tutorial_camera_events[0],
        crate::live_host::TutorialCameraAction::CityView
    );
    assert!(matches!(
        first.tutorial_camera_events[1],
        crate::live_host::TutorialCameraAction::Mode { mode: 12, .. }
    ));
    session
        .dispatch_session_event("InterfacePromptDismiss", 2, &assets, true)
        .unwrap();
    let second = host.status(&session);
    assert!(second.failure.is_none(), "{:?}", second.failure);
    assert_eq!(second.tutorial_camera_events.len(), 4);
    assert_eq!(
        &second.tutorial_camera_events[..2],
        &first.tutorial_camera_events
    );
    assert_eq!(
        &second.tutorial_camera_events[2..],
        &first.tutorial_camera_events
    );
    (first, second)
}
fn refresh_ui_resources(
    inputs: &SceneInputs,
) -> (crate::selection::Assets, crate::navigation::Navigation) {
    let index =
        openmt2001::archive::ArchiveIndex::parse(&inputs.directory, inputs.archive.len() as u64)
            .unwrap();
    let mut selection = selection_assets(inputs);
    inputs.style.apply_blocks(&mut selection.blocks).unwrap();
    let navigation =
        crate::navigation::Navigation::load(&index, &inputs.archive, &inputs.executable).unwrap();
    (selection, navigation)
}

#[test]
#[ignore = "supplied data, real authored Status/Inspection service/worker Inspect/refresh commit, CPU empty render products"]
fn actual_status_camera_suffix_survives_delayed_refresh_rejection_and_warm_commit_once() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let scratch = Scratch::new();
            let inputs = inputs(scratch.0.join("profiles"));
            let (first_status, later_status) = refresh_authored_statuses(&inputs);
            let (selection, navigation) = refresh_ui_resources(&inputs);
            let (tx, rx) = mpsc::channel();
            let source = Arc::clone(&inputs);
            let launch = setup(&inputs);
            let thread = std::thread::Builder::new()
                .stack_size(16 * 1024 * 1024)
                .spawn(move || {
                    source.session_worker_with_sources(
                        WorkerStart::Fresh(launch),
                        fresh_controls().camera,
                        tx,
                        None,
                        false,
                        true,
                    );
                })
                .unwrap();
            let mut loaded = rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
            let initial = Initial::from_loaded(&loaded, fresh_controls()).unwrap();
            let epoch = loaded.camera_epoch.unwrap();
            let mut parked = Some(camera_producer::new_process_owner().unwrap());
            let producer = Producer::attach(
                &mut parked,
                loaded.camera_publisher.take().unwrap(),
                &initial,
            )
            .unwrap();
            let worker_sender = loaded.fresh_session.as_ref().unwrap().clone();
            // Complete startup before using an admitted camera receipt to hold
            // the worker at a legal, deterministic event boundary.
            let mut run = Worker {
                sender: Some(worker_sender),
                loaded: Some(loaded),
                thread: Some(thread),
                producer: None,
            };
            let sender = run.sender.as_ref().unwrap().clone();
            let mut view = Inspection::camera_refresh_test_fixture(
                &inputs,
                run.loaded.as_ref().unwrap().scene.clone(),
                &navigation,
            );
            view._fresh_session = Some(sender.clone());
            view.camera_epoch = Some(epoch);
            initial.active.put(&mut view);
            view.camera_producer = Some(producer);
            let raw_before = view.camera_producer.as_ref().unwrap().capture();
            let (reply, ready) = mpsc::channel();
            sender.send(FreshRequest::UiCameraReady { reply }).unwrap();
            ready
                .recv_timeout(Duration::from_secs(15))
                .unwrap()
                .unwrap();
            let (reply, gate) = mpsc::channel();
            sender
                .send(FreshRequest::CameraTurn {
                    turn: camera_producer::Turn {
                        epoch,
                        building: None,
                    },
                    reply,
                })
                .unwrap();
            let initial_gate = gate.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
            view.apply_status_camera_events(&first_status, &selection, &navigation)
                .unwrap();
            assert_eq!(view.tutorial_camera_seen, 2);
            assert_eq!(view.camera_turns.len(), 2);
            assert!(matches!(
                view.camera_turns[0].event,
                camera_producer::Event::ChooseView { mode: 0, .. }
            ));
            assert!(matches!(
                view.camera_turns[1].event,
                camera_producer::Event::Block {
                    origin: camera_producer::BlockOrigin::Tutorial,
                    ..
                }
            ));
            let mut loading = None;
            let mut refreshing = false;
            let mut retry = refresh::Retry::default();
            camera_producer::request_refresh(
                Some(&mut view),
                &selection,
                None,
                &mut loading,
                &mut refreshing,
                &mut retry,
            )
            .unwrap();
            assert!(refreshing && loading.is_some());
            assert_eq!(
                view.camera_turns.len(),
                2,
                "refresh must retain both already-seen authored commands before payload admission"
            );
            assert_eq!(view.camera_producer.as_ref().unwrap().capture(), raw_before);
            assert_eq!(view.tutorial_camera_seen, 2);
            initial_gate
                .receipt
                .complete(Err("controlled pre-refresh admitted gate cancelled".into()));
            // UI drain used by Intro before loading/modal early returns. Inspect
            // can reply only after both UI/raw/publication terminal receipts.
            let deadline = Instant::now() + Duration::from_secs(30);
            let snapshot = loop {
                view.service_camera_turns(&selection, None);
                match loading.as_ref().unwrap().try_recv() {
                    Ok(result) => break result.unwrap(),
                    Err(mpsc::TryRecvError::Empty) => {
                        assert!(Instant::now() < deadline);
                        std::thread::yield_now();
                    }
                    Err(error) => {
                        panic!("Inspect must complete after retained camera suffix: {error}")
                    }
                }
            };
            assert!(view.camera_turns.is_empty());
            assert_eq!(view.tutorial_camera_seen, 2);
            let category = match first_status.tutorial_camera_events[1] {
                crate::live_host::TutorialCameraAction::Mode { mode: 12, target } => target,
                _ => unreachable!(),
            };
            assert_eq!(view.camera_producer.as_ref().unwrap().kind(), 12);
            assert!(!view.strategic);
            assert_eq!(
                selection.blocks[view.selected.unwrap().category].id,
                category as usize
            );
            assert_eq!(
                view.camera_producer.as_ref().unwrap().capture().records()[1],
                raw_before.records()[1]
            );
            // A concrete admitted camera receipt holds the worker after Inspect.
            // It supplies deterministic scheduling without sleep/test-only worker
            // hooks and will be cancelled without a success suffix below.
            let turn = camera_producer::Turn {
                epoch,
                building: None,
            };
            let (reply, gate) = mpsc::channel();
            sender
                .send(FreshRequest::CameraTurn { turn, reply })
                .unwrap();
            let held = gate.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
            view.apply_status_camera_events(&later_status, &selection, &navigation)
                .unwrap();
            assert_eq!(view.tutorial_camera_seen, 4);
            assert_eq!(view.camera_turns.len(), 2);
            let after_first = view.camera_producer.as_ref().unwrap().capture();
            assert!(
                camera_producer::request_refresh(
                    Some(&mut view),
                    &selection,
                    None,
                    &mut loading,
                    &mut refreshing,
                    &mut retry
                )
                .is_err()
            );
            assert_eq!(
                view.camera_turns.len(),
                2,
                "already-pending refresh refusal must retain the later authored suffix"
            );
            assert_eq!(
                view.camera_producer.as_ref().unwrap().capture(),
                after_first
            );
            // Reject a real detached preparation too, preserving the SAME source,
            // pending suffix and seen cursor for the subsequent successful commit.
            let mut next = Inspection::camera_refresh_test_fixture(
                &inputs,
                snapshot.scene.clone(),
                &navigation,
            );
            next._fresh_session = view._fresh_session.clone();
            next.camera_ui = None;
            next.ui_graph = Some(camera_live::ui_graph::refresh_fixture([0; 600]));
            let old_buildings = view.scene.inspection_buildings.clone();
            let new_scene = next.scene.clone();
            let adjustment = crate::business_adjustment::refresh_assets_fixture();
            let params = || refresh::Inputs {
                previous_buildings: &old_buildings,
                next_scene: (&new_scene).into(),
                locator_extents: [0; 2],
                business_owner: None,
            };
            assert!(
                refresh::prepare(
                    &refresh::Owners::of(&mut view),
                    &mut refresh::Owners::of(&mut next),
                    params(),
                    &adjustment,
                    &mut retry
                )
                .is_err()
            );
            assert_eq!(view.camera_turns.len(), 2);
            assert_eq!(view.tutorial_camera_seen, 4);
            assert_eq!(
                view.camera_producer.as_ref().unwrap().capture(),
                after_first
            );
            next.ui_graph = None;
            let plan = refresh::prepare(
                &refresh::Owners::of(&mut view),
                &mut refresh::Owners::of(&mut next),
                params(),
                &adjustment,
                &mut retry,
            )
            .unwrap();
            next.selected = view.selected;
            next.return_camera = view.return_camera;
            refresh::commit(
                &mut refresh::Owners::of(&mut view),
                &mut refresh::Owners::of(&mut next),
                plan,
                &mut retry,
            );
            camera_producer::commit_refresh(&mut view, &mut next);
            assert!(view.camera_producer.is_none() && view.camera_turns.is_empty());
            assert_eq!(next.camera_epoch, Some(epoch));
            assert_eq!(next.tutorial_camera_seen, 4);
            assert_eq!(next.camera_turns.len(), 2);
            assert_eq!(
                next.camera_producer.as_ref().unwrap().capture(),
                after_first
            );
            drop(loading.take());
            refreshing = false;
            assert!(!refreshing);
            held.receipt
                .complete(Err("controlled post-Inspect gate cancelled".into()));
            let deadline = Instant::now() + Duration::from_secs(30);
            while !next.camera_turns.is_empty() {
                next.service_camera_turns(&selection, None);
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert!(next.camera_producer.as_ref().unwrap().error().is_none());
            assert_eq!(next.camera_producer.as_ref().unwrap().kind(), 12);
            assert_eq!(next.tutorial_camera_seen, 4);
            let committed = next.camera_producer.as_ref().unwrap().capture();
            next.apply_status_camera_events(&later_status, &selection, &navigation)
                .unwrap();
            next.apply_status_camera_events(&later_status, &selection, &navigation)
                .unwrap();
            assert!(
                next.camera_turns.is_empty(),
                "seen cursor must not replay committed commands after warm adoption"
            );
            assert_eq!(next.camera_producer.as_ref().unwrap().capture(), committed);
            // SaveSelected reaches the actual same worker/S6 source consumer after
            // the post-Inspect suffix, with the complete committed mode/raw pair.
            let (reply, options) = mpsc::channel();
            sender
                .send(FreshRequest::Options { open: true, reply })
                .unwrap();
            options
                .recv_timeout(Duration::from_secs(15))
                .unwrap()
                .unwrap();
            let (reply, saved) = mpsc::channel();
            sender
                .send(FreshRequest::SaveSelected {
                    target: crate::save_menu::Target::NewName("Refresh suffix".into()),
                    camera: next.camera,
                    capture: None,
                    game_mode: 1,
                    interface_mode: 0,
                    reply,
                })
                .unwrap();
            let path = saved
                .recv_timeout(Duration::from_secs(30))
                .unwrap()
                .unwrap();
            assert_eq!(read_pair(&path), committed);
            let bytes = fs::read(&path).unwrap();
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let ext = openmt2001::save::extension::Extension::parse(&save)
                .unwrap()
                .unwrap();
            assert_eq!(ext.required(15).unwrap(), &0u32.to_le_bytes());
            let (owner, result) = next.camera_producer.take().unwrap().retire();
            result.unwrap();
            parked = Some(owner);
            next._fresh_session = None;
            view._fresh_session = None;
            drop(snapshot);
            drop(sender);
            run.sender.take();
            run.loaded.take();
            run.thread.take().unwrap().join().unwrap();
            assert!(parked.is_some());
        })
        .unwrap()
        .join()
        .unwrap();
}
