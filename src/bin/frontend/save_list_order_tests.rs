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
        level_text: openmt2001::level_text::LevelTextCatalog::default(),
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

    // Inserting an earlier row must not leave the former row index selected:
    // the refresh after a write selects nothing (0x497660), and the next click
    // targets the file now displayed in that row.
    click(&mut view, [30., 45. + 2. * assets.list_layout.pitch]);
    assert_eq!(view.selected_name(), Some("Banana"));
    write_target(&root, &Target::NewName("0new".into()), &bytes).unwrap();
    view.saved(Ok(root.join("saves/Banana.sav")), &root);
    assert_eq!(view.selected, None);
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
    assert_eq!(view.selected, None);
    click(&mut view, [30., 45. + 3. * assets.list_layout.pitch]);
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

/// #510: after Save As, Save (overwrite) and Delete the original rebuilds the
/// rows with nothing selected, so the Overwrite panel is empty, Save and Delete
/// are disabled, Save As and Back stay enabled, and no status line is shown
/// (0x4965c0/0x497660, 0x498110, 0x498030).
#[test]
fn save_screen_returns_unselected_after_each_write() {
    let root = openmt2001::fixtures::scratch("save_screen_after_write").join(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bytes = [255, 255, 255, 255, 0, 0, 0, 0];
    for name in ["beta", "delta"] {
        write_target(&root, &Target::NewName(name.into()), &bytes).unwrap();
    }
    let names = |view: &View| {
        view.slots
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>()
    };
    let assert_unselected = |view: &View, after: &str| {
        assert_eq!(view.selected, None, "{after}: no row selected");
        assert!(
            view.selected_slot().is_none(),
            "{after}: Overwrite panel empty"
        );
        assert!(view.selected_name().is_none(), "{after}: no Name field");
        assert!(!view.button_enabled(Button::Save), "{after}: Save disabled");
        assert!(
            !view.button_enabled(Button::Delete),
            "{after}: Delete disabled"
        );
        assert!(
            view.button_enabled(Button::SaveAs),
            "{after}: Save As enabled"
        );
        assert!(view.button_enabled(Button::Back), "{after}: Back enabled");
        assert_eq!(view.message, None, "{after}: no status text");
        assert!(!view.busy, "{after}: idle");
    };
    let mut view = View::default();
    view.open(&root);
    // Each action starts from a selected row and an in-flight status line;
    // neither survives the refresh.
    let select = |view: &mut View, name: &str| {
        view.selected = view.slots.iter().position(|s| s.name == name);
        assert!(view.selected.is_some(), "{name} is listed");
    };

    // Save As a new name, sorted between the existing rows.
    select(&mut view, "delta");
    view.busy = true;
    view.message = Some("Saving...".into());
    let path = write_target(&root, &Target::NewName("charlie".into()), &bytes).unwrap();
    view.saved(Ok(path), &root);
    assert_eq!(names(&view), ["beta", "charlie", "delta"]);
    assert_unselected(&view, "Save As");

    // Save over the selected game.
    select(&mut view, "charlie");
    assert!(view.button_enabled(Button::Save) && view.button_enabled(Button::Delete));
    view.busy = true;
    view.message = Some("Saving...".into());
    let path = write_target(&root, &Target::ExistingSlot("charlie.sav".into()), &bytes).unwrap();
    view.saved(Ok(path), &root);
    assert_eq!(names(&view), ["beta", "charlie", "delta"]);
    assert_unselected(&view, "Save");

    // Delete the selected game.
    select(&mut view, "beta");
    view.busy = true;
    view.message = Some("Deleting...".into());
    view.deleted(delete_slot(&root, "beta.sav"), &root);
    assert_eq!(names(&view), ["charlie", "delta"]);
    assert_unselected(&view, "Delete");

    // A failure is still reported and keeps the selection for a retry.
    select(&mut view, "delta");
    view.saved(Err("disk full".into()), &root);
    assert_eq!(view.selected_name(), Some("delta"));
    assert_eq!(view.message.as_deref(), Some("Save failed: disk full"));
    fs::remove_dir_all(root).unwrap();
}
