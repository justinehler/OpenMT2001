//! Live-city 004b5760 light points from original flashing-light locators.
use openmt2001::{
    archive::ArchiveIndex, flyby::CameraSample, mesh::CachedMesh,
    pulse_lights::PulseLights, render_scene::OwnedDraw,
};

use crate::fresh_city::result::{fireworks::Point, particles_gpu};

pub struct Assets {
    positions: [[f32; 3]; 4],
}

impl Assets {
    pub fn load(index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let entry = index.find("gamedata2/landing_lights/flashing_lights.tm")
            .ok_or("original flashing-light locator model missing")?;
        let mesh = CachedMesh::parse(archive.get(entry.offset as usize
            ..(entry.offset + entry.length) as usize)
            .ok_or("flashing-light locator range")?)?;
        if mesh.vertex_count != 0 || mesh.frames != 1 || mesh.opaque_records_28.len() != 4*28 {
            return Err("unexpected original flashing-light locator topology".into());
        }
        let locator = |i: usize| {
            let bytes = &mesh.opaque_records_28[i*28..i*28+12];
            std::array::from_fn(|axis| {
                f32::from_le_bytes(bytes[axis*4..axis*4+4].try_into().unwrap())
            })
        };
        let mut first = locator(0);
        let mut second = locator(1);
        // 004b57a1 adds the caller's literal 0.5 to these two heights.
        first[1] = (f64::from(first[1]) + f64::from(0.5f32)) as f32;
        second[1] = (f64::from(second[1]) + f64::from(0.5f32)) as f32;
        Ok(Self { positions: [first, second,
            [f32::from_bits(0xc35b199a), f32::from_bits(0x42d36666), f32::from_bits(0xc3c54000)],
            [f32::from_bits(0xc4d670f6), f32::from_bits(0x4308999a), f32::from_bits(0x448ac429)]] })
    }

    /// The first three 004ec0b0 points use the emitter's size=5. The last
    /// 004ec120 point scales that size by row3's original integer 7.
    pub fn points(&self, state: &PulseLights) -> [Point; 4] {
        std::array::from_fn(|i| Point {
            position: self.positions[i],
            argb: state.rows()[i].color(),
            size: if i == 3 { 5. * state.rows()[i].size() as f32 } else { 5. },
        })
    }

    pub fn empty_draw(&self) -> OwnedDraw { particles_gpu::empty_draw() }

    pub fn fill(&self, draw: &mut OwnedDraw, state: &PulseLights,
        camera: CameraSample) -> Result<(), String> {
        particles_gpu::fill(draw, &self.points(state), camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::pe::PeImage;

    #[test]
    #[ignore = "requires original v1.4 archive and executable"]
    fn native_four_light_submissions() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&index, &archive).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let mut state = PulseLights::from_original(&image).unwrap();
        for _ in 0..50 { state.tick(); }
        let points = assets.points(&state);
        let positions = [
            [0x439800a2, 0x42da5b7e, 0xc47b02de],
            [0x438df838, 0x432f6b21, 0xc47fb79d],
            [0xc35b199a, 0x42d36666, 0xc3c54000],
            [0xc4d670f6, 0x4308999a, 0x448ac429],
        ];
        let colors = [0x0fff0000, 0xf0ff0000, 0x0f0000ff, 0xc3ffffff];
        for (i, point) in points.iter().enumerate() {
            for (axis, expected) in positions[i].into_iter().enumerate() {
                assert_eq!(point.position[axis].to_bits(), expected,
                    "point {i} axis {axis}");
            }
            assert_eq!(point.argb, colors[i], "point {i} color");
            assert_eq!(point.size, if i == 3 { 35. } else { 5. });
        }
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original flashing-light model and GPU"]
    fn captures_native_pulsing_lights() {
        use glam::Mat4;
        use openmt2001::{camera, render::Renderer};
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&index, &archive).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let mut state = PulseLights::from_original(&image).unwrap();
        for _ in 0..40 { state.tick(); }
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let folder = std::path::PathBuf::from(std::env::var("OPENMT2001_CITY_PULSE_CAPTURE").unwrap());
        std::fs::create_dir_all(&folder).unwrap();
        for (name, eye, target) in [
            ("two-red", [294., 162., 850.], [294., 142., 1013.]),
            ("blue", [-219., 130., 250.], [-219., 106., 394.5]),
            ("white", [-1715., 155., -950.], [-1715., 137., -1110.]),
        ] {
            let camera = CameraSample { eye, target, up: [0., 1., 0.], vertical_fov: 45. };
            let mut draw = assets.empty_draw();
            assets.fill(&mut draw, &state, camera).unwrap();
            let [near, far] = camera::city_clip(eye[1]).unwrap();
            let projection = camera::projection(45., near, far, 1280, 960, [0., 1., 0., 1.]).unwrap();
            let view = camera::view_with_up(eye, target, camera.up);
            let vp = (Mat4::from_cols_array(&projection)
                * Mat4::from_cols_array(&view)).to_cols_array();
            let prepared = renderer.prepare(&[draw.borrowed()], vp, None).unwrap();
            let path = folder.join(format!("city-pulse-{name}.png"));
            crate::capture_test::capture_standalone(&renderer, &prepared, &path.to_string_lossy());
            let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
                .read_info().unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let frame = reader.next_frame(&mut pixels).unwrap();
            assert!(pixels[..frame.buffer_size()].chunks_exact(4)
                .any(|pixel| pixel[..3] != [6, 9, 13]), "{name} pulse absent from GPU image");
        }
    }
}
