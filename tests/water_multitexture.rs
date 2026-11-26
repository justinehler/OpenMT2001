//! Literal synthetic expectations from retained v1.4 instruction observations.
//! No original asset, executable, gate, emulator or GPU is needed to run these.
use openmt2001::water::{
    WaterPhase,
    multitexture::{self, Material, Pass, Phases, Route, RouteFacts},
};

#[test]
fn original_five_row_route_contract() {
    // 004c5466..004c5508: E, validated15, validated16, signed Detail.
    for (e, v15, v16, d, expected) in [
        (false, true, true, 2, &[20][..]),
        (true, true, false, 2, &[20][..]),
        (true, true, true, 0, &[16][..]),
        (true, false, true, 2, &[16][..]),
        (true, true, true, 1, &[16, 15][..]),
    ] {
        let facts = RouteFacts::new(e, v15, v16, d, "v14:004c5466-004c5508/literal").unwrap();
        assert_eq!(facts.route().modes(), expected);
        assert_eq!(facts.provenance(), "v14:004c5466-004c5508/literal");
    }
}

#[test]
fn detail_sign_and_both_failed_validation_boundaries() {
    for (detail, expected) in [
        (i32::MIN, &[16][..]),
        (-1, &[16][..]),
        (0, &[16][..]),
        (1, &[16, 15][..]),
        (2, &[16, 15][..]),
        (i32::MAX, &[16, 15][..]),
    ] {
        assert_eq!(
            RouteFacts::new(true, true, true, detail, "literal")
                .unwrap()
                .route()
                .modes(),
            expected
        );
        assert_eq!(
            RouteFacts::new(true, false, true, detail, "literal")
                .unwrap()
                .route()
                .modes(),
            &[16]
        );
        assert_eq!(
            RouteFacts::new(true, false, false, detail, "literal")
                .unwrap()
                .route()
                .modes(),
            &[20]
        );
        assert_eq!(
            RouteFacts::new(false, true, true, detail, "literal")
                .unwrap()
                .route()
                .modes(),
            &[20]
        );
    }
}

#[test]
fn route_facts_require_provenance() {
    for missing in ["", " ", "\t\n"] {
        assert!(RouteFacts::new(true, true, true, 2, missing).is_err());
    }
}

#[test]
fn all_fixed_material_words_match_original_initializer() {
    // Native absolute MOVs in004599ee..00459bdc after560-byte zeroing;
    // EAX=0 EDX=1 ESI=2 EBX=3 EBP=4. No production material helper predicts this.
    let expected_16: &[(usize, u32)] = &[
        (0x10, 1),
        (0x14, 2),
        (0x18, 2),
        (0x1c, 2),
        (0x20, 2),
        (0x38, 2),
        (0x3c, 1),
        (0x40, 4),
        (0x44, 2),
        (0x6c, 2),
        (0x70, 2),
        (0x7c, 2),
        (0x80, 2),
        (0x84, 1),
        (0x88, 1),
        (0x8c, 1),
        (0x90, 1),
        (0x94, 3),
        (0x98, 1),
        (0x9c, 1),
        (0xa0, 3),
        (0xa4, 1),
        (0xa8, 2),
        (0xb4, 2),
        (0xb8, 2),
        (0xbc, 1),
        (0xc0, 1),
        (0xc4, 1),
        (0xc8, 1),
        (0xcc, 2),
        (0xd0, 1),
        (0xd4, 1),
        (0xd8, 2),
        (0xdc, 1),
        (0xe0, 1),
    ];
    let expected_15: &[(usize, u32)] = &[
        (0x10, 1),
        (0x14, 2),
        (0x18, 2),
        (0x1c, 2),
        (0x20, 2),
        (0x38, 2),
        (0x3c, 2),
        (0x40, 2),
        (0x44, 2),
        (0x6c, 2),
        (0x70, 2),
        (0x7c, 2),
        (0x80, 2),
        (0x84, 1),
        (0x88, 1),
        (0x8c, 1),
        (0x90, 1),
        (0x94, 3),
        (0x98, 1),
        (0x9c, 1),
        (0xa0, 3),
        (0xa4, 1),
        (0xa8, 2),
        (0xb4, 2),
        (0xb8, 2),
        (0xbc, 1),
        (0xc0, 1),
        (0xc4, 1),
        (0xc8, 1),
        (0xcc, 2),
        (0xd0, 1),
        (0xd4, 1),
        (0xd8, 2),
        (0xdc, 1),
        (0xe0, 1),
    ];
    for (pass, mode, words) in [
        (Pass::First, 16, expected_16),
        (Pass::Quality, 15, expected_15),
    ] {
        let material = Material::original(pass);
        assert_eq!(material.mode(), mode);
        for offset in (0x10..0x230).step_by(4) {
            let expected = words
                .iter()
                .find(|(at, _)| *at == offset)
                .map_or(0, |(_, value)| *value);
            assert_eq!(
                material.field(offset),
                Some(expected),
                "mode{mode} offset{offset:x}"
            );
        }
        assert_eq!(material.field(0x78), Some(0)); // zero LOD bias, unlike fallback
        assert_eq!(material.field(0xb0), Some(0));
        assert!(material.field(0xc).is_none()); // availability is not fabricated
        assert!(material.field(0x11).is_none());
        assert!(material.field(0x230).is_none());
        assert!(material.stage(8).is_none());
    }
}

struct Observation {
    phase: u32,
    previous: u32,
    tick: u32,
    sun: [u32; 3],
    after: u32,
    uv: [u32; 6],
    secondary: [u32; 6],
    colors: [u32; 3],
}

// Original audit runs004c4cd0/004c5140 + actual004e2b20/004c4690 at PC64.
// Fixed source inputs: camera[50,80,100], positions below; no pointer aliases or
// injected color callback/count mutation. These are literals, not Rust predictions.
const FIRST: &[Observation] = &[
    Observation {
        phase: 0x00000000,
        previous: 4294967295,
        tick: 100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x00000000,
        uv: [
            0x3de1f8a1, 0x3e2e410c, 0xbf68f5c3, 0x3eba5e35, 0x40403127, 0xc0948312,
        ],
        secondary: [
            0x3de1f8a1, 0x3ef0ba20, 0xbf68f5c3, 0x3f29fbe8, 0x40403127, 0xc08ae979,
        ],
        colors: [0x00f6ffff, 0x00f6ffff, 0x00f6ffff],
    },
    Observation {
        phase: 0x3e800000,
        previous: 100,
        tick: 333,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3e9dd2f2,
        uv: [
            0x3f3a1206, 0x3e2e410c, 0xbe9645a1, 0x3eba5e35, 0x4067a5e3, 0xc0948312,
        ],
        secondary: [
            0xbf0193de, 0x3ef0ba20, 0xbfc3645a, 0x3f29fbe8, 0x4018bc6a, 0xc08ae979,
        ],
        colors: [0x00f6ffff, 0x00f6ffff, 0x00f6ffff],
    },
    Observation {
        phase: 0x3f666666,
        previous: 100,
        tick: 2100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3ecccccd,
        uv: [
            0x3f690be1, 0x3e2e410c, 0xbde147ac, 0x3eba5e35, 0x4073645a, 0xc0948312,
        ],
        secondary: [
            0xbf308db9, 0x3ef0ba20, 0xbfdae148, 0x3f29fbe8, 0x400cfdf4, 0xc08ae979,
        ],
        colors: [0x00f6ffff, 0x00f6ffff, 0x00f6ffff],
    },
    Observation {
        phase: 0x3f800000,
        previous: 100,
        tick: 100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3f800000,
        uv: [
            0x40070fc5, 0x3e2e410c, 0x3f8b851f, 0x3eba5e35, 0x40a01893, 0xc0948312,
        ],
        secondary: [
            0xbff1e076, 0x3ef0ba20, 0xc03a3d71, 0x3f29fbe8, 0x3f80624e, 0xc08ae979,
        ],
        colors: [0x00f6ffff, 0x00f6ffff, 0x00f6ffff],
    },
    Observation {
        phase: 0x3f800347,
        previous: 100,
        tick: 100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x38d1c000,
        uv: [
            0x3de26181, 0x3e2e410c, 0xbf68e8a7, 0x3eba5e35, 0x4040346e, 0xc0948312,
        ],
        secondary: [
            0x3de18fc1, 0x3ef0ba20, 0xbf6902df, 0x3f29fbe8, 0x40402de0, 0xc08ae979,
        ],
        colors: [0x00f6ffff, 0x00f6ffff, 0x00f6ffff],
    },
    Observation {
        phase: 0x3e000000,
        previous: 100,
        tick: 13000,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3eb33338,
        uv: [
            0x3f4f724c, 0x3e2e410c, 0xbe570a2a, 0x3eba5e35, 0x406cfdf5, 0xc0948312,
        ],
        secondary: [
            0xbf16f424, 0x3ef0ba20, 0xbfce147d, 0x3f29fbe8, 0x40136459, 0xc08ae979,
        ],
        colors: [0x00f6ffff, 0x00f6ffff, 0x00f6ffff],
    },
    Observation {
        phase: 0x00000000,
        previous: 4294967295,
        tick: 100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x00000000,
        uv: [
            0x3de1f8a1, 0x3e2e410c, 0xbf68f5c3, 0x3eba5e35, 0x40403127, 0xc0948312,
        ],
        secondary: [
            0x3de1f8a1, 0x3ef0ba20, 0xbf68f5c3, 0x3f29fbe8, 0x40403127, 0xc08ae979,
        ],
        colors: [0x002a533d, 0x002a533d, 0x002a533d],
    },
    Observation {
        phase: 0x3e800000,
        previous: 100,
        tick: 333,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3e9dd2f2,
        uv: [
            0x3f3a1206, 0x3e2e410c, 0xbe9645a1, 0x3eba5e35, 0x4067a5e3, 0xc0948312,
        ],
        secondary: [
            0xbf0193de, 0x3ef0ba20, 0xbfc3645a, 0x3f29fbe8, 0x4018bc6a, 0xc08ae979,
        ],
        colors: [0x002a533d, 0x002a533d, 0x002a533d],
    },
    Observation {
        phase: 0x3f666666,
        previous: 100,
        tick: 2100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3ecccccd,
        uv: [
            0x3f690be1, 0x3e2e410c, 0xbde147ac, 0x3eba5e35, 0x4073645a, 0xc0948312,
        ],
        secondary: [
            0xbf308db9, 0x3ef0ba20, 0xbfdae148, 0x3f29fbe8, 0x400cfdf4, 0xc08ae979,
        ],
        colors: [0x002a533d, 0x002a533d, 0x002a533d],
    },
    Observation {
        phase: 0x3f800000,
        previous: 100,
        tick: 100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3f800000,
        uv: [
            0x40070fc5, 0x3e2e410c, 0x3f8b851f, 0x3eba5e35, 0x40a01893, 0xc0948312,
        ],
        secondary: [
            0xbff1e076, 0x3ef0ba20, 0xc03a3d71, 0x3f29fbe8, 0x3f80624e, 0xc08ae979,
        ],
        colors: [0x002a533d, 0x002a533d, 0x002a533d],
    },
    Observation {
        phase: 0x3f800347,
        previous: 100,
        tick: 100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x38d1c000,
        uv: [
            0x3de26181, 0x3e2e410c, 0xbf68e8a7, 0x3eba5e35, 0x4040346e, 0xc0948312,
        ],
        secondary: [
            0x3de18fc1, 0x3ef0ba20, 0xbf6902df, 0x3f29fbe8, 0x40402de0, 0xc08ae979,
        ],
        colors: [0x002a533d, 0x002a533d, 0x002a533d],
    },
    Observation {
        phase: 0x3e000000,
        previous: 100,
        tick: 13000,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3eb33338,
        uv: [
            0x3f4f724c, 0x3e2e410c, 0xbe570a2a, 0x3eba5e35, 0x406cfdf5, 0xc0948312,
        ],
        secondary: [
            0xbf16f424, 0x3ef0ba20, 0xbfce147d, 0x3f29fbe8, 0x40136459, 0xc08ae979,
        ],
        colors: [0x002a533d, 0x002a533d, 0x002a533d],
    },
];

const QUALITY: &[Observation] = &[
    Observation {
        phase: 0x00000000,
        previous: 4294967295,
        tick: 100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x00000000,
        uv: [
            0xbcd39b02, 0xbda5cae5, 0xbdd182a9, 0xbdc38b05, 0x3e438b05, 0x3e8f2a5a,
        ],
        secondary: [
            0xbf1649bb, 0xbfeb8034, 0xc014cccd, 0xc00ae148, 0x408ae148, 0x40cb5c29,
        ],
        colors: [0xff8498b9, 0xff95abd0, 0xffa7c0ea],
    },
    Observation {
        phase: 0x3e800000,
        previous: 100,
        tick: 333,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3e9dd2f2,
        uv: [
            0x3f661f93, 0xbda5cae5, 0x3f528c16, 0xbdc38b05, 0x3f8ecf96, 0x3e8f2a5a,
        ],
        secondary: [
            0xbe8ec083, 0xbfeb8034, 0xc001126f, 0xc00ae148, 0x4094be77, 0x40cb5c29,
        ],
        colors: [0xff8498b9, 0xff95abd0, 0xffa7c0ea],
    },
    Observation {
        phase: 0x3f666666,
        previous: 100,
        tick: 2100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3ecccccd,
        uv: [
            0x3f964b2e, 0xbda5cae5, 0x3f8c816f, 0xbdc38b05, 0x3fb20afa, 0x3e8f2a5a,
        ],
        secondary: [
            0xbe3f8d50, 0xbfeb8034, 0xbff66667, 0xc00ae148, 0x4097ae15, 0x40cb5c29,
        ],
        colors: [0xff8498b9, 0xff95abd0, 0xffa7c0ea],
    },
    Observation {
        phase: 0x3f800000,
        previous: 100,
        tick: 100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3f800000,
        uv: [
            0x403e58ca, 0xbda5cae5, 0x403973eb, 0xbdc38b05, 0x404c38b0, 0x3e8f2a5a,
        ],
        secondary: [
            0x3ed36c8b, 0xbfeb8034, 0xbfa9999a, 0xc00ae148, 0x40aae148, 0x40cb5c29,
        ],
        colors: [0xff8498b9, 0xff95abd0, 0xffa7c0ea],
    },
    Observation {
        phase: 0x3f800347,
        previous: 100,
        tick: 100,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x38d1c000,
        uv: [
            0xbcd125c2, 0xbda5cae5, 0xbdd0e559, 0xbdc38b05, 0x3e43d9ad, 0x3e8f2a5a,
        ],
        secondary: [
            0xbf16432d, 0xbfeb8034, 0xc014cb29, 0xc00ae148, 0x408ae21a, 0x40cb5c29,
        ],
        colors: [0xff8498b9, 0xff95abd0, 0xffa7c0ea],
    },
    Observation {
        phase: 0x3e000000,
        previous: 100,
        tick: 13000,
        sun: [0x00000000, 0x3f800000, 0x00000000],
        after: 0x3eb33338,
        uv: [
            0x3f8317fe, 0xbda5cae5, 0x3f729c7f, 0xbdc38b05, 0x3f9ed7cb, 0x3e8f2a5a,
        ],
        secondary: [
            0xbe72c079, 0xbfeb8034, 0xbffccccc, 0xc00ae148, 0x4096147b, 0x40cb5c29,
        ],
        colors: [0xff8498b9, 0xff95abd0, 0xffa7c0ea],
    },
    Observation {
        phase: 0x00000000,
        previous: 4294967295,
        tick: 100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x00000000,
        uv: [
            0xbcd39b02, 0xbda5cae5, 0xbdd182a9, 0xbdc38b05, 0x3e438b05, 0x3e8f2a5a,
        ],
        secondary: [
            0xbf1649bb, 0xbfeb8034, 0xc014cccd, 0xc00ae148, 0x408ae148, 0x40cb5c29,
        ],
        colors: [0xff000000, 0xff000000, 0xff000000],
    },
    Observation {
        phase: 0x3e800000,
        previous: 100,
        tick: 333,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3e9dd2f2,
        uv: [
            0x3f661f93, 0xbda5cae5, 0x3f528c16, 0xbdc38b05, 0x3f8ecf96, 0x3e8f2a5a,
        ],
        secondary: [
            0xbe8ec083, 0xbfeb8034, 0xc001126f, 0xc00ae148, 0x4094be77, 0x40cb5c29,
        ],
        colors: [0xff000000, 0xff000000, 0xff000000],
    },
    Observation {
        phase: 0x3f666666,
        previous: 100,
        tick: 2100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3ecccccd,
        uv: [
            0x3f964b2e, 0xbda5cae5, 0x3f8c816f, 0xbdc38b05, 0x3fb20afa, 0x3e8f2a5a,
        ],
        secondary: [
            0xbe3f8d50, 0xbfeb8034, 0xbff66667, 0xc00ae148, 0x4097ae15, 0x40cb5c29,
        ],
        colors: [0xff000000, 0xff000000, 0xff000000],
    },
    Observation {
        phase: 0x3f800000,
        previous: 100,
        tick: 100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3f800000,
        uv: [
            0x403e58ca, 0xbda5cae5, 0x403973eb, 0xbdc38b05, 0x404c38b0, 0x3e8f2a5a,
        ],
        secondary: [
            0x3ed36c8b, 0xbfeb8034, 0xbfa9999a, 0xc00ae148, 0x40aae148, 0x40cb5c29,
        ],
        colors: [0xff000000, 0xff000000, 0xff000000],
    },
    Observation {
        phase: 0x3f800347,
        previous: 100,
        tick: 100,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x38d1c000,
        uv: [
            0xbcd125c2, 0xbda5cae5, 0xbdd0e559, 0xbdc38b05, 0x3e43d9ad, 0x3e8f2a5a,
        ],
        secondary: [
            0xbf16432d, 0xbfeb8034, 0xc014cb29, 0xc00ae148, 0x408ae21a, 0x40cb5c29,
        ],
        colors: [0xff000000, 0xff000000, 0xff000000],
    },
    Observation {
        phase: 0x3e000000,
        previous: 100,
        tick: 13000,
        sun: [0x3f4ccccd, 0xbf19999a, 0x00000000],
        after: 0x3eb33338,
        uv: [
            0x3f8317fe, 0xbda5cae5, 0x3f729c7f, 0xbdc38b05, 0x3f9ed7cb, 0x3e8f2a5a,
        ],
        secondary: [
            0xbe72c079, 0xbfeb8034, 0xbffccccc, 0xc00ae148, 0x4096147b, 0x40cb5c29,
        ],
        colors: [0xff000000, 0xff000000, 0xff000000],
    },
];

fn check_observations(pass: Pass, rows: &[Observation]) {
    let positions = [[12.125, 0., 18.7], [-100., 0., 40.], [330., -1., -510.]];
    assert_eq!(rows.len(), 12);
    for (case, expected) in rows.iter().enumerate() {
        let mut state = WaterPhase {
            phase: f32::from_bits(expected.phase),
            previous_tick: expected.previous,
        };
        let result = multitexture::vertices(
            &mut state,
            pass,
            &positions,
            [50., 80., 100.],
            expected.sun.map(f32::from_bits),
            expected.tick,
        )
        .unwrap();
        assert_eq!(state.phase.to_bits(), expected.after, "case{case} phase");
        assert_eq!(state.previous_tick, expected.tick);
        assert_eq!(result.len(), 3);
        for (index, vertex) in result.iter().enumerate() {
            assert_eq!(
                vertex.uv.map(f32::to_bits),
                expected.uv[index * 2..index * 2 + 2],
                "case{case} vertex{index} UV0"
            );
            assert_eq!(
                vertex.secondary_uv.map(f32::to_bits),
                expected.secondary[index * 2..index * 2 + 2],
                "case{case} vertex{index} UV1"
            );
            assert_eq!(
                vertex.color, expected.colors[index],
                "case{case} vertex{index} color"
            );
        }
    }
}

#[test]
fn first_vertices_match_retained_original_pc64() {
    check_observations(Pass::First, FIRST);
}

#[test]
fn quality_vertices_match_retained_original_pc64() {
    check_observations(Pass::Quality, QUALITY);
}

#[test]
fn both_models_and_independent_phase_owners_follow_route() {
    let mut phases = Phases::fresh();
    phases.first = WaterPhase {
        phase: 0.25,
        previous_tick: 100,
    };
    let positions = [[12.125, 0., 18.7], [-100., 0., 40.], [330., -1., -510.]];
    let models = [&positions[..1], &positions[1..]];
    let facts = RouteFacts::new(
        true,
        true,
        true,
        2,
        "retained native first/quality case inputs",
    )
    .unwrap();
    let prepared = phases
        .prepare(facts, models, [50., 80., 100.], [0., 1., 0.], 333)
        .unwrap();
    assert_eq!(prepared.facts, facts);
    assert_eq!(prepared.batches.len(), 2);
    assert_eq!(prepared.batches[0].material.mode(), 16);
    assert_eq!(prepared.batches[1].material.mode(), 15);
    assert_eq!(phases.first.phase.to_bits(), 0x3e9dd2f2);
    assert_eq!(phases.quality.phase.to_bits(), 0);
    for (batch, expected) in prepared.batches.iter().zip([&FIRST[1], &QUALITY[0]]) {
        assert_eq!(batch.vertices[0].len(), 1);
        assert_eq!(batch.vertices[1].len(), 2);
        for (i, vertex) in batch.vertices.iter().flatten().enumerate() {
            assert_eq!(vertex.uv.map(f32::to_bits), expected.uv[i * 2..i * 2 + 2]);
            assert_eq!(
                vertex.secondary_uv.map(f32::to_bits),
                expected.secondary[i * 2..i * 2 + 2]
            );
            assert_eq!(vertex.color, expected.colors[i]);
        }
    }
    let snapshot = phases;
    let fallback = RouteFacts::new(false, true, true, 2, "original feature disabled").unwrap();
    let prepared = phases
        .prepare(fallback, models, [50., 80., 100.], [0., 1., 0.], 500)
        .unwrap();
    assert_eq!(prepared.facts.route(), Route::Fallback);
    assert!(prepared.batches.is_empty()); // delegated to existing fallback owner
    assert_eq!(phases.first.phase.to_bits(), snapshot.first.phase.to_bits());
    assert_eq!(phases.first.previous_tick, 333);
    assert_eq!(phases.quality.previous_tick, 333);
    let first_only = RouteFacts::new(true, true, true, 0, "original Detail0").unwrap();
    assert_eq!(
        phases
            .prepare(first_only, models, [50., 80., 100.], [0., 1., 0.], 500)
            .unwrap()
            .batches
            .len(),
        1
    );
    assert_eq!(phases.first.previous_tick, 500);
    assert_eq!(phases.quality.previous_tick, 333);
}

#[test]
fn empty_models_advance_but_invalid_input_does_not_partially_commit() {
    let facts = RouteFacts::new(true, true, true, 1, "literal source protocol").unwrap();
    let mut phases = Phases::fresh();
    assert_eq!(
        phases
            .prepare(facts, [&[], &[]], [50., 80., 100.], [0., 1., 0.], 100)
            .unwrap()
            .batches
            .len(),
        2
    );
    assert_eq!(phases.first.previous_tick, 100);
    assert_eq!(phases.quality.previous_tick, 100);
    let good = [[12.125, 0., 18.7]];
    let bad = [[f32::NAN, 0., 0.]];
    assert!(
        phases
            .prepare(facts, [&good, &bad], [50., 80., 100.], [0., 1., 0.], 333)
            .is_err()
    );
    assert_eq!(phases.first.previous_tick, 100);
    assert_eq!(phases.quality.previous_tick, 100);
    assert_eq!(phases.first.phase.to_bits(), 0);
    assert_eq!(phases.quality.phase.to_bits(), 0);
}
