use super::*;
use crate::save_menu::{self, PreparedAutosave, Target};
use std::{
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{SystemTime, UNIX_EPOCH},
};

fn row(seed: u8, kind: u32) -> [u8; 196] {
    let mut bytes = std::array::from_fn(|i| (i as u8).wrapping_mul(73).wrapping_add(seed));
    bytes[..4].copy_from_slice(&kind.to_le_bytes());
    bytes
}
fn pair(seed: u8, kind: u32) -> Capture {
    Capture::from_records([row(seed, kind), row(seed.wrapping_add(19), u32::MAX)])
}
fn sample() -> CameraSample {
    CameraSample {
        eye: [26., 586., -695.],
        target: [26., 463., -595.],
        up: [0., 1., 0.],
        vertical_fov: 53.,
    }
}

#[test]
fn primary_projection_has_exact_pose_mask_and_keeps_all_return_bytes() {
    let before = pair(17, 12);
    let mut ui = LegacyPair::new(Bank::from_capture(before), 0x5a);
    ui.project_primary(CameraSample {
        up: [0.25, 0.5, -0.75],
        ..sample()
    })
    .unwrap();
    let after = ui.capture();
    assert_eq!(after.records()[1], before.records()[1]);
    assert_eq!(ui.kind_dirty(), 0x5a);
    for i in 0..196 {
        if !(0x4c..0x70).contains(&i) {
            assert_eq!(after.records()[0][i], before.records()[0][i], "byte{i:x}");
        }
    }
    let expected = [26f32, 586., -695.].map(f32::to_le_bytes);
    assert_eq!(&after.records()[0][0x4c..0x58], expected.as_flattened());
    let expected_target = [26f32, 463., -595.].map(f32::to_le_bytes);
    let expected_up = [0.25f32, 0.5, -0.75].map(f32::to_le_bytes);
    assert_eq!(
        &after.records()[0][0x58..0x64],
        expected_target.as_flattened()
    );
    assert_eq!(&after.records()[0][0x64..0x70], expected_up.as_flattened());
    for bad in [
        CameraSample {
            eye: [f32::NAN, 0., 0.],
            ..sample()
        },
        CameraSample {
            up: [0.; 3],
            ..sample()
        },
    ] {
        assert!(ui.project_primary(bad).is_err());
        assert_eq!(ui.capture(), after);
    }
}

#[test]
fn strategic_transition_discriminator_and_full_copy_are_raw_not_tuple_based() {
    let original = pair(31, 12);
    let mut ui = LegacyPair::new(Bank::from_capture(original), 0x5a);
    assert_eq!(ui.strategic_after_ui_clear(), StrategicTransition::Entered);
    assert_eq!(ui.primary_kind(), 4);
    assert_eq!(ui.capture().records()[1], original.records()[0]);
    assert_eq!(ui.kind_dirty(), 1);
    ui.project_primary(sample()).unwrap();
    assert_eq!(ui.capture().records()[1], original.records()[0]);
    assert_eq!(ui.strategic_after_ui_clear(), StrategicTransition::Returned);
    assert_eq!(ui.capture().into_records(), [original.records()[0]; 2]);

    // A cold rawkind4 returns even with no modern mode/return tuple and an
    // unknown secondary numeric mode. Return must not run a kind setter.
    let cold = pair(55, 4);
    let mut ui = LegacyPair::new(Bank::from_capture(cold), 0xa7);
    assert_eq!(ui.strategic_after_ui_clear(), StrategicTransition::Returned);
    assert_eq!(ui.capture().into_records(), [cold.records()[1]; 2]);
    assert_eq!(ui.kind_dirty(), 0xa7);
    ui.restore(original);
    assert_eq!(ui.capture(), original);
    assert_eq!(ui.kind_dirty(), 0xa7);
}

#[test]
fn missing_mismatched_and_retired_sources_never_supply_an_old_fallback() {
    let (mut publisher, reader) = channel_serial(17);
    assert!(reader.read_serial(17).is_err());
    publisher.publish(pair(7, 6), 0).unwrap();
    assert!(reader.read_serial(18).is_err());
    assert!(publisher.publish(pair(9, 6), 3).is_err());
    assert!(
        reader.read_serial(17).is_err(),
        "mismatch cannot leave prior sample current"
    );
    let city_view = pair(9, 4);
    publisher.publish(city_view, 0).unwrap();
    let city_cut = reader.read_serial(17).unwrap();
    assert_eq!(city_cut.interface_mode(), 0);
    assert_eq!(
        city_cut.capture().records(),
        city_view.records(),
        "CityView preserves both raw rows, including kind4 and secondary"
    );
    assert!(publisher.publish(pair(9, 4), 2).is_err());
    publisher.publish(pair(9, 4), 3).unwrap();
    publisher.invalidate().unwrap();
    assert!(reader.read_serial(17).is_err());
    publisher.publish(pair(11, 6), 0).unwrap();
    publisher.retire().unwrap();
    assert!(publisher.publish(pair(13, 6), 0).is_err());
    assert!(reader.read_serial(17).is_err());
    assert!(publisher.invalidate().is_err());

    let (mut publisher, reader) = channel_serial(17);
    publisher.publish(pair(7, 6), 0).unwrap();
    drop(publisher);
    assert!(reader.read_serial(17).is_err());
}

#[test]
fn snapshots_keep_pair_mode_origin_and_capture_cut_together() {
    let (mut publisher, reader) = channel_serial(23);
    let first = pair(3, 6);
    publisher.publish(first, 0).unwrap();
    let cut = reader.read_serial(23).unwrap();
    let second = pair(5, 4);
    publisher.publish(second, 3).unwrap();
    let next = reader.read_serial(23).unwrap();
    assert_eq!(
        (cut.epoch(), cut.capture(), cut.interface_mode()),
        (23, first, 0)
    );
    assert_eq!((next.capture(), next.interface_mode()), (second, 3));
    reader.with_live(&cut, || Ok(())).unwrap(); // newer sample does not alter cut
    let (_other_publisher, other_reader) = channel_serial(23);
    let mut called = false;
    assert!(
        other_reader
            .with_live(&cut, || {
                called = true;
                Ok(())
            })
            .is_err()
    );
    assert!(
        !called,
        "same epoch cannot substitute another publisher's source"
    );
}

#[test]
fn concurrent_publication_reads_never_mix_mode_and_raw_pair() {
    let (mut publisher, reader) = channel_serial(29);
    let normal = pair(17, 6);
    let strategic = pair(31, 4);
    publisher.publish(normal, 0).unwrap();
    let (begin, start) = mpsc::channel();
    let (finish, done) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        start.recv().unwrap();
        for _ in 0..500 {
            publisher.publish(strategic, 3).unwrap();
            publisher.publish(normal, 0).unwrap();
        }
        finish.send(()).unwrap();
        publisher // keep source live for all reader iterations
    });
    begin.send(()).unwrap();
    for _ in 0..1000 {
        let cut = reader.read_serial(29).unwrap();
        assert!(
            (cut.capture(), cut.interface_mode()) == (normal, 0)
                || (cut.capture(), cut.interface_mode()) == (strategic, 3)
        );
    }
    done.recv().unwrap();
    drop(worker.join().unwrap());
    assert!(reader.read_serial(29).is_err());
}

#[test]
fn publication_and_retirement_have_one_ordered_boundary() {
    let (mut publisher, reader) = channel_serial(31);
    publisher.publish(pair(11, 6), 0).unwrap();
    let cut = reader.read_serial(31).unwrap();
    let (started, start) = mpsc::channel();
    let (retired, retirement) = mpsc::channel();
    let mut worker = None;
    reader
        .with_live(&cut, || {
            worker = Some(std::thread::spawn(move || {
                // Observe the real fence mutex before signaling; a pre-lock
                // scheduling signal alone cannot prove publication exclusion.
                let blocked = matches!(
                    publisher.shared.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                );
                started.send(blocked).unwrap();
                publisher.retire().unwrap();
                retired.send(()).unwrap();
            }));
            assert!(
                start.recv().unwrap(),
                "publication closure must hold the retirement mutex"
            );
            assert!(
                matches!(retirement.try_recv(), Err(mpsc::TryRecvError::Empty)),
                "retirement cannot cross a publication already inside its fence"
            );
            Ok(())
        })
        .unwrap();
    worker.unwrap().join().unwrap();
    retirement.recv().unwrap();
    let mut called = false;
    assert!(
        reader
            .with_live(&cut, || {
                called = true;
                Ok(())
            })
            .is_err()
    );
    assert!(!called, "retired cut cannot publish");
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = openmt2001::fixtures::scratch("TEST_SCRATCH");
        fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!(
            "ui-save-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
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
fn native(cash: u32) -> Vec<u8> {
    [0x2000u32, 16, 7, 12, cash, 3, u32::MAX, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

#[test]
fn manual_and_automatic_publication_share_cut_and_retirement_protection() {
    let scratch = Scratch::new();
    let (mut publisher, reader) = channel_serial(37);
    publisher.publish(pair(3, 6), 0).unwrap();
    let cut = reader.read_serial(37).unwrap();
    let target = Target::NewName("Manual".into());
    let manual =
        save_menu::write_target_from_snapshot(&scratch.0, &target, &native(17), &reader, &cut)
            .unwrap();
    let automatic = PreparedAutosave::prepare(&scratch.0)
        .unwrap()
        .publish_from_snapshot(native(17), &reader, &cut)
        .unwrap();
    assert_eq!(fs::read(&manual).unwrap(), fs::read(&automatic).unwrap());
    let stage = PreparedAutosave::prepare(&scratch.0).unwrap();
    publisher.retire().unwrap();
    assert!(
        stage
            .publish_from_snapshot(native(31), &reader, &cut)
            .is_err()
    );
    assert!(
        save_menu::write_target_from_snapshot(
            &scratch.0,
            &Target::ExistingSlot("Manual.sav".into()),
            &native(31),
            &reader,
            &cut
        )
        .is_err()
    );
    assert!(
        save_menu::write_target_from_snapshot(
            &scratch.0,
            &Target::NewName("Rejected".into()),
            &native(31),
            &reader,
            &cut
        )
        .is_err()
    );
    assert_eq!(fs::read(&manual).unwrap(), native(17));
    assert_eq!(fs::read(&automatic).unwrap(), native(17));
    assert_eq!(
        fs::read_dir(manual.parent().unwrap()).unwrap().count(),
        2,
        "failed guarded publications leave only the two previous complete slots"
    );
}
