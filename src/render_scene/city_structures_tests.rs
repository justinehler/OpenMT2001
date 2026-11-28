use super::*;

#[test]
#[ignore = "requires original v1.4 scripts, BuildFile and city assets"]
fn starting_out_and_goodspread_render_fixed_city_structures() {
    let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
    let executable = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build_file = read("BUILD_FILE");
    let image = PeImage::parse(&executable).unwrap();
    let board = Board::from_v14(&image).unwrap();
    let default = crate::fixtures::path("DEFAULT_SETTINGS");
    let scripts = default.parent().unwrap().parent().unwrap();
    for map in ["apartments2", "goodspread"] {
        let map = std::fs::read_to_string(scripts.join(format!("MAPS/{map}.lua"))).unwrap();
        let city = assemble(Request {
            build_file: &build_file,
            source: Source::Map(&map),
            original: Some(OriginalAssets {
                executable: &executable,
                directory: &directory,
                archive: &archive,
            }),
            suncols: None,
            phase: None,
            camera: None,
            width: 640,
            height: 480,
            animated: false,
        })
        .unwrap();
        let scripted = scene::parse_static_map(&map).unwrap();
        assert_eq!(city.building_count, scripted.len());
        assert_eq!(city.inspection_buildings.len(), scripted.len());
        assert_eq!(city.city_structures.len(), 5);
        let point = |p: [f32; 3]| p.map(|v| (v * 1000.).round() as i64);
        let rendered: std::collections::BTreeSet<_> = city
            .draws
            .iter()
            .flat_map(|draw| &draw.vertices)
            .map(|v| point([v[0], v[1], v[2]]))
            .collect();
        // Model indices and property pairing independently read from the
        // original 004891f0 searches over the supplied v9 model table.
        for (building, (property, model, z_offset, variation)) in city.city_structures.iter().zip([
            (2, 298, 0., 0x50),
            (35, 295, 0., 0x50),
            (42, 296, 0., 0x50),
            (39, 299, 0., 0x50),
            (31, 300, 40., 0x190),
        ]) {
            assert_eq!(
                (building.block_id, building.selected.index),
                (property, model)
            );
            assert_eq!(building.spec.variation, variation);
            assert!(!building.selected.transposed);
            let block = board.blocks.iter().find(|b| b.id == property).unwrap();
            let profile = geometry::prepare(&building.layers);
            let facade = geometry::facade_vertices_with_seed(&profile, 0);
            assert!(!facade.is_empty());
            for vertex in &facade {
                let [x, y, z] = vertex.position;
                // Utility profiles are untranslated raw models at the
                // original object's origin, with the scene's Z reversal.
                let world = [
                    x + block.center[0] - 35.,
                    y + 0.5,
                    block.center[1] + 35. + z_offset - z,
                ];
                assert!(
                    rendered.contains(&point(world)),
                    "property {property}, vertex {world:?}"
                );
            }
        }
    }
}
