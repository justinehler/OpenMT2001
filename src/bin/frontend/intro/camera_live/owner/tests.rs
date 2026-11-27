use super::*;
fn put(raw: &mut RawRecord, at: usize, value: f32) {
    raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn bank(mode: u8) -> Bank {
    let mut p = [0u8; 196];
    p[0] = mode;
    p[0x70] = 1;
    p[0x71] = 1;
    p[0x74] = 10;
    for (at, v) in [
        (0x34, 100.),
        (0x38, 120.),
        (0x68, 1.),
        (0x88, 0.),
        (0x90, 1.),
        (0xb0, 80.),
    ] {
        put(&mut p, at, v);
    }
    p[0x24..0x28].copy_from_slice(&0x7fc12345u32.to_le_bytes());
    Bank::from_capture(Capture::from_records([p, [0xcdu8; 196]]))
}
fn input() -> Input {
    Input {
        mode12: Mode12Input {
            build_visible: false,
            enable_translation: true,
            enable_keyboard: true,
            enable_rotation: true,
            enable_zoom: true,
            buttons: [false; 2],
            delta: [0; 2],
            keys: [true, false, false, false],
            sensitivity: 50,
            legacy_yaw: false,
            invert_vertical: false,
            low_height: false,
        },
        mode13: Mode13Input {
            enable_translation: true,
            enable_keyboard: true,
            enable_zoom: true,
            buttons: [false; 2],
            delta: [0; 2],
            keys: [true, false, false, false],
            sensitivity: 50,
            legacy_yaw: false,
            invert_vertical: false,
            low_height: false,
        },
    }
}
#[test]
fn mode6_ignores_mode12_input_but_runs_common_and_sample_from_actual_ratio() {
    let mut owner = Owner::new(bank(6), 7);
    let before = owner.capture();
    let input = input();
    owner.central(input).unwrap();
    assert_eq!(owner.current().eye[0], 10f32.to_bits());
    assert_eq!(owner.sampled().eye[0], 0);
    let sample = owner.sample(2.).unwrap();
    assert_eq!(sample.eye[0], 28f32.to_bits());
    assert_eq!(owner.current().eye[0], 10f32.to_bits());
    assert_eq!(owner.capture().records()[1], before.records()[1]);
    assert_eq!(
        &owner.capture().records()[0][0xb4..0xc0],
        &before.records()[0][0xb4..0xc0]
    );
    assert_eq!(owner.kind_dirty(), 7);
}
#[test]
fn cold_strategic_first_action_restores_complete_secondary_without_modern_tuple() {
    let mut owner = Owner::new(bank(6), 0);
    owner.central(input()).unwrap();
    owner.sample(0.5).unwrap();
    let ordinary = owner.capture();
    assert_eq!(
        owner.strategic_after_ui_clear(),
        StrategicTransition::Entered
    );
    assert_eq!(owner.kind(), 4);
    owner.central(input()).unwrap();
    owner.sample(0.5).unwrap();
    let save = owner.capture();
    assert_eq!(save.records()[1], ordinary.records()[0]);
    let mut loaded = Owner::new(Bank::from_capture(save), 9);
    assert_eq!(
        loaded.strategic_after_ui_clear(),
        StrategicTransition::Returned
    );
    assert_eq!(loaded.kind(), 6);
    assert_eq!(loaded.capture().records()[0], ordinary.records()[0]);
    assert_eq!(loaded.capture().records()[1], ordinary.records()[0]);
    assert_eq!(loaded.kind_dirty(), 9);
}
#[test]
fn saved_capture_does_not_change_with_later_common_sample_or_restore() {
    let mut owner = Owner::new(bank(6), 0);
    owner.central(input()).unwrap();
    let captured = owner.capture();
    let old_sample = owner.sampled();
    owner.sample(0.5).unwrap();
    owner.central(input()).unwrap();
    assert_ne!(owner.capture(), captured);
    assert_eq!(
        captured.records()[0][0x4c..0x58],
        old_sample
            .eye
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    owner.restore(captured);
    assert_eq!(owner.capture(), captured);
    assert_eq!(owner.kind_dirty(), 0);
}
#[test]
fn invalid_callback_input_rolls_back_raw_primary_and_secondary() {
    let mut owner = Owner::new(bank(12), 4);
    let before = owner.capture();
    let mut i = input();
    i.mode12.buttons = [true, true];
    i.mode12.delta = [i32::MIN, 0];
    assert!(owner.central(i).is_err());
    assert_eq!(owner.capture(), before);
    assert_eq!(owner.kind_dirty(), 4);
}
#[test]
fn split_strategic_stores_preserve_real_ui_pre_and_post_boundaries() {
    let mut owner = Owner::new(bank(6), 0);
    let before = owner.capture();
    owner.copy_primary_to_secondary();
    assert_eq!(owner.kind(), 6, "mode3 UI prelude sees the ordinary camera");
    owner.target([1f32, 2., 3.].map(f32::to_bits)); // bounded reentrant prelude witness
    assert_eq!(owner.capture().records()[1], before.records()[0]);
    owner.enter_strategic_fields().unwrap();
    assert_eq!(owner.kind(), 4);
    assert_eq!(
        &owner.capture().records()[0][4..0x1c],
        &before.records()[0][4..0x1c],
        "entry does not run common"
    );
    owner.set_kind(13); // a postlude's completed write must not be overwritten
    assert_eq!(owner.kind(), 13);
    assert_eq!(owner.capture().records()[1], before.records()[0]);
    owner.copy_secondary_to_primary();
    assert_eq!(owner.capture().records()[0], before.records()[0]);
    assert_eq!(
        owner.kind_dirty(),
        1,
        "return copy does not run the kind setter"
    );
}
