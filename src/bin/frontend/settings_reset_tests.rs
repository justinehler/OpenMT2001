use super::*;
use crate::profile_settings::{Snapshot, profile_path};

fn changed() -> Snapshot {
    Snapshot {
        audio: Levels {
            music: 0,
            effects: 1,
        },
        graphics: GraphicsLevels {
            detail: 0,
            filtering: 0,
            distance: 99,
            ..GraphicsLevels::default()
        },
        gameplay: GameplayLevels {
            auto_rotate: false,
            invert_vertical: true,
            hardware_cursor: true,
            mouse_sensitivity: 0,
        },
    }
}

fn original_defaults() -> Snapshot {
    // 4c1c50: profile +408/+40c/+410/+418/+41c/+420/+421/+424/+428.
    // The normal startup mode at 8b26cc is 2, so the cursor default is false.
    Snapshot {
        audio: Levels {
            music: 63,
            effects: 83,
        },
        graphics: GraphicsLevels {
            water_effects: true,
            halos: true,
            detail: 2,
            filtering: 1,
            distance: 35,
        },
        gameplay: GameplayLevels {
            auto_rotate: true,
            invert_vertical: false,
            hardware_cursor: false,
            mouse_sensitivity: 50,
        },
    }
}

fn synthetic_assets() -> Assets {
    Assets {
        panels: std::array::from_fn(|_| Panel {
            base: Vec::new(),
            regions: (0..35)
                .map(|i| [10., i as f32 * 20., 50., i as f32 * 20. + 10.])
                .collect(),
        }),
        fonts: Vec::new(),
        font1: Vec::new(),
        font3: Vec::new(),
        slider_textures: Vec::new(),
        radio_buttons: std::array::from_fn(|_| Surface::solid()),
        checkboxes: std::array::from_fn(|_| Surface::solid()),
        tab: Vec::new(),
        tab1: Vec::new(),
        tab3: Vec::new(),
        strings: Vec::new(),
    }
}

fn point(assets: &Assets, page: Page, button: Button) -> [f32; 2] {
    let [l, t, r, b] = assets.bounds(page, button).unwrap();
    [(l + r) * 0.5, (t + b) * 0.5]
}

fn snapshot(view: &View, audio: Levels) -> Snapshot {
    Snapshot {
        audio,
        graphics: view.graphics,
        gameplay: view.gameplay,
    }
}

#[test]
fn settings_reset_restores_original_defaults_and_persists_only_selected_profile() {
    let assets = synthetic_assets();
    let initial = changed();
    let mut audio = initial.audio;
    let mut view = View {
        graphics: initial.graphics,
        gameplay: initial.gameplay,
        ..View::default()
    };
    view.open();
    let reset = point(&assets, Page::Main, Button::Reset);
    assert!(assets.action_cursor_hover(Page::Main, Some(reset), true));
    assert!(!view.mouse(&assets, Some(reset), true, &mut audio));
    assert!(view.pointer_click_cue(&assets, Some(reset)));
    assert_eq!(
        snapshot(&view, audio),
        initial,
        "press alone does not reset"
    );
    view.mouse(&assets, None, false, &mut audio);
    assert_eq!(snapshot(&view, audio), initial, "release outside cancels");
    view.mouse(&assets, Some(reset), true, &mut audio);
    assert!(
        view.mouse(&assets, Some(reset), false, &mut audio),
        "audio change is forwarded"
    );
    assert_eq!(view.page, Some(Page::Main), "no confirmation or navigation");
    assert_eq!(snapshot(&view, audio), original_defaults());
    assert!(view.take_reset());
    assert!(!view.take_reset());
    assert_eq!(
        Snapshot::default(),
        original_defaults(),
        "new profiles use the same defaults"
    );

    let root = openmt2001::fixtures::scratch("TEST_SCRATCH").join(format!(
        "settings-reset-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let selected = profile_path(&root, b"Selected");
    let other = profile_path(&root, b"Other");
    initial.save(&selected).unwrap();
    initial.save(&other).unwrap();
    snapshot(&view, audio).save(&selected).unwrap();
    assert_eq!(Snapshot::load(&selected).unwrap(), original_defaults());
    assert_eq!(Snapshot::load(&other).unwrap(), initial);
    assert_eq!(
        Snapshot::load(&profile_path(&root, b"New")).unwrap(),
        original_defaults()
    );
    // Repeating Reset is harmless and does not claim an audio level change.
    view.mouse(&assets, Some(reset), true, &mut audio);
    assert!(!view.mouse(&assets, Some(reset), false, &mut audio));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn settings_reset_is_disabled_in_city_and_does_not_reset_system_multitexture() {
    let assets = synthetic_assets();
    let initial = changed();
    let mut audio = initial.audio;
    let mut view = View {
        graphics: initial.graphics,
        gameplay: initial.gameplay,
        ..View::default()
    };
    let reset = point(&assets, Page::Main, Button::Reset);
    view.open_in_game();
    assert!(!assets.action_cursor_hover(Page::Main, Some(reset), false));
    view.mouse(&assets, Some(reset), true, &mut audio);
    assert!(!view.pointer_click_cue(&assets, Some(reset)));
    assert!(!view.mouse(&assets, Some(reset), false, &mut audio));
    assert_eq!(snapshot(&view, audio), initial);
    assert!(!view.take_reset());
    // Subpages have only OK, not a page-local reset action.
    for page in [Page::Sound, Page::Graphics, Page::Gameplay] {
        assert!(assets.bounds(page, Button::Reset).is_none());
    }
    view.open();
    view.graphics.water_effects = false;
    view.mouse(&assets, Some(reset), true, &mut audio);
    view.mouse(&assets, Some(reset), false, &mut audio);
    let mut expected = original_defaults();
    expected.graphics.water_effects = false;
    assert_eq!(snapshot(&view, audio), expected);
}

#[test]
#[ignore = "requires original v1.4 executable and Settings assets"]
fn settings_reset_original_defaults_and_controls() {
    let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    assert_eq!(
        image.at(0x4c1d58, 10).unwrap(),
        &[0xc7, 0x85, 8, 4, 0, 0, 63, 0, 0, 0]
    );
    assert_eq!(
        image.at(0x4c1d62, 10).unwrap(),
        &[0xc7, 0x85, 12, 4, 0, 0, 83, 0, 0, 0]
    );
    assert_eq!(
        image.at(0x4c1d7e, 10).unwrap(),
        &[0xc7, 0x85, 28, 4, 0, 0, 35, 0, 0, 0]
    );
    assert_eq!(
        image.at(0x4c1d94, 10).unwrap(),
        &[0xc7, 0x85, 36, 4, 0, 0, 50, 0, 0, 0]
    );
    // Detail and lighting share ECX=2, filtering uses EBX=1, and invert
    // uses EAX=0; these registers also seed the adjacent boolean defaults.
    assert_eq!(
        image.at(0x4c1d4b, 7).unwrap(),
        &[0x33, 0xc0, 0xb9, 2, 0, 0, 0]
    );
    assert_eq!(
        image.at(0x4c1d6c, 18).unwrap(),
        &[
            0x89, 0x8d, 0x10, 4, 0, 0, 0x89, 0x8d, 0x14, 4, 0, 0, 0x89, 0x9d, 0x18, 4, 0, 0,
        ]
    );
    assert_eq!(
        image.at(0x4c1d88, 12).unwrap(),
        &[0x88, 0x9d, 0x20, 4, 0, 0, 0x88, 0x85, 0x21, 4, 0, 0,]
    );
    assert_eq!(
        image.at(0x4b91c3, 10).unwrap(),
        &[0xc7, 5, 0xcc, 0x26, 0x8b, 0, 2, 0, 0, 0]
    );
    // Action6 calls defaults, then applies them directly, then returns.
    assert_eq!(
        image.at(0x496add, 23).unwrap(),
        &[
            0xb9, 0xd0, 2, 0x92, 0, 0xe8, 0x69, 0xb1, 2, 0, 0xb9, 0xd0, 2, 0x92, 0, 0xe8, 0xdf,
            0xba, 2, 0, 0xc2, 12, 0,
        ]
    );
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameters.parent().unwrap().parent().unwrap();
    let exe = openmt2001::fixtures::path("EXE");
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let assets = Assets::load(&index, &archive, installer, &language).unwrap();
    let mut view = View::default();
    let initial = changed();
    let mut audio = initial.audio;
    view.graphics = initial.graphics;
    view.gameplay = initial.gameplay;
    view.open();
    let reset = point(&assets, Page::Main, Button::Reset);
    assert_eq!(assets.button(Page::Main, reset), Some(Button::Reset));
    view.mouse(&assets, Some(reset), true, &mut audio);
    view.mouse(&assets, Some(reset), false, &mut audio);
    assert_eq!(snapshot(&view, audio), original_defaults());

    #[cfg(feature = "capture")]
    {
        let renderer = pollster::block_on(Renderer::new(
            wgpu::InstanceDescriptor::new_without_display_handle(),
        ))
        .unwrap();
        let root = openmt2001::fixtures::scratch("SETTINGS_RESET_CAPTURE");
        for (name, settings) in [("before", initial), ("after", snapshot(&view, audio))] {
            for page in [Page::Main, Page::Sound, Page::Graphics, Page::Gameplay] {
                let scene = assets
                    .prepare_with_options(
                        &renderer,
                        MenuState {
                            page,
                            hovered: None,
                            pressed: None,
                            levels: settings.audio,
                            graphics: settings.graphics,
                            gameplay: settings.gameplay,
                            reset_enabled: true,
                        },
                    )
                    .unwrap();
                crate::capture_test::capture_standalone(
                    &renderer,
                    &scene,
                    root.join(format!("{name}-{page:?}.png")).to_str().unwrap(),
                );
            }
        }
    }
}
