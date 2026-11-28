//! Supplied-art integration through the actual Menu producers; no GPU/window.
use super::*;

#[test]
#[ignore = "requires supplied menu assets/scripts"]
fn attract_real_menu_counts_input_and_screen_returns_and_retains_the_profile() {
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

    for _ in 0..5999 {
        assert!(!menu.attract_event1());
    }
    menu.pointer([1., 1.]);
    menu.navigate(1);
    menu.input("ignored on main");
    menu.mouse_button(true);
    menu.cancel_input();
    menu.back(); // Escape on the main panel is not a fresh show.
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
        for _ in 0..4 {
            if menu.main_panel_shown() {
                break;
            }
            menu.back();
        }
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
