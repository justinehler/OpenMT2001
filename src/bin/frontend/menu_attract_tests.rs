//! Supplied-art integration through the actual Menu producers; no GPU/window.
use super::*;

/// Return to the main panel the way each screen's Back or Cancel control
/// does; the frontend has no Escape route (4ba6bc skips UI mode 4).
fn close_screens(menu: &mut Menu) {
    if menu.multiplayer.visible {
        menu.multiplayer.back();
    }
    while menu.settings.page.is_some() {
        menu.settings.back();
    }
    if menu.single.visible {
        menu.single.back();
    }
    menu.single_mode = false;
    menu.load.visible = false;
    if menu.profile.visible {
        menu.profile.back();
    }
    menu.selected = None;
    menu.sync_main_panel();
}

#[test]
#[ignore = "requires supplied menu assets/scripts"]
fn attract_and_settings_credits_real_menu_retain_profile_and_clear_selection() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let executable = read("EXE");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let exe = openmt2001::fixtures::path("EXE");
    let root = openmt2001::fixtures::scratch("TEST_SCRATCH").join(format!(
        "attract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut menu = Menu::new(
        Assets::load(
            &index,
            &archive,
            parameter.parent().unwrap().parent().unwrap(),
            &exe.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa"),
            &openmt2001::fixtures::path("SCRIPTS"),
            root.clone(),
            root.join("saves"),
            &executable,
        )
        .unwrap(),
    );
    menu.participant_test_named_profile("Attract Player");
    let profile = menu.current_settings_path();

    for skip_at in [None, Some(300), Some(2000)] {
        menu.select(Some(5), false);
        menu.activate();
        let bounds = menu
            .assets
            .settings
            .bounds(
                crate::settings_menu::Page::Main,
                crate::settings_menu::Button::Credits,
            )
            .unwrap();
        menu.pointer([(bounds[0] + bounds[2]) / 2., (bounds[1] + bounds[3]) / 2.]);
        menu.mouse_button(true);
        menu.mouse_button(false);
        assert_eq!(menu.settings.page, None);
        assert!(menu.take_credits());
        assert!(!menu.take_credits());
        let mut random = openmt2001::startup_rng::Snapshot::from_seed(1234).state(0x8b06e8);
        let rows = crate::fresh_city::result::credit_lines(
            &openmt2001::pe::PeImage::parse(&executable).unwrap(),
            &mut random,
        )
        .unwrap();
        assert_eq!(rows.len(), 32);
        assert_eq!(rows[0].heading_id, Some(0x6eb));
        assert_eq!(rows[14].names.len(), 4);
        assert_eq!(rows[24].names.len(), 5);
        assert_eq!(rows[31].heading_id, Some(0x704));
        let mut clock = openmt2001::opening_sequence::Controller::with_credits(rows.len());
        for tick in 0..=10023 {
            if skip_at == Some(tick) {
                clock.request_end();
            }
            clock.advance_to(tick);
            if clock.ended() {
                break;
            }
        }
        assert!(clock.ended());
        menu.return_from_attract();
        assert!(menu.main_panel_shown());
        assert_eq!(menu.current_settings_path(), profile);
        assert_eq!(menu.selected, None);
    }

    for _ in 0..5999 {
        assert!(!menu.attract_event1());
    }
    menu.pointer([1., 1.]);
    menu.input("ignored on main");
    menu.commit_text();
    menu.mouse_button(true);
    menu.cancel_input();
    assert!(menu.attract_event1());
    // This is the same retained Menu returned by Intro::open_menu.
    menu.return_from_attract();
    assert_eq!(menu.current_settings_path(), profile);
    for _ in 0..5999 {
        assert!(!menu.attract_event1());
    }
    assert!(menu.attract_event1());
    menu.return_from_attract();

    for action in 0..6 {
        for _ in 0..5999 {
            assert!(!menu.attract_event1());
        }
        menu.select(Some(action), false);
        menu.activate();
        assert!(!menu.main_panel_shown(), "main action {action}");
        for _ in 0..6001 {
            assert!(!menu.attract_event1());
        }
        close_screens(&mut menu);
        assert!(menu.main_panel_shown(), "return from action {action}");
        for _ in 0..5999 {
            assert!(!menu.attract_event1());
        }
        assert!(menu.attract_event1(), "fresh counter after action {action}");
        menu.return_from_attract();
    }
    for _ in 0..5999 {
        assert!(!menu.attract_event1());
    }
    menu.select(Some(6), false);
    menu.activate();
    assert!(menu.desktop_quit_active());
    assert!(menu.attract_event1(), "Quit keeps the main panel shown");
    menu.return_from_attract();
    assert_eq!(menu.current_settings_path(), profile);
}
