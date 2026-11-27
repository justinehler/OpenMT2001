//! Retained original-instruction and own-code hardware observations stay
//! separate. Neither is a universal x87 or live frontend equivalence claim.
use super::*;
use crate::x87::portable_trig::PortableTrig;

fn put(raw: &mut [u8; 196], at: usize, value: u32) {
    raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn state(
    mode: u32,
    center: [f32; 3],
    yaw: f32,
    height: f32,
    radius: f32,
    caches: [u32; 2],
) -> Primary {
    let mut raw = [0u8; 196];
    put(&mut raw, 0, mode);
    for (at, value) in [(0x38, height), (0x7c, yaw), (0x98, yaw), (0xb0, radius)] {
        put(&mut raw, at, value.to_bits());
    }
    for (i, v) in center.into_iter().enumerate() {
        put(&mut raw, 0xb4 + i * 4, v.to_bits());
    }
    put(&mut raw, 0x74, 20);
    put(&mut raw, 0x88, caches[0]);
    put(&mut raw, 0x90, caches[1]);
    Primary::decode(raw).unwrap()
}
fn input() -> Mode12Input {
    Mode12Input {
        build_visible: false,
        enable_translation: true,
        enable_keyboard: true,
        enable_rotation: true,
        enable_zoom: true,
        buttons: [false; 2],
        delta: [0; 2],
        keys: [false; 4],
        sensitivity: 50,
        legacy_yaw: false,
        invert_vertical: false,
        low_height: false,
    }
}
// Initializer cache bits from the separately retained own-code x87 direct rows
// for the original fixture's three seeds. They are not inferred expected output.
fn caches(yaw: f32) -> [u32; 2] {
    match yaw.to_bits() {
        0xbfc90fdb => [0xbf800000, 0xb33bbd2e],
        0x3f333333 => [0x3f24eb73, 0x3f43ccb3],
        0x4019999a => [0x3f2ceb27, 0xbf3cc5d7],
        _ => panic!("unrecorded fixture seed"),
    }
}

#[test]
#[ignore = "requires retained PC64 original-instruction mode12 observations"]
fn retained_original_mode12_keyboard_and_mouse() {
    let mut compared = 0;
    for (key, magic, stride) in [
        ("OPENMT2001_NATIVE_BLOCK_CAMERA", b"MTBCAM02", 60),
        ("OPENMT2001_NATIVE_BLOCK_MOUSE", b"MTBMOU02", 76),
    ] {
        let bytes = std::fs::read(std::env::var(key).unwrap()).unwrap();
        assert_eq!(&bytes[..8], magic);
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x37f);
        assert_eq!(bytes.len(), 16 + word(8) as usize * stride);
        for n in 0..word(8) as usize {
            let base = 16 + n * stride;
            let f = |i: usize| f32::from_bits(word(base + i * 4));
            let mut p = state(
                12,
                [f(0), f(1), f(2)],
                f(3),
                f(4),
                if stride == 76 { f(5) } else { 80. },
                caches(f(3)),
            );
            let mut i = input();
            if stride == 76 {
                let flags = word(base + 36);
                i.buttons = [false, true];
                i.delta = [word(base + 24) as i32, word(base + 28) as i32];
                i.sensitivity = word(base + 32) as i32;
                i.invert_vertical = flags & 1 != 0;
                i.legacy_yaw = flags & 2 != 0;
                i.low_height = flags & 4 != 0;
            } else {
                let mask = word(base + 20);
                i.keys = std::array::from_fn(|j| mask & (1 << j) != 0);
            }
            p.mode12(i, &mut PortableTrig).unwrap();
            let v = p.view();
            let actual: Vec<u32> = v
                .desired
                .into_iter()
                .flatten()
                .chain(if stride == 76 {
                    vec![v.yaw, v.desired[0][1], v.radius]
                } else {
                    v.center.to_vec()
                })
                .map(f32::to_bits)
                .collect();
            let expected_at = base + if stride == 76 { 40 } else { 24 };
            for (j, a) in actual.into_iter().enumerate() {
                assert_eq!(a, word(expected_at + j * 4), "{key} case{n} component{j}");
            }
            compared += 1;
        }
    }
    assert_eq!(compared, 408);
}

#[test]
#[ignore = "requires retained own-code x87 diagnostics, not original gameplay"]
fn retained_own_hardware_common_and_direct() {
    let bytes = std::fs::read(std::env::var("OPENMT2001_CAMERA_OWN_X87").unwrap()).unwrap();
    assert_eq!(&bytes[..8], b"MTX87C01");
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!((word(8), word(12)), (295, 291));
    let mut at = 16;
    for n in 0..295 {
        let f = |offset| f32::from_bits(word(at + offset));
        let mut p = state(12, [f(16), 0., f(20)], f(0), 120., f(12), [0, 0]);
        put(&mut p.raw, 0x98, word(at + 4));
        put(&mut p.raw, 0x74, word(at + 8));
        p.raw[0x72] = 1;
        struct Witness<'a> {
            q: &'a [u8],
            provider: PortableTrig,
        }
        impl AngularTrig for Witness<'_> {
            fn sine(&mut self, q: Sum) -> Result<Sum> {
                // Recover the recorded exact finite dyadic q without a
                // binary64 conversion, using the existing arithmetic API.
                let mantissa = u64::from_le_bytes(self.q[..8].try_into().unwrap());
                let se = u16::from_le_bytes(self.q[8..].try_into().unwrap());
                let mut expected = Sum::default();
                for bit in (0..64).rev() {
                    expected.mul_f32(2.)?;
                    if mantissa & (1 << bit) != 0 {
                        expected.add_f32(1.)?;
                    }
                }
                let exponent = (se & 0x7fff) as i32 - 16383 - 63;
                for _ in 0..exponent.unsigned_abs() {
                    expected.mul_f32(if exponent < 0 { 0.5 } else { 2. })?;
                }
                if se & 0x8000 != 0 {
                    expected = expected.negated();
                }
                assert_eq!(
                    q.compare(expected),
                    Ordering::Equal,
                    "retained q80 identity"
                );
                self.provider.sine(q)
            }
            fn cosine(&mut self, q: Sum) -> Result<Sum> {
                self.provider.cosine(q)
            }
        }
        p.common_with_trig(&mut Witness {
            q: &bytes[at + 24..at + 34],
            provider: PortableTrig,
        })
        .unwrap();
        let v = p.view();
        let actual = [v.yaw, v.sine, v.cosine, v.desired[0][0], v.desired[0][2]].map(f32::to_bits);
        assert_eq!(
            actual,
            std::array::from_fn(|j| word(at + 34 + j * 4)),
            "own common row{n}"
        );
        at += 54;
    }
    for n in 0..291 {
        let f = |offset| f32::from_bits(word(at + offset));
        let mut p = state(
            12,
            [f(8), f(44), f(12)],
            f(0),
            f(40),
            f(4),
            [word(at + 16), word(at + 20)],
        );
        let (s, c) = PortableTrig.pair(Sum::from_f32(f(0)).unwrap()).unwrap();
        assert_eq!(
            [s.to_f32().to_bits(), c.to_f32().to_bits()],
            [word(at + 16), word(at + 20)]
        );
        p.mode12(input(), &mut PortableTrig).unwrap();
        let v = p.view();
        assert_eq!(
            [
                v.desired[0][0],
                v.desired[0][2],
                v.desired[1][0],
                v.desired[1][2]
            ]
            .map(f32::to_bits),
            std::array::from_fn(|j| word(at + 24 + j * 4)),
            "own direct row{n}"
        );
        at += 48;
    }
    assert_eq!(at, bytes.len());
}
