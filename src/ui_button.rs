//! Original colored button background (4428a0/442b10), in UI screen units.
//! State bytes retain their original offsets until input dispatch is recovered.

#[derive(Debug, Clone, Copy, Default)]
pub struct State {
    pub flag_330: bool,
    pub flag_333: bool,
    pub flag_334: bool,
    pub flag_335: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    /// Corner order: top-left, bottom-left, bottom-right, top-right.
    Gradient {
        bounds: [f32; 4],
        colors: [u32; 4],
    },
    Outline {
        bounds: [f32; 4],
        color: u32,
    },
    Line {
        start: [f32; 2],
        end: [f32; 2],
        color: u32,
    },
}

pub fn background(bounds: [f32; 4], rgb: [u8; 3], state: State) -> Result<Vec<Primitive>, String> {
    if !bounds
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
        || bounds[2] - bounds[0] < 8.0
        || bounds[3] - bounds[1] < 8.0
    {
        return Err("unsupported UI button bounds".into());
    }
    let pack =
        |c: [u8; 3]| 0xff000000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
    let base = pack(rgb);
    let light = pack(rgb.map(|v| v.saturating_add(32)));
    let dark = pack(rgb.map(|v| v.saturating_sub(32)));
    let darker = pack(rgb.map(|v| v.saturating_sub(64)));
    let fill = if state.flag_335 {
        if !state.flag_330 {
            light
        } else if state.flag_334 {
            dark
        } else {
            base
        }
    } else if state.flag_333 {
        0x80808080
    } else {
        base
    };
    let colors = if !state.flag_335 && state.flag_333 {
        [fill; 4]
    } else {
        [
            fill,
            fill,
            (fill & 0xffffff) | 0x5a000000,
            (fill & 0xffffff) | 0x5a000000,
        ]
    };
    let [l, t, r, b] = bounds;
    let [il, it, ir, ib] = [l + 4.0, t + 4.0, r - 4.0, b - 4.0];
    let (outline, first, second) = if state.flag_333 {
        (0x80808080, 0xff808080, 0xff323232)
    } else if state.flag_330 && state.flag_334 {
        (light, darker, light)
    } else {
        (light, light, darker)
    };
    let mut result = vec![Primitive::Gradient {
        bounds: [il, it, ir, ib],
        colors,
    }];
    for bounds in [[l, t, r, it], [l, t, il, b], [ir, t, r, b], [l, ib, r, b]] {
        result.push(Primitive::Gradient {
            bounds,
            colors: [0xff000000; 4],
        });
    }
    result.push(Primitive::Outline {
        bounds,
        color: outline,
    });
    for (start, end, color) in [
        ([ir, it - 0.5], [ir, ib + 0.5], first),
        ([ir - 0.5, ib], [il, ib], second),
        ([il, ib], [il, it], second),
        ([il, it], [ir, it], first),
    ] {
        result.push(Primitive::Line { start, end, color });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original UI button fixture"]
    fn matches_native_button_primitives() {
        let raw = std::fs::read(std::env::var("OPENMT2001_NATIVE_UI_BUTTON").unwrap()).unwrap();
        assert_eq!(&raw[..8], b"MTBUT001");
        let mut offset = 8;
        fn word(raw: &[u8], offset: &mut usize) -> u32 {
            let v = u32::from_le_bytes(raw[*offset..*offset + 4].try_into().unwrap());
            *offset += 4;
            v
        }
        for _ in 0..word(&raw, &mut offset) {
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&raw, &mut offset)));
            let rgb = raw[offset..offset + 3].try_into().unwrap();
            let flags = raw[offset + 3];
            offset += 4;
            let state = State {
                flag_330: flags & 1 != 0,
                flag_333: flags & 2 != 0,
                flag_334: flags & 4 != 0,
                flag_335: flags & 8 != 0,
            };
            let actual = background(bounds, rgb, state).unwrap();
            assert_eq!(actual.len(), word(&raw, &mut offset) as usize);
            for primitive in actual {
                let kind = word(&raw, &mut offset);
                let geometry: [f32; 4] =
                    std::array::from_fn(|_| f32::from_bits(word(&raw, &mut offset)));
                let expected = match kind {
                    0 => Primitive::Gradient {
                        bounds: geometry,
                        colors: std::array::from_fn(|_| word(&raw, &mut offset)),
                    },
                    1 => Primitive::Outline {
                        bounds: geometry,
                        color: word(&raw, &mut offset),
                    },
                    2 => Primitive::Line {
                        start: [geometry[0], geometry[1]],
                        end: [geometry[2], geometry[3]],
                        color: word(&raw, &mut offset),
                    },
                    _ => panic!("unknown primitive"),
                };
                assert_eq!(primitive, expected, "flags {flags}");
            }
        }
        assert_eq!(offset, raw.len());
    }
}
