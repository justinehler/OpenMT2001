//! Original sky dome geometry and daylight colors, 004c3de0/004c4840.

pub struct Sky {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u16>,
}

#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub position: [f32; 3],
    pub color: u32,
}

/// Original004c41c0 camera-space celestial sprite, before projection.
#[derive(Clone, Copy, Debug)]
pub struct CelestialQuad {
    pub positions: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub texture: usize,
    pub color: u32,
}

/// The scene camera matrix remains explicit:004c41c0 transforms world centers
/// through saved93b248 while the renderer temporarily uses a +Z-facing camera.
pub fn celestial_quads(
    origin: [f32; 3],
    scene_view: [f32; 12],
    angle: f32,
    sun: [f32; 3],
    sun_color: u32,
) -> [CelestialQuad; 2] {
    let sun_center =
        std::array::from_fn(|i| (f64::from(sun[i]) * 100. + f64::from(origin[i])) as f32);
    let moon_angle = f64::from(angle) + 3.;
    let moon_center = [
        (moon_angle.cos() * 100. + f64::from(origin[0])) as f32,
        (f64::from(moon_angle as f32).sin() * 100. + f64::from(origin[1])) as f32,
        origin[2],
    ];
    std::array::from_fn(|texture| {
        let center =
            crate::camera::transform_scene_point(scene_view, [sun_center, moon_center][texture]);
        let [x, y, z] = center;
        CelestialQuad {
            positions: [
                [x + 6., y - 6., z],
                [x + 6., y + 6., z],
                [x - 6., y + 6., z],
                [x - 6., y - 6., z],
            ],
            uv: [[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
            texture,
            color: if texture == 0 { sun_color } else { 0xffffffff },
        }
    })
}

/// Original004c3de0 compacts the embedded catalog in input order, retaining
/// positive Y. Coordinates are already in the original sky coordinate system.
pub fn stars(image: &crate::pe::PeImage<'_>) -> Result<Vec<Star>, String> {
    let data = image.at(0x54cda8, 584 * 16)?;
    let mut stars = Vec::new();
    for record in data.chunks_exact(16) {
        let values: [f32; 4] = std::array::from_fn(|i| {
            f32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap())
        });
        if !values.into_iter().all(f32::is_finite) {
            return Err("non-finite original star record".into());
        }
        if values[1] > 0. {
            let converted = (f64::from(values[3]) * -32.).trunc() as i32;
            let alpha = 128i32.wrapping_sub(converted).min(255);
            stars.push(Star {
                position: [values[0], values[1], values[2]],
                color: (alpha as u32).wrapping_shl(24).wrapping_add(0xffffff),
            });
        }
    }
    Ok(stars)
}

/// Original004c4510 incremental cloud state. Offset3 is retained even though
/// the original vertex loop uses the offset2 delta for both secondary axes.
#[derive(Clone, Copy, Debug, Default)]
pub struct CloudMotion {
    pub offsets: [f32; 4],
    pub previous_tick: u32,
}

impl CloudMotion {
    pub fn advance(&mut self, tick: u32, coordinates: &mut [[f32; 4]]) {
        if tick == self.previous_tick {
            return;
        }
        let elapsed = f64::from(tick.wrapping_sub(self.previous_tick) as f32);
        self.previous_tick = tick;
        let mut first = elapsed * f64::from(0.001f32);
        let mut vertical = 0f64;
        let mut second = elapsed * f64::from(0.0037f32);
        self.offsets[0] = (f64::from(self.offsets[0]) + first) as f32;
        let mut secondary = f64::from(self.offsets[2]) + second;
        self.offsets[2] = secondary as f32;
        self.offsets[3] = (f64::from(self.offsets[3])
            + f64::from((elapsed * f64::from(0.0007f32)) as f32)) as f32;
        let mut primary = f64::from(self.offsets[0]);
        while primary > 1. {
            primary -= 1.;
            first -= 1.;
        }
        self.offsets[0] = primary as f32;
        let mut v = f64::from(self.offsets[1]);
        while v > 1. {
            v -= 1.;
            vertical -= 1.;
        }
        self.offsets[1] = v as f32;
        if secondary > 1. {
            while secondary > 1. {
                secondary -= 1.;
                second -= 1.;
            }
            self.offsets[2] = secondary as f32;
        }
        for uv in coordinates {
            for (value, delta) in uv.iter_mut().zip([first, vertical, second, second]) {
                *value = (f64::from(*value) + delta) as f32;
            }
        }
    }
}

/// Original004f1260 upper view angle with default Y-up, in degrees.
pub fn upper_view_angle(eye: [f32; 3], target: [f32; 3], fov: f32) -> Result<f32, String> {
    if !eye
        .into_iter()
        .chain(target)
        .chain([fov])
        .all(f32::is_finite)
        || !(0.0..180.0).contains(&fov)
        || fov == 0.
    {
        return Err("invalid sky camera".into());
    }
    let [x, y, z] = std::array::from_fn(|i| f64::from(eye[i]) - f64::from(target[i]));
    let pitch = -y.atan2((z * z + x * x).sqrt()) * f64::from(f32::from_bits(0x42652ee1));
    Ok((pitch + f64::from(fov) * 0.5) as f32)
}

/// Original004c4970 skips the dome below -20 degrees and otherwise submits
/// a prefix of its16 strips. The division retains x87 precision until FTOL.
pub fn visible_strips(upper_angle: f32) -> Result<usize, String> {
    if !upper_angle.is_finite() {
        return Err("non-finite sky angle".into());
    }
    Ok(if upper_angle < -20. {
        0
    } else {
        ((f64::from(upper_angle) / 6. + 2.).trunc() as i32).clamp(1, 16) as usize
    })
}

impl Sky {
    /// 004c4840 updates upper cloud colors only; bottom ring remains zero.
    pub fn cloud_colors(&self, sun: [f32; 3]) -> Vec<u32> {
        let mut colors = self.colors(sun);
        colors[..64].fill(0);
        for color in &mut colors[64..] {
            *color = (*color >> 1) & 0x7f7f7f7f;
        }
        colors
    }

    /// Initial cloud coordinates from004c3de0: two texture layers per vertex.
    pub fn cloud_coordinates(&self) -> Vec<[f32; 4]> {
        let mut uv = vec![[0.; 4]; self.positions.len()];
        let mut phi = 0f64;
        for ring in 0..16 {
            let tangent = ((f64::from(f32::from_bits(0x3fc90fdb))
                - (phi * f64::from(0.8f32) + f64::from(0.2f32)))
                * f64::from(0.85f32))
            .tan();
            let mut theta = 0f64;
            for sector in 0..64 {
                let u = tangent * theta.cos();
                let v = tangent * f64::from(theta.sin() as f32);
                uv[(ring + 1) * 64 + sector] = [
                    (u * f64::from(0.3f32)) as f32,
                    (v * f64::from(0.3f32)) as f32,
                    (u * f64::from(1.3f32)) as f32,
                    (f64::from(v as f32) * f64::from(1.3f32)) as f32,
                ];
                theta += f64::from(f32::from_bits(0x3dc90fdb));
            }
            phi += f64::from(f32::from_bits(0x3dd67750));
        }
        // The initializer replaces provisional bottom-ring UVs by extrapolation.
        for sector in 0..64 {
            uv[sector] = std::array::from_fn(|i| {
                let first = f64::from(uv[64 + sector][i]);
                (first - (f64::from(uv[128 + sector][i]) - first)) as f32
            });
        }
        uv
    }

    pub fn original() -> Self {
        let mut positions = Vec::with_capacity(1088);
        let step = f64::from(f32::from_bits(0x3dc90fdb));
        let mut theta = 0f64;
        for _ in 0..64 {
            positions.push([
                (theta.cos() * 3. * 200.) as f32,
                -90.,
                -(theta.sin() * 3. * 200.) as f32,
            ]);
            theta += step;
        }
        let mut phi = 0f64;
        for _ in 0..16 {
            let cosine = phi.cos();
            let radius = (cosine + cosine + 1.) as f32;
            let cosine = f64::from(cosine as f32);
            let radius = f64::from(radius);
            let y = (phi.sin() * radius * 200.) as f32;
            theta = 0.;
            for _ in 0..64 {
                positions.push([
                    (theta.cos() * cosine * radius * 200.) as f32,
                    y,
                    -(theta.sin() * cosine * radius * 200.) as f32,
                ]);
                theta += step;
            }
            phi += f64::from(f32::from_bits(0x3dd67750));
        }
        let mut indices = Vec::with_capacity(6144);
        for ring in 0..16u16 {
            for sector in 0..64u16 {
                let a = ring * 64 + sector;
                let b = ring * 64 + (sector + 1) % 64;
                indices.extend_from_slice(&[a, b, a + 64, b, b + 64, a + 64]);
            }
        }
        Self { positions, indices }
    }

    pub fn colors(&self, sun: [f32; 3]) -> Vec<u32> {
        let mut colors = vec![0; self.positions.len()];
        for (position, color) in self.positions[64..].iter().zip(&mut colors[64..]) {
            let direction =
                position.map(|v| (f64::from(v) * f64::from(f32::from_bits(0x3ba3d70a))) as f32);
            let [r, g, b] = crate::water::sky_color(direction, sun);
            *color = 0xff000000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
        }
        colors.copy_within(64..128, 0);
        colors
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original celestial quad observations"]
    fn matches_native_celestial_quads() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_CELESTIAL").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(&bytes[..8], b"MTCELES1");
        assert_eq!((word(8), word(12)), (36, 0x027f));
        let mut at = 16;
        for case in 0..36 {
            let origin = std::array::from_fn(|i| float(at + i * 4));
            let angle = float(at + 12);
            let sun = std::array::from_fn(|i| float(at + 16 + i * 4));
            let matrix = std::array::from_fn(|i| float(at + 28 + i * 4));
            at += 76;
            for quad in super::celestial_quads(origin, matrix, angle, sun, 0xffabcdef) {
                assert_eq!(word(at), [0x11111111, 0x22222222][quad.texture]);
                assert_eq!(word(at + 4), quad.color);
                at += 8;
                for value in quad
                    .positions
                    .into_iter()
                    .flatten()
                    .chain(quad.uv.into_iter().flatten())
                {
                    assert_eq!(
                        value.to_bits(),
                        word(at),
                        "celestial case {case} texture {} offset {at}",
                        quad.texture
                    );
                    at += 4;
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    use super::*;
    #[test]
    #[ignore = "requires original executable and initialized star observations"]
    fn matches_native_stars() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_STARS").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSTARS1");
        assert_eq!(word(12), 0x037f);
        let stars = stars(&image).unwrap();
        assert_eq!(stars.len(), word(8) as usize);
        assert_eq!(bytes.len(), 16 + stars.len() * 16);
        for (i, star) in stars.iter().enumerate() {
            assert_eq!(
                star.position.map(f32::to_bits),
                std::array::from_fn(|j| word(16 + i * 16 + j * 4)),
                "star{i} position"
            );
            assert_eq!(star.color, word(28 + i * 16), "star{i} color");
        }
    }
    #[test]
    #[ignore = "requires original incremental cloud animation observations"]
    fn matches_native_cloud_motion() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CLOUD_MOTION").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTCANIM1");
        assert_eq!((word(8), word(16)), (1088, 0x037f));
        let mut motion = CloudMotion {
            offsets: std::array::from_fn(|i| f32::from_bits(word(20 + i * 4))),
            previous_tick: word(36),
        };
        let mut uv = Sky::original().cloud_coordinates();
        let mut at = 40;
        let check_uv = |uv: &[[f32; 4]], at: &mut usize| {
            for (i, vertex) in uv.iter().enumerate() {
                for (j, value) in vertex.iter().enumerate() {
                    assert_eq!(value.to_bits(), word(*at), "vertex{i} coordinate{j} at{at}");
                    *at += 4;
                }
            }
        };
        check_uv(&uv, &mut at);
        for case in 0..word(12) {
            motion.advance(word(at), &mut uv);
            at += 4;
            for (i, value) in motion.offsets.iter().enumerate() {
                assert_eq!(value.to_bits(), word(at + i * 4), "case{case} offset{i}");
            }
            assert_eq!(motion.previous_tick, word(at + 16));
            at += 20;
            check_uv(&uv, &mut at);
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original cloud coordinate observations"]
    fn matches_native_cloud_coordinates() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_CLOUD_UVS").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTCLOUD2");
        assert_eq!((word(8), word(12)), (1088, 0x037f));
        assert_eq!(word(16), 6);
        for (vertex, uv) in Sky::original().cloud_coordinates().iter().enumerate() {
            for (channel, value) in uv.iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(20 + vertex * 16 + channel * 4),
                    "vertex{vertex} channel{channel}"
                );
            }
        }
        let mut at = 20 + 1088 * 16;
        for case in 0..word(16) {
            let sun = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            at += 12;
            for (i, color) in Sky::original().cloud_colors(sun).iter().enumerate() {
                assert_eq!(*color, word(at), "cloud color{case} vertex{i}");
                at += 4;
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original sky camera observations"]
    fn matches_native_sky_visibility() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SKY_VISIBILITY").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSKYVIS");
        assert_eq!(word(12), 0x027f);
        let mut at = 16;
        for case in 0..word(8) {
            let eye = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            let target = std::array::from_fn(|i| f32::from_bits(word(at + 12 + i * 4)));
            let upper = upper_view_angle(eye, target, f32::from_bits(word(at + 24))).unwrap();
            assert_eq!(upper.to_bits(), word(at + 28), "case{case} upper angle");
            assert_eq!(
                visible_strips(upper).unwrap(),
                word(at + 32) as usize,
                "case{case} strips"
            );
            at += 36;
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original sky observations"]
    fn matches_native_sky_geometry_and_colors() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_SKY").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSKY001");
        let sky = Sky::original();
        assert_eq!((word(8), word(12), word(20)), (1088, 6144, 0x037f));
        let mut at = 24;
        for (i, position) in sky.positions.iter().enumerate() {
            assert_eq!(
                position.map(f32::to_bits),
                [word(at), word(at + 4), word(at + 8)],
                "sky vertex{i}"
            );
            at += 12;
        }
        for &index in &sky.indices {
            assert_eq!(
                index,
                u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
            );
            at += 2;
        }
        for case in 0..word(16) {
            let sun = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            at += 12;
            for (i, color) in sky.colors(sun).into_iter().enumerate() {
                assert_eq!(color, word(at), "sky color{case} vertex{i}");
                at += 4;
            }
        }
        assert_eq!(at, bytes.len());
    }
}
