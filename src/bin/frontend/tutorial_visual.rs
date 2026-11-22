//! Original tutor mesh overlay used by Interface.AddArrow (4bd04e/4c5f50).
//! The native prompt arrow target is retained separately from the two meshes.
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    people,
    render::{PreparedScene, Renderer, SurfaceMode},
    render_scene::OwnedDraw,
    scene_assets,
};

const PATHS: [u32; 2] = [0x546448, 0x546424];
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
    1.,
    0.,
    -1.,
    1.,
    0.,
    1.,
];

struct Model {
    frames: Vec<Vec<[f32; 3]>>,
    uvs: Vec<[f32; 2]>,
    parts: Vec<(people::PreparedPart, usize)>,
}

/// Textures, meshes and animation frames are read from the supplied v1.4 files.
pub struct Assets {
    models: Vec<Model>,
    pages: Vec<OwnedDraw>,
}

impl Assets {
    pub fn load(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let textures = scene_assets::tutor_textures(image, index, archive, 1280)?;
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let pages = pages
            .into_iter()
            .map(|page| {
                let mut draw = OwnedDraw::texture_page(
                    SurfaceMode::Overlay,
                    page.side,
                    page.side,
                    page.rgba.into(),
                );
                draw.specular = None;
                draw.diffuse_alpha = Some(Vec::new());
                draw.mipmaps = false;
                draw
            })
            .collect();
        let mut models = Vec::new();
        for address in PATHS {
            let path = image.at(address, 260)?;
            let end = path
                .iter()
                .position(|&byte| byte == 0)
                .ok_or("unterminated tutor model path")?;
            let path = std::str::from_utf8(&path[..end]).map_err(|e| e.to_string())?;
            let entry = index
                .find(path)
                .ok_or_else(|| format!("Missing tutor model {path}"))?;
            let mesh = CachedMesh::parse(
                archive
                    .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                    .ok_or("Tutor mesh outside archive")?,
            )?;
            let materials = mesh
                .texture_names
                .iter()
                .map(|name| {
                    let key = name.rsplit('\\').next().unwrap();
                    textures
                        .iter()
                        .position(|texture| {
                            texture.name.as_ref().is_some_and(|name| {
                                name.as_bytes().get(..key.len()).is_some_and(|prefix| {
                                    prefix.eq_ignore_ascii_case(key.as_bytes())
                                })
                            })
                        })
                        .map(|i| &regions[i])
                        .ok_or_else(|| format!("Unresolved tutor texture {key}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let uvs = scene_assets::scenery_mesh_uvs(
                &mesh,
                &materials.iter().map(|region| region.uv).collect::<Vec<_>>(),
            )?;
            let prepared = people::prepare_mesh(&mesh, &mesh)?;
            let parts = prepared
                .parts
                .into_iter()
                .map(|part| {
                    let slot =
                        u16::from_le_bytes(part.material_header[4..6].try_into().unwrap()) as usize;
                    Ok((part, materials.get(slot).ok_or("Tutor material slot")?.page))
                })
                .collect::<Result<_, String>>()?;
            let frames = (0..mesh.frames)
                .map(|frame| mesh.positions(frame))
                .collect::<Result<_, _>>()?;
            models.push(Model { frames, uvs, parts });
        }
        Ok(Self { models, pages })
    }

    /// Move only the arrow origin with Interface.PositionPrompt's prompt
    /// widget. 4c5f50 fixes both tutor meshes at (250,450,100), scale 50,
    /// independent of prompt placement; AddArrow keeps its absolute target.
    /// Native head rotation is a separate animated state; this frame is unrotated.
    pub fn prepare_positioned(
        &self,
        renderer: &Renderer,
        origin: [f32; 2],
        target: [f32; 2],
        frame: usize,
        offset: [f32; 2],
    ) -> Result<PreparedScene, String> {
        let mut draws = self.pages.clone();
        for model in &self.models {
            let positions = model
                .frames
                .get(frame % model.frames.len())
                .ok_or("Tutor frame missing")?;
            for (part, page) in &model.parts {
                let draw = &mut draws[*page];
                for triangle in &part.triangles {
                    for &local in triangle {
                        let local = local as usize;
                        let source = part.source_vertices[local];
                        let p = positions[source];
                        let uv = model.uvs[source];
                        draw.vertices.push([
                            250. + p[0] * 50.,
                            450. - p[1] * 50.,
                            0.5 + p[2] * 0.001,
                            1.,
                            1.,
                            1.,
                            uv[0],
                            uv[1],
                        ]);
                        draw.diffuse_alpha.as_mut().unwrap().push(255);
                    }
                }
            }
        }
        draws.retain(|draw| !draw.vertices.is_empty());
        let origin = [origin[0] + offset[0], origin[1] + offset[1]];
        if let Some(arrow) = arrow_draw(origin, target) {
            draws.push(arrow);
        }
        renderer.prepare(
            &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
            VIEW,
            None,
        )
    }
}

/// 429eef..42a0be submits one tapered quad from the prompt's elliptical
/// border to the indicated screen point. The first two vertices are half-
/// alpha red; the last two are opaque white. Native 4e2b70 normalizes the
/// centre-to-target vector under FPCW 0x037f.
fn arrow_draw(origin: [f32; 2], target: [f32; 2]) -> Option<OwnedDraw> {
    let dx = target[0] - origin[0];
    let dy = target[1] - origin[1];
    let length = dx.hypot(dy);
    if length < 1. {
        return None;
    }
    let dir = [dx / length, dy / length];
    let start = [origin[0] + dir[0] * 160., origin[1] + dir[1] * 80.];
    let points = [
        [start[0] + dir[1] * 5., start[1] - dir[0] * 5.],
        [start[0] - dir[1] * 5., start[1] + dir[0] * 5.],
        [target[0] - 1., target[1]],
        [target[0] + 1., target[1]],
    ];
    let mut draw = OwnedDraw::texture_page(SurfaceMode::Overlay, 1, 1, vec![255u8; 4].into());
    draw.specular = None;
    draw.diffuse_alpha = Some(Vec::new());
    draw.mipmaps = false;
    for index in [0, 1, 2, 0, 2, 3] {
        let point = points[index];
        let red = index < 2;
        draw.vertices.push([point[0], point[1], 0.1,
            1., if red { 0. } else { 1. }, if red { 0. } else { 1. }, 0., 0.]);
        draw.diffuse_alpha.as_mut().unwrap().push(if red { 127 } else { 255 });
    }
    Some(draw)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_arrow_quad() {
        // 429eef..42a0be under FPCW 0x037f, original v1.4 mc.exe. The
        // independent Unicorn oracle is work/frontend/native_tutorial_arrow.py.
        for (target, expected) in [
            ([534., 461.], [[434.894, 293.9932], [427.7101, 300.9496],
                [533., 461.], [535., 461.]]),
            ([100., 100.], [[182.3298, 201.2683], [187.6985, 192.8316],
                [99., 100.], [101., 100.]]),
            ([540., 120.], [[458.069, 197.3024], [462.8575, 206.0813],
                [539., 120.], [541., 120.]]),
            ([100., 420.], [[199.333, 294.5288], [193.0007, 286.7892],
                [99., 420.], [101., 420.]]),
        ] {
            let draw = arrow_draw([320., 240.], target).unwrap();
            for (i, expected) in expected.iter().enumerate() {
                let vertex = draw.vertices[[0, 1, 2, 5][i]];
                assert!((vertex[0] - expected[0]).abs() < 0.003);
                assert!((vertex[1] - expected[1]).abs() < 0.003);
                assert_eq!(vertex[2].to_bits(), 0.1f32.to_bits());
                assert_eq!(vertex[3..6], if i < 2 { [1., 0., 0.] } else { [1.; 3] });
            }
            assert_eq!(draw.diffuse_alpha.as_deref(), Some(&[127, 127, 255, 127, 255, 255][..]));
        }
    }
    #[test]
    #[ignore = "requires original v1.4 tutor meshes and textures"]
    fn original_tutor_actor() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&image, &index, &archive).unwrap();
        assert_eq!(assets.models.len(), 2);
        assert_eq!(assets.models[0].frames.len(), 90);
        assert_eq!(assets.models[1].frames.len(), 90);
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_TUTOR_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let prepared = assets
                .prepare_positioned(&renderer, [320., 240.], [534., 461.], 50, [0., 0.])
                .unwrap();
            crate::capture_test::capture_standalone(&renderer, &prepared, &path);
        }
    }
}
