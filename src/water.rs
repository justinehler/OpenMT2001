//! Original fallback-water preparation (004c4f10). Scene timing and camera/sun
//! selection are explicit inputs; this does not supply an estimated environment.

/// Daylight markers recovered by 004c5740 from the original sun-color image.
#[derive(Clone, Copy, Debug)]
pub struct Daylight {
    pub sunrise: u32,
    pub sunset: u32,
}

impl Daylight {
    pub fn from_suncols(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 18 || bytes[0..3] != [0, 0, 2] || bytes[16] != 32 || bytes[17] != 8 {
            return Err("unsupported original sun-color TGA header".into());
        }
        let width = usize::from(u16::from_le_bytes(bytes[12..14].try_into().unwrap()));
        let height = usize::from(u16::from_le_bytes(bytes[14..16].try_into().unwrap()));
        if width < 384
            || height < 97
            || (bytes.len() as u64) < 18 + width as u64 * height as u64 * 4
        {
            return Err("truncated or undersized sun-color image".into());
        }
        // Original seeks to (height-97)*width*4+18, then reads 384 dwords.
        // It examines their low byte, without applying TGA display orientation.
        let at = 18 + (height - 97) * width * 4;
        let sunrise = (0..384)
            .find(|&i| (61..68).contains(&bytes[at + i * 4]))
            .ok_or("missing sunrise marker")?;
        let sunset = (0..384)
            .find(|&i| (189..196).contains(&bytes[at + i * 4]))
            .ok_or("missing sunset marker")?;
        if sunrise >= sunset {
            return Err("unsupported daylight marker order".into());
        }
        Ok(Self {
            sunrise: sunrise as u32,
            sunset: sunset as u32,
        })
    }

    /// 004c5a70's sun direction. Inputs are native calendar phase 00822950
    /// and within-phase counter 00822948, not the saved wall-clock timer words.
    pub fn sun_direction(self, phase: u32, counter: u32) -> [f32; 3] {
        let angle = self.phase_angle(phase, counter);
        normalized([
            (f64::from(angle).cos()) as f32,
            (f64::from(angle).sin()) as f32,
            0.25,
        ])
    }

    /// Original004c5a70 phase angle stored at00932cd8, shared by sun and moon.
    pub fn phase_angle(self, phase: u32, counter: u32) -> f32 {
        let time = f64::from(phase.wrapping_shl(4) as i32)
            + f64::from(counter.wrapping_shl(4) as i32) * f64::from(0.0016f32);
        let rise = f64::from(self.sunrise);
        let set = f64::from(self.sunset);
        let pi = f64::from(core::f32::consts::PI);
        (if time < rise || time > set {
            let time = if time < rise { time + 384. } else { time };
            (time - set) / (384. - (set - rise)) * pi + pi
        } else {
            (time - rise) / (set - rise) * pi
        }) as f32
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WaterPhase {
    pub previous_tick: u32,
    pub phase: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WaterVertex {
    pub uv: [f32; 2],
    pub secondary_uv: [f32; 2],
    pub color: u32,
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = v.map(f64::from);
    let inverse = 1. / ((x * x + y * y) + z * z).sqrt();
    [x, y, z].map(|c| (c * inverse) as f32)
}

/// 004c4690's color for a normalized camera-relative direction.
pub(crate) fn sky_color(direction: [f32; 3], sun: [f32; 3]) -> [i32; 3] {
    let y = f64::from(sun[1]);
    let base = [183., 210., 270.].map(|k| ((y * k) as f32).round_ties_even() as i32);
    let mut stretched = direction;
    stretched[1] = (f64::from(stretched[1]) * 20.) as f32;
    let n = normalized(stretched).map(f64::from);
    let s = sun.map(f64::from);
    let mut light = (s[2] * n[2] + s[1] * n[1]) + s[0] * n[0];
    let factor = if y < 0. {
        (1. + y) as f32
    } else {
        (1. - y) as f32
    };
    for _ in 0..if y < 0. { 4 } else { 2 } {
        light *= f64::from(factor);
    }
    let light = light as f32;
    [
        (base[0] + ((f64::from(light) * 256.) as f32).round_ties_even() as i32).clamp(0, 255),
        (base[1] + ((f64::from(light) * 64.) as f32).round_ties_even() as i32).clamp(0, 255),
        base[2].clamp(0, 255),
    ]
}

impl WaterPhase {
    pub fn fallback_vertices(
        &mut self,
        positions: &[[f32; 3]],
        camera: [f32; 3],
        sun: [f32; 3],
        tick: u32,
    ) -> Vec<WaterVertex> {
        let previous = if self.previous_tick == u32::MAX {
            tick
        } else {
            self.previous_tick
        };
        let mut retained =
            f64::from(tick.wrapping_sub(previous)) * f64::from(0.00025f32) + f64::from(self.phase);
        while retained > 1. {
            retained -= 1.;
        }
        self.phase = retained as f32;
        self.previous_tick = tick;
        positions
            .iter()
            .enumerate()
            .map(|(i, &[x, y, z])| {
                // Only the first primary U retains the unrounded phase on x87.
                let phase = if i == 0 {
                    retained
                } else {
                    f64::from(self.phase)
                };
                let uv = [
                    (f64::from(x) * 0.0091 + phase * 2.) as f32,
                    (f64::from(z) * 0.0091) as f32,
                ];
                let secondary_uv = [
                    (f64::from(x) * 0.0091 - f64::from(self.phase) * 2.) as f32,
                    (f64::from(z) * 0.0091 + f64::from(0.3f32)) as f32,
                ];
                let q = normalized([
                    (f64::from(x) - f64::from(camera[0])) as f32,
                    -(f64::from(y) - f64::from(camera[1])) as f32,
                    (f64::from(camera[2]) + f64::from(z)) as f32,
                ]);
                let sky = sky_color(q, sun);
                let rgb = [
                    (((sky[0] + 76) * 143) >> 8).min(255),
                    (((sky[1] + 96) * 223) >> 8).min(255),
                    (((sky[2] + 86) * 183) >> 8).min(255),
                ];
                WaterVertex {
                    uv,
                    secondary_uv,
                    color: ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original sun-color image and native direction observations"]
    fn matches_native_sun_direction() {
        let tga = std::fs::read(std::env::var("OPENMT2001_SUNCOLS").unwrap()).unwrap();
        let daylight = Daylight::from_suncols(&tga).unwrap();
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_SUN_PC64").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(&b[..8], b"MTSUNCY3");
        assert_eq!(daylight.sunrise, word(8));
        assert_eq!(daylight.sunset, word(12));
        assert_eq!(word(16), 16);
        assert_eq!(word(20), 0x037f);
        assert_eq!(b.len(), 24 + 16 * 24);
        for at in (24..b.len()).step_by(24) {
            assert_eq!(
                daylight.phase_angle(word(at), word(at + 4)).to_bits(),
                word(at + 20)
            );
            assert_eq!(
                daylight
                    .sun_direction(word(at), word(at + 4))
                    .map(f32::to_bits),
                [word(at + 8), word(at + 12), word(at + 16)],
                "phase {} counter {}",
                word(at),
                word(at + 4)
            );
        }
    }

    #[test]
    #[ignore = "requires original fallback-water observations"]
    fn matches_native_fallback_water() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_WATER").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f = |at| f32::from_bits(word(at));
        assert_eq!(&b[..8], b"MTWATER2");
        assert_eq!(word(8), 12);
        assert_eq!(word(12), 3);
        assert_eq!(word(16), 0x037f);
        let mut at = 20;
        for case in 0..12 {
            let camera = [f(at), f(at + 4), f(at + 8)];
            let sun = [f(at + 12), f(at + 16), f(at + 20)];
            let mut phase = WaterPhase {
                phase: f(at + 24),
                previous_tick: word(at + 28),
            };
            let tick = word(at + 32);
            let expected_phase = word(at + 36);
            at += 40;
            let positions: Vec<_> = (0..3)
                .map(|i| [f(at + i * 32), f(at + i * 32 + 4), f(at + i * 32 + 8)])
                .collect();
            let actual = phase.fallback_vertices(&positions, camera, sun, tick);
            assert_eq!(phase.phase.to_bits(), expected_phase, "phase case {case}");
            for (i, v) in actual.iter().enumerate() {
                assert_eq!(
                    v.uv.map(f32::to_bits),
                    [word(at + 12), word(at + 16)],
                    "UV case {case} vertex {i}"
                );
                assert_eq!(
                    v.secondary_uv.map(f32::to_bits),
                    [word(at + 20), word(at + 24)],
                    "secondary UV case {case} vertex {i}"
                );
                assert_eq!(v.color, word(at + 28), "color case {case} vertex {i}");
                at += 32;
            }
        }
        assert_eq!(at, b.len());
    }
}
