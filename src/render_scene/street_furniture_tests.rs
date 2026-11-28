use super::*;
use crate::street_furniture::{self, Kind};

#[test]
#[ignore = "requires original v1.4 city assets and locator meshes"]
fn original_street_furniture_reaches_city_draws_and_options() {
    let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build_file = read("BUILD_FILE");
    let suncols = read("SUNCOLS");
    let map = read("MAP");
    let image = PeImage::parse(&executable).unwrap();
    let board = Board::from_v14(&image).unwrap();
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let mesh = |path| {
        let entry = index.find(path).unwrap();
        crate::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap()
    };
    let lamps = street_furniture::lamps(&board.blocks);
    assert_eq!(
        lamps.len(),
        board
            .blocks
            .iter()
            .map(|b| b.street_sides.count_ones() as usize * 2)
            .sum::<usize>()
    );
    let benches = street_furniture::benches(&mesh(street_furniture::BENCH_LOCATORS)).unwrap();
    assert_eq!(benches.len(), 22);
    // The parking presentation must agree with the real road graph's gates.
    let (roads, segments) = board.traffic_roads().unwrap();
    let mut categories = [[0; 380]; 52];
    for block in &board.blocks {
        categories[block.id][4] = 1;
        for (axis, value) in block.center.into_iter().enumerate() {
            categories[block.id][0xe0 + axis * 4..0xe4 + axis * 4]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    let network = crate::traffic::road::construction::RoadNetwork::build(
        &roads,
        segments,
        &mut categories,
        52,
        &crate::traffic::road::construction::ConstructionSettings::from_v14(&image).unwrap(),
    )
    .unwrap();
    let parking = street_furniture::parking(&board).unwrap();
    assert_eq!(
        parking.iter().map(|p| p.position).collect::<Vec<_>>(),
        network.gates.iter().map(|g| g.position).collect::<Vec<_>>()
    );
    let block = board.blocks.iter().find(|b| b.name == "BOARDWALK").unwrap();
    let camera = crate::flyby::CameraSample {
        eye: [block.center[0], 112., block.center[1] + 55.],
        target: [block.center[0], 0., block.center[1]],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let mut scene = assemble(Request {
        build_file: &build_file,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &executable,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(13),
        camera: Some(camera),
        width: 1024,
        height: 768,
        animated: true,
    })
    .unwrap();
    let count = |scene: &Scene, kind| {
        scene
            .street_furniture
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, i)| scene.draws[*i].vertices.len())
            .sum::<usize>()
    };
    let triangle_vertices = |path| {
        mesh(path)
            .parts
            .iter()
            .map(|p| p.indices.len() / 2)
            .sum::<usize>()
    };
    assert_eq!(
        count(&scene, Kind::EarlyLamp),
        lamps.len() * triangle_vertices(street_furniture::EARLY_LAMP)
    );
    assert_eq!(
        count(&scene, Kind::LateLamp),
        lamps.len() * triangle_vertices(street_furniture::LATE_LAMP)
    );
    assert_eq!(
        count(&scene, Kind::Bench),
        benches.len() * triangle_vertices(street_furniture::BENCH)
    );
    assert_eq!(
        count(&scene, Kind::Entrance),
        board.blocks.len() * triangle_vertices(street_furniture::SUBWAY)
            + parking.len() * triangle_vertices(street_furniture::PARKING)
    );
    for (era, detail) in [(0, 0), (2, 1), (6, 2), (0, 2)] {
        scene.apply_street_furniture_options(era, detail).unwrap();
        for &(kind, i) in &scene.street_furniture {
            assert_eq!(
                scene.draws[i].active_vertices,
                Some(if kind.visible(era, detail) {
                    scene.draws[i].vertices.len()
                } else {
                    0
                })
            );
        }
    }
    let retained: Vec<_> = scene
        .street_furniture
        .iter()
        .map(|(_, i)| (*i, scene.draws[*i].vertices.clone()))
        .collect();
    scene.advance_daylight(13, 950, camera).unwrap();
    for (i, vertices) in retained {
        assert_eq!(
            scene.draws[i]
                .vertices
                .iter()
                .map(|v| &v[..3])
                .collect::<Vec<_>>(),
            vertices.iter().map(|v| &v[..3]).collect::<Vec<_>>()
        );
    }
}
