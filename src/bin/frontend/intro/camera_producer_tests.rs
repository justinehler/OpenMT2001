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
    assert_eq!(initial.active.camera.vertical_fov, 45.);
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
    assert_eq!(absent.active.camera.vertical_fov, 45.);
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
        receiver: Some(answer),custody:None,
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
