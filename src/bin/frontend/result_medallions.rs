//! Original 4b3230 owned-block corner quads in the result city.
use super::ResultPresentation;
use openmt2001::{
    archive::ArchiveIndex,
    board::Block,
    pe::PeImage,
    render::SurfaceMode,
    render_scene::OwnedDraw,
    scene_assets::{self, SceneryRegion, SceneryTexture},
    texture::{self, TextureSet},
};
use std::sync::Arc;

const GENERIC_PATH: &str = "gamedata2/GenericTextures/textures.gtd";
const QUAD_OFFSETS: [[[f32; 2]; 4]; 4] = [
    [[-42., -38.], [-38., -42.], [-34., -38.], [-38., -34.]],
    [[38., -42.], [42., -38.], [38., -34.], [34., -38.]],
    [[42., 38.], [38., 42.], [34., 38.], [38., 34.]],
    [[-38., 42.], [-42., 38.], [-38., 34.], [-34., 38.]],
];
const UV: [[f32; 2]; 4] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];

struct Texture {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
    mode: SurfaceMode,
}

pub struct Assets {
    /// Original 541af4 contains ten 0x28-byte player-character records.
    texture_for_character: [usize; 10],
    regions: Vec<SceneryRegion>,
    textures: Vec<Texture>,
}

impl Assets {
    pub fn load(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let entry = index
            .find(GENERIC_PATH)
            .ok_or("result generic textures missing")?;
        let bytes = archive
            .get(entry.offset as usize..(entry.offset + entry.length) as usize)
            .ok_or("result generic textures range")?;
        let set = TextureSet::parse(bytes).map_err(|e| e.to_string())?;
        let mut texture_for_character = [0; 10];
        for (kind, selected) in texture_for_character.iter_mut().enumerate() {
            let address = 0x541af4 + kind as u32 * 0x28;
            *selected = u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap()) as usize;
            if *selected >= set.textures.len() {
                return Err("result character medallion texture outside generic set".into());
            }
        }
        // 4c0e70 registers this GTD under group 12 with two pixels of page
        // padding. The vtable +13c draw resolves each slot's atlas rectangle.
        let records = set
            .textures
            .iter()
            .map(|source| SceneryTexture {
                name: None,
                texture: source.clone(),
                group: 12,
                padding: 2,
                dedicated: source.header[0] & 2 != 0,
            })
            .collect::<Vec<_>>();
        let (pages, regions) = scene_assets::scenery_pages(&records)?;
        let textures = pages
            .into_iter()
            .map(|page| Texture {
                width: page.side,
                height: page.side,
                rgba: page.rgba.into(),
                mode: if texture::uploaded_page_flags(page.flags) & 0x8000 != 0 {
                    SurfaceMode::StaticOpaque
                } else {
                    SurfaceMode::StaticLit
                },
            })
            .collect();
        Ok(Self {
            texture_for_character,
            regions,
            textures,
        })
    }

    /// The native renderer receives four horizontal quads per owned category.
    /// Quads are grouped by original texture while preserving category order
    /// within each group. 4b3230 already supplies world-space X/Z positions;
    /// only source mesh local Z is reversed by the retained scene assembler.
    pub fn draws(
        &self,
        blocks: &[Block],
        result: &ResultPresentation,
    ) -> Result<Vec<OwnedDraw>, String> {
        let mut batches: Vec<Option<OwnedDraw>> = vec![None; self.textures.len()];
        for block in blocks {
            let Some(character) = result.medallion_characters.get(block.id).copied().flatten()
            else {
                continue;
            };
            let texture_index = *self
                .texture_for_character
                .get(character as usize)
                .ok_or("result medallion character outside original table")?;
            let region = self
                .regions
                .get(texture_index)
                .ok_or("result medallion atlas region missing")?;
            let batch = batches[region.page].get_or_insert_with(|| {
                let texture = &self.textures[region.page];
                OwnedDraw::texture_page(
                    texture.mode,
                    texture.width,
                    texture.height,
                    Arc::clone(&texture.rgba),
                )
            });
            for quad in QUAD_OFFSETS {
                let vertices = quad.map(|[x, z]| {
                    [
                        block.center[0] + x,
                        0.5,
                        block.center[1] + z,
                        1.,
                        1.,
                        1.,
                        0.,
                        0.,
                    ]
                });
                // Native 4f2ca0 emits a four-corner strip; these two triangles
                // retain its upper-facing order in the scene camera.
                for index in [0, 1, 2, 0, 2, 3] {
                    let mut vertex = vertices[index];
                    vertex[6] = (f64::from(UV[index][0]) * f64::from(region.uv[0])
                        + f64::from(region.uv[2])) as f32;
                    vertex[7] = (f64::from(UV[index][1]) * f64::from(region.uv[1])
                        + f64::from(region.uv[3])) as f32;
                    batch.vertices.push(vertex);
                    batch.specular.as_mut().unwrap().push([0.; 3]);
                }
            }
        }
        Ok(batches.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::outcome_notice::Sequence;
    #[cfg(feature = "capture")]
    use openmt2001::{
        flyby::CameraSample,
        render_scene::{self, OriginalAssets, Request, Source},
    };
    use std::fs;
    #[cfg(feature = "capture")]
    use std::path::PathBuf;

    fn original_assets() -> (Vec<u8>, Vec<u8>, Vec<u8>, ArchiveIndex) {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        (exe, directory, archive, index)
    }

    #[test]
    #[ignore = "requires original v1.4 result medallion capture and archive"]
    fn matches_native_corner_quads() {
        let (exe, _, archive, index) = original_assets();
        let assets = Assets::load(&PeImage::parse(&exe).unwrap(), &index, &archive).unwrap();
        let fixture = fs::read(openmt2001::fixtures::path("NATIVE_RESULT_MEDALLIONS")).unwrap();
        assert_eq!(&fixture[..8], b"MTRESMD1");
        let word = |at| u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 4);
        for character in 0..10 {
            assert_eq!(
                assets.texture_for_character[character],
                word(12 + character * 4) as usize
            );
        }
        let mut at = 52;
        for case in 0..4 {
            let f32_at = |at| f32::from_bits(word(at));
            let center = [f32_at(at), f32_at(at + 4)];
            assert_eq!(word(at + 8), 4);
            at += 12;
            let mut result = ResultPresentation::new(Sequence::Win, (0, 1));
            result.medallion_characters[0] = Some(0);
            let block = Block {
                id: 0,
                name: String::new(),
                text_id: 0,
                kind: 0,
                center,
                size: [80.; 2],
                pavement_color: 0xffffffff,
                flags: 0,
                street_sides: 0,
            };
            let draws = assets.draws(&[block], &result).unwrap();
            assert_eq!(draws.len(), 1, "case {case}");
            let vertices = &draws[0].vertices;
            assert_eq!(vertices.len(), 24);
            let region = &assets.regions[assets.texture_for_character[0]];
            for quad in 0..4 {
                for (index, corner) in [0, 1, 2, 0, 2, 3].into_iter().enumerate() {
                    let source = at + (quad * 4 + corner) * 12;
                    let actual = vertices[quad * 6 + index];
                    assert_eq!(
                        actual[..3],
                        [f32_at(source), f32_at(source + 4), f32_at(source + 8)],
                        "case {case} quad {quad} corner {corner}"
                    );
                    assert_eq!(
                        actual[6..8],
                        [
                            (f64::from(UV[corner][0]) * f64::from(region.uv[0])
                                + f64::from(region.uv[2])) as f32,
                            (f64::from(UV[corner][1]) * f64::from(region.uv[1])
                                + f64::from(region.uv[3])) as f32,
                        ]
                    );
                }
            }
            at += 16 * 12;
            for quad in 0..4 {
                assert_eq!(word(at + quad * 4), 0xffffffff);
            }
            at += 16;
            for (corner, uv) in UV.iter().enumerate() {
                assert_eq!([f32_at(at + corner * 8), f32_at(at + corner * 8 + 4)], *uv);
            }
            at += 32;
        }
        assert_eq!(at, fixture.len());
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires original city assets and GPU capture path"]
    fn captures_medallions_in_original_city() {
        let (exe, directory, archive, index) = original_assets();
        let image = PeImage::parse(&exe).unwrap();
        let assets = Assets::load(&image, &index, &archive).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let block = &board.blocks[0];
        let camera = CameraSample {
            eye: [block.center[0], 155., block.center[1] + 85.],
            target: [block.center[0], 0., block.center[1]],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let build_file = fs::read(openmt2001::fixtures::path("BUILD_FILE")).unwrap();
        let map = fs::read(openmt2001::fixtures::path("MAP")).unwrap();
        let suncols = fs::read(openmt2001::fixtures::path("SUNCOLS")).unwrap();
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets {
                executable: &exe,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(12),
            camera: Some(camera),
            width: 1280,
            height: 960,
            animated: true,
        })
        .unwrap();
        let mut result = ResultPresentation::new(Sequence::Win, (0, 1));
        result.medallion_characters[block.id] = Some(0);
        let draws = assets.draws(&board.blocks, &result).unwrap();
        assert_eq!(
            draws.iter().map(|draw| draw.vertices.len()).sum::<usize>(),
            24
        );
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        let markers = renderer
            .prepare(
                &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
                scene.camera,
                scene.fog,
            )
            .unwrap();
        let path = PathBuf::from(std::env::var("OPENMT2001_RESULT_MEDALLION_CAPTURE").unwrap());
        let standalone = path.with_file_name("result-medallion-only.png");
        crate::capture_test::capture_standalone(&renderer, &markers, &standalone.to_string_lossy());
        let mut reader = png::Decoder::new(std::io::BufReader::new(
            fs::File::open(&standalone).unwrap(),
        ))
        .read_info()
        .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        assert!(
            pixels[..frame.buffer_size()]
                .chunks_exact(4)
                .any(|pixel| pixel[..3] != [6, 9, 13]),
            "result medallion vertices produced no GPU pixels"
        );
        scene.draws.extend(draws);
        let prepared = renderer
            .prepare(
                &scene
                    .draws
                    .iter()
                    .map(OwnedDraw::borrowed)
                    .collect::<Vec<_>>(),
                scene.camera,
                scene.fog,
            )
            .unwrap();
        crate::capture_test::capture_prepared(
            &renderer,
            &scene,
            &prepared,
            &path.to_string_lossy(),
        );
    }
}
