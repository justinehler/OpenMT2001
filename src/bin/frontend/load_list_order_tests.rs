use super::*;
use std::{path::Path, time::UNIX_EPOCH};

/// A Load menu over `profile`'s V2 saves and `imports`, with no assets.
fn synthetic_load(root: &Path, profile: &Path, imports: &Path) -> Load {
    let mut bounds = vec![[0.; 4]; 25];
    bounds[8] = [20., 40., 300., 327.];
    Load {
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
        directories: vec![profile.join("saves"), imports.to_path_buf()],
        app_root: root.to_path_buf(),
        import_directory: imports.to_path_buf(),
        campaign_filename: None,
        profile_root: profile.to_path_buf(),
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
        refusal_messages: refusal::Messages::default(),
        refusal_prompt: None,
    }
}

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
    let mut load = synthetic_load(&root, &profile, &imports);
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
                path.file_stem().unwrap().to_str().unwrap()
            );
            let index = load
                .hit([30., 45. + row as f32 * load.list_layout.pitch])
                .unwrap();
            load.activate(index);
            assert_eq!(load.chosen, Some(row));
            load.activate(0);
            assert_eq!(load.request.as_ref(), Some(path));
            // The original shows no loading caption while the save loads.
            assert!(load.busy && load.message.is_none());
            load.busy = false;
        }
        if pass == 0 {
            // Delete on an imported original save is inert: no prompt and no
            // invented "read-only" caption.
            load.activate(3 + 1); // aardvark
            load.activate(2);
            assert!(load.delete_confirmation.is_none() && load.message.is_none());
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

#[test]
fn accented_save_names_reach_the_list_and_delete_prompt_as_code_page_text() {
    let root = std::env::temp_dir().join(format!(
        "omt-load-accent-{}-{}",
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
    crate::save_menu::write_target(
        &profile,
        &crate::save_menu::Target::NewName("Caf\u{e9}".into()),
        &bytes,
    )
    .unwrap();
    let mut load = synthetic_load(&root, &profile, &imports);
    // A row 0 shaped like the US one: ASCII onto itself, 0xc0..=0xff onto
    // glyphs 112..=175.
    let row0: Vec<u8> = (32..=255u8)
        .map(|byte| match byte {
            0xc0..=0xff => byte - 0xc0 + 112,
            32..=126 => byte - 32,
            _ => 0,
        })
        .collect();
    load.strings = vec![vec![]; 0x661];
    load.strings[0] = row0.clone();
    load.strings[0x660] = b"Delete %s?".iter().map(|b| b - 32).collect();
    load.open();
    // 494ac0 lists the stem like the Save list does: its code-page bytes
    // drawn through row 0, so the é is a glyph rather than `?`.
    assert_eq!(load.entries[0].name, "Caf\u{e9}");
    let glyphs =
        |text: &[u8]| -> Vec<u8> { text.iter().map(|&b| row0[usize::from(b - 32)]).collect() };
    assert_eq!(
        load.name_glyphs(&[], &load.entries[0].name),
        glyphs(b"Caf\xe9")
    );
    // 4950f0 passes the same bytes to the Delete prompt.
    load.delete_confirmation = Some("Caf\u{e9}.sav".into());
    let dialog = load.generic_prompt().unwrap();
    assert_eq!(
        dialog.paragraphs(&load.strings).unwrap()[0],
        glyphs(b"Delete Caf\xe9?")
    );
    fs::remove_dir_all(root).unwrap();
}
