use super::*;

fn patterned(seed: u8, kind: u32) -> RawRecord {
    let mut raw = std::array::from_fn(|i| (i as u8).wrapping_mul(73).wrapping_add(seed));
    put(&mut raw, 0, kind);
    raw
}
fn bank(primary: RawRecord, secondary: RawRecord) -> Bank {
    Bank::from_capture(Capture::from_records([primary, secondary]))
}
fn preserved_outside(before: &RawRecord, after: &RawRecord, writes: &[(usize, usize)]) {
    for i in 0..RECORD_BYTES {
        if !writes.iter().any(|&(start, end)| (start..end).contains(&i)) {
            assert_eq!(before[i], after[i], "unexpected write at {i:#x}");
        }
    }
}

#[test]
fn arbitrary_records_capture_by_value_and_restore_without_decoding() {
    for kind in [0, 1, 4, 6, 12, 13, 0x80000000, u32::MAX] {
        let p = patterned(17, kind);
        let s = patterned(98, kind ^ 0x80000001);
        let mut owner = bank(p, s);
        let capture = owner.capture();
        assert_eq!(capture.into_records(), [p, s]);
        owner.restore_primary(patterned(23, 2));
        assert_eq!(capture.records(), &[p, s], "capture must not alias owner");
        assert_eq!(owner.secondary(), &s);
        owner.restore_secondary(patterned(44, 5));
        owner.restore(capture);
        assert_eq!(owner.capture(), capture);
        assert_eq!(owner.primary_kind(), kind);
    }
}

#[test]
fn raw_kind_setter_preserves_history_and_marks_only_external_dirty() {
    let p = patterned(71, 13);
    let s = patterned(99, u32::MAX);
    let mut owner = bank(p, s);
    let mut dirty = 0xa7;
    owner.set_kind(0x80000013, &mut dirty);
    assert_eq!(dirty, 1);
    assert_eq!(owner.primary_kind(), 0x80000013);
    assert_eq!(
        &owner.primary()[0x64..0x70],
        &[0, 0, 0, 0, 0, 0, 128, 63, 0, 0, 0, 0]
    );
    assert_eq!(word(owner.primary(), 0x78), 0);
    preserved_outside(&p, owner.primary(), &[(0, 4), (0x64, 0x70), (0x78, 0x7c)]);
    assert_eq!(owner.secondary(), &s);
    let capture = owner.capture();
    dirty = 0x55;
    owner.restore(capture);
    assert_eq!(
        dirty, 0x55,
        "save records cannot restore an external dirty byte"
    );
}

#[test]
fn target_setter_moves_coordinate_bits_but_preserves_current_sample_and_caches() {
    let p = patterned(38, 0xffffffef);
    let s = patterned(6, 13);
    let mut owner = bank(p, s);
    // Includes a NaN payload, negative zero and infinity: all are MOV-only here.
    owner.set_target_words([0x7fc12345, 0x80000000, 0x7f800000]);
    assert_eq!(word(owner.primary(), 0x34), 0x7fc12345);
    assert_eq!(word(owner.primary(), 0x38), 0x80000000);
    assert_eq!(word(owner.primary(), 0x3c), 0x7f800000);
    assert_eq!(&owner.primary()[0x40..0x4c], &[0; 12]);
    assert_eq!(&owner.primary()[0x70..0x73], &[1, 1, 0]);
    assert_eq!(word(owner.primary(), 0x74), 10);
    assert_eq!(word(owner.primary(), 0x7c), 0x3fc90fdb);
    assert_eq!(&owner.primary()[0x80..0x88], &[0; 8]);
    preserved_outside(
        &p,
        owner.primary(),
        &[(0x34, 0x4c), (0x70, 0x73), (0x74, 0x78), (0x7c, 0x88)],
    );
    assert_eq!(owner.secondary(), &s);
}

#[test]
fn immediate_setter_bias_flags_and_mutation_mask_are_exact() {
    let p = patterned(55, 13);
    let s = patterned(86, 1);
    let mut owner = bank(p, s);
    owner
        .set_immediate_words([0x7fc12345, 0x80000000, 0xc1f00000])
        .unwrap();
    let expected = [
        0x7fc12345, 0x80000000, 0xc1f0cccd, 0x7fc12345, 0, 0xc1f00000,
    ];
    for (i, value) in expected.into_iter().enumerate() {
        assert_eq!(word(owner.primary(), 0x34 + i * 4), value);
    }
    assert_eq!(&owner.primary()[0x70..0x73], &[0; 3]);
    assert_eq!(word(owner.primary(), 0x74), 10);
    preserved_outside(
        &p,
        owner.primary(),
        &[(0x34, 0x4c), (0x70, 0x73), (0x74, 0x78)],
    );
    assert_eq!(owner.secondary(), &s);
    let before = owner.capture();
    for nonfinite in [0x7fc00001, 0x7f800000, 0xff800000] {
        assert!(owner.set_immediate_words([0, 0, nonfinite]).is_err());
        assert_eq!(
            owner.capture(),
            before,
            "finite arithmetic rejection must be atomic"
        );
    }
    owner.set_immediate_words([0, 0, 0x80000000]).unwrap();
    assert_eq!(word(owner.primary(), 0x3c), 0xbdcccccd);
    assert_eq!(word(owner.primary(), 0x48), 0x80000000);
}

#[test]
fn strategic_capture_activity_return_and_recapture_keep_secondary_independent() {
    // A mode unsupported by the numeric core and distinctive restored up/threshold.
    let p = patterned(87, 13);
    let s = patterned(198, 0xffffffef);
    let mut owner = bank(p, s);
    let mut dirty = 0;
    assert_eq!(
        owner.strategic_after_ui_clear(&mut dirty),
        StrategicTransition::Entered
    );
    assert_eq!(owner.secondary(), &p, "capture all bytes before setters");
    assert_eq!(dirty, 1);
    assert_eq!(owner.primary_kind(), 4);
    let expected = [
        0xc35c0000, 0x44834000, 0xc1f0cccd, 0xc35c0000, 0, 0xc1f00000,
    ];
    for (i, value) in expected.into_iter().enumerate() {
        assert_eq!(word(owner.primary(), 0x34 + i * 4), value);
    }
    preserved_outside(
        &p,
        owner.primary(),
        &[(0, 4), (0x34, 0x4c), (0x64, 0x73), (0x74, 0x7c)],
    );
    // No common or sample ran on entry, even with dirty flags cleared.
    assert_eq!(&owner.primary()[4..0x34], &p[4..0x34]);
    assert_eq!(&owner.primary()[0x4c..0x64], &p[0x4c..0x64]);
    let captured = owner.capture();
    owner
        .try_update_primary(|raw| -> Result<(), ()> {
            raw[4..0x64].fill(0xaa);
            Ok(())
        })
        .unwrap();
    assert_eq!(owner.secondary(), &p);
    assert_eq!(captured.records()[1], p);
    // Return is a raw copy, with no mode setter touching restored up/threshold.
    dirty = 0x5a;
    assert_eq!(
        owner.strategic_after_ui_clear(&mut dirty),
        StrategicTransition::Returned
    );
    assert_eq!(owner.capture().into_records(), [p, p]);
    assert_eq!(dirty, 0x5a);
    let moved = patterned(5, 6);
    owner.restore_primary(moved);
    assert_eq!(owner.secondary(), &p);
    owner.strategic_after_ui_clear(&mut dirty);
    assert_eq!(
        owner.secondary(),
        &moved,
        "next entry captures the then-current owner"
    );
}

#[test]
fn cold_loaded_kind4_returns_even_with_arbitrary_secondary_and_no_modern_tuple() {
    let p = patterned(11, 4);
    let s = patterned(23, u32::MAX);
    let mut owner = bank(p, s);
    let mut dirty = 0xb6;
    assert_eq!(
        owner.strategic_after_ui_clear(&mut dirty),
        StrategicTransition::Returned
    );
    assert_eq!(owner.capture().into_records(), [s, s]);
    assert_eq!(dirty, 0xb6);
}

#[test]
fn process_initialization_is_partial_and_cold_zero_is_only_a_starting_allocation() {
    let mut cold = Bank::loader_zero_storage();
    assert_eq!(cold.capture().into_records(), [[0; RECORD_BYTES]; 2]);
    let mut dirty = 0x67;
    let effects = cold.initialize_process(50, 2, &mut dirty).unwrap();
    let mut expected = [0; RECORD_BYTES];
    // Independent conditional-cold table: all other words remain loader zero.
    for (at, value) in [
        (0x38, 0x40c00000),
        (0x3c, 0xc1200000),
        (0x68, 0x3f800000),
        (0x74, 10),
        (0x7c, 0x3fc90fdb),
        (0x9c, 50),
        (0xa0, 0x42c80000),
        (0xa4, 0x43480000),
        (0xa8, 2),
    ] {
        put(&mut expected, at, value);
    }
    expected[0x70..0x73].copy_from_slice(&[1, 1, 0]);
    assert_eq!(cold.primary(), &expected);
    assert_eq!(cold.secondary(), &[0; RECORD_BYTES]);
    assert_eq!(dirty, 0);
    assert_eq!(effects.cleared_callback_words, [0; 24]);
    assert_eq!(effects.scale_8_over_width, 0x3e23d70a);
    assert_eq!(effects.scale_80_over_width, 0x3fcccccd);

    let p = patterned(81, 13);
    let s = patterned(91, 4);
    let mut warm = bank(p, s);
    warm.initialize_process(50, 0x81234567, &mut dirty).unwrap();
    preserved_outside(
        &p,
        warm.primary(),
        &[
            (0, 4),
            (0x34, 0x4c),
            (0x64, 0x73),
            (0x74, 0x88),
            (0x9c, 0xac),
        ],
    );
    assert_eq!(word(warm.primary(), 0xa8), 0x81234567);
    assert_eq!(warm.secondary(), &s);
    assert_ne!(
        warm.primary(),
        cold.primary(),
        "initialization preserves incoming history"
    );
    let before = warm.capture();
    dirty = 0x92;
    assert!(warm.initialize_process(0, 2, &mut dirty).is_err());
    assert_eq!(warm.capture(), before);
    assert_eq!(dirty, 0x92);
}

#[test]
fn warm_world_reset_preserves_flags_current_sample_opaque_and_secondary() {
    let p = patterned(81, 12);
    let s = patterned(47, 13);
    let mut owner = bank(p, s);
    owner.reset_yaw();
    assert_eq!(word(owner.primary(), 0x7c), 0xbfc90fdb);
    preserved_outside(&p, owner.primary(), &[(0x7c, 0x80)]);
    // An intervening producer can write yaw; final stores must not replay it.
    owner
        .try_update_primary(|raw| -> Result<(), ()> {
            put(raw, 0x7c, 0x3e800000);
            Ok(())
        })
        .unwrap();
    let at_final = *owner.primary();
    let mut dirty = 0;
    owner.set_world_reset_final_fields(&mut dirty);
    assert_eq!(owner.primary_kind(), 6);
    assert_eq!(dirty, 1);
    let expected = [
        0x41d00000, 0x44128000, 0xc42dc000, 0x41d00000, 0x43e78000, 0xc414c000,
    ];
    for (i, value) in expected.into_iter().enumerate() {
        assert_eq!(word(owner.primary(), 0x34 + i * 4), value);
    }
    preserved_outside(
        &at_final,
        owner.primary(),
        &[(0, 4), (0x34, 0x4c), (0x64, 0x70), (0x78, 0x7c)],
    );
    assert_eq!(word(owner.primary(), 0x7c), 0x3e800000);
    assert_eq!(&owner.primary()[0x70..0x78], &p[0x70..0x78]);
    assert_eq!(owner.secondary(), &s);
    let mut cold = Bank::loader_zero_storage();
    cold.initialize_process(50, 0, &mut dirty).unwrap();
    cold.reset_yaw();
    cold.set_world_reset_final_fields(&mut dirty);
    assert_ne!(
        owner.capture(),
        cold.capture(),
        "warm reset must not recreate a cold template"
    );
}

#[test]
fn producer_seam_commits_one_authority_and_discards_failed_numeric_mutations() {
    let mut p = [0; RECORD_BYTES];
    put(&mut p, 0, 6);
    put(&mut p, 0x74, 10);
    put(&mut p, 0x34, 0x41200000);
    let s = patterned(43, 13);
    let mut owner = bank(p, s);
    owner
        .try_update_primary(|raw| -> Result<(), &'static str> {
            let mut numeric = crate::camera::primary::Primary::decode(*raw)?;
            numeric.common()?;
            numeric.sample(0.5)?;
            *raw = *numeric.raw();
            Ok(())
        })
        .unwrap();
    assert_eq!(word(owner.primary(), 4), 0x41200000);
    assert_eq!(word(owner.primary(), 0x4c), 0x41200000);
    assert_eq!(owner.secondary(), &s);
    let before = owner.capture();
    let failed = owner.try_update_primary(|raw| -> Result<(), &'static str> {
        raw.fill(0xee);
        Err("injected producer failure")
    });
    assert_eq!(failed, Err("injected producer failure"));
    assert_eq!(owner.capture(), before);
    let mut dirty = 0;
    owner.set_kind(5, &mut dirty);
    assert!(
        owner
            .try_update_primary(|raw| { crate::camera::primary::Primary::decode(*raw).map(|_| ()) })
            .is_err()
    );
    // Unknown mode remains capturable, not a blanket save refusal.
    assert_eq!(word(&owner.capture().records()[0], 0), 5);
}

#[test]
fn initialization_divides_by_the_signed_width_word() {
    let mut owner = Bank::loader_zero_storage();
    let mut dirty = 1;
    let effects = owner.initialize_process(-50, 0, &mut dirty).unwrap();
    assert_eq!(word(owner.primary(), 0x9c), (-50_i32) as u32);
    assert_eq!(word(owner.primary(), 0xa4), 0xc3480000);
    assert_eq!(effects.scale_8_over_width, 0xbe23d70a);
    assert_eq!(effects.scale_80_over_width, 0xbfcccccd);
    assert_eq!(dirty, 0);
}

#[test]
#[ignore = "reads SHA-pinned supplied saves through OPENMT2001_SAVE_ONE/TWO; needs sha256sum"]
fn supplied_bank_capture_entry_and_return_match_static_instruction_expectations() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    fn hash(bytes: &[u8], expected: &str) {
        let mut child = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap(),
            expected
        );
    }
    for (env, save_hash, primary_hash, entry_hash) in [
        (
            "OPENMT2001_SAVE_ONE",
            "2c3bfdbbdaeb2e12e08752e9f3ed05d6e5c4dc60aa48208113f46a84b329dded",
            "6db51fca309761af0e5e9c43dfa40ed55c8c72a05fe15f10810629bbf51c918a",
            "1fdd9c31358198736bfa913866a851385d59fd230487b918ebe179c8d20e603b",
        ),
        (
            "OPENMT2001_SAVE_TWO",
            "4cf97da570b3dc1ab19508fa983b2ab12d418b0e08ea750f38640fcea7121931",
            "820260e951da3f82ef09b9a8a020855624b21ba80f65f33f8995d272bf4691b7",
            "18358b169e9fad8eaf3abbc4cc1b7718e25d28dfaccfdf7b5203cee5d61b1c8d",
        ),
    ] {
        // Read-only runtime inputs. No include_bytes!, source payload or writes.
        let path = std::env::var_os(env).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        hash(&bytes, save_hash);
        let save = crate::save::SaveFile::parse(&bytes).unwrap();
        let [p, s] = save.cameras().unwrap();
        hash(p, primary_hash);
        hash(
            s,
            "6db51fca309761af0e5e9c43dfa40ed55c8c72a05fe15f10810629bbf51c918a",
        );
        let mut owner = bank(*p, *s);
        let no_toggle = owner.capture();
        assert_eq!(no_toggle.into_records(), [*p, *s]);
        let mut dirty = 0x5a;
        owner.strategic_after_ui_clear(&mut dirty);
        hash(owner.primary(), entry_hash);
        hash(owner.secondary(), primary_hash);
        assert_eq!(owner.secondary(), p);
        assert_eq!(dirty, 1);
        preserved_outside(
            p,
            owner.primary(),
            &[(0, 4), (0x34, 0x4c), (0x64, 0x73), (0x74, 0x7c)],
        );
        // Return before any common/sample or external UI callback, as specified.
        owner.strategic_after_ui_clear(&mut dirty);
        assert_eq!(owner.capture().into_records(), [*p, *p]);
        assert_eq!(dirty, 1);
        assert_eq!(no_toggle.into_records(), [*p, *s]);
        assert_eq!(
            std::fs::read(path).unwrap(),
            bytes,
            "supplied input stays unchanged"
        );
    }
}
