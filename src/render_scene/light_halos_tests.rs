use super::*;

#[test]
#[ignore = "requires supplied original city assets"]
fn original_city_halos_follow_clock_camera_and_option() {
    let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
    let exe = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build = read("BUILD_FILE");
    let suncols = read("SUNCOLS");
    let map = read("MAP");
    let camera = crate::flyby::CameraSample {
        eye: [26., 586., -695.],
        target: [26., 463., -595.],
        up: [0., 1., 0.],
        vertical_fov: 53.,
    };
    let mut scene = assemble(Request {
        build_file: &build,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &exe,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(12),
        camera: Some(camera),
        width: 1024,
        height: 768,
        animated: true,
    })
    .unwrap();
    // The junction signal glows have no night or option gate (0x44dbe0).
    let halo_count = |scene: &Scene| {
        let signals = scene.halos.as_ref().unwrap().signal_draw();
        scene
            .draws
            .iter()
            .enumerate()
            .filter(|&(i, d)| d.mode == SurfaceMode::LightHalos && Some(i) != signals)
            .map(|(_, d)| d)
            .map(|d| d.borrowed().vertices.len() / 6)
            .sum::<usize>()
    };
    assert_eq!(halo_count(&scene), 0);
    let groups = &scene.halos.as_ref().unwrap().groups;
    assert!(!groups[0].1.is_empty());
    assert!(groups[1].1.len() > 100);
    let mut midnight_count = 0;
    for (hour, minute, visible) in [
        (21, 50, true),
        (2, 35, true),
        (6, 0, true),
        (12, 0, false),
        (18, 14, false),
        (8, 10, false),
    ] {
        let updates = scene
            .advance_daylight(hour, minute * 625 / 60, camera)
            .unwrap();
        assert!(
            scene
                .halos
                .as_ref()
                .unwrap()
                .groups
                .iter()
                .all(|(i, _)| updates.contains(i))
        );
        assert_eq!(halo_count(&scene) > 0, visible, "{hour}:{minute}");
        if hour == 2 {
            midnight_count = halo_count(&scene);
        }
        scene.set_halos_enabled(false).unwrap();
        assert_eq!(halo_count(&scene), 0);
        scene.set_halos_enabled(true).unwrap();
        assert_eq!(halo_count(&scene) > 0, visible);
    }
    scene.advance_daylight(2, 0, camera).unwrap();
    assert_eq!(halo_count(&scene), midnight_count);
    scene.apply_halo_options(true, 0).unwrap();
    let groups = &scene.halos.as_ref().unwrap().groups;
    assert!(scene.draws[groups[0].0].vertices.is_empty());
    assert!(!scene.draws[groups[1].0].vertices.is_empty());
    scene.apply_halo_options(true, 2).unwrap();
    assert_eq!(halo_count(&scene), midnight_count);
    let before = scene.draws[scene.halos.as_ref().unwrap().groups[1].0]
        .vertices
        .clone();
    let turned = crate::flyby::CameraSample {
        eye: [100., 586., -695.],
        ..camera
    };
    scene.reframe(turned).unwrap();
    assert_ne!(
        scene.draws[scene.halos.as_ref().unwrap().groups[1].0].vertices,
        before
    );
    scene.reframe(camera).unwrap();
    assert_eq!(halo_count(&scene), midnight_count);
}

#[test]
#[ignore = "requires supplied original city assets"]
fn original_city_pools_follow_close_camera_and_night_option() {
    let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
    let exe = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build = read("BUILD_FILE");
    let suncols = read("SUNCOLS");
    let map = read("MAP");
    let mut scene = assemble(Request {
        build_file: &build,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &exe,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(12),
        camera: None,
        width: 640,
        height: 480,
        animated: true,
    })
    .unwrap();
    let lamp = scene.halos.as_ref().unwrap().groups[1].1[0];
    let camera = crate::flyby::CameraSample {
        eye: [lamp.position[0], 40., lamp.position[2] - 70.],
        target: [lamp.position[0], 0., lamp.position[2]],
        up: [0., 1., 0.],
        vertical_fov: 53.,
    };
    let count = |scene: &Scene| {
        scene
            .draws
            .iter()
            .filter(|d| {
                matches!(
                    d.mode,
                    SurfaceMode::LightPoolVolume | SurfaceMode::LightPoolGlow
                )
            })
            .map(|d| d.borrowed().vertices.len())
            .sum::<usize>()
    };
    scene.advance_daylight(2, 0, camera).unwrap();
    assert!(count(&scene) > 0);
    scene.set_halos_enabled(false).unwrap();
    assert_eq!(count(&scene), 0);
    scene.apply_halo_options(true, 0).unwrap();
    assert!(count(&scene) > 0, "lamp pools survive low detail");
    scene.advance_daylight(12, 0, camera).unwrap();
    assert_eq!(count(&scene), 0);
    scene.advance_daylight(2, 0, camera).unwrap();
    assert!(count(&scene) > 0);
}
