//! Fixed chimney-bearing station details in the original 0068a978 list.
use super::OwnedDraw;
use crate::street_furniture::Pose;

// 4c0b0d, 4c0b30 and 4c0be2 register these models in 00687a68.
// 450014..45006a copies them into 0068a978 with the default auxiliary:
// zero translation/rotation, unit scale (4df9c0). Positions are baked in.
// Materials come from the City Surround group (4c0240), retaining the TM's
// texture slots and 4ea510 atlas UVs in the ordinary state-1 pass (4f22d0).
const MODELS: [(&str, u32); 3] = [
    (
        "gamedata2/New_building stuff/train_stations/shortline_fact6.tm",
        0x54b978,
    ),
    (
        "gamedata2/New_building stuff/train_stations/bando_fact1.tm",
        0x54b93c,
    ),
    (
        "gamedata2/New_building stuff/train_stations/pensyl_fact2.tm",
        0x54b810,
    ),
];

pub(super) fn paths() -> impl Iterator<Item = &'static str> {
    MODELS.into_iter().map(|(path, _)| path)
}

pub(super) fn pose(path: &str) -> Option<Pose> {
    paths()
        .any(|registered| registered == path)
        .then(|| Pose::identity([0.; 3]))
}

/// Normal pass 4b39b1 -> 4e0d50 submits the list after the city surrounds.
/// 452570 enables it for nonzero detail, setting both distance thresholds to
/// 999999; there is no percentage-distance or era gate. The shared static
/// scenery path retains state-1 materials, daylight and normal camera clipping.
pub(super) fn set_detail(
    draws: &mut [OwnedDraw],
    indices: &[usize],
    detail: u8,
) -> Result<Vec<usize>, String> {
    let mut changed = Vec::new();
    for &index in indices {
        let draw = draws.get_mut(index).ok_or("station factory draw missing")?;
        let count = if detail != 0 { draw.vertices.len() } else { 0 };
        if draw.active_vertices != Some(count) {
            draw.active_vertices = Some(count);
            changed.push(index);
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::SurfaceMode;

    #[test]
    fn synthetic_registration_preserves_baked_positions_and_detail_gate() {
        let registered: Vec<_> = paths().collect();
        assert_eq!(registered.len(), 3);
        for name in ["shortline_fact6.tm", "bando_fact1.tm", "pensyl_fact2.tm"] {
            let path = format!("gamedata2/New_building stuff/train_stations/{name}");
            assert!(registered.contains(&path.as_str()));
            let transform = pose(&path).unwrap().transform();
            // The mesh already contains city coordinates; only native Z flips.
            assert_eq!(transform.position([-120., 42., 365.]), [-120., 42., -365.]);
        }
        assert!(pose("gamedata2/New_building stuff/train_stations/bando_fact2.tm").is_none());
        let mut draws = vec![OwnedDraw::texture_page(
            SurfaceMode::StaticOpaque,
            1,
            1,
            vec![255; 4].into(),
        )];
        draws[0].vertices = vec![[1.; 8]; 6];
        for detail in [0, 1, 0, 2, 0] {
            assert_eq!(set_detail(&mut draws, &[0], detail).unwrap(), [0]);
            assert_eq!(
                draws[0].active_vertices,
                Some(if detail == 0 { 0 } else { 6 })
            );
            assert!(set_detail(&mut draws, &[0], detail).unwrap().is_empty());
            assert_eq!(draws[0].vertices, vec![[1.; 8]; 6]);
        }
        assert!(set_detail(&mut draws, &[1], 1).is_err());
    }

    #[test]
    #[ignore = "requires original v1.4 city meshes, maps and save"]
    fn original_factories_reach_city_draws_with_materials_and_smoke_alignment() {
        use crate::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage, render_scene::*};
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let executable = read("EXE");
        let directory = read("ARCHIVE_DIR");
        let archive = read("ARCHIVE_BIN");
        let build_file = read("BUILD_FILE");
        let suncols = read("SUNCOLS");
        let save = read("SAVE_TWO");
        let default = crate::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default.parent().unwrap().parent().unwrap();
        let fresh = std::fs::read_to_string(scripts.join("MAPS/apartments2.lua")).unwrap();
        let skirmish = std::fs::read_to_string(scripts.join("MAPS/goodspread.lua")).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let mesh = |path| {
            let entry = index.find(path).unwrap();
            CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap()
        };
        let materials =
            crate::scene_assets::static_detail_textures(&image, &index, &archive, 1280).unwrap();
        let (pages, regions) = crate::scene_assets::scenery_pages(&materials).unwrap();
        let locator_mesh = mesh("gamedata2/NEW_building stuff/smoke_stack_locators.tm");
        let locators: Vec<[f32; 3]> = locator_mesh
            .opaque_records_28
            .as_chunks::<28>()
            .0
            .iter()
            .map(|row| {
                std::array::from_fn(|axis| {
                    f32::from_le_bytes(row[axis * 4..axis * 4 + 4].try_into().unwrap())
                })
            })
            .collect();
        assert_eq!(locators.len(), 3);
        let mut matched_locators = std::collections::BTreeSet::new();
        let mut expected = Vec::new();
        for (path, address) in MODELS {
            let bytes = image.at(address, 260).unwrap();
            let end = bytes.iter().position(|&b| b == 0).unwrap();
            assert_eq!(
                std::str::from_utf8(&bytes[..end])
                    .unwrap()
                    .replace('\\', "/"),
                path
            );
            let model = mesh(path);
            let positions = model.positions(0).unwrap();
            let top = positions
                .iter()
                .map(|v| v[1])
                .fold(f32::NEG_INFINITY, f32::max);
            let rim: Vec<_> = positions
                .iter()
                .filter(|v| (v[1] - top).abs() < 0.001)
                .collect();
            let middle = |axis| {
                let min = rim.iter().map(|v| v[axis]).fold(f32::INFINITY, f32::min);
                let max = rim
                    .iter()
                    .map(|v| v[axis])
                    .fold(f32::NEG_INFINITY, f32::max);
                (min + max) / 2.
            };
            // 4b5c10 adds two Y units to each locator before emitting smoke.
            // Preserve the authored sub-unit rim offsets, without snapping it.
            let matched: Vec<_> = locators
                .iter()
                .enumerate()
                .filter(|(_, p)| (p[0] - middle(0)).abs() < 0.1 && (p[2] - middle(2)).abs() < 0.1)
                .collect();
            assert_eq!(matched.len(), 1, "{path}");
            let (locator, position) = matched[0];
            assert!((position[1] + 2. - top).abs() < 0.75, "{path}");
            assert!(matched_locators.insert(locator));
            let registrations: Vec<_> = model
                .texture_names
                .iter()
                .map(|name| {
                    materials
                        .iter()
                        .position(|m| {
                            m.name
                                .as_ref()
                                .is_some_and(|key| key.eq_ignore_ascii_case(name))
                        })
                        .unwrap_or_else(|| panic!("missing original factory material {name}"))
                })
                .collect();
            let rectangles: Vec<_> = registrations.iter().map(|&i| regions[i].uv).collect();
            let uvs = crate::scene_assets::scenery_mesh_uvs(&model, &rectangles).unwrap();
            let mut parts = std::collections::BTreeMap::<_, Vec<_>>::new();
            for part in &model.parts {
                let vertices = parts.entry(usize::from(part.texture)).or_default();
                for i in part.vertex_indices() {
                    let [x, y, z] = positions[i];
                    let [u, v] = uvs[i];
                    vertices.push([x, y, -z, u, v].map(f32::to_bits));
                }
            }
            for (material, vertices) in parts {
                expected.push((vertices, regions[registrations[material]].page));
            }
        }
        assert_eq!(matched_locators.len(), 3);
        let camera = crate::flyby::CameraSample {
            eye: [100., 112., 500.],
            target: [100., 0., 0.],
            up: [0., 1., 0.],
            vertical_fov: 53.,
        };
        for source in [
            Source::Map(&fresh),
            Source::Map(&skirmish),
            Source::Save(&save),
            Source::RuntimeBuildings {
                records: &[],
                category_base: 0x6179a0,
            },
        ] {
            let mut city = assemble(Request {
                build_file: &build_file,
                source,
                original: Some(OriginalAssets {
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: Some(&suncols),
                phase: Some(13),
                camera: Some(camera),
                width: 640,
                height: 480,
                animated: true,
            })
            .unwrap();
            assert_eq!(city.station_factories.len(), expected.len());
            for (vertices, page) in &expected {
                let draw = city
                    .station_factories
                    .iter()
                    .map(|&i| &city.draws[i])
                    .find(|draw| {
                        draw.vertices
                            .iter()
                            .map(|v| [v[0], v[1], v[2], v[6], v[7]].map(f32::to_bits))
                            .eq(vertices.iter().copied())
                    })
                    .expect("factory material must retain every original indexed vertex and UV");
                let page = &pages[*page];
                assert_eq!((draw.width, draw.height), (page.side, page.side));
                assert_eq!(draw.rgba.as_ref(), page.rgba);
                assert_eq!(
                    draw.mode,
                    if crate::texture::uploaded_page_flags(page.flags) & 0x8000 != 0 {
                        SurfaceMode::StaticOpaque
                    } else {
                        SurfaceMode::StaticLit
                    }
                );
                assert!(draw.specular.is_some());
            }
            for detail in [0, 1, 2, 0] {
                city.set_station_factory_detail(detail).unwrap();
                for &i in &city.station_factories {
                    let draw = &city.draws[i];
                    assert_eq!(
                        draw.active_vertices,
                        Some(if detail == 0 { 0 } else { draw.vertices.len() })
                    );
                }
            }
            let positions = |city: &Scene| {
                city.station_factories
                    .iter()
                    .flat_map(|&i| {
                        city.draws[i]
                            .vertices
                            .iter()
                            .map(|v| [v[0], v[1], v[2]].map(f32::to_bits))
                    })
                    .collect::<Vec<_>>()
            };
            let before = positions(&city);
            city.advance_daylight(13, 950, camera).unwrap();
            assert_eq!(positions(&city), before);
            for &i in &city.station_factories {
                assert_eq!(city.draws[i].active_vertices, Some(0));
            }
        }
    }
}
