use super::*;
use std::time::UNIX_EPOCH;

#[test]
fn mixed_case_load_rows_target_the_displayed_file_after_refresh() {
    let root = std::env::temp_dir().join(format!(
        "omt-load-order-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let profile = root.join("profile");
    let imports = root.join("imports");
    fs::create_dir_all(&imports).unwrap();
    let mut bytes = Vec::new();
    for word in [0x2000u32, 16, 7, 12, 100, 3, u32::MAX, 0] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for name in ["cherry", "apple", "_x", "Banana", "1a"] {
        crate::save_menu::write_target(
            &profile,
            &crate::save_menu::Target::NewName(name.into()),
            &bytes,
        )
        .unwrap();
    }
    // Imports and profile slots interleave by filename, not directory path.
    // Identical names in distinct directories still have distinct targets.
    for name in ["aardvark", "apple"] {
        fs::write(imports.join(format!("{name}.sav")), &bytes).unwrap();
    }
    let mut bounds = vec![[0.; 4]; 25];
    bounds[8] = [20., 40., 300., 327.];
    let mut load = Load {
        base: Vec::new(),
        info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
            .unwrap(),
        bounds,
        fonts: Vec::new(),
        tab: Vec::new(),
        list_fonts: Vec::new(),
        list_tab: Vec::new(),
        list_layout: openmt2001::frontend_list::Layout::new([20., 40., 284., 302.], 11., 3.)
            .unwrap(),
        description_fonts: Vec::new(),
        description_tab: Vec::new(),
        slider_textures: Vec::new(),
        title_fonts: Vec::new(),
        title_tab: Vec::new(),
        strings: Vec::new(),
        level_text: LevelTextCatalog::default(),
        directories: vec![profile.join("saves"), imports.clone()],
        app_root: root.clone(),
        import_directory: imports.clone(),
        campaign_filename: None,
        profile_root: profile.clone(),
        entries: Vec::new(),
        chosen: None,
        first: 0,
        description_first: 0,
        over_description: false,
        pointer_position: None,
        visible: false,
        busy: false,
        request: None,
        message: None,
        delete_confirmation: None,
    };
    load.open();
    assert!(load.message.is_none());
    let mut expected = vec![
        profile.join("saves/1a.sav"),
        imports.join("aardvark.sav"),
        imports.join("apple.sav"),
        profile.join("saves/apple.sav"),
        profile.join("saves/Banana.sav"),
        profile.join("saves/cherry.sav"),
        profile.join("saves/_x.sav"),
    ];
    for pass in 0..2 {
        assert_eq!(
            load.entries.iter().map(|e| &e.path).collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        assert_eq!(load.chosen, None);
        for (row, path) in expected.iter().enumerate() {
            assert_eq!(
                load.entries[row].name,
                ascii(path.file_stem().unwrap().to_str().unwrap())
            );
            let index = load
                .hit([30., 45. + row as f32 * load.list_layout.pitch])
                .unwrap();
            load.activate(index);
            assert_eq!(load.chosen, Some(row));
            load.activate(0);
            assert_eq!(load.request.as_ref(), Some(path));
            load.busy = false;
        }
        if pass == 0 {
            load.activate(3 + 4); // Banana
            load.activate(2);
            assert_eq!(load.delete_confirmation.as_deref(), Some("Banana.sav"));
            load.activate(0); // Confirm and refresh
            assert!(!profile.join("saves/Banana.sav").exists());
            assert!(load.request.is_none());
            expected.remove(4);
        }
    }
    fs::remove_dir_all(root).unwrap();
}
