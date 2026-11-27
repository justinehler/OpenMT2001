//! Original v1.4 flag-zero 485200 rooftop and landscape details.
//!
//! 4846f0 copies a preloaded TM registration into each matching descriptor
//! row and clears its active flag. 485520 skips those rows, but the ordinary
//! 4b81cd renderer walk still submits their +0x28 model handles. This module
//! retains those original meshes and the checked row pose.

use std::collections::BTreeMap;

use crate::{object_scene, save_appearance};
use openmt2001::{
    archive::ArchiveIndex,
    lighting::{Palette, SunColors},
    mesh::CachedMesh,
    pe::PeImage,
    render::SurfaceMode,
    render_scene::OwnedDraw,
    save::appearance::RendererTemplate,
    scene::SceneBuilding,
    scene_assets,
};

pub const ROOFTOP11_DESCRIPTOR: u32 = 0x0200_000f;

/// Vertex span for one category's original registry detail in a shared page.
pub struct DrawSpan {
    pub category: usize,
    pub draw: usize,
    pub vertices: std::ops::Range<usize>,
    pub pivot_y: f32,
}

struct Source {
    descriptors: &'static [u32],
    path_address: u32,
    path: &'static str,
    vertices: usize,
    parts: usize,
    triangles: usize,
}

// Each path and registry pointer is linked by the original 4846f0 dispatch.
// Both brick-corner descriptor encodings read the same 0x688bb8 pointer.
const SOURCES: &[Source] = &[
    Source {
        descriptors: &[0x0200_0002],
        path_address: 0x5478f8,
        path: "gamedata2\\NEW_building stuff\\streetlight2.tm",
        vertices: 52,
        parts: 3,
        triangles: 24,
    },
    Source {
        descriptors: &[0x0200_0003],
        path_address: 0x5478c8,
        path: "gamedata2\\NEW_building stuff\\streetlight1.tm",
        vertices: 162,
        parts: 5,
        triangles: 84,
    },
    Source {
        descriptors: &[0x0200_0005],
        path_address: 0x547f64,
        path: "gamedata2\\NEW_building stuff\\rooftop1.tm",
        vertices: 48,
        parts: 3,
        triangles: 26,
    },
    Source {
        descriptors: &[0x0200_0006],
        path_address: 0x547f38,
        path: "gamedata2\\NEW_building stuff\\rooftop2.tm",
        vertices: 77,
        parts: 6,
        triangles: 38,
    },
    Source {
        descriptors: &[0x0200_0007],
        path_address: 0x547f0c,
        path: "gamedata2\\NEW_building stuff\\rooftop3.tm",
        vertices: 25,
        parts: 2,
        triangles: 12,
    },
    Source {
        descriptors: &[0x0200_0008],
        path_address: 0x547ee0,
        path: "gamedata2\\NEW_building stuff\\rooftop4.tm",
        vertices: 40,
        parts: 4,
        triangles: 20,
    },
    Source {
        descriptors: &[0x0200_0009],
        path_address: 0x547eb4,
        path: "gamedata2\\NEW_building stuff\\rooftop5.tm",
        vertices: 40,
        parts: 3,
        triangles: 20,
    },
    Source {
        descriptors: &[0x0200_000a],
        path_address: 0x547e88,
        path: "gamedata2\\NEW_building stuff\\rooftop6.tm",
        vertices: 50,
        parts: 3,
        triangles: 24,
    },
    Source {
        descriptors: &[0x0200_000b],
        path_address: 0x547e5c,
        path: "gamedata2\\NEW_building stuff\\rooftop7.tm",
        vertices: 109,
        parts: 2,
        triangles: 54,
    },
    Source {
        descriptors: &[0x0200_000c],
        path_address: 0x547e30,
        path: "gamedata2\\NEW_building stuff\\rooftop8.tm",
        vertices: 29,
        parts: 4,
        triangles: 13,
    },
    Source {
        descriptors: &[0x0200_000d],
        path_address: 0x547e04,
        path: "gamedata2\\NEW_building stuff\\rooftop9.tm",
        vertices: 56,
        parts: 3,
        triangles: 22,
    },
    Source {
        descriptors: &[0x0200_000e],
        path_address: 0x547dd8,
        path: "gamedata2\\NEW_building stuff\\rooftop10.tm",
        vertices: 55,
        parts: 2,
        triangles: 28,
    },
    Source {
        descriptors: &[ROOFTOP11_DESCRIPTOR],
        path_address: 0x547dac,
        path: "gamedata2\\NEW_building stuff\\rooftop11.tm",
        vertices: 20,
        parts: 2,
        triangles: 10,
    },
    Source {
        descriptors: &[0x0200_0010],
        path_address: 0x547d80,
        path: "gamedata2\\NEW_building stuff\\rooftop12.tm",
        vertices: 44,
        parts: 2,
        triangles: 28,
    },
    Source {
        descriptors: &[0x0200_0011],
        path_address: 0x547d54,
        path: "gamedata2\\NEW_building stuff\\rooftop13.tm",
        vertices: 96,
        parts: 4,
        triangles: 68,
    },
    Source {
        descriptors: &[0x0200_0012],
        path_address: 0x547d28,
        path: "gamedata2\\NEW_building stuff\\rooftop14.tm",
        vertices: 46,
        parts: 2,
        triangles: 34,
    },
    Source {
        descriptors: &[0x0200_0013],
        path_address: 0x547cfc,
        path: "gamedata2\\NEW_building stuff\\rooftop15.tm",
        vertices: 38,
        parts: 2,
        triangles: 18,
    },
    Source {
        descriptors: &[0x0200_0014],
        path_address: 0x547cd0,
        path: "gamedata2\\NEW_building stuff\\rooftop16.tm",
        vertices: 25,
        parts: 2,
        triangles: 12,
    },
    Source {
        descriptors: &[0x0200_0015],
        path_address: 0x547ca4,
        path: "gamedata2\\NEW_building stuff\\rooftop17.tm",
        vertices: 40,
        parts: 2,
        triangles: 20,
    },
    Source {
        descriptors: &[0x0200_0016],
        path_address: 0x547c78,
        path: "gamedata2\\NEW_building stuff\\rooftop18.tm",
        vertices: 40,
        parts: 2,
        triangles: 20,
    },
    Source {
        descriptors: &[0x0200_0017],
        path_address: 0x547c4c,
        path: "gamedata2\\NEW_building stuff\\rooftop19.tm",
        vertices: 34,
        parts: 2,
        triangles: 28,
    },
    Source {
        descriptors: &[0x0200_0018],
        path_address: 0x547c20,
        path: "gamedata2\\NEW_building stuff\\rooftop20.tm",
        vertices: 40,
        parts: 3,
        triangles: 20,
    },
    Source {
        descriptors: &[0x0200_0026],
        path_address: 0x546384,
        path: "gamedata2\\NEW_building stuff\\Industrial\\factory13.tm",
        vertices: 185,
        parts: 5,
        triangles: 132,
    },
    Source {
        descriptors: &[0x0200_0037],
        path_address: 0x54634c,
        path: "gamedata2\\NEW_building stuff\\Industrial\\warehouse5.tm",
        vertices: 104,
        parts: 6,
        triangles: 60,
    },
    Source {
        descriptors: &[0x0200_003b],
        path_address: 0x5462ac,
        path: "gamedata2\\NEW_building stuff\\Industrial\\tank_unit2.tm",
        vertices: 173,
        parts: 2,
        triangles: 124,
    },
    Source {
        descriptors: &[0x0200_003e],
        path_address: 0x546274,
        path: "gamedata2\\NEW_building stuff\\Industrial\\raised_tank3.tm",
        vertices: 166,
        parts: 7,
        triangles: 126,
    },
    Source {
        descriptors: &[0x0200_003f],
        path_address: 0x54623c,
        path: "gamedata2\\NEW_building stuff\\Industrial\\raised_tank4.tm",
        vertices: 175,
        parts: 6,
        triangles: 126,
    },
    Source {
        descriptors: &[0x0200_0041],
        path_address: 0x546318,
        path: "gamedata2\\NEW_building stuff\\Industrial\\plant.tm",
        vertices: 391,
        parts: 8,
        triangles: 314,
    },
    Source {
        descriptors: &[0x0200_0042],
        path_address: 0x5462e4,
        path: "gamedata2\\NEW_building stuff\\Industrial\\plant2.tm",
        vertices: 353,
        parts: 6,
        triangles: 254,
    },
    Source {
        descriptors: &[0x44, 0x0200_0044],
        path_address: 0x546640,
        path: "gamedata2\\NEW_building stuff\\landscape\\brick_corner.tm",
        vertices: 20,
        parts: 2,
        triangles: 10,
    },
    Source {
        descriptors: &[0x0200_0049],
        path_address: 0x546608,
        path: "gamedata2\\NEW_building stuff\\landscape\\park_bench.tm",
        vertices: 154,
        parts: 2,
        triangles: 90,
    },
    Source {
        descriptors: &[0x0200_004a],
        path_address: 0x5465d0,
        path: "gamedata2\\NEW_building stuff\\landscape\\flower_small.tm",
        vertices: 118,
        parts: 4,
        triangles: 64,
    },
    Source {
        descriptors: &[0x0200_004b],
        path_address: 0x546594,
        path: "gamedata2\\NEW_building stuff\\landscape\\flowerbed_1by1.tm",
        vertices: 147,
        parts: 6,
        triangles: 98,
    },
    Source {
        descriptors: &[0x0200_004c],
        path_address: 0x546558,
        path: "gamedata2\\NEW_building stuff\\landscape\\flowerbed_1by2.tm",
        vertices: 29,
        parts: 2,
        triangles: 34,
    },
    Source {
        descriptors: &[0x0200_004d],
        path_address: 0x54651c,
        path: "gamedata2\\NEW_building stuff\\landscape\\flowerbed_2by2.tm",
        vertices: 619,
        parts: 6,
        triangles: 354,
    },
    Source {
        descriptors: &[0x0200_004e],
        path_address: 0x5464e0,
        path: "gamedata2\\NEW_building stuff\\landscape\\fountain_wflowers.tm",
        vertices: 696,
        parts: 7,
        triangles: 432,
    },
    Source {
        descriptors: &[0x0200_004f],
        path_address: 0x5464ac,
        path: "gamedata2\\NEW_building stuff\\landscape\\fountain.tm",
        vertices: 696,
        parts: 7,
        triangles: 432,
    },
];

pub fn supports(descriptor: u32) -> bool {
    SOURCES
        .iter()
        .any(|source| source.descriptors.contains(&descriptor))
}

/// Retained, session-owned static models and their original atlas pages.
/// The native 4fff00 material registration is represented by the shared
/// scenery atlas. Exact GPU material state after 4e0d00 is not yet probed.
pub struct Resources {
    surfaces: Vec<OwnedDraw>,
    models: Vec<object_scene::Model>,
    descriptor_models: BTreeMap<u32, usize>,
}

impl Resources {
    pub fn load(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let textures = scene_assets::static_detail_textures(image, index, archive, 1280)?;
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let mut models = Vec::with_capacity(SOURCES.len());
        let mut descriptor_models = BTreeMap::new();
        for source in SOURCES {
            let path = image.at(source.path_address, source.path.len() + 1)?;
            if path != [source.path.as_bytes(), &[0]].concat().as_slice() {
                return Err(format!(
                    "v1.4 static TM path at {:#x} changed",
                    source.path_address
                ));
            }
            let entry = index
                .find(source.path)
                .ok_or_else(|| format!("original static TM absent: {}", source.path))?;
            let data = archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or_else(|| format!("original static TM exceeds archive: {}", source.path))?;
            let mesh = CachedMesh::parse(data)?;
            if mesh.frames != 1
                || mesh.vertex_count != source.vertices
                || mesh.parts.len() != source.parts
                || mesh
                    .parts
                    .iter()
                    .map(|part| part.indices.len() / 6)
                    .sum::<usize>()
                    != source.triangles
            {
                return Err(format!("v1.4 static TM topology changed: {}", source.path));
            }
            let model = object_scene::Model::from_mesh(&mesh, &textures, &regions, None)?;
            for &descriptor in source.descriptors {
                descriptor_models.insert(descriptor, models.len());
            }
            models.push(model);
        }
        let surfaces = pages
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
        Ok(Self {
            surfaces,
            models,
            descriptor_models,
        })
    }

    /// Build one snapshot's static instances. `save_appearance` implements
    /// the checked 488230 local-to-world row transform; the shared object
    /// preparation preserves original TM frame, UV and part ordering.
    pub fn draws(
        &self,
        instances: &[(&SceneBuilding, &RendererTemplate)],
        suncols: &[u8],
        hour: u32,
    ) -> Result<Vec<OwnedDraw>, String> {
        Ok(self.draws_with_spans(instances, suncols, hour)?.0)
    }

    pub fn draws_with_spans(
        &self,
        instances: &[(&SceneBuilding, &RendererTemplate)],
        suncols: &[u8],
        hour: u32,
    ) -> Result<(Vec<OwnedDraw>, Vec<DrawSpan>), String> {
        if instances.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let light = SunColors::parse(suncols)?.sample(hour, 0)?;
        let palette = Palette::new(light.diffuse, light.ambient, light.diffuse, 0, light.scale);
        let rgb = |value: u32| [16, 8, 0].map(|shift| ((value >> shift) & 255) as f32 / 255.);
        let mut draws = self.surfaces.clone();
        let mut spans = Vec::new();
        for &(building, row) in instances {
            let model = self
                .descriptor_models
                .get(&row.descriptor)
                .and_then(|&index| self.models.get(index))
                .ok_or("static draw received an unsupported descriptor")?;
            if u32::from_le_bytes(row.row[..4].try_into().unwrap()) != 0 {
                return Err("static draw received an active row".into());
            }
            let pose = save_appearance::saved_detail_pose(building, row)?;
            let scale: [f32; 3] = std::array::from_fn(|axis| {
                f32::from_le_bytes(row.row[24 + axis * 4..28 + axis * 4].try_into().unwrap())
            });
            if !scale
                .into_iter()
                .all(|component| component.is_finite() && component > 0.)
            {
                return Err("static row has invalid native scale".into());
            }
            let native_matrix = [
                (pose.x_axis * scale[0]).truncate().to_array(),
                (pose.y_axis * scale[1]).truncate().to_array(),
                ((-pose.z_axis) * scale[2]).truncate().to_array(),
                pose.w_axis.truncate().to_array(),
            ];
            let transform = crate::people_scene::transform_from_matrix(native_matrix);
            let sunlight = crate::people_scene::sunlight(native_matrix, light.direction);
            for part in &model.parts {
                let draw = draws
                    .get_mut(part.page)
                    .ok_or("static part page leaves original atlas")?;
                let start = draw.vertices.len();
                for (vertex, &normal) in part.vertices.iter().zip(&part.normals) {
                    let position = transform.position([vertex[0], vertex[1], vertex[2]]);
                    let shades = palette.shade_static(normal, sunlight)?;
                    let diffuse = rgb(shades[0]);
                    draw.vertices.push([
                        position[0],
                        position[1] + crate::category_distance::PASS3_Y_OFFSET,
                        position[2],
                        diffuse[0],
                        diffuse[1],
                        diffuse[2],
                        vertex[6],
                        vertex[7],
                    ]);
                    draw.specular.as_mut().unwrap().push(rgb(shades[1]));
                }
                if draw.vertices.len() > start {
                    spans.push((
                        building.block_id,
                        part.page,
                        start..draw.vertices.len(),
                        pose.w_axis.y + crate::category_distance::PASS3_Y_OFFSET,
                    ));
                }
            }
        }
        let mut indexed = draws
            .into_iter()
            .enumerate()
            .filter(|(_, draw)| !draw.vertices.is_empty())
            .collect::<Vec<_>>();
        indexed.sort_by_key(|(_, draw)| u8::from(draw.mode != SurfaceMode::StaticOpaque));
        let page_to_draw = indexed
            .iter()
            .enumerate()
            .map(|(draw, (page, _))| (*page, draw))
            .collect::<BTreeMap<_, _>>();
        let spans = spans
            .into_iter()
            .map(|(category, page, vertices, pivot_y)| {
                Ok(DrawSpan {
                    category,
                    draw: *page_to_draw
                        .get(&page)
                        .ok_or("static detail page has no draw")?,
                    vertices,
                    pivot_y,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok((indexed.into_iter().map(|(_, draw)| draw).collect(), spans))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::{
        build_file::{BuildFile, ModelMatch},
        save::appearance,
        scene::{BuildingSpec, SceneBuilding},
    };

    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    #[ignore = "requires pinned v1.4 original BuildFile, archive and native rooftop fixture"]
    fn rooftop11_static_row_reaches_original_mesh_and_draw_pose() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = PeImage::parse(&executable).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let identity = appearance::model_identity(
            &file,
            ModelMatch {
                index: 1059,
                transposed: false,
            },
            1,
        )
        .unwrap();
        assert_eq!(identity.renderer_templates.len(), 2);
        let row = &identity.renderer_templates[1];
        assert_eq!(row.descriptor, ROOFTOP11_DESCRIPTOR);
        assert_eq!(word(&row.row, 0), 0);

        let fixture = std::fs::read("work/save/appearance-gap-rooftop-draw.bin").unwrap();
        assert_eq!(&fixture[..8], b"MTROOFT1");
        assert_eq!(word(&fixture, 40), 0x037f);
        assert_eq!(word(&fixture, 44), 1059);
        let native_row = &fixture[48 + 52..48 + 104];
        assert_eq!(word(native_row, 0), 0);
        assert_eq!(word(native_row, 4), ROOFTOP11_DESCRIPTOR);
        for offset in (0..52).step_by(4).filter(|&offset| offset != 0x28) {
            assert_eq!(
                word(&row.row, offset),
                word(native_row, offset),
                "registry +{offset:#x}"
            );
        }
        let context = &fixture[48 + 104..48 + 104 + 0x60];
        assert_eq!(word(context, 0), word(native_row, 0x28));
        assert_eq!(&context[4..16], &native_row[12..24]);

        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let resources = Resources::load(&image, &index, &archive).unwrap();
        let rooftop = &resources.models[resources.descriptor_models[&ROOFTOP11_DESCRIPTOR]];
        assert_eq!(rooftop.parts.len(), 2);
        assert_eq!(
            rooftop
                .parts
                .iter()
                .map(|part| part.vertices.len())
                .sum::<usize>(),
            30
        );

        let building = SceneBuilding {
            spec: BuildingSpec {
                block: "PACIFIC_AVENUE".into(),
                lot: [0, 1, 1, 3],
                height: 1,
                kind: 0,
                variation: 0,
                quality: 0,
                businesses: Vec::new(),
            },
            block_id: 0,
            center: [0., 0.],
            selected: ModelMatch {
                index: 1059,
                transposed: false,
            },
            model_variants: 0, // Explicit profile fixture; no template query.
            angle: 0.,
            mirror: false,
            canonical_size: [1, 3],
            layers: file.profile(1059, 1).unwrap(),
        };
        // The fixture's 485200/4b81cd row verifies local pose and scale.
        // World placement follows the separately checked 488230 transform.
        let world = save_appearance::saved_detail_pose(&building, row).unwrap();
        assert_eq!(world.w_axis.truncate().to_array(), [-0.625, 7.5, 10.]);
        let suncols = read("OPENMT2001_SUNCOLS");
        let draws = resources.draws(&[(&building, row)], &suncols, 9).unwrap();
        assert_eq!(
            draws.iter().map(|draw| draw.vertices.len()).sum::<usize>(),
            30
        );
        assert!(
            draws
                .iter()
                .flat_map(|draw| &draw.vertices)
                .all(|vertex| vertex.iter().all(|value| value.is_finite()))
        );
    }

    #[test]
    #[ignore = "requires pinned v1.4 original BuildFile, PE, TM archive and suncols"]
    fn additional_static_classes_use_original_mesh_and_registry_rows() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = PeImage::parse(&executable).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let resources = Resources::load(&image, &index, &archive).unwrap();
        assert_eq!(resources.models.len(), SOURCES.len());
        for source in SOURCES {
            let model = &resources.models[resources.descriptor_models[&source.descriptors[0]]];
            assert_eq!(model.parts.len(), source.parts, "{}", source.path);
            assert_eq!(
                model
                    .parts
                    .iter()
                    .map(|part| part.vertices.len())
                    .sum::<usize>(),
                source.triangles * 3,
                "{}",
                source.path
            );
        }
        assert_eq!(
            resources.descriptor_models[&0x44],
            resources.descriptor_models[&0x0200_0044]
        );
        let suncols = read("OPENMT2001_SUNCOLS");
        for (index, descriptor, vertices) in [
            (261, 0x0200_0002, 72),
            (252, 0x0200_0003, 252),
            (1, 0x0200_0007, 36),
            (359, 0x0200_0012, 102),
            (13, 0x0200_0014, 36),
            (81, 0x0200_0015, 60),
            (45, 0x0200_0016, 60),
            (296, 0x0200_0026, 396),
            (297, 0x0200_0037, 180),
            (295, 0x0200_003b, 372),
            (299, 0x0200_003e, 378),
            (295, 0x0200_003f, 378),
            (299, 0x0200_0041, 942),
            (299, 0x0200_0042, 762),
            (233, 0x0200_0044, 30),
            (234, 0x44, 30),
            (232, 0x0200_0049, 270),
            (235, 0x0200_004b, 294),
            (267, 0x0200_004f, 1296),
        ] {
            let selected = ModelMatch {
                index,
                transposed: false,
            };
            let identity = appearance::model_identity(&file, selected, 1).unwrap();
            let row = identity
                .renderer_templates
                .iter()
                .find(|row| row.descriptor == descriptor)
                .unwrap();
            assert_eq!(word(&row.row, 0), 0, "descriptor {descriptor:#x}");
            let model = &resources.models[resources.descriptor_models[&descriptor]];
            assert_eq!(
                model
                    .parts
                    .iter()
                    .map(|part| part.vertices.len())
                    .sum::<usize>(),
                vertices
            );
            let building = SceneBuilding {
                spec: BuildingSpec {
                    block: "PACIFIC_AVENUE".into(),
                    lot: [0, 1, 1, 3],
                    height: 1,
                    kind: 0,
                    variation: 0,
                    quality: 0,
                    businesses: Vec::new(),
                },
                block_id: 0,
                center: [0., 0.],
                selected,
                model_variants: 0, // Explicit profile fixture; no template query.
                angle: 0.,
                mirror: false,
                canonical_size: [1, 3],
                layers: file.profile(index, 1).unwrap(),
            };
            let draws = resources.draws(&[(&building, row)], &suncols, 9).unwrap();
            assert_eq!(
                draws.iter().map(|draw| draw.vertices.len()).sum::<usize>(),
                vertices,
                "descriptor {descriptor:#x}"
            );
            assert!(
                draws
                    .iter()
                    .flat_map(|draw| &draw.vertices)
                    .all(|vertex| vertex.iter().all(|value| value.is_finite()))
            );
        }
    }
}
