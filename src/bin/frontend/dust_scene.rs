//! Native dust point streams using the retained portable billboard renderer.
//! Texture1 is our local counterpart of original687c78, not a process pointer.
use crate::result_presentation::{fireworks, particles_gpu};
use openmt2001::{
    archive::ArchiveIndex,
    demolition_effect::Point,
    flyby::CameraSample,
    render::{PreparedScene, Renderer},
    render_scene::OwnedDraw,
    texture::{TextureSet, loader_pixels},
};
use std::sync::Arc;

fn renderer_point(point: &Point) -> Result<fireworks::Point, String> {
    if point.model != 1 {
        return Err("dust point references an unbound texture".to_owned());
    }
    //4eb450 negates manager Z before4ec120 negates it again. The shared
    //helper models4ec120, so its input must retain the first negation.
    Ok(fireworks::Point {
        position: [point.position[0], point.position[1], -point.position[2]],
        argb: point.argb,
        size: point.size,
    })
}

pub struct Assets {
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}
impl Assets {
    pub fn load(index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        // Native4c0fe0 registers GTD records in source order with alpha1.
        //4b0c39 selects its first registered handle for each dust emitter.
        let entry = index
            .find("gamedata2/billboardrepeattextures/textures.gtd")
            .ok_or("original billboard-repeat textures missing")?;
        let set = TextureSet::parse(
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("billboard texture range")?,
        )
        .map_err(|error| error.to_string())?;
        let texture = set
            .textures
            .first()
            .ok_or("empty original billboard-repeat bank")?;
        Ok(Self {
            width: texture.width,
            height: texture.height,
            pixels: Arc::from(loader_pixels(texture)),
        })
    }
    pub(crate) fn empty(&self) -> OwnedDraw {
        let mut draw = particles_gpu::empty_draw();
        draw.width = self.width;
        draw.height = self.height;
        draw.rgba = self.pixels.clone();
        draw
    }
}
#[derive(Default)]
pub struct View {
    pub prepared: Option<PreparedScene>,
    pub active: bool,
    draws: Vec<OwnedDraw>,
}
impl View {
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        points: &[Point],
        camera: CameraSample,
        projection: [f32; 16],
    ) -> Result<(), String> {
        self.active = !points.is_empty();
        if !self.active {
            return Ok(());
        }
        if self.draws.is_empty() {
            self.draws = (0..16).map(|_| assets.empty()).collect();
        }
        // Original4eb580 clears/submits its512-point renderer separately for
        //each emitter. All sixteen500-capacity streams retain their own cap.
        for (slot, draw) in self.draws.iter_mut().enumerate() {
            let group = points
                .iter()
                .filter(|point| usize::from(point.slot) == slot)
                .map(renderer_point)
                .collect::<Result<Vec<_>, _>>()?;
            particles_gpu::fill_with_scale(draw, &group, camera, 1.)?;
        }
        if let Some(prepared) = &mut self.prepared {
            for (slot, draw) in self.draws.iter().enumerate() {
                renderer.update_dynamic_surface(
                    prepared,
                    slot,
                    &draw.borrowed(),
                    projection,
                    None,
                )?;
            }
        } else {
            let draws = self
                .draws
                .iter()
                .map(OwnedDraw::borrowed)
                .collect::<Vec<_>>();
            self.prepared = Some(renderer.prepare(&draws, projection, None)?);
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "full native4eb450/4ec120/4ec270/178 dust adapter oracle"]
    fn original_dust_point_adapter() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_DUST_POINT_ADAPTER")).unwrap();
        let mut at = 0;
        let take = |at: &mut usize, n: usize| {
            let row = &bytes[*at..*at + n];
            *at += n;
            row
        };
        let word = |at: &mut usize| u32::from_le_bytes(take(at, 4).try_into().unwrap());
        assert_eq!(take(&mut at, 8), b"MTDADP01");
        assert_eq!(word(&mut at), 0x37f);
        assert_eq!(word(&mut at), 3);
        for case in 0..3 {
            let position = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let size = f32::from_bits(word(&mut at));
            let native: [[u32; 7]; 4] =
                std::array::from_fn(|_| std::array::from_fn(|_| word(&mut at)));
            let point = renderer_point(&Point {
                position,
                size,
                argb: 0xecffffff,
                model: 1,
                slot: 0,
            })
            .unwrap();
            let mut draw = particles_gpu::empty_draw();
            particles_gpu::fill_with_scale(
                &mut draw,
                &[point],
                CameraSample {
                    eye: [0.; 3],
                    target: [0., 0., -1.],
                    up: [0., 1., 0.],
                    vertical_fov: 45.,
                },
                1.,
            )
            .unwrap();
            for (vertex, index) in draw.vertices.iter().zip([0, 1, 2, 0, 2, 3]) {
                assert_eq!(
                    vertex[..3].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    native[index][..3],
                    "dust XYZ case{case}"
                );
                assert_eq!(
                    vertex[6..].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    native[index][5..],
                    "dust UV case{case}"
                );
                assert_eq!(native[index][3], 0xecffffff);
            }
            assert_eq!(draw.diffuse_alpha.as_ref().unwrap(), &[236; 6]);
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original billboard texture, native dust manager and GPU"]
    fn original_dust_growth_and_retirement_capture() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&index, &archive).unwrap();
        assert!(assets.pixels.chunks_exact(4).any(|pixel| pixel[3] != 255));
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        //Controlled emitters use positive native Z123..179; face that region
        //rather than compensating for the former erroneous Z reflection.
        let camera = CameraSample {
            eye: [160., 140., -15.],
            target: [160., 20., 160.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let projection =
            openmt2001::camera::projection(45., 1., 2000., 1280, 960, [0., 1., 0., 1.]).unwrap();
        let view_matrix = openmt2001::camera::view_with_up(camera.eye, camera.target, camera.up);
        let vp = (glam::Mat4::from_cols_array(&projection)
            * glam::Mat4::from_cols_array(&view_matrix))
        .to_cols_array();
        let mut manager = openmt2001::demolition_effect::Manager::new(0);
        for slot in 0..8 {
            manager
                .create([
                    1,
                    (70. + slot as f32 * 25.).to_bits(),
                    (123. + slot as f32 * 8.).to_bits(),
                    20f32.to_bits(),
                    20f32.to_bits(),
                    500,
                ])
                .unwrap();
        }
        let mut view = View::default();
        for tick in 0..140 {
            manager.advance(&image).unwrap();
            if [0, 99, 139].contains(&tick) {
                let points = manager.points(&image).unwrap();
                view.update(&assets, &renderer, &points, camera, vp)
                    .unwrap();
                assert_eq!(
                    view.draws
                        .iter()
                        .map(|draw| draw.vertices.len())
                        .sum::<usize>(),
                    points.len() * 6
                );
                let path = format!("work/frontend/dust-{tick}.png");
                crate::capture_test::capture_standalone(
                    &renderer,
                    view.prepared.as_ref().unwrap(),
                    &path,
                );
            }
        }
        assert_eq!(
            manager.points(&image).unwrap().len(),
            8 * 128,
            "all eight original emitter streams remain beyond one512-point batch"
        );
        for slot in 0..8 {
            manager.release(slot).unwrap();
        }
        for _ in 0..129 {
            manager.advance(&image).unwrap();
        }
        view.update(
            &assets,
            &renderer,
            &manager.points(&image).unwrap(),
            camera,
            vp,
        )
        .unwrap();
        assert!(!view.active);
        assert!(!manager.allocated());
    }
}
