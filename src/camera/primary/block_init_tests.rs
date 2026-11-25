use super::*;
use crate::camera::bank::{Bank, Capture, RawRecord};

fn put(raw: &mut RawRecord, at: usize, value: u32) {
    raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn word(raw: &RawRecord, at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}
fn fixture() -> BlockInitialization {
    BlockInitialization {
        actor_translation: [1.0f32, 2.0, 3.0].map(f32::to_bits),
        model_offsets: [0.5f32, 1.0, -2.0].map(f32::to_bits),
        height_word: 0x42f00000,
        radius_word: 0x42a00000,
    }
}
fn raw(yaw: u32) -> RawRecord {
    let mut raw = std::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(9));
    // Unsupported kind and unrelated nonfinite data MUST remain raw transports.
    put(&mut raw, 0, 0xdeadbeef);
    put(&mut raw, 4, 0x7fc12345);
    put(&mut raw, 0x7c, yaw);
    raw
}
struct Witness {
    yaw: u32,
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
}
impl Witness {
    fn new(yaw: u32) -> Self {
        Self {
            yaw,
            calls: Vec::new(),
            fail: None,
        }
    }
    fn value(&mut self, name: &'static str, input: Sum, output: f32) -> Result<Sum> {
        assert_eq!(
            input.compare(Sum::from_f32(f32::from_bits(self.yaw))?),
            Ordering::Equal
        );
        assert_eq!(input.to_f32().to_bits(), self.yaw);
        self.calls.push(name);
        if self.fail == Some(name) {
            return Err("synthetic provider failure");
        }
        Sum::from_f32(output)
    }
}
impl AngularTrig for Witness {
    fn sine(&mut self, yaw: Sum) -> Result<Sum> {
        self.value("sine", yaw, 0.25)
    }
    fn cosine(&mut self, yaw: Sum) -> Result<Sum> {
        self.value("cosine", yaw, -0.75)
    }
}
fn commit(bank: &mut Bank, input: BlockInitialization, trig: &mut impl AngularTrig) -> Result<()> {
    bank.try_update_primary(|raw| {
        *raw = Primary::initialize_block_words(*raw, input, trig)?;
        Ok(())
    })
}

#[test]
fn by_value_initializer_preserves_both_raw_records_outside_exact_mask() {
    let primary = raw(0x3f000000);
    let secondary = [0xe7; 196];
    let mut bank = Bank::from_capture(Capture::from_records([primary, secondary]));
    let mut trig = Witness::new(0x3f000000);
    commit(&mut bank, fixture(), &mut trig).unwrap();
    assert_eq!(trig.calls, ["sine", "cosine"]);
    let mut expected = primary;
    for (at, bits) in [
        (0x38, 0x42f00000),
        (0x74, 20),
        (0x88, 0x3e800000),
        (0x90, 0xbf400000),
        (0xb0, 0x42a00000),
        (0xb4, 0x3fc00000),
        (0xb8, 0x40400000),
        (0xbc, 0x3f800000),
    ] {
        put(&mut expected, at, bits);
    }
    expected[0x70..0x73].copy_from_slice(&[1, 1, 0]);
    assert_eq!(bank.capture(), Capture::from_records([expected, secondary]));
    // Includes kind, current/sample, all five other desired coordinates,
    // up/threshold/yaw/final-yaw/parameter/cache padding/opaque byte 0x73.
    assert_eq!(word(bank.primary(), 0), 0xdeadbeef);
}

#[test]
fn center_nearest_even_and_signed_zero_keep_native_addition_and_store_domain() {
    let mut input = fixture();
    input.actor_translation = [0x3f800000, 0x3f800001, 0x80000000];
    input.model_offsets = [0x33800000, 0x33800000, 0x80000000];
    let result = Primary::initialize_block_words(raw(0), input, &mut Witness::new(0)).unwrap();
    assert_eq!(
        [
            word(&result, 0xb4),
            word(&result, 0xb8),
            word(&result, 0xbc)
        ],
        [0x3f800000, 0x3f800002, 0x80000000]
    );
}

#[test]
fn raw_height_and_radius_words_are_not_arithmetic_operands() {
    let mut input = fixture();
    input.height_word = 0x7fc12345;
    input.radius_word = 0xff800000;
    let result = Primary::initialize_block_words(raw(0), input, &mut Witness::new(0)).unwrap();
    assert_eq!(word(&result, 0x38), input.height_word);
    assert_eq!(word(&result, 0xb0), input.radius_word);
    assert_eq!(word(&result, 0x7c), 0);
}

#[test]
fn consumed_nonfinite_fields_and_overflow_leave_bank_unchanged() {
    for source in 0..7 {
        for invalid in [0x7f800000, 0xff800000, 0x7fc12345] {
            let yaw = if source == 6 { invalid } else { 0 };
            let before = Capture::from_records([raw(yaw), [0x79; 196]]);
            let mut bank = Bank::from_capture(before);
            let mut input = fixture();
            if source < 3 {
                input.actor_translation[source] = invalid;
            } else if source < 6 {
                input.model_offsets[source - 3] = invalid;
            }
            let mut trig = Witness::new(0);
            assert!(commit(&mut bank, input, &mut trig).is_err());
            assert_eq!(bank.capture(), before);
            assert!(trig.calls.is_empty());
        }
    }
    for axis in 0..3 {
        let before = Capture::from_records([raw(0), [0x79; 196]]);
        let mut bank = Bank::from_capture(before);
        let mut input = fixture();
        input.actor_translation[axis] = 0x7f7fffff;
        input.model_offsets[axis] = 0x7f7fffff;
        let mut trig = Witness::new(0);
        assert!(commit(&mut bank, input, &mut trig).is_err());
        assert_eq!(bank.capture(), before);
        assert!(trig.calls.is_empty());
    }
}

#[test]
fn trig_failure_and_nonfinite_cache_store_are_atomic() {
    for failure in ["sine", "cosine"] {
        let before = Capture::from_records([raw(0), [0x19; 196]]);
        let mut bank = Bank::from_capture(before);
        let mut trig = Witness::new(0);
        trig.fail = Some(failure);
        assert!(commit(&mut bank, fixture(), &mut trig).is_err());
        assert_eq!(bank.capture(), before);
        assert_eq!(
            trig.calls,
            if failure == "sine" {
                vec!["sine"]
            } else {
                vec!["sine", "cosine"]
            }
        );
    }
    struct Overflow(bool);
    impl AngularTrig for Overflow {
        fn sine(&mut self, _: Sum) -> Result<Sum> {
            Sum::from_f64(if self.0 { f64::MAX } else { 0.0 })
        }
        fn cosine(&mut self, _: Sum) -> Result<Sum> {
            Sum::from_f64(f64::MAX)
        }
    }
    for fail_sine in [true, false] {
        let before = Capture::from_records([raw(0), [0x19; 196]]);
        let mut bank = Bank::from_capture(before);
        assert!(commit(&mut bank, fixture(), &mut Overflow(fail_sine)).is_err());
        assert_eq!(bank.capture(), before);
    }
}

#[test]
fn caller_owns_yaw_reset_then_initializer_then_kind_twelve() {
    let mut bank = Bank::from_capture(Capture::from_records([raw(0x3f000000), [0x91; 196]]));
    let mut dirty = 0x55;
    // Represents ONLY the camera boundaries of 004c3c24/2e/35; the real
    // caller's earlier Lua/UI callbacks must already have completed.
    bank.reset_yaw();
    commit(&mut bank, fixture(), &mut Witness::new(0xbfc90fdb)).unwrap();
    assert_eq!(bank.primary_kind(), 0xdeadbeef);
    assert_eq!(dirty, 0x55); // initializer has no reference to this external byte
    let initialized = *bank.primary();
    bank.set_kind(12, &mut dirty);
    let mut expected = initialized;
    for (at, bits) in [(0, 12), (0x64, 0), (0x68, 0x3f800000), (0x6c, 0), (0x78, 0)] {
        put(&mut expected, at, bits);
    }
    assert_eq!(bank.primary(), &expected);
    assert_eq!(bank.secondary(), &[0x91; 196]);
    assert_eq!(dirty, 1);
}

/// Retained 512-byte expected image from the historical original/source
/// comparison, PC64 row (not a new original execution). See TEST-SPEC.md.
#[test]
#[ignore = "retained original PC64 expected projection; requires Linux sha256sum"]
fn retained_pick_animation_pc64_expected_full512_projection() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    struct ZeroYaw;
    impl AngularTrig for ZeroYaw {
        fn sine(&mut self, yaw: Sum) -> Result<Sum> {
            assert_eq!(yaw.to_f32().to_bits(), 0);
            Ok(Sum::default())
        }
        fn cosine(&mut self, yaw: Sum) -> Result<Sum> {
            assert_eq!(yaw.to_f32().to_bits(), 0);
            Ok(Sum::from_i32(1))
        }
    }
    let mut before = [0xa5; 196];
    put(&mut before, 0x7c, 0);
    let input = BlockInitialization {
        actor_translation: [9.0f32, -7.0, 2.0].map(f32::to_bits),
        model_offsets: [3.0f32, 2.0, -1.0].map(f32::to_bits),
        height_word: 0x42f00000,
        radius_word: 0x42a00000,
    };
    let result = Primary::initialize_block_words(before, input, &mut ZeroYaw).unwrap();
    // Historical window begins 0093af80 = primary +0x20 and extends past it.
    let mut window = [0xa5; 512];
    window[..196 - 0x20].copy_from_slice(&result[0x20..]);
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("retained projection requires sha256sum");
    child.stdin.take().unwrap().write_all(&window).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let hash = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        hash.split_whitespace().next().unwrap(),
        "80e0b699e06870ee3e017d3fc4954900176367dfb214a5d646f02527086baf1c"
    );
}
