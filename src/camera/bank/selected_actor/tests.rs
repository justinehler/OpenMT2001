use super::*;
use crate::camera::{
    bank::Capture,
    primary::{Mode13Input, Primary},
};
use crate::x87::portable_trig::PortableTrig;

fn owner(yaw: f32) -> Bank {
    let mut primary = [0xabu8; 196];
    put(&mut primary, 0xa8, 50);
    put(&mut primary, 0x7c, yaw.to_bits());
    put(&mut primary, 0x98, 0);
    Bank::from_capture(Capture::from_records([primary, [0xcdu8; 196]]))
}
fn input() -> SelectedActorInput {
    SelectedActorInput {
        selected_xz_words: [0, 0],
        actor_translation: [120., 3., -180.],
        actor_model_center: [1., 2., 3.],
        building_kind: 0,
        entrance: [-10., 6., -30.],
        auto_rotate: true,
    }
}

#[test]
fn selected_actor_setup_preserves_other_history_and_secondary() {
    let mut bank = owner(0.2);
    let before = bank.capture();
    bank.initialize_selected_actor(input(), &mut PortableTrig)
        .unwrap();
    let writes = [
        0x38..0x3c,
        0x70..0x78,
        0x7c..0x80,
        0x88..0x8c,
        0x90..0x94,
        0x98..0x9c,
        0xb0..0xc0,
    ];
    for at in 0..196 {
        if !writes.iter().any(|r| r.contains(&at)) {
            assert_eq!(bank.primary()[at], before.records()[0][at], "byte{at:x}");
        }
    }
    assert_eq!(bank.secondary(), &before.records()[1]);
    assert_eq!(&bank.primary()[0x70..0x73], &[1; 3]);
    assert_eq!(super::word(bank.primary(), 0x74), 20);
}

#[test]
fn special_parameter_does_not_consume_unused_actor_fields() {
    let mut bank = owner(-0.);
    bank.try_update_primary(|r| {
        put(r, 0xa8, 2);
        Ok::<_, ()>(())
    })
    .unwrap();
    let mut i = input();
    i.selected_xz_words = [0x7fc12345, 0x80000000];
    i.actor_translation = [f32::NAN; 3];
    i.entrance = [f32::NAN; 3];
    i.actor_model_center = [f32::NAN; 3];
    let before = bank.capture();
    bank.initialize_selected_actor(i, &mut PortableTrig)
        .unwrap();
    assert_eq!(super::word(bank.primary(), 0xb4), 0x7fc12345);
    assert_eq!(super::word(bank.primary(), 0xb8), 0);
    assert_eq!(super::word(bank.primary(), 0xbc), 0x80000000);
    for at in [0x38, 0x98, 0xb0] {
        assert_eq!(
            super::word(bank.primary(), at),
            word(&before.records()[0], at)
        );
    }
    assert_eq!(super::word(bank.primary(), 0x88), 0x80000000);
    assert_eq!(super::word(bank.primary(), 0x90), 1f32.to_bits());
}

#[test]
fn failed_initializer_rolls_back_both_records() {
    struct Fail;
    impl AngularTrig for Fail {
        fn sine(&mut self, _: Sum) -> Result<Sum> {
            Sum::from_f32(0.)
        }
        fn cosine(&mut self, _: Sum) -> Result<Sum> {
            Err("injected cosine failure")
        }
    }
    let mut bank = owner(0.2);
    let before = bank.capture();
    assert!(bank.initialize_selected_actor(input(), &mut Fail).is_err());
    assert_eq!(bank.capture(), before);
    let mut i = input();
    i.actor_translation[0] = f32::NAN;
    assert!(
        bank.initialize_selected_actor(i, &mut PortableTrig)
            .is_err()
    );
    assert_eq!(bank.capture(), before);
}

#[test]
fn retained_across_signed_abs_and_unspilled_yaw_delta_boundaries() {
    // Across remains extended, but forward spills to binary32. A premature
    // across spill loses the selected X; a retained forward keeps selected Z.
    let mut bank = owner(0.2);
    let mut i = input();
    i.building_kind = 2;
    i.actor_translation = [33554432., f32::NAN, 33554432.];
    i.selected_xz_words = [1f32.to_bits(); 2];
    i.actor_model_center = [f32::NAN; 3];
    bank.initialize_selected_actor(i, &mut PortableTrig)
        .unwrap();
    assert_eq!(word(bank.primary(), 0xb4), 1f32.to_bits());
    assert_eq!(word(bank.primary(), 0xbc), 0);
    assert_eq!(word(bank.primary(), 0x98), 0xbfc90fdb);

    // Original low-word FTOL and signed wrapped abs make abs(INT_MIN) negative.
    let mut bank = owner(0.2);
    let mut i = input();
    i.entrance = [-2147483648., f32::NAN, -1.];
    bank.initialize_selected_actor(i, &mut PortableTrig)
        .unwrap();
    assert_eq!(word(bank.primary(), 0x98), 0xbfc90fdb);

    // Retained yaw-goal is above pi; spilling it first would round back to pi.
    let mut bank = owner(f32::from_bits(0x40490fdb));
    bank.try_update_primary(|r| {
        put(r, 0x98, (-2f32.powi(-24)).to_bits());
        Ok::<_, ()>(())
    })
    .unwrap();
    let mut i = input();
    i.entrance = [0., f32::NAN, 0.];
    bank.initialize_selected_actor(i, &mut PortableTrig)
        .unwrap();
    assert_eq!(word(bank.primary(), 0x7c), 0x40490fdb);
    assert_eq!(word(bank.primary(), 0x98), 0x40c90fdb);
}

#[test]
#[ignore = "requires retained original PC53 building setup and mode13 sequences"]
fn retained_original_building_setups_and_callback_sequences() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILDING_CAMERA")).unwrap();
    assert_eq!(&bytes[..8], b"MTBLDC01");
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let cases = word(8) as usize;
    let ticks = word(12) as usize;
    assert_eq!((cases, ticks), (36, 60));
    let mut at = 16;
    for case in 0..cases {
        let x = f32::from_bits(word(at));
        let z = f32::from_bits(word(at + 4));
        let angle = f32::from_bits(word(at + 8));
        let kind = word(at + 12);
        at += 16;
        let mut bank = Bank::loader_zero_storage();
        bank.try_update_primary(|r| {
            put(r, 0xa8, 50);
            put(r, 0x7c, angle.to_bits());
            Ok::<_, ()>(())
        })
        .unwrap();
        let mut i = input();
        i.actor_translation = [120., 0., -180.];
        i.selected_xz_words = [(120. - x).to_bits(), (-180. - z).to_bits()];
        i.building_kind = kind;
        i.entrance = [-x, 6., -z];
        bank.initialize_selected_actor(i, &mut PortableTrig)
            .unwrap();
        let fields = [0xb4, 0xb8, 0xbc, 0x7c, 0x98, 0x38, 0xb0];
        let actual: Vec<u8> = fields
            .into_iter()
            .flat_map(|p| super::word(bank.primary(), p).to_le_bytes())
            .collect();
        assert_eq!(actual, &bytes[at..at + 28], "setup{case}");
        assert_eq!(&bank.primary()[0x70..0x73], &bytes[at + 28..at + 31]);
        assert_eq!(super::word(bank.primary(), 0x74), word(at + 31));
        at += 35;
        bank.set_kind(13, &mut 0);
        for tick in 0..ticks {
            let keys = std::array::from_fn(|j| tick % 11 == j);
            let delta = if tick >= 40 {
                [[-20, 15], [14, -10], [0, 0]][tick % 3]
            } else {
                [0, 0]
            };
            let i = Mode13Input {
                enable_translation: true,
                enable_keyboard: true,
                enable_zoom: true,
                buttons: [false, delta != [0; 2]],
                delta,
                keys,
                sensitivity: 50,
                legacy_yaw: false,
                invert_vertical: false,
                low_height: false,
            };
            bank.try_update_primary(|r| {
                let mut p = Primary::decode(*r)?;
                p.mode13(i, &mut PortableTrig)?;
                p.common_with_trig(&mut PortableTrig)?;
                *r = *p.raw();
                Ok::<_, &'static str>(())
            })
            .unwrap();
            let actual: Vec<u8> = fields
                .into_iter()
                .chain([0x34, 0x38, 0x3c, 0x40, 0x44, 0x48])
                .flat_map(|p| super::word(bank.primary(), p).to_le_bytes())
                .collect();
            assert_eq!(actual, &bytes[at..at + 52], "case{case} tick{tick}");
            at += 52;
        }
    }
    assert_eq!(at, bytes.len());
}

#[test]
#[ignore = "requires retained original AutoRotate-off setup cases"]
fn retained_original_auto_rotate_off_setup() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_AUTO_ROTATE_CAMERA")).unwrap();
    assert_eq!(&bytes[..8], b"MTAUTO01");
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(bytes.len(), 12 + word(8) as usize * 56);
    for case in 0..word(8) as usize {
        let at = 12 + case * 56;
        let f = |j: usize| f32::from_bits(word(at + j * 4));
        let mut bank = owner(f(6));
        let mut i = input();
        i.auto_rotate = false;
        i.actor_translation = std::array::from_fn(f);
        i.actor_model_center = std::array::from_fn(|j| f(j + 3));
        bank.initialize_selected_actor(i, &mut PortableTrig)
            .unwrap();
        let actual: Vec<u8> = [0xb4, 0xb8, 0xbc, 0x7c, 0x98, 0x38, 0xb0]
            .into_iter()
            .flat_map(|p| super::word(bank.primary(), p).to_le_bytes())
            .collect();
        assert_eq!(
            actual,
            &bytes[at + 28..at + 56],
            "AutoRotate-off case{case}"
        );
    }
}
