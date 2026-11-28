//! Actual Menu pointer/mouse/profile producers and the Cells consumer used by
//! App's event-finalization seam. No Intro renderer, decoder or device needed.
use super::*;
use crate::settings_menu::{Assets as SettingsAssets, Button, Page};
use std::{fs, path::PathBuf};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = openmt2001::fixtures::scratch("TEST_SCRATCH");
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
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let executable = read("OPENMT2001_EXE");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe = openmt2001::fixtures::path("EXE");
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let map = openmt2001::fixtures::path("MAP");
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

#[test]
#[ignore = "requires original Menu assets and executable; CPU only"]
fn settings_reset_actual_menu_commits_defaults_to_process_and_selected_profile() {
    let scratch = Scratch::new();
    let (mut menu, assets, owner) = fixture(&scratch.0);
    menu.participant_test_named_profile("Reset selected");
    let selected = menu.current_settings_path().unwrap();
    let initial = Snapshot {
        audio: Levels {
            music: 12,
            effects: 24,
        },
        graphics: GraphicsLevels {
            water_effects: false,
            halos: false,
            detail: 0,
            filtering: 0,
            distance: 90,
        },
        gameplay: GameplayLevels {
            auto_rotate: false,
            invert_vertical: true,
            hardware_cursor: true,
            mouse_sensitivity: 99,
        },
    };
    initial.save(&selected).unwrap();
    let other = crate::profile_settings::profile_path(&scratch.0, b"Reset other");
    initial.save(&other).unwrap();
    let token_path = crate::menu::preferred_token::path(&scratch.0, b"Reset selected");
    let other_token = crate::menu::preferred_token::path(&scratch.0, b"Reset other");
    crate::menu::preferred_token::Token::new(7)
        .unwrap()
        .save(&token_path)
        .unwrap();
    crate::menu::preferred_token::Token::new(8)
        .unwrap()
        .save(&other_token)
        .unwrap();
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
    assert_eq!(menu.settings_snapshot(), initial);
    menu.selected = Some(5);
    menu.activate();
    let [l, t, r, b] = assets.bounds(Page::Main, Button::Reset).unwrap();
    menu.pointer([(l + r) * 0.5, (t + b) * 0.5]);
    assert!(menu.enabled_cursor_hover());
    menu.mouse_button(true);
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    assert_eq!(menu.settings_snapshot(), initial);
    menu.mouse_button(false);
    cells.commit_menu(&mut menu, Some(&owner)).unwrap();
    let expected = Snapshot {
        graphics: GraphicsLevels {
            water_effects: false,
            halos: false,
            ..GraphicsLevels::default()
        },
        ..Snapshot::default()
    };
    assert_eq!(menu.settings.page, Some(Page::Main));
    assert_eq!(menu.settings_snapshot(), expected);
    assert_eq!(*cells.saved, expected);
    assert_eq!(
        (*cells.audio, *cells.graphics, *cells.gameplay),
        (expected.audio, expected.graphics, expected.gameplay)
    );
    assert_eq!(owner.borrow().audio.music_level(), 63);
    assert_eq!(
        owner.borrow().audio.factor(),
        crate::music_gain::normalized_percentage(83)
    );
    assert_eq!(Snapshot::load(&selected).unwrap(), expected);
    assert_eq!(Snapshot::load(&other).unwrap(), initial);
    assert_eq!(
        crate::menu::preferred_token::Token::load(&token_path)
            .unwrap()
            .index(),
        0
    );
    assert_eq!(
        crate::menu::preferred_token::Token::load(&other_token)
            .unwrap()
            .index(),
        8
    );
}
