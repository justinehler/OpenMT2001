//! Original interface-mode3 city map (4b2d5a), separate from the normal city.
//! The retained normal scene still supplies live selection and report data.
use crate::object_scene;
use openmt2001::{
    archive::ArchiveIndex,
    board::Block,
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer, SurfaceMode},
    render_scene::OwnedDraw,
    scene_assets,
    texture::TextureSet,
};

const MAP_PATH: &str = "gamedata2/temp_map/temp_map.tm";
const PAVEMENT_PATH: &str = "gamedata2/NEW_building stuff/pavement.tm";

pub struct Assets {
    pages: Vec<OwnedDraw>,
    map: object_scene::Model,
    pavement: object_scene::Model,
    ground: OwnedDraw,
    blocks: Vec<Block>,
    scale: f32,
    translation: [f32; 3],
}

#[derive(Default)]
pub struct View {
    key: Option<([u32; 16], Option<[u32; 52]>)>,
    prepared: Option<PreparedScene>,
}

impl View {
    /// Keep map images and geometry resident while reports/selection change.
    /// Reprepare only after the camera or retained category colors change.
    pub fn prepare(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        camera: [f32; 16],
        colors: Option<&[u32; 52]>,
    ) -> Result<&PreparedScene, String> {
        let key = (camera.map(f32::to_bits), colors.copied());
        if self.key != Some(key) {
            self.prepared = Some(assets.prepare(renderer, camera, colors)?);
            self.key = Some(key);
        }
        Ok(self.prepared.as_ref().unwrap())
    }
}

impl Assets {
    pub fn load(
        image: &PeImage<'_>,
        index: &ArchiveIndex,
        archive: &[u8],
        blocks: &[Block],
    ) -> Result<Self, String> {
        // Temp_map is in the original group6 registration and cache order.
        let textures = scene_assets::scenery_textures(image, index, archive, 1280)?;
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let pages = pages
            .into_iter()
            .map(|page| {
                OwnedDraw::texture_page(
                    if openmt2001::texture::uploaded_page_flags(page.flags) & 0x8000 != 0 {
                        SurfaceMode::StaticOpaque
                    } else {
                        SurfaceMode::StaticLit
                    },
                    page.side,
                    page.side,
                    page.rgba.into(),
                )
            })
            .collect();
        let mesh = |path| -> Result<CachedMesh<'_>, String> {
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing strategic model {path}"))?;
            CachedMesh::parse(
                archive
                    .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                    .ok_or("strategic model range")?,
            )
        };
        let map = object_scene::Model::from_mesh(&mesh(MAP_PATH)?, &textures, &regions, None)?;
        let pavement =
            object_scene::Model::from_mesh(&mesh(PAVEMENT_PATH)?, &textures, &regions, None)?;
        let entry = index
            .find("gamedata2/BuildingTextures/textures.gtd")
            .ok_or("strategic ground textures missing")?;
        let collection = TextureSet::parse(
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("strategic ground texture range")?,
        )
        .map_err(|error| error.to_string())?;
        let texture = collection
            .textures
            .get(1)
            .ok_or("strategic ground texture1 missing")?;
        let mut ground = OwnedDraw::texture_page(
            SurfaceMode::StaticOpaque,
            texture.width,
            texture.height,
            openmt2001::texture::loader_pixels(texture).into(),
        );
        for block in blocks.iter().filter(|block| block.flags & 3 == 0) {
            for index in [0, 1, 2, 0, 2, 3] {
                let x = image.f32_at(0x539a58 + index * 12)?;
                let y = image.f32_at(0x539a5c + index * 12)?;
                let z = image.f32_at(0x539a60 + index * 12)?;
                let u = image.f32_at(0x539a88 + index * 8)?;
                let v = image.f32_at(0x539a8c + index * 8)?;
                ground.vertices.push([
                    block.center[0] + x,
                    y,
                    block.center[1] - z,
                    1.,
                    1.,
                    1.,
                    u,
                    v,
                ]);
            }
        }
        ground.specular = None;
        Ok(Self {
            pages,
            map,
            pavement,
            ground,
            blocks: blocks.to_vec(),
            scale: image.f32_at(0x526600)?,
            translation: [-image.f32_at(0x5265fc)?, 0., -image.f32_at(0x525094)?],
        })
    }

    /// Original mode3 disables lighting (4ef210) and fog (4ef590), submits
    /// the map first, then native category grounds/pavements (4b8760).
    pub fn draws(&self, colors: Option<&[u32; 52]>) -> Vec<OwnedDraw> {
        let mut map = self.pages.clone();
        let mut pavements = self.pages.clone();
        for part in &self.map.parts {
            let draw = &mut map[part.page];
            for &source in &part.vertices {
                let mut vertex = source;
                vertex[..3].copy_from_slice(&map_point(
                    source[..3].try_into().unwrap(),
                    self.scale,
                    self.translation,
                ));
                draw.vertices.push(vertex);
            }
            draw.specular = None;
        }
        let mut ground = self.ground.clone();
        for (block, vertices) in self
            .blocks
            .iter()
            .filter(|block| block.flags & 3 == 0)
            .zip(ground.vertices.chunks_exact_mut(6))
        {
            let color = colors.map_or(block.pavement_color, |colors| colors[block.id]);
            for vertex in vertices {
                vertex[3..6].copy_from_slice(&rgb(color));
            }
        }
        for block in self.blocks.iter().filter(|block| block.flags & 1 == 0) {
            let color = colors.map_or(block.pavement_color, |colors| colors[block.id]);
            for part in &self.pavement.parts {
                let draw = &mut pavements[part.page];
                for &source in &part.vertices {
                    let mut vertex = source;
                    vertex[..3].copy_from_slice(&map_point(
                        source[..3].try_into().unwrap(),
                        1.,
                        [block.center[0], 0., block.center[1]],
                    ));
                    vertex[3..6].copy_from_slice(&rgb(color));
                    draw.vertices.push(vertex);
                }
                draw.specular = None;
            }
        }
        map.into_iter()
            .chain(std::iter::once(ground))
            .chain(pavements)
            .filter(|draw| !draw.vertices.is_empty())
            .collect()
    }

    pub fn prepare(
        &self,
        renderer: &Renderer,
        camera: [f32; 16],
        colors: Option<&[u32; 52]>,
    ) -> Result<PreparedScene, String> {
        let draws = self.draws(colors);
        renderer.prepare(
            &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
            camera,
            None,
        )
    }
}

fn rgb(color: u32) -> [f32; 3] {
    [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.)
}

fn map_point(source: [f32; 3], scale: f32, translation: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| {
        (f64::from(translation[i])
            + f64::from(scale) * f64::from(source[i]) * if i == 2 { -1. } else { 1. })
            as f32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original strategic controller/geometry and UV fixtures"]
    fn original_strategic_map_geometry_and_materials() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = PeImage::parse(&executable).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let native = read("OPENMT2001_NATIVE_STRATEGIC_SCENE");
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTSTRAT1");
        assert_eq!((word(8), word(12), word(16)), (2019, 61, 0x037f));
        let events = native[24..24 + word(20) as usize * 12]
            .chunks_exact(12)
            .map(|bytes| {
                std::array::from_fn::<_, 3, _>(|i| {
                    u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(events[0], [2, 28, 0]); // Native fog disable.
        assert_eq!(
            events
                .iter()
                .filter(|event| event[0] == 5)
                .copied()
                .collect::<Vec<_>>(),
            [[5, 0x4b8760, 0x688740], [5, 0x4b7770, 0x688740]]
        );
        assert_eq!(events.last(), Some(&[6, 0x164ad, 0])); // Native lighting disable.
        let aux_at = 24 + events.len() * 12;
        let at = aux_at + 284 + 48;
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let assets = Assets::load(&image, &index, &archive, &board.blocks).unwrap();
        let float = |at| f32::from_bits(word(at));
        assert_eq!(
            assets.translation,
            std::array::from_fn(|i| float(aux_at + 4 + i * 4))
        );
        assert_eq!(
            [assets.scale; 3],
            std::array::from_fn(|i| float(aux_at + 0x1c + i * 4))
        );
        let entry = index.find(MAP_PATH).unwrap();
        let mesh = CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let positions = mesh.positions(0).unwrap();
        assert_eq!(native.len(), at + positions.len() * 12);
        for (i, position) in positions.iter().enumerate() {
            let actual = map_point(*position, assets.scale, assets.translation);
            assert_eq!(
                actual.map(f32::to_bits),
                std::array::from_fn(|axis| word(at + i * 12 + axis * 4)),
                "original strategic vertex{i}"
            );
        }

        let records = scene_assets::scenery_textures(&image, &index, &archive, 1280).unwrap();
        let (_, regions) = scene_assets::scenery_pages(&records).unwrap();
        let native = read("OPENMT2001_NATIVE_STRATEGIC_UV");
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTSCUV01");
        assert_eq!(word(8), 2);
        let mut at = 12;
        for _ in 0..2 {
            let length = word(at) as usize;
            let vertices = word(at + 4) as usize;
            at += 8;
            let name = std::str::from_utf8(&native[at..at + length]).unwrap();
            at += length;
            let entry = index.find(name).unwrap();
            let mesh = CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            let rectangles = mesh
                .texture_names
                .iter()
                .map(|name| {
                    let key = name.rsplit('\\').next().unwrap();
                    let slot = records
                        .iter()
                        .position(|record| {
                            record.name.as_ref().is_some_and(|name| {
                                name.get(..key.len())
                                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key))
                            })
                        })
                        .unwrap();
                    regions[slot].uv
                })
                .collect::<Vec<_>>();
            let actual = scene_assets::scenery_mesh_uvs(&mesh, &rectangles).unwrap();
            assert_eq!(actual.len(), vertices);
            for (i, uv) in actual.iter().enumerate() {
                assert_eq!(
                    uv.map(f32::to_bits),
                    [word(at), word(at + 4)],
                    "original {name} UV{i}"
                );
                at += 8;
            }
        }
        assert_eq!(at, native.len());
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original strategic map assets and GPU"]
    fn capture_original_strategic_city_map() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = PeImage::parse(&executable).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let assets = Assets::load(&image, &index, &archive, &board.blocks).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let sample = crate::navigation::Navigation::load(&index, &archive, &executable)
            .unwrap()
            .camera;
        let [near, far] = openmt2001::camera::city_clip(sample.eye[1]).unwrap();
        let projection = openmt2001::camera::projection(
            sample.vertical_fov,
            near,
            far,
            1280,
            960,
            [0., 1., 0., 1.],
        )
        .unwrap();
        let view = openmt2001::camera::view_with_up(sample.eye, sample.target, sample.up);
        let camera = (glam::Mat4::from_cols_array(&projection)
            * glam::Mat4::from_cols_array(&view))
        .to_cols_array();
        let prepared = assets.prepare(&renderer, camera, None).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &prepared,
            "work/frontend/strategic-original-map.png",
        );
    }
}
