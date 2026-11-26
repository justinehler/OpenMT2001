//! Actual Menu pointer/mouse/profile producers and the Cells consumer used by
//! App's event-finalization seam. No Intro renderer, decoder or device needed.
use super::*;
use crate::settings_menu::{Assets as SettingsAssets, Button, Page};
use std::{fs, path::PathBuf};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = PathBuf::from(std::env::var_os("OPENMT2001_TEST_SCRATCH").unwrap());
        assert!(parent.is_absolute());
        fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!(
            "pf-preferences-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture(root: &std::path::Path) -> (Menu, SettingsAssets, Owner) {
    let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let executable = read("OPENMT2001_EXE");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
    let scripts = map.parent().unwrap().parent().unwrap();
    let assets = crate::menu::Assets::load(
        &index,
        &archive,
        installer,
        &language,
        scripts,
        root.to_path_buf(),
        root.join("saves"),
        &executable,
    )
    .unwrap();
    let menu = Menu::new(assets);
    let settings = SettingsAssets::load(&index, &archive, installer, &language).unwrap();
    let (process, mut activity) =
        crate::live_host::process_runtime::startup(&executable, "").unwrap();
    let owner = super::super::process_frontend::ProcessFrontend::new(
        activity.take_frontend_startup().unwrap(),
        process,
        winit::window::WindowId::from(7),
        Levels::default(),
    )
    .unwrap();
    owner.borrow_mut().observe(true).unwrap();
    (menu, settings, owner)
}
fn edit_to25(menu: &mut Menu, assets: &SettingsAssets, cells: &mut Cells<'_>, owner: &Owner) {
    menu.selected = Some(5);
    menu.activate();
    assert_eq!(menu.settings.page, Some(Page::Main));
    let b = assets.bounds(Page::Main, Button::Sound).unwrap();
    menu.pointer([(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5]);
    menu.mouse_button(true);
    cells.commit_menu(menu, Some(owner)).unwrap();
    menu.mouse_button(false);
    cells.commit_menu(menu, Some(owner)).unwrap();
    assert_eq!(menu.settings.page, Some(Page::Sound));
    for button in [Button::MusicTrack, Button::EffectsTrack] {
        let b = assets.bounds(Page::Sound, button).unwrap();
        let track = crate::ui::slider_track(b.map(f32::trunc), false);
        let x = (track[0] as i32..=track[2] as i32)
            .map(|v| v as f32 + 0.25)
            .find(|x| crate::ui::slider_selection(track, *x, 100, false) == 25)
            .unwrap();
        let point = [x, (b[1] + b[3]) * 0.5];
        menu.pointer(point);
        cells.commit_menu(menu, Some(owner)).unwrap();
        // Settings mouse handler returns early internally; App's outer event
        //commit still delivers this writer before a later window Focus event.
        menu.mouse_button(true);
        cells.commit_menu(menu, Some(owner)).unwrap();
        menu.pointer(point);
        cells.commit_menu(menu, Some(owner)).unwrap();
        menu.mouse_button(false);
        cells.commit_menu(menu, Some(owner)).unwrap();
    }
}
#[test]
#[ignore = "private original Menu assets, CPU only; same App event commit consumer"]
fn actual_menu_edit_then_loss_gain_commits_before_prefix_and_unchanged_draw_does_not_reseed() {
    let scratch = Scratch::new();
    let (mut menu, assets, owner) = fixture(&scratch.0);
    menu.participant_test_named_profile("PF edit");
    let mut profile = None;
    let mut saved = Snapshot::default();
    let mut error = None;
    let mut audio = Levels::default();
    let mut graphics = GraphicsLevels::default();
    let mut gameplay = GameplayLevels::default();
    let mut cells = Cells {
        profile: &mut profile,
        saved: &mut saved,
        error: &mut error,
        audio: &mut audio,
        graphics: &mut graphics,
        gameplay: &mut gameplay,
    };
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    edit_to25(&mut menu, &assets, &mut cells, &owner);
    assert_eq!(
        menu.settings_audio,
        Levels {
            music: 25,
            effects: 25
        }
    );
    assert_eq!(
        (
            owner.borrow().audio.music_level(),
            owner.borrow().audio.factor()
        ),
        (25, crate::music_gain::normalized_percentage(25))
    );
    owner.borrow_mut().observe(false).unwrap();
    assert!(owner.borrow().audio.music_paused());
    assert_eq!(owner.borrow().audio.music_level(), 0);
    // The actual unchanged event/draw consumer must not resend durable25/25.
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    assert_eq!(owner.borrow().audio.music_level(), 0);
    owner.borrow_mut().observe(true).unwrap();
    assert_eq!(
        (
            owner.borrow().audio.music_level(),
            owner.borrow().audio.factor()
        ),
        (25, crate::music_gain::normalized_percentage(25))
    );
    assert_eq!(
        Snapshot::load(cells.profile.as_ref().unwrap())
            .unwrap()
            .audio,
        Levels {
            music: 25,
            effects: 25
        }
    );
}
#[test]
#[ignore = "private original Menu assets, CPU only; actual changed profile identity with equal durable levels"]
fn equal_level_profile_change_forwards_once_preserving_saved_snapshot_and_desired_pause() {
    let scratch = Scratch::new();
    let (mut menu, assets, owner) = fixture(&scratch.0);
    menu.participant_test_named_profile("PF first");
    let mut profile = None;
    let mut saved = Snapshot::default();
    let mut error = None;
    let mut audio = Levels::default();
    let mut graphics = GraphicsLevels::default();
    let mut gameplay = GameplayLevels::default();
    let mut cells = Cells {
        profile: &mut profile,
        saved: &mut saved,
        error: &mut error,
        audio: &mut audio,
        graphics: &mut graphics,
        gameplay: &mut gameplay,
    };
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    let first = cells.profile.clone();
    owner.borrow_mut().observe(false).unwrap();
    assert_eq!(owner.borrow().audio.music_level(), 0);
    edit_to25(&mut menu, &assets, &mut cells, &owner); // real writers while suspended; saved remains63/83.
    owner.borrow_mut().observe(true).unwrap();
    assert_eq!(owner.borrow().audio.music_level(), 63);
    owner.borrow_mut().observe(false).unwrap();
    assert_eq!(owner.borrow().audio.music_level(), 0);
    menu.participant_test_named_profile("PF second");
    Snapshot {
        audio: Levels {
            music: 25,
            effects: 25,
        },
        ..Snapshot::default()
    }
    .save(&menu.current_settings_path().unwrap())
    .unwrap();
    // New identity has SAME durable25/25 as first. Explicit forwarding must
    //happen despite equality, while saved63/83 and desired pause survive.
    owner.borrow_mut().observe(false).unwrap();
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    assert_ne!(*cells.profile, first);
    assert_eq!(
        *cells.audio,
        Levels {
            music: 25,
            effects: 25
        }
    );
    assert_eq!(owner.borrow().audio.music_level(), 25);
    assert!(owner.borrow().audio.music_paused());
    owner.borrow_mut().observe(true).unwrap();
    assert_eq!(owner.borrow().audio.music_level(), 63);
    owner.borrow_mut().observe(false).unwrap();
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    assert_eq!(
        owner.borrow().audio.music_level(),
        0,
        "unchanged profile observation must not forward again"
    );
    owner.borrow_mut().observe(true).unwrap();
    assert_eq!(owner.borrow().audio.music_level(), 63);
}
