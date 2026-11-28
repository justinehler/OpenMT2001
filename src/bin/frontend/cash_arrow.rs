//! Original934088 cash model sequence and93440a presentation latch.
//! Advancement is driven by the481819 animation boundary, never wall time.
use crate::ui::Surface;
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer, SurfaceMode, TexturedDraw},
    scene_assets,
};

const VIEW: [f32; 16] = [
    2. / 640.,
    0.,
    0.,
    0.,
    0.,
    -2. / 480.,
    0.,
    0.,
    0.,
    0.,
    1. / 10000.,
    0.,
    -1.,
    1.,
    0.,
    1.,
];

/// Fixed constructor/draw inputs from4c607e..4c6244 and the actual4e3380
/// output under037f. This is the money object's UI matrix, without the
/// world-space Z reversal used by city objects.
fn matrix() -> [[f32; 3]; 4] {
    [
        [20., -0., 0.],
        [-0., -20., f32::from_bits(0x35eaac7a)],
        [0., f32::from_bits(0xb5eaac7a), -20.],
        [503., 66., 1000.],
    ]
}

fn transform(position: [f32; 3], pose: &[[f32; 3]; 4]) -> [f32; 3] {
    std::array::from_fn(|axis| {
        (((f64::from(position[0]) * f64::from(pose[0][axis])
            + f64::from(position[1]) * f64::from(pose[1][axis]))
            + f64::from(position[2]) * f64::from(pose[2][axis]))
            + f64::from(pose[3][axis])) as f32
    })
}

pub struct Assets {
    surfaces: Vec<Surface>,
    frames: Vec<Vec<Vec<[f32; 8]>>>,
}
impl Assets {
    pub fn load(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let entry = index
            .find("gamedata2/money/lose_money.tm")
            .ok_or("Original money model missing")?;
        let mesh = CachedMesh::parse(
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Money model outside archive")?,
        )?;
        if mesh.frames != 95 {
            return Err("Original money model frame count changed".into());
        }
        // The original money mesh is the third startup group8 model. Reuse
        // its complete registration/packing order, including earlier tutors.
        let textures = scene_assets::tutor_textures(image, index, archive, 640)?;
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let model = crate::object_scene::Model::from_mesh(&mesh, &textures, &regions, None)?;
        let mut used = Vec::new();
        for part in &model.parts {
            if !used.contains(&part.page) {
                used.push(part.page);
            }
        }
        let surfaces = used
            .iter()
            .map(|&page| {
                let source = &pages[page];
                Surface {
                    vertices: vec![],
                    alpha: vec![],
                    width: source.side,
                    height: source.side,
                    pixels: source.rgba.clone(),
                }
            })
            .collect();
        let mut pose = matrix();
        //4f24d0 reverses the third object row before the vertex emitter.
        pose[2] = pose[2].map(|value| -value);
        let mut frames = Vec::new();
        for frame in 0..mesh.frames {
            let model = crate::object_scene::Model::from_mesh_frame(
                &mesh, &textures, &regions, None, frame,
            )?;
            let mut draws = vec![Vec::new(); used.len()];
            for part in model.parts {
                let page = used.iter().position(|&page| page == part.page).unwrap();
                for mut vertex in part.vertices {
                    let position = transform([vertex[0], vertex[1], vertex[2]], &pose);
                    vertex[..3].copy_from_slice(&position);
                    draws[page].push(vertex);
                }
            }
            frames.push(draws);
        }
        Ok(Self { surfaces, frames })
    }
    fn frame_draws(&self, frame: usize) -> Result<Vec<TexturedDraw<'_>>, String> {
        let vertices = self
            .frames
            .get(frame)
            .ok_or("Cash arrow frame outside model")?;
        Ok(self
            .surfaces
            .iter()
            .zip(vertices)
            .map(|(source, vertices)| TexturedDraw {
                view_projection: None,
                vertices,
                specular: None,
                diffuse_alpha: None,
                mode: SurfaceMode::Overlay,
                width: source.width,
                height: source.height,
                rgba: &source.pixels,
                mipmaps: false,
                secondary: None,
            })
            .collect())
    }
}

#[derive(Default)]
pub struct View {
    frame: Option<i32>,
    pub active: bool,
    pub prepared: Option<PreparedScene>,
}
impl View {
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        state: State,
    ) -> Result<(), String> {
        self.active = state.active;
        if state.active && self.frame != Some(state.frame) {
            let draws = assets.frame_draws(
                usize::try_from(state.frame).map_err(|_| "Negative cash model frame")?,
            )?;
            if let Some(scene) = &mut self.prepared {
                for (index, draw) in draws.iter().enumerate() {
                    renderer.update_surface(scene, index, draw, VIEW, None)?;
                }
            } else {
                self.prepared = Some(renderer.prepare(&draws, VIEW, None)?);
            }
            self.frame = Some(state.frame);
        }
        Ok(())
    }
}
pub use openmt2001::cash_arrow::State;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original cash vertex emitter observation"]
    fn matches_native_cash_vertices() {
        let bytes =
            std::fs::read(openmt2001::fixtures::path("NATIVE_CASH_ARROW_VERTICES")).unwrap();
        assert_eq!(&bytes[..8], b"MTCPVRT1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(12)), (0x037f, 6));
        let mut pose = matrix();
        pose[2] = pose[2].map(|value| -value);
        let mut at = 16;
        for frame in [0, 20, 39, 41, 65, 94] {
            assert_eq!(word(at), frame);
            let count = word(at + 4) as usize;
            assert_eq!(count, 105);
            at += 8;
            for vertex in 0..count {
                let source =
                    std::array::from_fn(|axis| f32::from_bits(word(at + vertex * 12 + axis * 4)));
                let expected: [u32; 3] =
                    std::array::from_fn(|axis| word(at + count * 12 + vertex * 12 + axis * 4));
                assert_eq!(
                    transform(source, &pose).map(f32::to_bits),
                    expected,
                    "native frame {frame}, vertex {vertex}"
                );
            }
            at += count * 24;
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original cash render matrix observation"]
    fn matches_native_cash_matrix() {
        let bytes =
            std::fs::read(openmt2001::fixtures::path("NATIVE_CASH_ARROW_PROJECTION")).unwrap();
        assert_eq!(&bytes[..8], b"MTCPRJ01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(12)), (0x037f, 3));
        assert_eq!(bytes.len(), 16 + 3 * 128);
        for (case, row) in bytes[16..].chunks_exact(128).enumerate() {
            let values: Vec<_> = row[80..]
                .chunks_exact(4)
                .map(|row| u32::from_le_bytes(row.try_into().unwrap()))
                .collect();
            assert_eq!(
                matrix()
                    .as_flattened()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                values
            );
            if case == 0 {
                let projection: Vec<_> = row[16..80]
                    .chunks_exact(4)
                    .map(|row| u32::from_le_bytes(row.try_into().unwrap()))
                    .collect();
                assert_eq!(VIEW.map(f32::to_bits).as_slice(), projection);
            }
        }
    }
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original money model and GPU"]
    fn capture_original_cash_frames() {
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&image, &index, &archive).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut view = View::default();
        for frame in [0, 20, 39, 41, 65, 94] {
            view.update(&assets, &renderer, {
                let mut state = State::default();
                state.start(true);
                state.frame = frame;
                state
            })
            .unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                view.prepared.as_ref().unwrap(),
                &format!("frontend/cash-arrow-{frame}.png"),
            );
        }
    }
}
