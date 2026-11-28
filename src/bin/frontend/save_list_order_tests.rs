use super::*;
use crate::save_menu::{Target, delete_slot, write_target};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn mixed_case_save_rows_target_the_displayed_file_after_refresh() {
    let root = std::env::temp_dir().join(format!(
        "omt-save-order-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bytes = [255, 255, 255, 255, 0, 0, 0, 0];
    for name in ["cherry", "apple", "_x", "Banana", "1a"] {
        write_target(&root, &Target::NewName(name.into()), &bytes).unwrap();
    }
    let mut bounds = vec![[0.; 4]; 26];
    bounds[8] = [20., 40., 120., 327.];
    bounds[23] = [110., 360., 200., 390.]; // Save
    bounds[22] = [210., 360., 300., 390.]; // Delete
    let assets = Assets {
        base: Vec::new(),
        info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
            .unwrap(),
        bounds,
        list_layout: openmt2001::frontend_list::Layout::new([20., 40., 104., 302.], 12., 3.)
            .unwrap(),
        font1: Vec::new(),
        font2: Vec::new(),
        font4: Vec::new(),
        tab1: Vec::new(),
        tab2: Vec::new(),
        tab4: Vec::new(),
        strings: Vec::new(),
        level_text: crate::load::LevelTextCatalog::default(),
        slider_textures: Vec::new(),
    };
    let click = |view: &mut View, point| {
        assert!(view.mouse(&assets, Some(point), true).is_none());
        assert!(view.mouse(&assets, Some(point), false).is_none());
    };
    let mut view = View::default();
    view.open(&root);
    assert!(view.message.is_none());
    assert_eq!(
        view.slots
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["1a", "apple", "Banana", "cherry", "_x"]
    );
    for (row, name) in ["1a", "apple", "Banana", "cherry", "_x"].iter().enumerate() {
        click(
            &mut view,
            [30., 45. + row as f32 * assets.list_layout.pitch],
        );
        assert_eq!(view.selected_name(), Some(*name));
        click(&mut view, [150., 375.]);
        assert_eq!(
            view.enter(),
            Some(Action::ExistingSlot(format!("{name}.sav")))
        );
        click(&mut view, [250., 375.]);
        assert_eq!(
            view.enter(),
            Some(Action::DeleteSlot(format!("{name}.sav")))
        );
    }

    // Inserting an earlier row must restore the saved file by identity, not
    // retain its former row index. Subsequent overwrite/delete use that file.
    click(&mut view, [30., 45. + 2. * assets.list_layout.pitch]);
    assert_eq!(view.selected_name(), Some("Banana"));
    write_target(&root, &Target::NewName("0new".into()), &bytes).unwrap();
    view.saved(Ok(root.join("saves/Banana.sav")), &root);
    assert_eq!(view.selected, Some(3));
    assert_eq!(view.selected_name(), Some("Banana"));
    assert_eq!(
        view.slots
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["0new", "1a", "apple", "Banana", "cherry", "_x"]
    );
    click(&mut view, [30., 45. + 3. * assets.list_layout.pitch]);
    click(&mut view, [150., 375.]);
    let Some(Action::ExistingSlot(id)) = view.enter() else {
        panic!("expected overwrite")
    };
    let path = write_target(&root, &Target::ExistingSlot(id), &bytes).unwrap();
    assert_eq!(path.file_name().unwrap(), "Banana.sav");
    view.saved(Ok(path), &root);
    assert_eq!(view.selected, Some(3));
    assert_eq!(view.selected_name(), Some("Banana"));
    click(&mut view, [250., 375.]);
    let Some(Action::DeleteSlot(id)) = view.enter() else {
        panic!("expected delete")
    };
    view.deleted(delete_slot(&root, &id), &root);
    assert!(!root.join("saves/Banana.sav").exists());
    assert_eq!(view.selected, None);
    assert_eq!(
        view.slots
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["0new", "1a", "apple", "cherry", "_x"]
    );
    click(&mut view, [30., 45. + 3. * assets.list_layout.pitch]);
    click(&mut view, [150., 375.]);
    assert_eq!(
        view.enter(),
        Some(Action::ExistingSlot("cherry.sav".into()))
    );
    fs::remove_dir_all(root).unwrap();
}
