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

#[test]
#[ignore = "requires original v1.4 city assets"]
fn original_junction_traffic_lights_reach_city_draws_era_models_and_glows() {
    use crate::traffic_lights::{self, Glow};
    let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build_file = read("BUILD_FILE");
    let suncols = read("SUNCOLS");
    let map = read("MAP");
    let image = PeImage::parse(&executable).unwrap();
    let board = Board::from_v14(&image).unwrap();
    let posts = traffic_lights::posts(&board).unwrap();
    // Every post belongs to a junction with more than two links, and the
    // original board has full 4-way crossings.
    let mut per_crossing = BTreeMap::<usize, usize>::new();
    for post in &posts {
        *per_crossing.entry(post.crossing).or_default() += 1;
    }
    assert!(per_crossing.values().all(|&n| n == 3 || n == 4));
    assert!(per_crossing.values().any(|&n| n == 4));
    let lanes = board.fresh_crossing_lanes();
    assert!(posts.iter().all(|p| p.crossing < lanes.len()));

    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let triangle_vertices = |path| {
        let entry = index.find(path).unwrap();
        crate::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap()
        .parts
        .iter()
        .map(|p| p.indices.len() / 2)
        .sum::<usize>()
    };
    let junction = posts
        .iter()
        .find(|p| per_crossing[&p.crossing] == 4)
        .unwrap();
    let camera = crate::flyby::CameraSample {
        eye: [junction.position[0], 90., junction.position[2] - 80.],
        target: [junction.position[0], 0., junction.position[2]],
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
    assert_eq!(
        count(&scene, Kind::EarlySignal),
        posts.len() * triangle_vertices(traffic_lights::EARLY_POST)
    );
    assert_eq!(
        count(&scene, Kind::LateSignal),
        posts.len() * triangle_vertices(traffic_lights::LATE_POST)
    );
    let active = |scene: &Scene, kind| {
        scene
            .street_furniture
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, i)| scene.draws[*i].active_vertices.unwrap())
            .sum::<usize>()
    };
    scene.set_street_furniture_era(1).unwrap();
    assert!(active(&scene, Kind::EarlySignal) > 0);
    assert_eq!(active(&scene, Kind::LateSignal), 0);
    scene.set_street_furniture_era(2).unwrap();
    assert_eq!(active(&scene, Kind::EarlySignal), 0);
    assert!(active(&scene, Kind::LateSignal) > 0);

    // Glows follow the crossing masks the scene is given, and the camera.
    let glow_draw = scene.halos.as_ref().unwrap().signal_draw().unwrap();
    let view = crate::camera::scene_view_with_up(camera.eye, camera.target, camera.up);
    let expected = |lanes: &[[u8; 32]]| {
        posts
            .iter()
            .filter_map(|p| p.glow(lanes).unwrap().map(|g| (p, g)))
            .filter(|(p, _)| p.glow_alpha(&view) > 0)
            .collect::<Vec<_>>()
    };
    let colors = |scene: &Scene| {
        let mut colors = scene.draws[glow_draw]
            .vertices
            .iter()
            .map(|v| [v[3], v[4], v[5]].map(|c| c as u8))
            .collect::<Vec<_>>();
        colors.dedup();
        colors
    };
    // Fresh crossings are green for directions 1|4. Looking along +Z, only
    // the yaw-0 posts (direction 1) face the camera, so every point is green.
    let fresh = expected(&lanes);
    assert!(!fresh.is_empty());
    assert!(fresh.iter().all(|(p, g)| *g == Glow::Green && p.yaw == 0.));
    assert_eq!(colors(&scene), [[0, 1, 0]]);
    let all = |green: u32, amber: u32| {
        let mut row = [0; 32];
        row[..4].copy_from_slice(&green.to_le_bytes());
        row[4..8].copy_from_slice(&amber.to_le_bytes());
        vec![row; lanes.len()]
    };
    let changed = scene.set_signal_lanes(&all(15, 0)).unwrap();
    assert_eq!(changed, [glow_draw]);
    assert!(!scene.draws[glow_draw].vertices.is_empty());
    assert_eq!(colors(&scene), [[0, 1, 0]]);
    assert!(scene.set_signal_lanes(&all(15, 0)).unwrap().is_empty());
    scene.set_signal_lanes(&all(0, 0)).unwrap();
    assert_eq!(colors(&scene), [[1, 0, 0]]);
    // Amber removes every glow point but keeps the post meshes.
    scene.set_signal_lanes(&all(0, 15)).unwrap();
    assert!(scene.draws[glow_draw].vertices.is_empty());
    assert!(active(&scene, Kind::LateSignal) > 0);
    // Points appear only for posts whose axis faces along the view depth.
    scene.set_signal_lanes(&all(15, 0)).unwrap();
    let facing = expected(&all(15, 0)).len();
    assert!(facing > 0 && facing < posts.len());
    assert!(scene.draws[glow_draw].vertices.len() <= facing * 2 * 6);
    let alpha = scene.draws[glow_draw].diffuse_alpha.as_ref().unwrap();
    assert!(alpha.iter().all(|&a| a > 0));
}
