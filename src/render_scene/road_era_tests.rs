use super::*;

/// A scene holding only the two road pages, `width` square, as draws 1 and 2.
fn road_scene(width: u32) -> Scene {
    let page = |shade: u8| {
        OwnedDraw::texture_page(
            SurfaceMode::StaticOpaque,
            width,
            width,
            vec![shade; width as usize * width as usize * 4].into(),
        )
    };
    Scene {
        draws: vec![page(9), page(1), page(2)],
        camera: [0.; 16],
        fog: None,
        building_count: 0,
        city_structures: Vec::new(),
        category_model_heights: BTreeMap::new(),
        inspection_buildings: Vec::new(),
        saved_category_owners: None,
        saved_business_catalog: None,
        saved_commodity_rows: None,
        saved_players: Vec::new(),
        vertex_count: 0,
        diagnostics: Vec::new(),
        animation: None,
        runtime_patches: None,
        ground_colors: [0; 2],
        street_furniture: Vec::new(),
        street_furniture_era: 0,
        street_furniture_detail: 2,
        halos: None,
        station_details: Vec::new(),
        road_draws: vec![1, 2],
        road_era: None,
    }
}

#[test]
fn road_pages_change_only_when_the_clamped_era_changes() {
    let mut scene = road_scene(2);
    let mut requests = Vec::new();
    let mut set = |scene: &mut Scene, era| {
        scene
            .set_road_era(era, |selected, widths| {
                requests.push((selected, widths));
                let page = |variant: u8| -> Arc<[u8]> {
                    vec![selected as u8 * 10 + variant; 2 * 2 * 4].into()
                };
                Ok([page(0), page(1)])
            })
            .unwrap()
    };
    // The first draw replaces the built-in pages even in the 1930s.
    assert_eq!(set(&mut scene, 0), [1, 2]);
    assert_eq!(scene.road_era(), Some(0));
    assert_eq!(scene.draws[1].rgba[0], 0);
    assert_eq!(scene.draws[2].rgba[0], 1);
    assert!(set(&mut scene, 0).is_empty());
    // An era change, such as loading a 1955 save, swaps both pages.
    assert_eq!(set(&mut scene, 2), [1, 2]);
    assert_eq!(scene.draws[1].rgba[0], 20);
    assert_eq!(scene.draws[2].rgba[0], 21);
    // Eras after the 2000s keep the 2000s pages without reloading them.
    assert_eq!(set(&mut scene, 9), [1, 2]);
    assert!(set(&mut scene, 7).is_empty());
    assert!(set(&mut scene, 8).is_empty());
    assert_eq!(scene.road_era(), Some(7));
    assert_eq!(requests, [(0, [2, 2]), (2, [2, 2]), (7, [2, 2])]);
    // The unrelated draw is untouched.
    assert_eq!(scene.draws[0].rgba[0], 9);
}

#[test]
fn failed_road_loads_keep_the_previous_pages_and_era() {
    let mut scene = road_scene(2);
    assert!(
        scene
            .set_road_era(3, |_, _| Err("missing road texture".into()))
            .is_err()
    );
    assert_eq!(scene.road_era(), None);
    assert_eq!(scene.draws[1].rgba[0], 1);
    // A page of the wrong size is refused before it reaches a draw.
    let wrong: Arc<[u8]> = vec![0; 4].into();
    assert!(
        scene
            .set_road_era(3, |_, _| Ok([wrong.clone(), wrong.clone()]))
            .is_err()
    );
    assert_eq!(scene.road_era(), None);
    // Scenes without the roads mesh, such as building previews, are unchanged.
    scene.road_draws.clear();
    assert!(
        scene
            .set_road_era(3, |_, _| unreachable!())
            .unwrap()
            .is_empty()
    );
}

#[test]
#[ignore = "requires original v1.4 city assets and the installer's road textures"]
fn original_city_road_pages_follow_the_era() {
    let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build_file = read("BUILD_FILE");
    let suncols = read("SUNCOLS");
    let map = read("MAP");
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
        camera: None,
        width: 1024,
        height: 768,
        animated: true,
    })
    .unwrap();
    assert_eq!(scene.road_draws.len(), 2);
    assert_eq!(scene.road_era(), None);
    let built_in: Vec<_> = scene
        .road_draws
        .iter()
        .map(|&i| scene.draws[i].rgba.clone())
        .collect();
    for &index in &scene.road_draws {
        assert_eq!(
            (scene.draws[index].width, scene.draws[index].height),
            (256, 256)
        );
    }
    let textures = crate::fixtures::path("INSTALLER").join("Standard/roadtextures");
    let mut loader = crate::road_textures::Loader::default();
    let changed = scene
        .set_road_era(2, |era, widths| loader.pages(&textures, era, widths))
        .unwrap();
    assert_eq!(changed, scene.road_draws);
    for (part, name) in crate::road_textures::file_names(2).iter().enumerate() {
        let draw = &scene.draws[scene.road_draws[part]];
        assert_ne!(draw.rgba, built_in[part], "{name}");
        let path = std::fs::read_dir(&textures)
            .unwrap()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
            .unwrap()
            .path();
        let expected =
            crate::road_textures::page_pixels(&std::fs::read(path).unwrap(), 256).unwrap();
        assert!(draw.rgba[..] == expected[..], "{name}");
    }
    assert!(
        scene
            .set_road_era(2, |_, _| unreachable!())
            .unwrap()
            .is_empty()
    );
}
