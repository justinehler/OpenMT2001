//! Original sun-color inputs and 004ebbc0's discrete lighting palettes.
use crate::water::Daylight;

pub struct SunColors {
    pub daylight: Daylight,
    diffuse: [u32; 384],
    ambient: [u32; 384],
    scale: [f32; 384],
    fog_color: [u32; 384],
    fog_start: [f32; 384],
    fog_range: [f32; 384],
    night_threshold: [u8; 384],
}

pub struct SceneLight {
    pub direction: [f32; 3],
    pub diffuse: u32,
    pub ambient: u32,
    pub scale: f32,
    /// Discrete SUNCOLS night-effect sample (004c5a70), without interpolation.
    pub night_threshold: u8,
    pub fog: SceneFog,
}

#[derive(Clone, Copy, Debug)]
pub struct SceneFog {
    pub color: u32,
    /// Original start and width fractions, scaled by 3000 in 004b2b60.
    pub start_fraction: f32,
    pub range_fraction: f32,
}

impl SceneFog {
    /// 004b2c6a..004b2caa: round start before adding the retained width product.
    pub fn distances(&self) -> [f32; 2] {
        let start = (f64::from(self.start_fraction) * 3000.) as f32;
        let end = (f64::from(self.range_fraction) * 3000. + f64::from(start)) as f32;
        [start, end]
    }
}

impl SunColors {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let daylight = Daylight::from_suncols(bytes)?;
        let width = usize::from(u16::from_le_bytes(bytes[12..14].try_into().unwrap()));
        let height = usize::from(u16::from_le_bytes(bytes[14..16].try_into().unwrap()));
        if height < 177 {
            return Err("sun-color image lacks original fog/night rows".into());
        }
        let row = |from_top: usize| {
            let at = 18 + (height - from_top) * width * 4;
            std::array::from_fn::<_, 384, _>(|i| {
                u32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().unwrap())
            })
        };
        Ok(Self {
            daylight,
            diffuse: row(33).map(|c| c | 0xff000000),
            ambient: row(65).map(|c| c | 0xff000000),
            scale: row(49).map(|c| {
                (f64::from(c & 255) * f64::from(f32::from_bits(0x3b808081)) * f64::from(0.3f32)
                    + 1.) as f32
            }),
            fog_color: row(129).map(|c| c | 0xff000000),
            fog_start: row(145).map(|c| (c & 255) as f32),
            fog_range: row(161).map(|c| (c & 255) as f32),
            night_threshold: row(177).map(|c| c as u8),
        })
    }

    /// 004c5a70's color interpolations; the scale selects the current
    /// sample without interpolation. Inputs are original calendar/counter words.
    pub fn sample(&self, phase: u32, counter: u32) -> Result<SceneLight, String> {
        let subphase = (f64::from(counter.wrapping_shl(4) as i32) * f64::from(0.0016f32)) as f32;
        if !subphase.is_finite() || subphase < 0. {
            return Err("invalid lighting counter".into());
        }
        let integer = subphase.trunc() as i32;
        let fraction = (f64::from(subphase) - f64::from(integer)) as f32;
        let inverse = (1. - f64::from(fraction)) as f32;
        let index = (phase.wrapping_shl(4) as i32).wrapping_add(integer);
        let wrap = |i: i32| {
            let i = if i >= 384 { i - 384 } else { i };
            if (0..384).contains(&i) {
                Ok(i as usize)
            } else {
                Err("lighting sample outside native day")
            }
        };
        let first = wrap(index)?;
        let next = wrap(index.wrapping_add(1))?;
        // Fog distance uses a separately stored within-hour fraction, despite
        // retaining the adjacent subphase indexes used by the color tables.
        let fog_fraction = ((f64::from(index & 15) + f64::from(fraction)) * 0.0625) as f32;
        let interpolate = |table: &[u32; 384]| {
            let mut color = 0xff000000;
            for shift in [16, 8, 0] {
                let a = f64::from((table[first] >> shift) & 255);
                let b = f64::from((table[next] >> shift) & 255);
                let value = (a * f64::from(inverse) + b * f64::from(fraction)).trunc() as u32;
                color |= value << shift;
            }
            color
        };
        let fog_distance = |table: &[f32; 384]| {
            // This inverse remains on x87, unlike the earlier stored inverse
            // used for color interpolation (004c5df9..004c5e73).
            let inverse = 1. - f64::from(fog_fraction);
            ((f64::from(table[next]) * f64::from(fog_fraction) + f64::from(table[first]) * inverse)
                * f64::from(f32::from_bits(0x3b808081))) as f32
        };
        Ok(SceneLight {
            direction: self.daylight.sun_direction(phase, counter),
            diffuse: interpolate(&self.diffuse),
            ambient: interpolate(&self.ambient),
            scale: self.scale[first],
            night_threshold: self.night_threshold[first],
            fog: SceneFog {
                color: interpolate(&self.fog_color),
                start_fraction: fog_distance(&self.fog_start),
                range_fraction: fog_distance(&self.fog_range),
            },
        })
    }
}

#[derive(Clone)]
pub struct Palette {
    pub primary: [u32; 256],
    pub secondary: [u32; 512],
}

impl Palette {
    /// 004f2580 with bit4 clear and an identity static-object transform.
    /// The original scene transform flips Z before scaling sunlight by127.
    pub fn shade_static(&self, normal: [f32; 3], sun: [f32; 3]) -> Result<[u32; 2], String> {
        let color = self.primary_shade(normal, sun)?;
        Ok([color | 0xff000000, self.secondary_shade(color, 0)?])
    }

    /// 004f2580 with object flag 4, set by 004f1b40 while the scene's
    /// specular switch (009701e0) is on. The diffuse word matches
    /// `shade_static`; the secondary (specular) word moves up the secondary
    /// palette by a highlight from the 004f2390 half vector: ((n.h)/2+1/2)^16
    /// mapped onto 0..127. `half` comes from `specular_half_vector`.
    pub fn shade_specular(
        &self,
        normal: [f32; 3],
        sun: [f32; 3],
        half: [f32; 3],
    ) -> Result<[u32; 2], String> {
        let color = self.primary_shade(normal, sun)?;
        // 004f2686..004f26d5: the z, y, x product order and the f32 store of
        // the scaled power before FISTP.
        let [x, y, z] = std::array::from_fn(|i| f64::from(normal[i]) * f64::from(half[i]));
        let cosine = (z + y) + x;
        if !cosine.is_finite() {
            return Err("non-finite specular lighting normal".into());
        }
        let mut power = cosine * 0.5 + 0.5;
        for _ in 0..3 {
            power *= power;
        }
        let highlight = ((power * power * 254. - 127.) as f32).round_ties_even();
        Ok([
            color | 0xff000000,
            self.secondary_shade(color, (highlight as i32).max(0))?,
        ])
    }

    fn primary_shade(&self, normal: [f32; 3], sun: [f32; 3]) -> Result<u32, String> {
        let vector = scaled_sunlight(sun);
        let products =
            std::array::from_fn::<_, 3, _>(|i| f64::from(normal[i]) * f64::from(vector[i]));
        let dot = products[2] + (products[1] + products[0]);
        if !dot.is_finite() {
            return Err("non-finite static lighting normal".into());
        }
        let shade = dot.round_ties_even();
        if !(-127. ..=128.).contains(&shade) {
            return Err("static normal exceeds original lighting palette".into());
        }
        Ok(self.primary[(shade as i32 + 127) as usize])
    }

    /// The secondary palette is addressed from its middle entry (00945554)
    /// by the primary word's overbright byte plus any specular highlight.
    fn secondary_shade(&self, color: u32, highlight: i32) -> Result<u32, String> {
        let index = usize::try_from((color as i32 >> 24) + highlight + 127)
            .map_err(|_| "secondary lighting index precedes palette")?;
        self.secondary
            .get(index)
            .copied()
            .ok_or_else(|| "secondary lighting index exceeds palette".into())
    }

    /// Five original 004ebbc0 inputs. The normal scene passes diffuse again
    /// as specular and a zero offset color; other modes remain explicit.
    pub fn new(diffuse: u32, ambient: u32, specular: u32, offset: u32, scale: f32) -> Self {
        let normalize = |color: u32| {
            [16, 8, 0]
                .map(|s| f64::from((color >> s) & 255) * f64::from(f32::from_bits(0x3b808081)))
        };
        let diffuse = normalize(diffuse);
        let mut ambient = normalize(ambient);
        ambient[2] = f64::from(ambient[2] as f32);
        let channel = |v: f64| (v.trunc() as i32).clamp(0, 255) as u32;
        let primary = std::array::from_fn(|i| {
            let value =
                (i as f64 * f64::from(f32::from_bits(0x3b808081)) * f64::from(scale)) as f32;
            let (factor, extra) = if value > 1. {
                (1., f64::from(value) - 1.)
            } else {
                (f64::from(value), 0.)
            };
            let colors = std::array::from_fn::<_, 3, _>(|c| {
                channel(f64::from((factor * diffuse[c] + ambient[c]) as f32) * 255.)
            });
            channel(extra * 255.) << 24 | colors[0] << 16 | colors[1] << 8 | colors[2]
        });
        let specular = normalize(specular);
        let secondary = std::array::from_fn(|i| {
            let factor =
                (((i as i32 - 127) as f64 * f64::from(f32::from_bits(0x3c010204))) as f32).max(0.);
            let mut values = specular.map(|v| v * f64::from(factor));
            values[1] = f64::from(values[1] as f32);
            values[2] = f64::from(values[2] as f32);
            let colors = [16, 8, 0].map(|shift| ((offset >> shift) & 255) as i32);
            let rgb = std::array::from_fn::<_, 3, _>(|c| {
                (colors[c] - (values[c] * -255.).trunc() as i32).clamp(0, 255) as u32
            });
            0xff000000 | rgb[0] << 16 | rgb[1] << 8 | rgb[2]
        });
        Self { primary, secondary }
    }
}

/// 004f2390 stores object-local sunlight after the local-Z flip, scaled
/// by127.
fn scaled_sunlight(sun: [f32; 3]) -> [f32; 3] {
    [sun[0], sun[1], -sun[2]].map(|v| (f64::from(v) * 127.) as f32)
}

/// 004e2b20: unit direction from an object's descriptor position to the
/// camera eye (renderer 164e0), in world coordinates.
pub fn eye_direction(eye: [f32; 3], position: [f32; 3]) -> Result<[f32; 3], String> {
    let offset: [f32; 3] = std::array::from_fn(|i| eye[i] - position[i]);
    let [x, y, z] = offset.map(f64::from);
    let inverse = 1. / ((x * x + y * y) + z * z).sqrt();
    if !inverse.is_finite() {
        return Err("specular eye direction has no length".into());
    }
    Ok(offset.map(|v| (inverse * f64::from(v)) as f32))
}

/// 004f2390 with the specular switch on: the half vector between
/// object-local sunlight and the object-local eye direction, both given
/// before the local-Z flip, as `people_scene::sunlight` returns them. Each
/// sum is stored as f32 before 004e2ba0 normalizes it; a near-zero sum uses
/// 004e2ba0's fixed reciprocal.
pub fn specular_half_vector(sun: [f32; 3], eye: [f32; 3]) -> [f32; 3] {
    let sun = scaled_sunlight(sun);
    let eye = [eye[0], eye[1], -eye[2]];
    let sum: [f32; 3] =
        std::array::from_fn(|i| (f64::from(eye[i]) * 127. + f64::from(sun[i])) as f32);
    let [x, y, z] = sum.map(f64::from);
    let length = ((x * x + y * y) + z * z).sqrt();
    let inverse = if length < f64::from(f32::from_bits(0x3a83126f)) {
        f64::from(f32::from_bits(0x4479ffff))
    } else {
        1. / length
    };
    sum.map(|v| (inverse * f64::from(v)) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_palette() -> Palette {
        // Grey diffuse, dark ambient, white specular at full scale.
        Palette::new(0x808080, 0x202020, 0xffffff, 0, 1.)
    }

    #[test]
    fn specular_shade_keeps_diffuse_and_adds_highlight_towards_half_vector() {
        let palette = test_palette();
        let sun = [0., 1., 0.];
        let up = [0., 1., 0.];
        let half = specular_half_vector(sun, [0., 1., 0.]);
        assert_eq!(half, [0., 1., 0.]);
        let [plain_diffuse, plain_specular] = palette.shade_static(up, sun).unwrap();
        let [diffuse, specular] = palette.shade_specular(up, sun, half).unwrap();
        assert_eq!(diffuse, plain_diffuse);
        // Without the highlight the unlit secondary entry is black; a normal
        // on the half vector reaches the last entry: the full specular colour.
        assert_eq!(plain_specular, 0xff000000);
        assert_eq!(specular, 0xffffffff);
        assert_eq!(specular, palette.secondary[127 + 127]);
    }

    #[test]
    fn specular_highlight_falls_off_with_sixteenth_power() {
        let palette = test_palette();
        let sun = [0., 1., 0.];
        let half = [0., 1., 0.];
        // cos 60 deg: (0.75)^16 * 254 - 127 is negative, so no highlight.
        let side = [0.866_025_4, 0.5, 0.];
        let [_, specular] = palette.shade_specular(side, sun, half).unwrap();
        assert_eq!(specular, palette.shade_static(side, sun).unwrap()[1]);
        // cos 10 deg: ((1+0.98481)/2)^16 = 0.885, so 0.885*254-127 rounds to 98.
        let near = [0.173_648_18, 0.984_807_7, 0.];
        let [_, specular] = palette.shade_specular(near, sun, half).unwrap();
        assert_eq!(specular, palette.secondary[127 + 98]);
        assert_ne!(specular, 0xff000000);
    }

    #[test]
    fn half_vector_bisects_flipped_sun_and_eye() {
        // Object-local inputs arrive before the local-Z flip.
        let half = specular_half_vector([1., 0., 0.], [0., 0., -1.]);
        let expected = std::f32::consts::FRAC_1_SQRT_2;
        assert!((half[0] - expected).abs() < 1e-6);
        assert_eq!(half[1], 0.);
        assert!((half[2] - expected).abs() < 1e-6);
        // Opposite vectors cancel; 004e2ba0 then scales by its fixed reciprocal.
        assert_eq!(
            specular_half_vector([0., 1., 0.], [0., -1., 0.]),
            [0., 0., 0.]
        );
    }

    #[test]
    fn eye_direction_is_unit_from_position_to_eye() {
        let direction = eye_direction([3., 4., 0.], [0., 0., 0.]).unwrap();
        assert_eq!(direction, [0.6, 0.8, 0.]);
        assert!(eye_direction([1., 2., 3.], [1., 2., 3.]).is_err());
    }
    #[test]
    #[ignore = "requires original sun-color image and lighting observations"]
    fn matches_native_lighting() {
        let sun =
            SunColors::parse(&std::fs::read(crate::fixtures::path("SUNCOLS")).unwrap()).unwrap();
        let path = crate::fixtures::path("NATIVE_LIGHTING");
        let bytes = std::fs::read(&path).unwrap();
        let vertices = std::fs::read(crate::fixtures::path("NATIVE_LIGHTING_VERTICES")).unwrap();
        let vertex_word = |at| u32::from_le_bytes(vertices[at..at + 4].try_into().unwrap());
        assert_eq!(&vertices[..8], b"MTLITV01");
        let normal_count = vertex_word(12) as usize;
        let mut vertex_at = 16;
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTLIGHT3");
        let mut at = 12;
        for case in 0..word(8) {
            let light = sun.sample(word(at), word(at + 4)).unwrap();
            assert_eq!(light.diffuse, word(at + 8), "diffuse{case}");
            assert_eq!(light.ambient, word(at + 12), "ambient{case}");
            assert_eq!(light.scale.to_bits(), word(at + 24), "scale{case}");
            for (i, value) in light.direction.iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(at + 28 + i * 4),
                    "sun{case} component{i}"
                );
            }
            let palette = Palette::new(
                light.diffuse,
                light.ambient,
                word(at + 16),
                word(at + 20),
                light.scale,
            );
            for (i, value) in light.direction.iter().enumerate() {
                assert_eq!(value.to_bits(), vertex_word(vertex_at + i * 4));
            }
            vertex_at += 12;
            for _ in 0..normal_count {
                let normal =
                    std::array::from_fn(|i| f32::from_bits(vertex_word(vertex_at + i * 4)));
                let colors = palette.shade_static(normal, light.direction).unwrap();
                assert_eq!(
                    colors,
                    [vertex_word(vertex_at + 12), vertex_word(vertex_at + 16)],
                    "vertex case{case}"
                );
                vertex_at += 20;
            }
            assert_eq!(light.fog.color, word(at + 40), "fog color{case}");
            assert_eq!(
                light.fog.start_fraction.to_bits(),
                word(at + 44),
                "fog start{case}"
            );
            assert_eq!(
                light.fog.range_fraction.to_bits(),
                word(at + 48),
                "fog width{case}"
            );
            let distances = light.fog.distances();
            assert_eq!(
                distances.map(f32::to_bits),
                [word(at + 52), word(at + 56)],
                "fog renderer arguments{case}"
            );
            at += 60;
            for (i, color) in palette
                .primary
                .into_iter()
                .chain(palette.secondary)
                .enumerate()
            {
                assert_eq!(color, word(at + i * 4), "palette case{case} cell{i}");
            }
            at += 768 * 4;
        }
        assert_eq!(at, bytes.len());
        assert_eq!(vertex_at, vertices.len());
    }
}
