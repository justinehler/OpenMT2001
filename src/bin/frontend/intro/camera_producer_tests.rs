use super::*;
use openmt2001::camera::bank::RECORD_BYTES;

pub(super) fn row(seed: u8, kind: u32, eye: [f32; 3], target: [f32; 3]) -> RawRecord {
    let mut raw = std::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(seed));
    raw[..4].copy_from_slice(&kind.to_le_bytes());
    for (at, values) in [(0x4c, eye), (0x58, target), (0x64, [0., 1., 0.])] {
        for (axis, value) in values.into_iter().enumerate() {
            raw[at + axis * 4..at + axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    raw
}
pub(super) fn pair() -> Capture {
    Capture::from_records([
        row(11, 4, [-220., 1050., -30.], [-300., 5., -30.]),
        row(79, 12, [100., 120., 0.], [20., 5., 0.]),
    ])
}
fn core(capture: Capture) -> Core {
    Core::new(camera_live::owner::Owner::new(
        Bank::from_capture(capture),
        0x5a,
    ))
}
fn fresh() -> Controls {
    let (camera, controls) = super::super::fresh_city_camera();
    Controls {
        camera,
        controls,
        inertia: BlockCameraInertia::new(camera.eye, camera.target),
        saved: false,
        building: false,
        building_angle: 0.,
        selected: None,
    }
}
#[test]
fn primary_projection_validates_all_operands_and_changes_only_literal_nine_words() {
    let before = pair();
    let mut core = core(before);
    let sample = CameraSample {
        eye: [-0., 26., 586.],
        target: [3., -4., 5.],
        up: [0., 0., 1.],
        vertical_fov: 53.,
    };
    core.project(sample).unwrap();
    let after = core.capture();
    assert_eq!(after.records()[1], before.records()[1]);
    assert_eq!(core.owner.kind_dirty(), 0x5a);
    for i in 0..RECORD_BYTES {
        if !(0x4c..0x70).contains(&i) {
            assert_eq!(after.records()[0][i], before.records()[0][i], "byte{i:x}");
        }
    }
    let literal: [u32; 9] = [
        0x80000000, 0x41d00000, 0x44128000, 0x40400000, 0xc0800000, 0x40a00000, 0, 0, 0x3f800000,
    ];
    assert_eq!(
        &after.records()[0][0x4c..0x70],
        literal
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    for bad in [
        CameraSample {
            eye: [f32::NAN, 0., 0.],
            ..sample
        },
        CameraSample {
            target: [0., f32::INFINITY, 0.],
            ..sample
        },
        CameraSample {
            up: [0.; 3],
            ..sample
        },
    ] {
        assert!(core.project(bad).is_err());
        assert_eq!(core.capture(), after);
        assert_eq!(core.owner.kind_dirty(), 0x5a);
    }
}
#[test]
fn cold_kind4_first_return_preserves_full_secondary_and_next_callback_pose() {
    let original = pair();
    let initial = Initial::prepare(Some(original), Some(3), fresh()).unwrap();
    assert_eq!(initial.active.camera.vertical_fov, 53.);
    assert_eq!(initial.active.camera.eye, [-220., 1050., -30.]);
    let mut core = core(original);
    let next = core
        .navigate(initial.active, None, initial.active.camera)
        .unwrap();
    assert!(!next.strategic);
    assert!(next.normal_return.is_none());
    let mut returned = next.active;
    assert_eq!(returned.camera.vertical_fov, 53.);
    assert!(returned.saved);
    assert!(!returned.building);
    assert_eq!(returned.controls.center, [20., 5., 25.]);
    assert_eq!(returned.controls.angle, 0.);
    assert_eq!(returned.controls.radius, 80.);
    assert_eq!(returned.controls.height, 120.);
    assert_eq!(core.capture().into_records(), [original.records()[1]; 2]);
    assert_eq!(core.owner.kind_dirty(), 0x5a);
    let pose = super::super::mode12_input_tick_with_height(
        &mut returned.controls,
        true,
        [false; 4],
        [0; 2],
        50,
        false,
        false,
    )
    .unwrap();
    assert_eq!(pose, ([100., 120., 0.], [20., 5., 0.]));
    returned.inertia.tick(pose.0, pose.1);
    for fraction in [0., 0.5, 1.] {
        assert_eq!(
            returned.inertia.sample(fraction).unwrap(),
            ([100., 120., 0.], [20., 5., 0.])
        );
    }
    core.project(returned.camera).unwrap();
    assert_eq!(core.capture().into_records(), [original.records()[1]; 2]);
}
#[test]
fn entry_copies_entire_current_primary_and_return_never_runs_kind_setter() {
    let original = pair();
    let normal = Capture::from_records([original.records()[1], original.records()[0]]);
    let mut core = core(normal);
    let active = Controls::from_raw(&normal.records()[0], false).unwrap();
    let next = core.navigate(active, None, fresh().camera).unwrap();
    assert!(next.strategic);
    assert_eq!(next.normal_return.unwrap().camera.eye, active.camera.eye);
    let entry = core.capture();
    assert_eq!(entry.records()[1], normal.records()[0]);
    assert_eq!(core.kind(), 4);
    // Literal independent native store oracles; sampled/current bytes are not a callback.
    assert_eq!(
        &entry.records()[0][0x34..0x40],
        &[0xc35c0000u32, 0x44834000, 0xc1f0cccd]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &entry.records()[0][0x40..0x4c],
        &[0xc35c0000u32, 0, 0xc1f00000]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &entry.records()[0][0x4c..0x64],
        &normal.records()[0][0x4c..0x64]
    );
    assert_eq!(&entry.records()[0][0x70..0x73], &[0, 0, 0]);
    assert_eq!(core.owner.kind_dirty(), 1);
    assert_eq!(
        core.strategic_after_ui_clear(),
        StrategicTransition::Returned
    );
    assert_eq!(core.capture().into_records(), [normal.records()[0]; 2]);
    assert_eq!(core.owner.kind_dirty(), 1);
}
#[test]
fn mismatch_and_unsupported_return_preflight_never_normalize_raw_pair() {
    let original = pair();
    // Root CityView correction supersedes the former biconditional oracle:
    // UI mode0 has no raw setter and can legitimately retain rawkind4.
    let city = Initial::prepare(Some(original), Some(0), fresh()).unwrap();
    assert!(!city.strategic());
    assert_eq!(city.metadata, Some(0));
    assert_eq!(city.active.camera.vertical_fov, 53.);
    assert_eq!(city.imported, Some(original));
    let normal = Capture::from_records([original.records()[1], original.records()[0]]);
    assert!(Initial::prepare(Some(normal), Some(3), fresh()).is_err());
    assert!(Initial::prepare(Some(normal), Some(2), fresh()).is_err());
    let mut rows = original.into_records();
    rows[1][..4].copy_from_slice(&0xfeedbeefu32.to_le_bytes());
    let unknown = Capture::from_records(rows);
    let initial = Initial::prepare(Some(unknown), Some(3), fresh()).unwrap();
    assert!(initial.normal_return.is_none());
    let core = core(unknown);
    assert!(core.preflight_return(None).is_err());
    assert_eq!(core.capture(), unknown);
    // The raw core itself does copy an opaque unknown return; the UI refuses
    // only its required rendering boundary before invoking it.
    let mut raw = core;
    assert_eq!(
        raw.strategic_after_ui_clear(),
        StrategicTransition::Returned
    );
    assert_eq!(raw.capture().into_records(), [unknown.records()[1]; 2]);
}
#[test]
fn warm_return_retains_controls_inertia_selection_and_checks_raw_pose() {
    let original = pair();
    let core = core(original);
    let mut cached = Controls::from_raw(&original.records()[1], false).unwrap();
    cached.controls.angle = 0.75;
    cached.building_angle = 2.;
    cached.saved = false;
    cached.selected = Some(crate::selection::Hit {
        category: 7,
        building: None,
    });
    cached.inertia.tick([80., 100., -10.], [1., 2., 3.]);
    let expected = cached.inertia.sample(0.5).unwrap();
    let warm = core.preflight_return(Some(cached)).unwrap();
    assert_eq!(warm.controls.angle, 0.75);
    assert_eq!(warm.building_angle, 2.);
    assert_eq!(warm.selected, cached.selected);
    assert!(!warm.saved);
    assert_eq!(warm.inertia.sample(0.5).unwrap(), expected);
    cached.camera.target[2] = 1.;
    assert!(core.preflight_return(Some(cached)).is_err());
    assert_eq!(core.capture(), original);
}
#[test]
fn same_owner_moves_legacy_park_native_park_legacy_without_losing_containing_state() {
    use camera_live::ui_selection::{RelatedKey, SelectionState};
    let mut owner = new_process_owner().unwrap();
    owner
        .install_ui_selection(SelectionState::installed(
            None,
            None,
            None,
            RelatedKey::from_native_cell(0x12345678),
            Some(19),
            Some(23),
            29,
            31,
        ))
        .unwrap();
    owner.restore(pair());
    let dirty = owner.kind_dirty();
    let core = Core::new(owner);
    let before = core.capture();
    let mut parked = Some(core.into_owner());
    let (worker, inbox) = crate::live_host::camera_bridge::channel(71);
    let endpoint = camera_live::endpoint::Endpoint::new(parked.take().unwrap(), inbox);
    assert!(parked.is_none());
    assert_eq!(endpoint.capture(), before);
    parked = Some(endpoint.into_owner());
    drop(worker);
    let core = Core::new(parked.take().unwrap());
    let owner = core.into_owner();
    assert_eq!(owner.capture(), before);
    assert_eq!(owner.kind_dirty(), dirty);
    let state = owner.ui_selection().unwrap();
    assert_eq!(state.related.unwrap().native_key(), 0x12345678);
    assert_eq!(
        (
            state.selected_building,
            state.sequence_index,
            state.list_business,
            state.list_commodity
        ),
        (Some(19), Some(23), 29, 31)
    );
}
#[test]
fn separate_world_reset_stores_preserve_process_secondary_and_intervening_yaw() {
    let mut owner = new_process_owner().unwrap();
    owner.restore(pair());
    let secondary = owner.capture().records()[1];
    owner.reset_yaw();
    let mut rows = owner.capture().into_records();
    rows[0][0x7c..0x80].copy_from_slice(&0.75f32.to_le_bytes());
    owner.restore(Capture::from_records(rows));
    owner.world_reset_final_fields();
    let after = owner.capture();
    assert_eq!(after.records()[1], secondary);
    assert_eq!(owner.kind(), 6);
    assert_eq!(owner.kind_dirty(), 1);
    assert_eq!(&after.records()[0][0x7c..0x80], &0.75f32.to_le_bytes());
    assert_eq!(
        &after.records()[0][0x34..0x40],
        &[0x41d00000u32, 0x44128000, 0xc42dc000]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
}

#[test]
fn absent_metadata_legacy_attachment_is_distinct_from_explicit_city_view_without_raw_normalization()
{
    let raw = pair();
    let absent = Initial::prepare(Some(raw), None, fresh()).unwrap();
    assert_eq!(absent.provenance, Attachment::LegacyRawKind4);
    assert_eq!(absent.metadata, None);
    assert!(absent.strategic());
    assert_eq!(absent.active.camera.vertical_fov, 53.);
    assert_eq!(absent.imported, Some(raw));
    let city = Initial::prepare(Some(raw), Some(0), fresh()).unwrap();
    assert_eq!(city.provenance, Attachment::Explicit(0));
    assert!(!city.strategic());
    assert_eq!(city.active.camera.vertical_fov, 53.);
    assert_eq!(city.imported, Some(raw));
    let normal = Capture::from_records([raw.records()[1]; 2]);
    assert!(Initial::prepare(Some(normal), Some(3), fresh()).is_err());
}
#[test]
fn every_receipt_has_exactly_one_terminal_success_or_drop_cancellation() {
    let (sender, answer) = std::sync::mpsc::channel();
    Receipt(Some(sender)).complete(Ok(Vec::new()));
    assert_eq!(answer.recv().unwrap(), Ok(Vec::new()));
    assert!(matches!(
        answer.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
    let (sender, answer) = std::sync::mpsc::channel();
    drop(Receipt(Some(sender)));
    assert!(answer.recv().unwrap().is_err());
    assert!(matches!(
        answer.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn successful_receipt_preserves_typed_post_order_and_drop_emits_none() {
    let (sender, answer) = std::sync::mpsc::channel();
    let posts = vec![
        Post::TutorialControl(73),
        Post::TutorialPick(crate::tutorial_two_interaction::Pick::BlockEntry { category_id: 5 }),
    ];
    Receipt(Some(sender)).complete(Ok(posts.clone()));
    assert_eq!(answer.recv().unwrap(), Ok(posts));
    assert!(matches!(
        answer.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
    let (sender, answer) = std::sync::mpsc::channel();
    drop(Receipt(Some(sender)));
    assert!(answer.recv().unwrap().is_err());
    assert!(matches!(
        answer.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
}
#[test]
fn worker_terminal_disconnect_is_not_clean_completion_and_is_consumed_once() {
    let (sender, answer) = std::sync::mpsc::channel();
    let mut worker = WorkerExit {
        receiver: Some(answer),
        custody: None,
    };
    assert!(worker.poll().is_none());
    drop(sender);
    assert!(worker.poll().unwrap().is_err());
    assert!(worker.poll().is_none());
    let slot = None;
    let failure = Some("terminal outcome unknown".to_owned());
    assert!(!startup_ready(&slot, &failure));
}
#[test]
fn off_ui_join_reports_callback_failure_and_panic_without_releasing_startup() {
    for panic in [false, true] {
        let failures = ExitFailures::default();
        let errors = std::sync::Arc::clone(&failures);
        let worker = std::thread::spawn(move || {
            if panic {
                panic!("terminal fixture panic");
            }
            record_exit(&errors, "partial callback failed after commit".into());
        });
        let mut slot = Some(supervise(worker, failures, "camera-terminal-fixture"));
        assert!(!startup_ready(&slot, &None));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let outcome = loop {
            if let Some(result) = slot.as_mut().unwrap().poll() {
                break result;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert!(outcome.is_err());
        let failure = outcome.err();
        slot = None;
        assert!(!startup_ready(&slot, &failure));
    }
}

#[test]
fn fresh_mode6_keeps_start_camera_outside_block_pan_bounds() {
    // The same setup and callback gate used by Intro::draw/inspection_update.
    let initial = Initial::prepare(None, Some(0), fresh()).unwrap();
    let mut active = initial.active;
    let mut owner = new_process_owner().unwrap();
    owner.reset_yaw();
    owner.world_reset_final_fields();
    let mut core = Core::new(owner);
    core.project(active.camera).unwrap();
    assert_eq!(core.kind(), 6);
    for _ in 0..500 {
        if core.legacy_callback_enabled(false) {
            let (eye, target) = super::super::mode12_input_tick(
                &mut active.controls,
                active.saved,
                [false; 4],
                [0; 2],
                50,
                false,
            )
            .unwrap();
            active.inertia.tick(eye, target);
            let (eye, target) = active.inertia.sample(0.5).unwrap();
            active.camera.eye = eye;
            active.camera.target = target;
        }
        core.project(active.camera).unwrap();
    }
    assert_eq!(active.camera.eye, [26., 586., -695.]);
    assert_eq!(active.camera.target, [26., 463., -595.]);
    assert_eq!(active.camera.up, [0., 1., 0.]);
    assert_eq!(active.camera.vertical_fov, 53.);
    assert_eq!(active.controls.center, [51., 463., -595.]);
    assert!(active.same_pose(&core.capture().records()[0]));
    // A subsequent block/district selection still admits normal controls.
    for kind in [12, 13] {
        core.owner.set_kind(kind);
        assert!(core.legacy_callback_enabled(false));
        assert!(!core.legacy_callback_enabled(true));
    }
}

#[cfg(feature = "capture")]
#[test]
#[ignore = "requires supplied assets and GPU; writes initial-view comparison captures"]
fn capture_untouched_campaign_and_skirmish_start() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use openmt2001::skirmish::roster::{Kind, Participant};
            let output = openmt2001::fixtures::scratch("INITIAL_CAMERA_CAPTURE");
            std::fs::create_dir_all(&output).unwrap();
            let inputs = crate::intro::camera_producer_live_tests::inputs(output.join("profile"));
            let mut setup = crate::intro::camera_producer_live_tests::setup(&inputs);
            setup.preview = crate::singleplayer::Preview::Scenario(200001);
            let skirmish = crate::menu::skirmish_route::Request {
                catalog: setup.catalog.clone(),
                save_root: output.join("skirmish"),
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
                            name: b"Camera human".to_vec(),
                            ready: true,
                            difficulty: 1,
                        },
                        Participant {
                            kind: Kind::Ai,
                            token: Some(1),
                            name: b"Camera AI".to_vec(),
                            ready: true,
                            difficulty: 1,
                        },
                    ],
                },
            };
            let camera = super::super::fresh_city_camera().0;
            let descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            for (name, (mut loaded, session)) in [
                (
                    "starting-out",
                    inputs
                        .fresh_preview(setup, camera, &mut super::super::SnapshotCache::default())
                        .unwrap(),
                ),
                (
                    "skirmish-empty",
                    inputs
                        .fresh_skirmish(
                            skirmish,
                            camera,
                            &mut super::super::SnapshotCache::default(),
                        )
                        .unwrap(),
                ),
            ] {
                let (publisher, _reader) =
                    crate::save_menu::camera_source::channel(session.world.topology.epoch());
                let initial = Initial::from_loaded(&loaded, fresh()).unwrap();
                let mut parked = Some(new_process_owner().unwrap());
                let producer = Producer::attach(&mut parked, publisher, &initial).unwrap();
                assert!(!producer.legacy_callback_enabled(false));
                // Replay the former erroneous idle mode12 route for a controlled
                // before image. The after image retains the production kind6 pose.
                let mut before = initial.active;
                for _ in 0..500 {
                    let pose = super::super::mode12_input_tick(
                        &mut before.controls,
                        false,
                        [false; 4],
                        [0; 2],
                        50,
                        false,
                    )
                    .unwrap();
                    before.inertia.tick(pose.0, pose.1);
                }
                let (eye, target) = before.inertia.sample(0.5).unwrap();
                before.camera.eye = eye;
                before.camera.target = target;
                for (suffix, sample) in
                    [("before", before.camera), ("after", initial.active.camera)]
                {
                    let scene = &mut loaded.scene;
                    scene.reframe(sample).unwrap();
                    let path = output.join(format!("{name}-{suffix}.png"));
                    crate::capture_test::capture_scene_at_size(
                        &renderer,
                        scene,
                        path.to_str().unwrap(),
                        [1024, 768],
                    );
                    eprintln!(
                        "{name}-{suffix}: eye {:?}, target {:?}, FOV {} -> {}",
                        sample.eye,
                        sample.target,
                        sample.vertical_fov,
                        path.display()
                    );
                }
                producer.retire().1.unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ordinary_level_blockcam_uses_player_block_entry_and_information_target() {
    use openmt2001::{
        board::Block,
        level_script,
        lua4::{Runtime, Value},
    };
    // IDs deliberately differ from vector indices: scripts pass board IDs.
    let assets = crate::selection::Assets::descriptor_test_assets(
        [2, 7]
            .into_iter()
            .enumerate()
            .map(|(index, id)| Block {
                id,
                name: format!("Authored block {id}"),
                text_id: 0,
                kind: 0,
                center: [100. * index as f32, 30.],
                size: [82.; 2],
                pavement_color: 0,
                flags: 0,
                street_sides: 0,
            })
            .collect(),
        [0; 116],
    );
    let initial = View {
        active: fresh(),
        strategic: true,
        normal_return: Some(fresh()),
    };
    let mut startup = TutorialStart::new(true);
    assert!(startup.holds_loading(0, false, false));
    // Receiving the event alone is insufficient: the UI turn still has to commit.
    assert!(startup.holds_loading(1, true, false));
    let mut scripted = initial;
    let mut script_core = Core::new(new_process_owner().unwrap());
    script_core.project(initial.active.camera).unwrap();
    let mut vm = Runtime::new().unwrap();
    vm.execute(
        "Camera table",
        b"Camera={}; BLOCKCAM=12",
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    level_script::camera::register(&mut vm).unwrap();
    vm.execute(
        "ordinary level event",
        b"function EVENT_Level() Camera.SetMode(BLOCKCAM, 7) end",
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    vm.call("EVENT_Level", &[0.], &mut |id, args| {
        assert_eq!(id, 701);
        let (command, args) = level_script::camera::command(args)?;
        assert!(matches!(command, level_script::Command::TutorialCameraMode));
        let [Value::Number(12.), Value::Number(target)] = args else {
            panic!("block camera arguments")
        };
        script_core.event(
            &mut scripted,
            Event::script_block(*target as u32, &assets)?,
            Resolved::Local,
            &assets,
        )?;
        Ok(None)
    })
    .unwrap();
    let selected = crate::selection::Hit {
        category: 1,
        building: None,
    };
    let mut clicked = initial;
    let mut click_core = Core::new(new_process_owner().unwrap());
    click_core.project(initial.active.camera).unwrap();
    click_core
        .event(
            &mut clicked,
            Event::Block {
                category: 7,
                selected,
                origin: BlockOrigin::Selection,
            },
            Resolved::Local,
            &assets,
        )
        .unwrap();
    assert!(!startup.holds_loading(1, false, false));
    // Later camera turns must not re-enter startup loading.
    assert!(!startup.holds_loading(2, true, false));
    assert!(!TutorialStart::new(false).holds_loading(0, false, false));
    assert!(!TutorialStart::new(true).holds_loading(0, false, true));
    // A tutorial that never sends a camera event still leaves loading.
    let mut silent = TutorialStart::new(true);
    for _ in 0..TutorialStart::MAX_HELD_FRAMES {
        assert!(silent.holds_loading(0, false, false));
    }
    assert!(!silent.holds_loading(0, false, false));
    assert_eq!(script_core.kind(), 12);
    assert_eq!(script_core.capture(), click_core.capture());
    assert_eq!(scripted.active.selected, Some(selected));
    assert!(!scripted.strategic);
    assert!(scripted.normal_return.is_none());
    assert!(
        matches!(super::super::information_target(scripted.active.selected, &[]), super::super::InformationTarget::Block(hit) if hit == selected)
    );
}

#[test]
fn tutorial_blockcam_flies_into_block_view_while_camera_control_is_disabled() {
    use super::super::{CameraCallbackInput, LegacyCamera, legacy_camera_callback};
    let assets = crate::selection::Assets::descriptor_test_assets(
        vec![openmt2001::board::Block {
            id: 7,
            name: "Authored block 7".into(),
            text_id: 0,
            kind: 0,
            center: [200., -150.],
            size: [82.; 2],
            pavement_color: 0,
            flags: 0,
            street_sides: 0,
        }],
        [0; 116],
    );
    // A fresh tutorial starts on the passive mode6 overview.
    let mut view = View {
        active: fresh(),
        strategic: false,
        normal_return: None,
    };
    let overview = view.active.camera;
    // The same reset boundaries as Producer::attach on a fresh world.
    let mut owner = new_process_owner().unwrap();
    owner.reset_yaw();
    owner.world_reset_final_fields();
    let mut core = Core::new(owner);
    core.project(overview).unwrap();
    assert_eq!(core.kind(), 6);
    assert!(!core.legacy_callback_enabled(view.strategic));
    // EVENT_Level: BlockCam (Camera.SetMode(BLOCKCAM, 7)), then
    // Interface.DisableCameraControl before the first callback.
    core.event(
        &mut view,
        Event::script_block(7, &assets).unwrap(),
        Resolved::Local,
        &assets,
    )
    .unwrap();
    assert_eq!(core.kind(), 12);
    assert!(core.legacy_callback_enabled(view.strategic));
    let mut active = view.active;
    let mut block_view = active.controls;
    let (block_eye, block_target) = block_view.tick([false; 4]);
    // The player holds every key and drags with both buttons: none of it may
    // reach the camera, but the callback still eases toward the block.
    let held = CameraCallbackInput {
        keys: [true; 4],
        drag: [14, 9],
        buttons: [true, true],
    }
    .gated(true);
    let mut sampled = Vec::new();
    for _ in 0..250 {
        legacy_camera_callback(
            LegacyCamera {
                controls: &mut active.controls,
                building_angle: &mut active.building_angle,
                building_camera: active.building,
                saved_camera: active.saved,
                inertia: &mut active.inertia,
            },
            held,
            50,
            false,
            false,
        )
        .unwrap();
        sampled.push(active.inertia.sample(0.).unwrap());
    }
    assert_eq!(active.controls.center, view.active.controls.center);
    assert_eq!(active.controls.angle, view.active.controls.angle);
    // The camera has left the overview within the first second ...
    assert!(sampled[50].0[1] < overview.eye[1] - 200.);
    // ... and after five seconds holds the close, low block view.
    let (eye, target) = *sampled.last().unwrap();
    for (actual, expected) in eye
        .into_iter()
        .zip(block_eye)
        .chain(target.into_iter().zip(block_target))
    {
        assert!((actual - expected).abs() < 0.01, "{eye:?} {target:?}");
    }
    assert!(eye[1] < 200.);
}

#[test]
fn open_prompt_removes_camera_input_without_stopping_the_callback() {
    use super::super::camera_input_disabled;
    use crate::live_host::TutorialControls;
    let enabled = TutorialControls::default();
    // A modal prompt takes the player's input away on its own, in a tutorial
    // or not; the frame loop still runs the camera callback under it.
    assert!(camera_input_disabled(true, None));
    assert!(camera_input_disabled(true, Some(enabled)));
    assert!(!camera_input_disabled(false, None));
    assert!(!camera_input_disabled(false, Some(enabled)));
    for controls in [
        TutorialControls {
            camera_disabled: true,
            ..enabled
        },
        TutorialControls {
            all_disabled: true,
            ..enabled
        },
    ] {
        assert!(camera_input_disabled(false, Some(controls)));
    }
    // Disabled building picking leaves the camera to the player.
    assert!(!camera_input_disabled(
        false,
        Some(TutorialControls {
            picking_disabled: true,
            ..enabled
        })
    ));
}
