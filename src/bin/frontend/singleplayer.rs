//! Original campaign and Sandbox setup, launching a retained city session.
use crate::{
    load::{ascii, level_labels, wrap_lines, wrapped},
    ui::{self, Surface},
};
use openmt2001::{
    archive::ArchiveIndex,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "singleplayer/custom.rs"]
mod custom;

const TUTORIAL_TITLE_COLORS: [u32; 4] = [0xffffb400, 0xffffb400, 0x80ffb400, 0x80ffb400];
const TUTORIAL_ROW_HEIGHT: f32 = 12.;
// 49cb18 reads generic texture 297 (68ae28 + 297 * 4).
const TUTORIAL_COMPLETION_TICK: usize = 297;
// 499817/49994e call original 43f510 with the Info8/Backing3 mesh bounds.
const SCENARIO_INFO_SCROLL: [f32; 4] = [588., 202., 604., 396.];
const SCENARIO_LIST_SCROLL: [f32; 4] = [303., 97., 319., 445.];
// 49b5b5/49b68d/49b6bb place three completion cups on each 12px list row.
const SCENARIO_ROW_HEIGHT: f32 = 12.;
const SCENARIO_CUP_EMPTY: usize = 296;

#[derive(Clone, Copy, Debug)]
pub enum Preview {
    Scenario(u32),
    Sandbox,
}

#[cfg(test)]
mod campaign_picker_tests {
    use super::*;
    use openmt2001::level_catalog::{Catalog, Labels, Registration};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(
                std::env::temp_dir()
                    .join(format!("campaign-picker-{}-{nonce}", std::process::id())),
            )
        }
        fn profile(&self) -> PathBuf {
            crate::campaign_store::profile_path(&self.0, b"Player")
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn catalog(ids: &[i32]) -> Catalog {
        let mut catalog = Catalog::default();
        for &id in ids {
            catalog
                .register(
                    Registration {
                        id,
                        directory: format!("level{id}").into_bytes(),
                        labels: Labels::Literal {
                            title: format!("Title {id}").into_bytes(),
                            description: format!("Description {id}").into_bytes(),
                        },
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
            catalog.unlock(id as u32);
        }
        catalog
    }

    // Exercise production open, activation and label resolution with synthetic
    // panels; these tests need neither original files nor a graphics device.
    fn panel() -> SinglePlayer {
        let panel = || Panel {
            base: vec![],
            bounds: vec![[0., 0., 640., 480.]; 71],
        };
        SinglePlayer {
            custom: None,
            scenario: panel(),
            tutorial_panel: panel(),
            character_panel: panel(),
            generic: vec![],
            characters: vec![],
            fonts: vec![],
            tab: vec![],
            scenario_list_fonts: vec![],
            scenario_list_tab: vec![],
            title_fonts: vec![],
            title_tab: vec![],
            strings: vec![vec![]; 0x800],
            visible: false,
            character_screen: false,
            tutorial_screen: false,
            sandbox: false,
            difficulty: 2,
            scenario_id: Some(200003),
            scenario_rows: vec![],
            tutorial_index: None,
            character: 0,
            inspected_slot: 0,
            biography_first: std::cell::Cell::new(0),
            name: vec![],
            catalog: None,
            profile_path: None,
            request: None,
            busy: false,
            message: None,
            description_first: 0,
            pointer_position: None,
        }
    }

    #[test]
    fn native_flags_preserve_registration_order_and_real_ids() {
        let mut catalog = catalog(&[200901, 300001, 200003, 100000, 500001, 200902, 200900]);
        catalog.levels[5].bytes[0x86c..0x870].fill(0); // locked
        // Nonzero native fields are truthy, not limited to the value one.
        catalog.levels[0].bytes[1] = 7;
        catalog.levels[0].bytes[0x86c..0x870].copy_from_slice(&256u32.to_le_bytes());
        catalog.levels[3].bytes[0x85c] = 2;
        catalog.levels[4].bytes[2] = 2;
        assert_eq!(scenario_rows_for(&catalog), [200901, 200003, 200900]);
    }

    #[test]
    fn saved_profile_selects_first_entirely_unfinished_row_on_easy() {
        let scratch = Scratch::new();
        let ids = [300001, 200901, 100000, 200003, 200902, 200900, 200903];
        let mut saved = catalog(&ids);
        for (id, completed) in [
            (200901, [0, 1, 0]),
            (200003, [0, 0, 2]),
            (200902, [1, 0, 0]),
        ] {
            saved.set_level_info(id, 1, completed);
        }
        crate::campaign_store::save_progress(&saved, &scratch.profile()).unwrap();
        let mut loaded = catalog(&ids);
        crate::campaign_store::load_progress(&mut loaded, &scratch.profile()).unwrap();
        loaded.difficulty = 2;
        let mut picker = panel();
        picker.open(false, "Player", loaded, scratch.profile());
        assert!(!picker.busy && picker.message.is_none());
        assert_eq!(
            picker.scenario_rows,
            [200901, 200003, 200902, 200900, 200903]
        );
        assert_eq!(picker.scenario_id, Some(200900));
        assert_eq!(picker.difficulty, 0);
        assert_eq!(picker.catalog.as_ref().unwrap().difficulty, 0);
        assert!(picker.keyboard_enabled(0));
        picker.activate(0);
        assert!(picker.character_screen);
        picker.activate(0);
        let setup = picker.request.unwrap();
        assert!(matches!(setup.preview, Preview::Scenario(200900)));
        assert!(matches!(setup.difficulty, Difficulty::Easy));
    }

    #[test]
    fn all_completed_or_empty_leaves_no_selection_and_disables_confirmation() {
        let scratch = Scratch::new();
        let mut completed = catalog(&[200003, 200900, 200901]);
        for (id, flags) in [
            (200003, [1, 0, 0]),
            (200900, [0, 1, 0]),
            (200901, [0, 0, 1]),
        ] {
            completed.set_level_info(id, 1, flags);
        }
        let mut picker = panel();
        picker.strings[0x66e] = ascii("Scenario information");
        for catalog in [completed, Catalog::default()] {
            picker.open(false, "Player", catalog, scratch.profile());
            assert_eq!(picker.scenario_id, None);
            assert!(!picker.keyboard_enabled(0));
            assert!(!picker.generic_click_enabled(0));
            assert!(picker.keyboard_enabled(1));
            assert_eq!(
                picker.scenario_record_text(true),
                ascii("Scenario information")
            );
            assert!(picker.scenario_record_text(false).is_empty());
            picker.activate(0);
            assert!(!picker.character_screen);
            assert!(picker.request.is_none());
            if let Some(&id) = picker.scenario_rows.first() {
                // Completed rows remain available for an explicit replay.
                picker.activate(5);
                assert_eq!(picker.scenario_id, Some(id));
                assert!(picker.keyboard_enabled(0));
                picker.activate(0);
                assert!(picker.character_screen);
            }
        }
    }

    #[test]
    fn displayed_non_builtin_registration_reaches_participant_setup_with_own_labels_and_id() {
        let scratch = Scratch::new();
        for labels in [
            Labels::Literal {
                title: b"New scenario".to_vec(),
                description: b"New description".to_vec(),
            },
            Labels::Localized {
                title: 17,
                description: 18,
            },
        ] {
            let mut catalog = catalog(&[200003]);
            // Register both forms through the real registration path.
            catalog
                .register(
                    Registration {
                        id: 200900,
                        directory: b"extra".to_vec(),
                        labels,
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
            catalog.unlock(200900);
            assert!(!crate::scenarios::CAMPAIGN.iter().any(|s| s.id == 200900));
            let mut picker = panel();
            picker.strings[17] = ascii("New scenario");
            picker.strings[18] = ascii("New description");
            picker.open(false, "Player", catalog, scratch.profile());
            assert_eq!(picker.scenario_id, Some(200003));
            picker.activate(6); // displayed second row, through real activation
            assert_eq!(picker.scenario_id, Some(200900));
            assert_eq!(picker.scenario_record_text(true), ascii("New scenario"));
            assert_eq!(picker.scenario_record_text(false), ascii("New description"));
            picker.activate(0);
            assert!(picker.character_screen && picker.message.is_none());
            assert_eq!(picker.scenario_record_text(true), ascii("New scenario"));
            picker.activate(0);
            let setup = picker.request.unwrap();
            assert!(matches!(setup.preview, Preview::Scenario(200900)));
            let text = crate::load::LevelTextCatalog::new(&setup.catalog, &picker.strings);
            assert_eq!(text.title(200900, &picker.strings), ascii("New scenario"));
            assert_eq!(
                text.description(200900, &picker.strings),
                Some(ascii("New description").as_slice())
            );
        }
    }
}

#[cfg(test)]
mod tutorial_progress_tests {
    use super::{tutorial_preselection, tutorial_tick_rect, tutorial_tick_rows};
    use openmt2001::level_catalog::{Catalog, Labels, Registration};

    fn catalog() -> Catalog {
        let mut catalog = Catalog::default();
        // Deliberately differ from picker order and include a completed scenario.
        for id in
            std::iter::once(200003).chain(crate::scenarios::TUTORIALS.iter().rev().map(|t| t.id))
        {
            catalog
                .register(
                    Registration {
                        id: id as i32,
                        directory: format!("level{id}").into_bytes(),
                        labels: Labels::Localized {
                            title: 0,
                            description: 0,
                        },
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
        }
        catalog.set_level_info(200003, 1, [1; 3]);
        catalog
    }

    #[test]
    fn none_done_selects_first_without_ticks() {
        let catalog = catalog();
        assert_eq!(tutorial_preselection(&catalog), Some(0));
        assert_eq!(tutorial_tick_rows(Some(&catalog)).collect::<Vec<_>>(), []);
        assert_eq!(tutorial_tick_rows(None).collect::<Vec<_>>(), []);
    }

    #[test]
    fn some_done_selects_first_gap_and_ticks_only_easy_completions() {
        let mut catalog = catalog();
        for (id, flags) in [
            (100000, [1, 0, 0]),
            (100001, [0, 1, 0]),
            (100002, [0, 0, 1]),
            (100004, [1, 1, 1]),
            (100012, [1, 0, 0]),
        ] {
            catalog.set_level_info(id, 0, flags);
        }
        assert_eq!(tutorial_preselection(&catalog), Some(3));
        assert_eq!(
            tutorial_tick_rows(Some(&catalog)).collect::<Vec<_>>(),
            [0, 4, 12]
        );
        assert_eq!(tutorial_tick_rect(0), [430., 94., 446., 106.]);
        assert_eq!(tutorial_tick_rect(4), [430., 142., 446., 154.]);
        assert_eq!(tutorial_tick_rect(12), [430., 238., 446., 250.]);
    }

    #[test]
    fn all_done_leaves_no_visible_selection_and_all_ticks() {
        let mut catalog = catalog();
        for tutorial in crate::scenarios::TUTORIALS {
            catalog.set_level_info(tutorial.id, 0, [1, 0, 0]);
        }
        assert_eq!(tutorial_preselection(&catalog), None);
        assert_eq!(
            tutorial_tick_rows(Some(&catalog)).collect::<Vec<_>>(),
            (0..13).collect::<Vec<_>>()
        );
    }

    #[test]
    fn restart_and_profile_switch_read_saved_tutorial_progress() {
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = Scratch(
            std::env::temp_dir().join(format!("tutorial-progress-{}-{nonce}", std::process::id())),
        );
        let alice = crate::campaign_store::profile_path(&scratch.0, b"Alice");
        let bob = crate::campaign_store::profile_path(&scratch.0, b"Bob");
        let mut active = catalog();
        for id in [100000, 100001, 100004] {
            active.set_level_info(id, 0, [1, 0, 0]);
        }
        crate::campaign_store::save_progress(&active, &alice).unwrap();

        // Rebuild registrations as a new process would, then load the profile.
        let mut restarted = catalog();
        crate::campaign_store::load_progress(&mut restarted, &alice).unwrap();
        assert_eq!(tutorial_preselection(&restarted), Some(2));
        assert_eq!(
            tutorial_tick_rows(Some(&restarted)).collect::<Vec<_>>(),
            [0, 1, 4]
        );
        crate::campaign_store::load_progress(&mut restarted, &bob).unwrap();
        assert_eq!(tutorial_preselection(&restarted), Some(0));
        assert_eq!(tutorial_tick_rows(Some(&restarted)).collect::<Vec<_>>(), []);
        crate::campaign_store::load_progress(&mut restarted, &alice).unwrap();
        assert_eq!(tutorial_preselection(&restarted), Some(2));
        assert_eq!(
            tutorial_tick_rows(Some(&restarted)).collect::<Vec<_>>(),
            [0, 1, 4]
        );
    }
}

#[cfg(test)]
mod click_cue_tests {
    use super::native_generic_click;

    #[test]
    fn scenario_tutorial_and_character_generic_children_only() {
        for i in 0..2 {
            assert!(native_generic_click(true, false, false, i));
            assert!(native_generic_click(true, false, true, i));
        }
        // Scenario difficulty radios and Tutorial list rows are separate
        // widgets; Character arrows are original 4a6ec0 generic children.
        for i in 2..5 {
            assert!(!native_generic_click(true, false, false, i));
        }
        assert!(native_generic_click(true, false, true, 2));
        assert!(native_generic_click(true, false, true, 3));
        assert!(!native_generic_click(true, false, true, 4));
        assert!(!native_generic_click(false, false, true, 0));
        assert!(!native_generic_click(true, true, true, 0));
        assert!(!native_generic_click(true, true, true, 2));
        assert!(native_generic_click(true, true, true, 1));
    }
}

#[cfg(test)]
mod tutorial_title_tests {
    #[cfg(feature = "capture")]
    use super::SinglePlayer;
    use super::{TUTORIAL_TITLE_COLORS, ui};
    use openmt2001::archive::ArchiveIndex;

    #[test]
    #[ignore = "requires original v1.4 Tutorial title fixture and archive"]
    fn tutorial_title_matches_original_painter_and_mesh() {
        let fixture = std::fs::read(openmt2001::fixtures::path("NATIVE_TUTORIAL_TITLE")).unwrap();
        assert_eq!(&fixture[..8], b"MTTUTL01");
        let mut at = 40;
        let mut word = || {
            let value = u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
            at += 4;
            value
        };
        let part = word() as usize;
        assert_eq!(part, 6);
        let bounds: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word()));
        let colors: [u32; 4] = std::array::from_fn(|_| word());
        assert_eq!(colors, TUTORIAL_TITLE_COLORS);
        assert_eq!(f32::from_bits(word()), 0.5);
        assert_eq!(at, fixture.len());
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let (_, regions) = ui::panel_base(
            &index,
            &archive,
            "gamedata2\\brand_new_panels\\tutorial_panel_v2.tm",
            &[part],
        )
        .unwrap();
        assert_eq!(regions[part], bounds);
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires original v1.4 assets and offscreen graphics"]
    fn capture_singleplayer_panels_over_city() {
        use openmt2001::{
            flyby::CameraSample,
            render::Renderer,
            render_scene::{self, OriginalAssets, Request, Source},
        };
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let executable = openmt2001::fixtures::path("EXE");
        let installer = openmt2001::fixtures::path("PARAMETERS")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = std::fs::read(language.join("tycoon.off")).unwrap();
        let glyphs = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0usize;
        let rows = offsets
            .chunks_exact(2)
            .map(|length| {
                let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
                let row = &glyphs[at..end];
                at = end;
                row
            })
            .collect::<Vec<_>>();
        let strings = crate::tooltip::localized_strings(&installer, &rows).unwrap();
        let refs = strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let exe = read("OPENMT2001_EXE");
        let mut panel = SinglePlayer::new(&index, &archive, &installer, &refs, &exe).unwrap();
        // Open the original registration catalog with a fresh V2 profile.
        // This exercises the same progression filter as the visible menu.
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let map_path = openmt2001::fixtures::path("MAP");
        let scripts = map_path.parent().unwrap().parent().unwrap();
        let mut catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
        let fresh = std::path::PathBuf::from("work/frontend/capture-fresh-profile-missing.lua");
        assert!(!fresh.exists());
        crate::campaign_store::load_progress(&mut catalog, &fresh).unwrap();
        assert_eq!(super::scenario_rows_for(&catalog).len(), 1);
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets {
                executable: &exe,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(12),
            camera: Some(CameraSample {
                eye: [26., 586., -695.],
                target: [26., 463., -595.],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            }),
            width: 1280,
            height: 960,
            animated: true,
        })
        .unwrap();
        let overlay = panel.draw_tutorial(&renderer, None, false).unwrap();
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&overlay],
            "work/frontend/tutorial-caption-city.png",
        );
        panel.open(false, "Capture", catalog, fresh);
        let scenario = panel.draw(&renderer, None, false).unwrap();
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&scenario],
            "work/frontend/scenario-scrollbars-city.png",
        );
        panel.activate(0); // the unlocked Starting Out row opens Character Selection
        assert!(panel.character_screen);
        panel.activate(3);
        panel.activate(3); // right arrow reaches Dog from the new-profile Racecar
        assert_eq!(panel.characters[panel.character].name, 1746);
        let dog = panel.draw(&renderer, None, false).unwrap();
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&dog],
            "work/frontend/character-dog-biography-city.png",
        );
    }
}
#[cfg(test)]
mod scenario_scroll_tests {
    use super::{
        SCENARIO_INFO_SCROLL, SCENARIO_LIST_SCROLL, scenario_rows_for, scenario_scroll_hit, ui,
    };

    #[test]
    #[ignore = "requires original v1.4 Scenario completion-cup fixture"]
    fn scenario_completion_cups_match_original_painter() {
        let fixture = std::fs::read(openmt2001::fixtures::path("NATIVE_SCENARIO_CUPS")).unwrap();
        assert_eq!(&fixture[..8], b"MTSCCP01");
        assert_eq!(fixture.len(), 8 + 32 + 12 * 4);
        let values: Vec<u32> = fixture[40..]
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect();
        assert_eq!(&values[..7], &[242, 20, 98, 12, 17, 14, 0x874]);
        assert_eq!(
            &values[7..11],
            &[293, 294, 295, super::SCENARIO_CUP_EMPTY as u32]
        );
        assert_eq!(values[11], 2);
        for row in [0, 1, 5] {
            for cup in 0..3 {
                let left = values[0] + cup * values[1];
                let top = values[2] + row * values[3];
                assert_eq!(
                    super::scenario_cup_rect(row as usize, cup as usize),
                    [
                        left as f32,
                        top as f32,
                        (left + values[4]) as f32,
                        (top + values[5]) as f32
                    ]
                );
            }
        }
    }

    #[test]
    fn native_scenario_registration_flags_keep_one_display_row() {
        use openmt2001::level_catalog::{Catalog, Level, RECORD_SIZE};
        let mut catalog = Catalog::default();
        for (id, unlocked, tutorial, custom) in [
            (200003u32, true, false, false),
            (200004, false, false, false),
            (100000, true, true, false),
            (500001, true, false, true),
        ] {
            let mut bytes = [0u8; RECORD_SIZE];
            bytes[1] = 1;
            bytes[0x44c..0x450].copy_from_slice(&id.to_le_bytes());
            bytes[0x86c] = u8::from(unlocked);
            bytes[0x85c] = u8::from(tutorial);
            bytes[2] = u8::from(custom);
            catalog.levels.push(Level { bytes });
        }
        // Original 49b1da/49b1e4/49b1eb rejects tutorial, locked and
        // custom records before appending a native list row.
        assert_eq!(scenario_rows_for(&catalog), vec![200003]);
    }

    #[test]
    fn description_arrows_and_track_admit_only_an_overflowing_list() {
        let base = 6;
        assert_eq!(scenario_scroll_hit(0, 1, [596., 210.]), None);
        assert_eq!(scenario_scroll_hit(4, 1, [596., 210.]), Some(base));
        assert_eq!(scenario_scroll_hit(4, 1, [596., 388.]), Some(base + 1));
        assert_eq!(scenario_scroll_hit(4, 1, [596., 300.]), Some(base + 2));
        assert_eq!(scenario_scroll_hit(4, 1, [580., 300.]), None);
        let track = ui::slider_track(SCENARIO_INFO_SCROLL, true);
        assert_eq!(ui::slider_selection(track, 220., 4, true), 0);
        assert_eq!(ui::slider_selection(track, 377., 4, true), 4);
    }

    #[test]
    #[ignore = "requires original v1.4 scenario scrollbar fixture"]
    fn scenario_scrollbars_match_original_constructor_geometry() {
        let fixture =
            std::fs::read(openmt2001::fixtures::path("NATIVE_SCENARIO_SCROLLBARS")).unwrap();
        assert_eq!(&fixture[..8], b"MTSCSL01");
        let mut at = 40usize;
        let mut word = || {
            let value = u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
            at += 4;
            value
        };
        assert_eq!(word(), 0x037f);
        assert_eq!(word(), 2);
        for (part, total) in [(24, SCENARIO_INFO_SCROLL), (8, SCENARIO_LIST_SCROLL)] {
            assert_eq!(word(), part);
            let source: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word()));
            assert_eq!(source[2].round() as i32 - 16, total[0] as i32);
            let args: [i32; 4] = std::array::from_fn(|_| word() as i32);
            assert_eq!(
                args,
                [
                    total[0] as i32,
                    total[1] as i32,
                    (total[3] - total[1]) as i32,
                    1
                ]
            );
            let outer: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word()));
            let track: [f32; 5] = std::array::from_fn(|_| f32::from_bits(word()));
            assert_eq!(outer, ui::slider_track(total, true));
            assert_eq!(track[..4], outer);
            assert_eq!(track[4], 0.5);
            for (expected, arrow) in [105, 93].into_iter().zip(ui::slider_arrows(total, true)) {
                assert_eq!(word(), expected);
                let actual: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word()));
                assert_eq!(actual, arrow);
            }
        }
        assert_eq!(at, fixture.len());
    }
}
#[cfg(test)]
mod character_biography_tests {
    use super::{ascii, character_biography_lines, ui};
    use openmt2001::archive::ArchiveIndex;

    #[test]
    #[ignore = "requires original v1.4 Character biography fixture and archive"]
    fn original_dog_biography_uses_font2_style0_and_fits() {
        let fixture =
            std::fs::read(openmt2001::fixtures::path("NATIVE_CHARACTER_BIOGRAPHY")).unwrap();
        assert_eq!(&fixture[..8], b"MTCHBIO1");
        let mut at = 40;
        let mut word = || {
            let value = u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
            at += 4;
            value
        };
        let bounds: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word()));
        assert_eq!(word(), 0); // style0
        assert_eq!(word(), 2); // original font index2
        let line_height = word();
        assert_eq!(line_height, 10);
        let length = word() as usize;
        let dog = &fixture[at..at + length];
        assert_eq!(at + length, fixture.len());

        let installer = openmt2001::fixtures::path("PARAMETERS")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let font = std::fs::read(installer.join("Standard/fonts/tahoma11noantip.tab")).unwrap();
        let lines = character_biography_lines(&font, dog, bounds[2] - bounds[0]).unwrap();
        assert_eq!(lines.len(), 11);
        assert_eq!(
            lines[0],
            ascii("Dog gets her name from her favorite pet. She will")
        );
        assert_eq!(lines[10], ascii("shows true compassion for."));
        assert!(lines.len() as f32 * line_height as f32 <= bounds[3] - bounds[1]);

        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let (_, regions) = ui::panel_base(
            &index,
            &archive,
            "gamedata2\\brand_new_panels\\sp_token_V2.tm",
            &[68],
        )
        .unwrap();
        assert_eq!(regions[68], bounds);
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}
#[derive(Clone, Debug)]
pub struct Setup {
    pub preview: Preview,
    pub difficulty: Difficulty,
    pub character_index: usize,
    pub profile_name: Vec<u8>,
    /// The selected profile's authoritative progression at launch time.
    pub catalog: openmt2001::level_catalog::Catalog,
    pub profile_path: PathBuf,
}
struct Panel {
    base: Vec<Surface>,
    bounds: Vec<[f32; 4]>,
}
struct Character {
    name: usize,
    description: usize,
    color: u32,
    token: usize,
    portrait: usize,
}
pub struct SinglePlayer {
    custom: Option<custom::Custom>,
    scenario: Panel,
    tutorial_panel: Panel,
    character_panel: Panel,
    generic: Vec<Surface>,
    characters: Vec<Character>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    scenario_list_fonts: Vec<Surface>,
    scenario_list_tab: Vec<u8>,
    title_fonts: Vec<Surface>,
    title_tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    pub visible: bool,
    character_screen: bool,
    tutorial_screen: bool,
    sandbox: bool,
    difficulty: usize,
    scenario_id: Option<u32>,
    scenario_rows: Vec<u32>,
    tutorial_index: Option<usize>,
    character: usize,
    inspected_slot: usize,
    // Concrete text-control first-line survives clear/repopulate and Back.
    biography_first: std::cell::Cell<usize>,
    // Runtime/profile bytes; convert to glyph indices only at the draw boundary.
    name: Vec<u8>,
    catalog: Option<openmt2001::level_catalog::Catalog>,
    profile_path: Option<PathBuf>,
    pub request: Option<Setup>,
    pub busy: bool,
    pub message: Option<String>,
    description_first: usize,
    pointer_position: Option<[f32; 2]>,
}
fn native_generic_click(visible: bool, busy: bool, character_screen: bool, i: usize) -> bool {
    visible && (!busy || i == 1) && if character_screen { i < 4 } else { i < 2 }
}
fn scenario_scroll_hit(maximum: usize, rows: usize, point: [f32; 2]) -> Option<usize> {
    if maximum == 0 {
        return None;
    }
    let inside =
        |[l, t, r, b]: [f32; 4]| point[0] >= l && point[0] <= r && point[1] >= t && point[1] <= b;
    let base = 5 + rows;
    if let Some(side) = ui::slider_arrows(SCENARIO_INFO_SCROLL, true)
        .into_iter()
        .position(inside)
    {
        return Some(base + side);
    }
    inside(ui::slider_track(SCENARIO_INFO_SCROLL, true)).then_some(base + 2)
}
fn scenario_levels(
    catalog: &openmt2001::level_catalog::Catalog,
) -> impl Iterator<Item = &openmt2001::level_catalog::Level> {
    // 48d9f0(mode0) supplies single-player records; 49b1b0 then checks
    // tutorial, unlock and kind flags and associates each row with its real ID.
    catalog.levels.iter().filter(|level| {
        level.single_player() && !level.tutorial() && level.unlocked() && level.bytes[2] == 0
    })
}
fn scenario_rows_for(catalog: &openmt2001::level_catalog::Catalog) -> Vec<u32> {
    scenario_levels(catalog).map(|level| level.id()).collect()
}
fn scenario_preselection(catalog: &openmt2001::level_catalog::Catalog) -> Option<u32> {
    // 49b250 leaves the selection empty if every eligible row has any cup.
    scenario_levels(catalog)
        .find(|level| level.completed() == [false; 3])
        .map(|level| level.id())
}
fn scenario_cup_rect(row: usize, cup: usize) -> [f32; 4] {
    let left = 242. + 20. * cup as f32;
    let top = 98. + SCENARIO_ROW_HEIGHT * row as f32;
    [left, top, left + 17., top + 14.]
}

fn tutorial_completed(catalog: Option<&openmt2001::level_catalog::Catalog>, id: u32) -> [bool; 3] {
    catalog
        .and_then(|c| c.find(id).map(|i| c.levels[i].completed()))
        .unwrap_or([false; 3])
}

fn tutorial_preselection(catalog: &openmt2001::level_catalog::Catalog) -> Option<usize> {
    // 49c890 skips tutorials completed at any difficulty. When all are done,
    // the original requests the row past the list, leaving no visible selection.
    crate::scenarios::TUTORIALS.iter().position(|t| {
        !tutorial_completed(Some(catalog), t.id)
            .into_iter()
            .any(|done| done)
    })
}

fn tutorial_tick_rows(
    catalog: Option<&openmt2001::level_catalog::Catalog>,
) -> impl Iterator<Item = usize> {
    // 49cb0a reads only the Easy flag, unlike the open-time selection rule.
    crate::scenarios::TUTORIALS
        .iter()
        .enumerate()
        .filter_map(move |(row, t)| tutorial_completed(catalog, t.id)[0].then_some(row))
}

fn tutorial_tick_rect(row: usize) -> [f32; 4] {
    let top = 94. + TUTORIAL_ROW_HEIGHT * row as f32;
    [430., top, 446., top + 12.]
}

// Ordinary campaign description: 499707..499848 builds a Font2 rich-text
// child from Info8 minus the always-reserved scrollbar. 440650 sets inset3;
// 441279..44129d subtracts both insets before 4e7a90 wraps with glyph gap -1.
// Custom, Tutorial and Character keep their independent layout contracts.
struct CampaignDescription {
    offsets: Vec<usize>,
    text: Vec<u8>,
    child: [f32; 4],
    origin: [f32; 2],
    visible: usize,
}
impl CampaignDescription {
    fn new(tab: &[u8], text: &[u8], bounds: [f32; 4]) -> Result<Self, String> {
        let [left, top, right, bottom] = bounds;
        let strip = SCENARIO_INFO_SCROLL[2] - SCENARIO_INFO_SCROLL[0];
        let child = [left, top, right - strip, bottom];
        // Keep the fractional mesh edges until native toward-zero conversion.
        let width = (f64::from(child[2]) - f64::from(left) - 6.).trunc();
        if bounds.iter().any(|v| !v.is_finite())
            || bottom <= top
            || width < 1.
            || width > f64::from(i32::MAX)
        {
            return Err("invalid campaign description bounds".into());
        }
        let (offsets, text) = crate::prompt::wrap_with_gap(tab, text, width as i32, -1)?;
        Ok(Self {
            offsets,
            text,
            child,
            // 4406a0 style0: integer left + inset3 + 2, integer(top+3).
            origin: [left.trunc() + 5., (top + 3.).trunc()],
            // Preserve the existing whole-row capacity, independent of inset.
            // W height10 + native vertical gap2 remains a 12-unit pitch.
            visible: ((bottom - top) / SCENARIO_ROW_HEIGHT) as usize,
        })
    }
    fn rows(&self) -> impl Iterator<Item = &[u8]> {
        self.offsets.windows(2).map(|range| {
            let end = range[1].saturating_sub(1).min(self.text.len());
            &self.text[range[0].min(end)..end]
        })
    }
    fn limit(&self) -> usize {
        self.offsets
            .len()
            .saturating_sub(1)
            .saturating_sub(self.visible)
    }
    fn wheel(&self, first: usize, delta: i32) -> usize {
        (first as i64 + i64::from(delta)).clamp(0, self.limit() as i64) as usize
    }
    fn paint(&self, pages: &mut [Surface], tab: &[u8], first: usize) -> Result<(), String> {
        // Use exactly the same visible-row capacity as limit/wheel/track.
        // ui::text retains its existing glyph renderer and child-edge clipping.
        for (line, text) in self.rows().skip(first).take(self.visible).enumerate() {
            ui::text(
                pages,
                tab,
                text,
                [
                    self.origin[0],
                    self.origin[1] + line as f32 * SCENARIO_ROW_HEIGHT,
                    self.child[2],
                    self.child[3],
                ],
                false,
                0xffffffff,
            )?;
        }
        Ok(())
    }
}

fn character_biography_lines(
    tab: &[u8],
    encoded: &[u8],
    width: f32,
) -> Result<Vec<Vec<u8>>, String> {
    let (offsets, wrapped) = crate::prompt::wrap_with_gap(tab, encoded, width.trunc() as i32, -1)?;
    Ok(offsets
        .windows(2)
        .map(|range| {
            let end = range[1].saturating_sub(1).min(wrapped.len());
            wrapped[range[0].min(end)..end].to_vec()
        })
        .collect())
}
impl SinglePlayer {
    pub fn new(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        executable: &[u8],
    ) -> Result<Self, String> {
        let panel = |name: &str, parts: &[usize]| -> Result<Panel, String> {
            let (base, bounds) = ui::panel_base(index, archive, name, parts)?;
            Ok(Panel { base, bounds })
        };
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing setup asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("setup asset range".into())
        };
        let texture = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let generic = TextureSet::parse(payload("gamedata2\\GenericTextures640\\textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let pe = PeImage::parse(executable)?;
        let mut characters = vec![];
        // 4a8c68 indexes these40-byte descriptors; 4a8cb2/4a8cd9 draw token/portrait.
        for i in 0..10 {
            let raw = pe.at(0x541ae8 + i * 40, 24)?;
            let w = |at| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
            characters.push(Character {
                name: w(0) as usize,
                description: w(4) as usize,
                color: w(8),
                token: w(16) as usize,
                portrait: w(20) as usize,
            });
        }
        Ok(Self {
            custom: None,
            scenario: panel(
                "gamedata2\\brand_new_panels\\SP_Scenario_V2.tm",
                &[0, 1, 2, 3, 4, 5, 7],
            )?,
            tutorial_panel: panel(
                "gamedata2\\brand_new_panels\\tutorial_panel_v2.tm",
                &(0..14).collect::<Vec<_>>(),
            )?,
            character_panel: panel(
                "gamedata2\\brand_new_panels\\sp_token_V2.tm",
                &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14],
            )?,
            generic,
            characters,
            fonts: texture.textures[2..4].iter().map(Surface::new).collect(),
            tab: fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab"))
                .map_err(|e| e.to_string())?,
            // Native 4997d3/4997e5 and 499904/499916 select font2 for
            // the Scenario description and list columns.
            scenario_list_fonts: texture.textures[4..6].iter().map(Surface::new).collect(),
            scenario_list_tab: fs::read(installer.join("Standard/fonts/tahoma11noantip.tab"))
                .map_err(|e| e.to_string())?,
            title_fonts: texture.textures[8..10].iter().map(Surface::new).collect(),
            title_tab: fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
                .map_err(|e| e.to_string())?,
            strings: strings
                .iter()
                .map(|s| crate::scenarios::currency_text(s, strings))
                .collect::<Result<_, _>>()?,
            visible: false,
            character_screen: false,
            tutorial_screen: false,
            sandbox: false,
            difficulty: 0,
            scenario_id: None,
            scenario_rows: vec![],
            tutorial_index: None,
            character: 0,
            inspected_slot: 0,
            biography_first: std::cell::Cell::new(0),
            name: vec![],
            catalog: None,
            profile_path: None,
            request: None,
            busy: false,
            message: None,
            description_first: 0,
            pointer_position: None,
        })
    }
    pub fn open(
        &mut self,
        sandbox: bool,
        name: &str,
        mut catalog: openmt2001::level_catalog::Catalog,
        profile_path: PathBuf,
    ) {
        self.custom = None;
        self.visible = true;
        self.tutorial_screen = false;
        self.sandbox = sandbox;
        self.character_screen = sandbox;
        self.inspected_slot = 0;
        self.difficulty = 0;
        // Fail closed before loading another profile's selection/catalog.
        self.request = None;
        self.busy = true;
        self.catalog = None;
        self.scenario_id = None;
        self.scenario_rows.clear();
        self.name = name.as_bytes().to_vec();
        self.profile_path = Some(profile_path.clone());
        let root = profile_path.parent().and_then(|p| p.parent());
        let token = root
            .ok_or("Profile root missing".to_string())
            .and_then(|root| {
                crate::menu::preferred_token::Token::load(&crate::menu::preferred_token::path(
                    root,
                    name.as_bytes(),
                ))
            });
        self.character = match token {
            Ok(t) => t.index(),
            Err(e) => {
                self.message = Some(e);
                self.custom = None;
                self.visible = true;
                return;
            }
        };
        self.name = name.as_bytes().to_vec();
        // Original 49b1b0 walks registration order and appends only
        // non-tutorial, unlocked, non-custom scenario records.
        self.scenario_rows = scenario_rows_for(&catalog);
        self.scenario_id = scenario_preselection(&catalog);
        catalog.difficulty = 0;
        self.catalog = Some(catalog);
        self.profile_path = Some(profile_path);
        self.request = None;
        self.busy = false;
        self.message = None;
        self.description_first = 0;
        self.pointer_position = None;
    }
    pub fn open_tutorial(
        &mut self,
        name: &str,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: PathBuf,
    ) {
        self.custom = None;
        self.visible = true;
        self.tutorial_screen = true;
        self.character_screen = false;
        self.sandbox = false;
        self.difficulty = 0;
        self.character = 0;
        self.name = name.as_bytes().to_vec();
        self.tutorial_index = tutorial_preselection(&catalog);
        self.catalog = Some(catalog);
        self.profile_path = Some(profile_path);
        self.request = None;
        self.busy = false;
        self.message = None;
        self.description_first = 0;
        self.pointer_position = None;
    }
    pub fn reload_progress(&mut self) -> Result<(), String> {
        let Some(path) = &self.profile_path else {
            return Ok(());
        };
        let catalog = self.catalog.as_mut().ok_or("Campaign catalog missing")?;
        crate::campaign_store::load_progress(catalog, path)?;
        if self.tutorial_screen {
            self.tutorial_index = tutorial_preselection(catalog);
            self.description_first = 0;
        } else if self.custom.is_none() {
            self.scenario_rows = scenario_rows_for(catalog);
            if !self
                .scenario_id
                .is_some_and(|id| self.scenario_rows.contains(&id))
            {
                self.scenario_id = scenario_preselection(catalog);
            }
        }
        self.message = None;
        Ok(())
    }
    pub fn count(&self) -> usize {
        if self.custom.is_some() && !self.character_screen {
            return self.custom_count();
        }
        if self.tutorial_screen {
            2 + crate::scenarios::TUTORIALS.len()
        } else if self.character_screen {
            4
        } else {
            5 + self.scenario_rows.len()
                + if self.scenario_description_limit() > 0 {
                    3
                } else {
                    0
                }
        }
    }
    /// The Scenario and Tutorial bottom children and the Character bottom
    /// children/arrows are original 43ed20 widgets with 443360 press audio.
    /// Scenario difficulty radios and both list families use other widgets.
    pub fn generic_click_enabled(&self, i: usize) -> bool {
        if !self.keyboard_enabled(i) {
            return false;
        }
        native_generic_click(self.visible, self.busy, self.character_screen, i)
    }
    /// Identity of the actual active displayed roster slot, never an editable
    /// human selection. Ordinary non-Custom display has only its human slot.
    fn participant_character(&self, slot: usize) -> Option<usize> {
        if slot >= 6 {
            return None;
        }
        if let Some(roster) = self.custom.as_ref().and_then(|c| c.roster.as_ref()) {
            let record = roster.records[slot];
            return (record.kind != -1 && (0..10).contains(&record.character))
                .then_some(record.character as usize);
        }
        (slot == 0).then_some(self.character)
    }
    fn biography_character(&self) -> usize {
        self.participant_character(self.inspected_slot)
            .unwrap_or(self.character)
    }
    /// Root fallback only: real generic children own their rectangles first.
    /// Called once on physical Left's fresh DOWN by Menu, never by pointer motion.
    pub fn inspect_participant_press(&mut self, point: [f32; 2], right: bool) -> bool {
        if !self.visible
            || !self.character_screen
            || self.busy
            || self.message.is_some()
            || right
            || self.hit(point).is_some()
        {
            return false;
        }
        let mut admitted = false;
        for slot in 0..6 {
            let [l, t, r, b] = self.character_panel.bounds[35 + slot];
            if point[0] > l
                && point[0] < r
                && point[1] >= t
                && point[1] <= b
                && self.participant_character(slot).is_some()
            {
                self.inspected_slot = slot;
                // Same-slot press still invalidates/rebuilds the menu scene.
                // Original clear+identify does not reset first-line.
                admitted = true;
            }
        }
        admitted
    }
    fn biography_first_for_rows(&self, rows: usize) -> usize {
        let first = self.biography_first.get();
        if first > rows {
            self.biography_first.set(0);
            0
        } else {
            first
        }
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        self.pointer_position = Some(p);
    }
    pub fn is_scroll_track(&self, origin: Option<usize>) -> bool {
        if self.custom.is_some() {
            return !self.character_screen && matches!(origin, Some(7 | 10));
        }
        self.visible
            && !self.tutorial_screen
            && !self.character_screen
            && self.scenario_description_limit() > 0
            && origin == Some(7 + self.scenario_rows.len())
    }
    pub fn drag_track(&mut self, origin: Option<usize>, p: [f32; 2]) -> bool {
        if self.custom.is_some() && self.is_scroll_track(origin) {
            self.pointer_position = Some(p);
            self.custom_scroll_control(7, origin == Some(10));
            return true;
        }
        if !self.is_scroll_track(origin) {
            return false;
        }
        let next = ui::slider_selection(
            ui::slider_track(SCENARIO_INFO_SCROLL, true),
            p[1],
            self.scenario_description_limit() as i32,
            true,
        ) as usize;
        let changed = next != self.description_first;
        self.description_first = next;
        changed
    }
    fn campaign_description(&self) -> Result<CampaignDescription, String> {
        CampaignDescription::new(
            &self.scenario_list_tab,
            &self.scenario_record_text(false),
            self.scenario.bounds[24],
        )
    }
    fn scenario_description_limit(&self) -> usize {
        if self.custom.is_none() {
            return self
                .campaign_description()
                .map(|layout| layout.limit())
                .unwrap_or(0);
        }
        let text = self.scenario_record_text(false);
        let b = self.scenario.bounds[24];
        let visible = ((b[3] - b[1]) / SCENARIO_ROW_HEIGHT) as usize;
        wrap_lines(&self.scenario_list_tab, &text, b[2] - b[0])
            .map(|lines| lines.len().saturating_sub(visible))
            .unwrap_or(0)
    }
    fn scenario_record_text(&self, title: bool) -> Vec<u8> {
        if self.custom.is_some() {
            return self.custom_text(title);
        }
        let Some(id) = self.scenario_id else {
            return if title {
                self.strings.get(0x66e).cloned().unwrap_or_default()
            } else {
                vec![]
            };
        };
        self.scenario_record_text_at(id, title)
    }
    fn scenario_record_text_at(&self, id: u32, title: bool) -> Vec<u8> {
        let fallback = level_labels(id)
            .and_then(|labels| self.strings.get(if title { labels.0 } else { labels.1 }))
            .cloned()
            .unwrap_or_default();
        let Some(level) = self
            .catalog
            .as_ref()
            .and_then(|c| c.find(id).map(|i| &c.levels[i]))
        else {
            return fallback;
        };
        match level.labels() {
            openmt2001::level_catalog::Labels::Localized {
                title: name,
                description,
            } => self
                .strings
                .get((if title { name } else { description }) as usize)
                .cloned()
                .unwrap_or(fallback),
            openmt2001::level_catalog::Labels::Literal {
                title: name,
                description,
            } => (if title { name } else { description })
                .into_iter()
                .map(|byte| {
                    if (32..=126).contains(&byte) {
                        byte - 32
                    } else {
                        b'?' - 32
                    }
                })
                .collect(),
        }
    }
    pub fn back(&mut self) {
        self.busy = false;
        self.request = None;
        self.message = None;
        if self.tutorial_screen {
            self.tutorial_screen = false;
            self.visible = false;
        } else if self.character_screen && !self.sandbox {
            self.character_screen = false;
        } else {
            self.visible = false;
        }
        self.description_first = 0;
    }
    fn choose_character(&mut self, value: usize) -> Result<(), String> {
        let root = self
            .profile_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .ok_or("Profile root missing")?;
        crate::menu::preferred_token::Token::new(value)?
            .save(&crate::menu::preferred_token::path(root, &self.name))?;
        self.character = value;
        self.inspected_slot = 0;
        if self.custom.is_some() {
            self.custom_roster()?;
        }
        Ok(())
    }
    pub fn activate(&mut self, i: usize) {
        if self.busy && i != 1 {
            return;
        }
        if self.custom.is_some() && !self.character_screen {
            self.custom_activate(i);
            if self.character_screen {
                self.inspected_slot = 0;
            }
            return;
        }
        if self.tutorial_screen {
            match i {
                0 => {
                    let Some(row) = self.tutorial_index else {
                        return;
                    };
                    let tutorial = crate::scenarios::TUTORIALS[row];
                    if !crate::scenarios::live_tutorial(tutorial.id) {
                        self.message =
                            Some("This tutorial's live session is not yet available.".into());
                        return;
                    }
                    let Some(catalog) = self.catalog.clone() else {
                        self.message = Some("Selected profile catalog is unavailable".into());
                        return;
                    };
                    let Some(profile_path) = self.profile_path.clone() else {
                        self.message = Some("Selected profile path is unavailable".into());
                        return;
                    };
                    self.request = Some(Setup {
                        preview: Preview::Scenario(tutorial.id),
                        difficulty: Difficulty::Easy,
                        character_index: 0,
                        profile_name: self.name.clone(),
                        catalog,
                        profile_path,
                    });
                    self.busy = true;
                    self.message = Some("Loading tutorial...".into());
                }
                1 => self.back(),
                2.. if i < self.count() => {
                    self.tutorial_index = Some(i - 2);
                    self.description_first = 0;
                    self.message = None;
                }
                _ => (),
            }
        } else if self.character_screen {
            match i {
                0 => {
                    if self.custom.is_some() {
                        if let Err(e) = self.custom_roster() {
                            self.message = Some(e);
                            return;
                        }
                    }
                    let Some(catalog) = self.catalog.clone() else {
                        self.message = Some("Selected profile catalog is unavailable".into());
                        return;
                    };
                    let Some(profile_path) = self.profile_path.clone() else {
                        self.message = Some("Selected profile path is unavailable".into());
                        return;
                    };
                    self.request = Some(Setup {
                        preview: if let Some(c) = &self.custom {
                            Preview::Scenario(
                                c.selected
                                    .and_then(|i| self.catalog.as_ref()?.levels.get(i))
                                    .map(|l| l.id())
                                    .unwrap(),
                            )
                        } else if self.sandbox {
                            Preview::Sandbox
                        } else {
                            let Some(id) = self.scenario_id else {
                                return;
                            };
                            Preview::Scenario(id)
                        },
                        difficulty: [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard]
                            [self.difficulty],
                        character_index: self.character,
                        profile_name: self.name.clone(),
                        catalog,
                        profile_path,
                    });
                    self.busy = true;
                    self.message = Some("Loading city preview...".into());
                }
                1 => self.back(),
                2 => {
                    if let Err(e) = self.choose_character((self.character + 9) % 10) {
                        self.message = Some(e);
                        return;
                    }
                    self.description_first = 0;
                }
                3 => {
                    if let Err(e) = self.choose_character((self.character + 1) % 10) {
                        self.message = Some(e);
                        return;
                    }
                    self.description_first = 0;
                }
                _ => (),
            }
        } else {
            match i {
                0 => {
                    let Some(id) = self.scenario_id else {
                        return;
                    };
                    let unlocked = self
                        .catalog
                        .as_ref()
                        .and_then(|c| c.find(id).map(|i| c.levels[i].unlocked()))
                        .unwrap_or(false);
                    if !unlocked {
                        self.message =
                            Some("This scenario is locked in the selected profile.".into());
                        return;
                    }
                    self.character_screen = true;
                    self.inspected_slot = 0;
                    self.description_first = 0;
                }
                1 => self.back(),
                2..=4 => self.difficulty = i - 2,
                i if i == 5 + self.scenario_rows.len() => self.scroll(-1),
                i if i == 6 + self.scenario_rows.len() => self.scroll(1),
                i if i == 7 + self.scenario_rows.len() => {
                    if let Some(point) = self.pointer_position {
                        self.drag_track(Some(i), point);
                    }
                }
                5.. if i - 5 < self.scenario_rows.len() => {
                    self.scenario_id = Some(self.scenario_rows[i - 5]);
                    self.description_first = 0;
                    self.message = None;
                }
                _ => (),
            }
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.custom.is_none() && !self.tutorial_screen && !self.character_screen {
            if let Ok(layout) = self.campaign_description() {
                self.description_first = layout.wheel(self.description_first, delta);
            }
            return;
        }
        if self.custom.is_some()
            && !self.character_screen
            && self.pointer_position.is_some_and(|p| p[0] < 320.)
        {
            let max = self.custom_list_limit();
            let c = self.custom.as_mut().unwrap();
            c.first = (c.first as i32 + delta).clamp(0, max as i32) as usize;
            return;
        }
        let scenario_text = if !self.tutorial_screen && !self.character_screen {
            self.scenario_record_text(false)
        } else {
            vec![]
        };
        let (text, b) = if self.tutorial_screen {
            let Some(row) = self.tutorial_index else {
                return;
            };
            (
                &self.strings[level_labels(crate::scenarios::TUTORIALS[row].id).unwrap().1],
                self.tutorial_panel.bounds[11],
            )
        } else if self.character_screen {
            (
                &self.strings[self.characters[self.biography_character()].description],
                self.character_panel.bounds[68],
            )
        } else {
            (&scenario_text, self.scenario.bounds[24])
        };
        let tab = if self.tutorial_screen {
            &self.tab
        } else {
            &self.scenario_list_tab
        };
        let row_height = if self.tutorial_screen {
            14.
        } else if self.character_screen {
            10.
        } else {
            SCENARIO_ROW_HEIGHT
        };
        let lines = if self.character_screen {
            character_biography_lines(tab, text, b[2] - b[0])
        } else {
            wrap_lines(tab, text, b[2] - b[0])
        };
        if let Ok(lines) = lines {
            let visible = ((b[3] - b[1]) / row_height) as usize;
            // Existing modern wheel convenience retained, scoped explicitly:
            // no original Character wheel producer is claimed. It now scrolls
            // the inspected biography; original paint retention remains separate.
            if self.character_screen {
                self.biography_first.set(
                    (self.biography_first.get() as i32 + delta)
                        .clamp(0, lines.len().saturating_sub(visible) as i32)
                        as usize,
                );
            } else {
                self.description_first = (self.description_first as i32 + delta)
                    .clamp(0, lines.len().saturating_sub(visible) as i32)
                    as usize;
            }
        }
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        if self.custom.is_some() && !self.character_screen {
            return self.custom_hit(p);
        }
        let inside = |[l, t, r, b]: [f32; 4]| p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b;
        if self.tutorial_screen {
            for (i, part) in [13, 12].iter().enumerate() {
                if inside(self.tutorial_panel.bounds[*part]) {
                    return Some(i);
                }
            }
            if inside(self.tutorial_panel.bounds[8]) {
                let row =
                    ((p[1] - self.tutorial_panel.bounds[8][1]) / TUTORIAL_ROW_HEIGHT) as usize;
                return (row < crate::scenarios::TUTORIALS.len()).then_some(2 + row);
            }
        } else if self.character_screen {
            for (i, part) in [70, 69, 53, 54].iter().enumerate() {
                if inside(self.character_panel.bounds[*part]) {
                    return Some(i);
                }
            }
        } else {
            if let Some(slider) = scenario_scroll_hit(
                self.scenario_description_limit(),
                self.scenario_rows.len(),
                p,
            ) {
                return Some(slider);
            }
            for (i, part) in [26, 25, 19, 18, 17].iter().enumerate() {
                if inside(self.scenario.bounds[*part]) {
                    return Some(i);
                }
            }
            if inside(self.scenario.bounds[8]) {
                let row = ((p[1] - self.scenario.bounds[8][1]) / SCENARIO_ROW_HEIGHT) as usize;
                return (row < self.scenario_rows.len()).then_some(5 + row);
            }
        }
        None
    }
    fn image(&self, out: &mut Vec<Surface>, index: usize, b: [f32; 4]) -> Result<(), String> {
        let mut s = self
            .generic
            .get(index)
            .ok_or("setup texture index")?
            .clone();
        let [l, t, r, b] = b;
        s.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        out.push(s);
        Ok(())
    }
    fn draw_tutorial(
        &self,
        renderer: &Renderer,
        selected: Option<usize>,
        pressed: bool,
    ) -> Result<PreparedScene, String> {
        let b = &self.tutorial_panel.bounds;
        let mut surfaces = self.tutorial_panel.base.clone();
        let mut shapes = Surface::solid();
        let mut fonts = self.fonts.clone();
        let mut title_fonts = self.title_fonts.clone();
        // 49c970 paints the Tutorial title-area child with a gold 433370
        // rectangle before its text. Vertex order is TL, BL, BR, TR.
        shapes.rect(b[6], TUTORIAL_TITLE_COLORS);
        for part in [8, 11] {
            shapes.rect(b[part], [0xbb000a28; 4]);
        }
        // Original 49c9e8..49ca29 paints both caption backings after the
        // body fills; their reverse-facing mesh quads are layout regions.
        for part in [9, 10] {
            shapes.rect(b[part], [0xff909090, 0xff909090, 0x80909090, 0x80909090]);
        }
        // 49c3b6/49c4a1 construct font4 and font1 style9 caption widgets.
        ui::text_widget_style9(
            &mut title_fonts,
            &self.title_tab,
            &self.strings[1667],
            b[6],
            0xffffffff,
        )?;
        ui::text_widget_style9(&mut fonts, &self.tab, &self.strings[1668], b[9], 0xffffffff)?;
        let labels = self
            .tutorial_index
            .and_then(|row| level_labels(crate::scenarios::TUTORIALS[row].id));
        ui::text(
            &mut fonts,
            &self.tab,
            &self.strings[labels.map_or(1646, |(title, _)| title)],
            b[10],
            true,
            0xffffffff,
        )?;
        let [l, t, r, _] = b[8];
        for (row, tutorial) in crate::scenarios::TUTORIALS.iter().enumerate() {
            let top = t + row as f32 * TUTORIAL_ROW_HEIGHT;
            if self.tutorial_index == Some(row) || selected == Some(2 + row) {
                shapes.rect(
                    [l, top, r, top + TUTORIAL_ROW_HEIGHT],
                    [if self.tutorial_index == Some(row) {
                        0xff008ed0
                    } else {
                        0x99505050
                    }; 4],
                );
            }
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[level_labels(tutorial.id).unwrap().0],
                [l + 3., top, 430., top + TUTORIAL_ROW_HEIGHT],
                false,
                0xffffffff,
            )?;
        }
        if let Some((_, description)) = labels {
            wrapped(
                &mut fonts,
                &self.tab,
                &self.strings[description],
                b[11],
                self.description_first,
            )?;
        }
        for (i, part, label, color) in [(0, 13, 1435, [80, 148, 34]), (1, 12, 1439, [175, 6, 6])] {
            ui::button(
                &mut shapes,
                b[part],
                color,
                selected == Some(i),
                pressed && selected == Some(i),
                i == 0 && (self.busy || self.tutorial_index.is_none()),
            )?;
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[label],
                b[part],
                true,
                0xffffffff,
            )?;
        }
        wrapped(
            &mut fonts,
            &self.tab,
            &ascii(self.message.as_deref().unwrap_or("")),
            [32., 449., 609., 466.],
            0,
        )?;
        surfaces.push(shapes);
        surfaces.extend(fonts);
        surfaces.extend(title_fonts);
        for row in tutorial_tick_rows(self.catalog.as_ref()) {
            self.image(
                &mut surfaces,
                TUTORIAL_COMPLETION_TICK,
                tutorial_tick_rect(row),
            )?;
        }
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            [
                2. / 640.,
                0.,
                0.,
                0.,
                0.,
                -2. / 480.,
                0.,
                0.,
                0.,
                0.,
                1.,
                0.,
                -1.,
                1.,
                0.,
                1.,
            ],
            None,
        )
    }
    pub fn draw(
        &self,
        renderer: &Renderer,
        selected: Option<usize>,
        pressed: bool,
    ) -> Result<PreparedScene, String> {
        if self.tutorial_screen {
            return self.draw_tutorial(renderer, selected, pressed);
        }
        let panel = if self.character_screen {
            &self.character_panel
        } else {
            &self.scenario
        };
        let b = &panel.bounds;
        let mut surfaces = panel.base.clone();
        let mut shapes = Surface::solid();
        let mut fonts = self.fonts.clone();
        let mut scenario_list_fonts = self.scenario_list_fonts.clone();
        let mut title_fonts = self.title_fonts.clone();
        let mut images = vec![];
        let mut completion_cups = vec![];
        let mut slider_controls = vec![];
        let title_part = if self.character_screen { 21 } else { 6 };
        shapes.rect(
            b[title_part],
            [0xffffff00, 0xffffff00, 0x80ffff00, 0x80ffff00],
        );
        let mut title = b[title_part];
        title[1] += 3.;
        ui::text(
            &mut title_fonts,
            &self.title_tab,
            &self.strings[if self.character_screen {
                0x756
            } else if self.custom.is_some() {
                2240
            } else {
                0x755
            }],
            title,
            true,
            0xffffffff,
        )?;
        if self.character_screen {
            for part in [8, 62, 68] {
                shapes.rect(b[part], [0xee000530; 4]);
            }
            for (part, id) in [(22, 0x694), (61, 0x6a7)] {
                shapes.rect(b[part], [0xff909090, 0xff909090, 0x80909090, 0x80909090]);
                let mut r = b[part];
                r[1] += 4.;
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &self.strings[id],
                    r,
                    true,
                    0xffffffff,
                )?;
            }
            let profile_glyphs = ascii(&String::from_utf8_lossy(&self.name));
            for slot in 0..6 {
                let namepart = 34 - slot;
                shapes.rect(b[namepart], [0xff000530; 4]);
                let mut r = b[namepart];
                r[0] += 3.;
                ui::text(
                    &mut fonts,
                    &self.tab,
                    if slot == 0 {
                        &profile_glyphs
                    } else {
                        self.custom
                            .as_ref()
                            .and_then(|c| c.roster.as_ref())
                            .and_then(|r| {
                                let record = r.records[slot];
                                (record.kind != -1).then(|| {
                                    self.strings[self.characters[record.character as usize].name]
                                        .as_slice()
                                })
                            })
                            .unwrap_or(&self.strings[0x6db])
                    },
                    r,
                    false,
                    0xffffffff,
                )?;
                let mut tint = b[41 + slot];
                let authored = self
                    .custom
                    .as_ref()
                    .and_then(|c| c.roster.as_ref())
                    .and_then(|r| {
                        let v = r.records[slot];
                        (v.kind != -1).then_some(v.character as usize)
                    });
                if slot == 0 || authored.is_some() {
                    let token = authored.unwrap_or(self.character);
                    let c = self.characters[token].color;
                    shapes.rect(
                        tint,
                        [
                            c,
                            c,
                            (c & 0xffffff) | 0x7f000000,
                            (c & 0xffffff) | 0x7f000000,
                        ],
                    );
                    self.image(&mut images, self.characters[token].portrait, b[35 + slot])?;
                    self.image(&mut images, self.characters[token].token, b[47 + slot])?;
                } else {
                    shapes.rect(tint, [0xff909090, 0xff909090, 0x7f909090, 0x7f909090]);
                    tint = b[35 + slot];
                    shapes.rect(tint, [0x7f000000; 4]);
                }
                let difficulty_icon = self
                    .custom
                    .as_ref()
                    .and_then(|c| c.roster.as_ref())
                    .map(|r| {
                        let record = r.records[slot];
                        if record.kind == 0 {
                            259
                        } else if record.kind == 1 {
                            258
                        } else {
                            257
                        }
                    })
                    .unwrap_or(if slot == 0 { 259 } else { 257 });
                self.image(&mut images, difficulty_icon, b[55 + slot])?;
            }
            // 4a80fe/4a8117: normal, hover and pressed textures for the two arrows.
            for (i, part, texture) in [(2, 53, 97), (3, 54, 101)] {
                let texture = texture
                    + if selected == Some(i) {
                        if pressed { 2 } else { 1 }
                    } else {
                        0
                    };
                self.image(&mut images, texture, b[part])?;
            }
            let c = &self.characters[self.biography_character()];
            shapes.rect(
                b[67],
                [
                    c.color,
                    c.color,
                    (c.color & 0xffffff) | 0x7f000000,
                    (c.color & 0xffffff) | 0x7f000000,
                ],
            );
            let mut r = b[67];
            r[1] += 4.;
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[c.name],
                r,
                true,
                0xffffffff,
            )?;
            // Native 4a7e26 creates the biography widget as style0 and
            // 4a7e4b selects font2. Its W glyph advances each wrapped row.
            let biography = b[68];
            let biography_lines = character_biography_lines(
                &self.scenario_list_tab,
                &self.strings[c.description],
                biography[2] - biography[0],
            )?;
            let first = self.biography_first_for_rows(biography_lines.len());
            for (line, text) in biography_lines.iter().skip(first).enumerate() {
                let top = biography[1].trunc() + line as f32 * 10.;
                if top + 10. > biography[3] {
                    break;
                }
                ui::text_widget_style0(
                    &mut scenario_list_fonts,
                    &self.scenario_list_tab,
                    text,
                    [biography[0], top, biography[2], biography[3]],
                    0xffffffff,
                )?;
            }
            for (part, id) in [(63, 0x64f), (64, 0x757)] {
                shapes.rect(b[part], [0xff01762f; 4]);
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &self.strings[id],
                    b[part],
                    true,
                    0xff00ff00,
                )?;
            }
            ui::text(
                &mut fonts,
                &self.tab,
                &if self.sandbox {
                    self.strings[level_labels(400002).unwrap().0].clone()
                } else {
                    self.scenario_record_text(true)
                },
                b[65],
                true,
                0xffffffff,
            )?;
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[0x754 - self.difficulty],
                b[66],
                true,
                0xffffffff,
            )?;
        } else {
            let selected_title = self.scenario_record_text(true);
            let selected_description = self.scenario_record_text(false);
            for part in [8, 11] {
                shapes.rect(b[part], [0xbb000a28; 4]);
            }
            for (part, label) in [
                (9, self.strings[0x66d].as_slice()),
                (10, selected_title.as_slice()),
            ] {
                shapes.rect(b[part], [0xff909090, 0xff909090, 0x80909090, 0x80909090]);
                let mut r = b[part];
                r[1] += 4.;
                ui::text(&mut fonts, &self.tab, label, r, true, 0xffffffff)?;
            }
            let [l, t, r, _] = b[8];
            if self.custom.is_some() {
                self.custom_list_draw(&mut shapes, &mut scenario_list_fonts)?;
                self.custom_cups_draw(&mut completion_cups)?;
            }
            for (row, &id) in self
                .scenario_rows
                .iter()
                .enumerate()
                .filter(|_| self.custom.is_none())
            {
                let top = t + row as f32 * SCENARIO_ROW_HEIGHT;
                if self.scenario_id == Some(id) || selected == Some(5 + row) {
                    shapes.rect(
                        [l, top, r, top + SCENARIO_ROW_HEIGHT],
                        [if self.scenario_id == Some(id) {
                            0xff008ed0
                        } else {
                            0x99505050
                        }; 4],
                    );
                }
                ui::text(
                    &mut scenario_list_fonts,
                    &self.scenario_list_tab,
                    &self.scenario_record_text_at(id, true),
                    [l + 3., top, r, top + SCENARIO_ROW_HEIGHT],
                    false,
                    0xffffffff,
                )?;
                let completed = self
                    .catalog
                    .as_ref()
                    .and_then(|catalog| {
                        catalog
                            .find(id)
                            .map(|index| catalog.levels[index].completed())
                    })
                    .unwrap_or([false; 3]);
                for (cup, earned) in completed.into_iter().enumerate() {
                    self.image(
                        &mut completion_cups,
                        if earned {
                            293 + cup
                        } else {
                            SCENARIO_CUP_EMPTY
                        },
                        scenario_cup_rect(row, cup),
                    )?;
                }
            }
            for (part, id) in [(12, 0x757), (23, 0x75b)] {
                shapes.rect(b[part], [0xff01762f; 4]);
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &self.strings[id],
                    b[part],
                    true,
                    0xff00ff00,
                )?;
            }
            for i in 0..3 {
                self.image(&mut images, 293 + i, b[22 - i])?;
                self.image(
                    &mut images,
                    if self.difficulty == i { 174 } else { 175 },
                    b[19 - i],
                )?;
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &self.strings[0x754 - i],
                    b[16 - i],
                    false,
                    0xffffffff,
                )?;
            }
            if self.custom.is_none() {
                self.campaign_description()?.paint(
                    &mut scenario_list_fonts,
                    &self.scenario_list_tab,
                    self.description_first,
                )?;
            } else {
                for (line, text) in wrap_lines(
                    &self.scenario_list_tab,
                    &selected_description,
                    b[24][2] - b[24][0],
                )?
                .iter()
                .skip(self.description_first)
                .enumerate()
                {
                    let top = b[24][1] + line as f32 * SCENARIO_ROW_HEIGHT;
                    if top + SCENARIO_ROW_HEIGHT > b[24][3] {
                        break;
                    }
                    ui::text(
                        &mut scenario_list_fonts,
                        &self.scenario_list_tab,
                        text,
                        [b[24][0], top, b[24][2], b[24][3]],
                        false,
                        0xffffffff,
                    )?;
                }
            }
            // 499817/49994e attach original 43f510 children beside the
            // description and list. The 17 scenario rows fit the native
            // list viewport, so its arrows remain visibly disabled.
            for (bounds, value, maximum, target) in [
                (
                    SCENARIO_INFO_SCROLL,
                    self.description_first,
                    self.scenario_description_limit(),
                    if self.custom.is_some() {
                        5
                    } else {
                        5 + self.scenario_rows.len()
                    },
                ),
                (
                    SCENARIO_LIST_SCROLL,
                    self.custom.as_ref().map_or(0, |c| c.first),
                    self.custom.as_ref().map_or(0, |_| self.custom_list_limit()),
                    if self.custom.is_some() { 8 } else { usize::MAX },
                ),
            ] {
                ui::Slider {
                    bounds,
                    vertical: true,
                    value: value as f32,
                    range: [0., maximum as f32],
                    pointer: self.pointer_position,
                    pressed: if pressed
                        && target != usize::MAX
                        && selected.is_some_and(|i| i >= target && i <= target + 2)
                    {
                        self.pointer_position
                    } else {
                        None
                    },
                }
                .paint(&mut shapes, &mut slider_controls, &self.generic);
            }
        }
        for (i, part) in if self.character_screen {
            [70, 69]
        } else {
            [26, 25]
        }
        .iter()
        .enumerate()
        {
            let r = b[*part];
            ui::button(
                &mut shapes,
                r,
                if i == 0 { [80, 148, 34] } else { [175, 6, 6] },
                selected == Some(i),
                pressed && selected == Some(i),
                i == 0 && (self.busy || !self.keyboard_enabled(i)),
            )?;
            let mut text = r;
            text[1] += (r[3] - r[1] - 14.) / 2.;
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[if i == 1 {
                    1439
                } else if self.character_screen {
                    1440
                } else {
                    1435
                }],
                text,
                true,
                0xffffffff,
            )?;
        }
        let notice = self.message.as_deref().unwrap_or("");
        wrapped(
            &mut fonts,
            &self.tab,
            &ascii(notice),
            [32., 449., 609., 466.],
            0,
        )?;
        surfaces.push(shapes);
        surfaces.extend(images);
        surfaces.extend(slider_controls);
        surfaces.extend(fonts);
        surfaces.extend(scenario_list_fonts);
        // Original 49b380 submits list text through 430b50, then the
        // per-row completion cups through 432e50.
        surfaces.extend(completion_cups);
        surfaces.extend(title_fonts);
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            [
                2. / 640.,
                0.,
                0.,
                0.,
                0.,
                -2. / 480.,
                0.,
                0.,
                0.,
                0.,
                1.,
                0.,
                -1.,
                1.,
                0.,
                1.,
            ],
            None,
        )
    }
}

#[cfg(test)]
mod preferred_flow_tests {
    use super::*;
    #[test]
    #[ignore = "requires supplied original assets; source-only candidate"]
    fn ordinary_arrows_persist_active_name_and_reopen_isolated() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let installer = openmt2001::fixtures::path("PARAMETERS")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        let language = openmt2001::fixtures::path("EXE")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = std::fs::read(language.join("tycoon.off")).unwrap();
        let glyphs = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let rows = offsets
            .chunks_exact(2)
            .map(|b| {
                let end = at + u16::from_le_bytes(b.try_into().unwrap()) as usize;
                let row = &glyphs[at..end];
                at = end;
                row
            })
            .collect::<Vec<_>>();
        let strings = crate::tooltip::localized_strings(&installer, &rows).unwrap();
        let refs = strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let map = openmt2001::fixtures::path("MAP");
        let scripts = map.parent().unwrap().parent().unwrap();
        let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
        let mut panel = SinglePlayer::new(&index, &archive, &installer, &refs, &exe).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ordinary-token-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let a = crate::campaign_store::profile_path(&root, b"Alice");
        let b = crate::campaign_store::profile_path(&root, b"Bob");
        panel.open(true, "Alice", catalog.clone(), a.clone());
        assert_eq!(panel.character, 0);
        panel.activate(3);
        assert_eq!(panel.character, 1);
        let token_path = crate::menu::preferred_token::path(&root, b"Alice");
        assert_eq!(
            crate::menu::preferred_token::Token::load(&token_path)
                .unwrap()
                .index(),
            1
        );
        // Same typed value consumed by Route::open; model a persisted ordinary-profile choice.
        crate::menu::preferred_token::Token::new(7)
            .unwrap()
            .save(&token_path)
            .unwrap();
        panel.open(false, "Alice", catalog.clone(), a);
        assert_eq!(panel.character, 7);
        panel.open(true, "Bob", catalog, b);
        assert_eq!(panel.character, 0);
        assert_eq!(
            crate::menu::preferred_token::Token::load(&token_path)
                .unwrap()
                .index(),
            7
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod participant_biography_contract_tests {
    use super::*;
    use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};
    fn left(menu: &mut crate::menu::Menu, physical: &mut crate::app::ParticipantLeft, down: bool) {
        let event = WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: if down {
                ElementState::Pressed
            } else {
                ElementState::Released
            },
            button: MouseButton::Left,
        };
        let fresh = physical.sample(&event);
        crate::app::route_menu_left(menu, down, fresh);
    }
    /// Real supplied menu/control dispatch and roster data; synthetic nonzero
    /// first-line is explicitly a FIELD contract, not an original wheel flow.
    #[test]
    #[ignore = "requires original assets and supplied patch Essential Custom scripts; no game window"]
    fn supplied_participant_press_child_priority_identity_launch_and_first_line() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let executable = read("OPENMT2001_EXE");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let exe = openmt2001::fixtures::path("EXE");
        let scripts = openmt2001::fixtures::path("SCRIPTS");
        let parent = openmt2001::fixtures::scratch("BIOGRAPHY_TEST_ROOT");
        assert!(parent.is_dir());
        let root = parent.join(format!(
            "participant-biography-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let assets = crate::menu::Assets::load(
            &index,
            &archive,
            parameter.parent().unwrap().parent().unwrap(),
            &exe.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa"),
            &scripts,
            root.clone(),
            root.join("saves"),
            &executable,
        )
        .unwrap();
        let mut menu = crate::menu::Menu::new(assets);
        let mut physical = crate::app::ParticipantLeft::default();
        // Enter the real named-profile Main -> SinglePlayer -> Custom route.
        // single_mode intentionally survives as background navigation state.
        menu.participant_test_named_profile("Participant");
        let profile = crate::campaign_store::profile_path(&root, b"Participant");
        crate::menu::preferred_token::Token::new(0)
            .unwrap()
            .save(&crate::menu::preferred_token::path(&root, b"Participant"))
            .unwrap();
        menu.select(Some(2), false);
        menu.activate(); // Main SinglePlayer
        assert!(menu.participant_test_single_mode_background());
        menu.select(Some(3), false);
        menu.activate(); // Custom Scenarios
        assert!(menu.single.visible && !menu.single.character_screen);
        // First authored Custom registration from the supplied patch scripts.
        let id: u32 = 500000;
        assert_eq!(id / 100000, 5);
        let chosen = menu
            .single
            .catalog
            .as_ref()
            .unwrap()
            .find(id)
            .expect("supplied Custom level ID");
        let row = menu
            .single
            .custom
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .position(|row| matches!(row, custom::Row::Level(i) if *i == chosen))
            .expect("supplied level must be an actual Custom menu row");
        menu.select(Some(11 + row), false);
        menu.activate(); // actual authored level row
        menu.select(Some(2), false);
        menu.activate(); // Easy
        menu.select(Some(0), false);
        menu.activate(); // Start -> Character + players.lua
        assert!(menu.single.character_screen && menu.single.visible && !menu.single.busy);
        assert!(
            menu.participant_test_single_mode_background(),
            "real Character route retains SinglePlayer background mode; do not clear it to pass"
        );
        let human = menu.single.character;
        let slot = (1..6)
            .find(|&slot| {
                menu.single
                    .participant_character(slot)
                    .is_some_and(|c| c != human)
            })
            .expect("fixture needs distinct active opponent");
        let other = menu.single.participant_character(slot).unwrap();
        let roster_before = menu
            .single
            .custom
            .as_ref()
            .unwrap()
            .roster
            .as_ref()
            .unwrap()
            .records
            .iter()
            .map(|r| (r.kind, r.character))
            .collect::<Vec<_>>();
        let bounds = menu.single.character_panel.bounds[35 + slot];
        let center = |b: [f32; 4]| [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5];
        let point = center(bounds);
        assert!(
            menu.single.hit(point).is_none(),
            "fixture must reach real root fallback"
        );
        menu.pointer(point); // motion never selects biography
        assert_eq!(menu.single.inspected_slot, 0);
        left(&mut menu, &mut physical, true); // actual Menu -> root-equivalent fresh Left DOWN
        assert_eq!(menu.single.inspected_slot, slot);
        assert_eq!(menu.single.biography_character(), other);
        assert_eq!(menu.single.character, human);
        left(&mut menu, &mut physical, false);
        // Controlled first-line field, same-slot press must not reset to0.
        let rows = character_biography_lines(
            &menu.single.scenario_list_tab,
            &menu.single.strings[menu.single.characters[other].description],
            menu.single.character_panel.bounds[68][2] - menu.single.character_panel.bounds[68][0],
        )
        .unwrap()
        .len();
        assert!(rows > 1);
        menu.single.biography_first.set(1);
        left(&mut menu, &mut physical, true);
        left(&mut menu, &mut physical, false);
        assert_eq!(menu.single.biography_first.get(), 1);
        assert_eq!(menu.single.biography_first_for_rows(rows), 1);
        menu.single.biography_first.set(rows);
        assert_eq!(
            menu.single.biography_first_for_rows(rows),
            rows,
            "equality is retained"
        );
        menu.single.biography_first.set(rows + 1);
        assert_eq!(
            menu.single.biography_first_for_rows(rows),
            0,
            "only > resets"
        );
        // Right down and modal consumption cannot manufacture a new left edge.
        menu.single.inspected_slot = 0;
        menu.desktop_right_button(true);
        left(&mut menu, &mut physical, true);
        assert_eq!(menu.single.inspected_slot, 0);
        menu.desktop_right_button(false);
        left(&mut menu, &mut physical, true);
        assert_eq!(
            menu.single.inspected_slot, 0,
            "repeated down is not original4"
        );
        left(&mut menu, &mut physical, false);
        left(&mut menu, &mut physical, true);
        left(&mut menu, &mut physical, false);
        assert_eq!(menu.single.inspected_slot, slot);
        // Exact blocker: press reaches panel, release is physically sampled
        // while App's pending gate filters it, keyboard dismissal, next fresh
        // portrait press succeeds. No direct Menu release is delivered.
        menu.single.inspected_slot = 0;
        menu.pending = Some(0);
        left(&mut menu, &mut physical, true); // swallowed pending press
        left(&mut menu, &mut physical, false); // swallowed pending release
        assert_eq!(menu.single.inspected_slot, 0);
        menu.pending = None; // controlled keyboard dismissal; no MouseInput
        assert!(!physical.sample(&WindowEvent::RedrawRequested));
        left(&mut menu, &mut physical, true);
        assert_eq!(menu.single.inspected_slot, slot);
        menu.pending = Some(0);
        left(&mut menu, &mut physical, false); // release after an admitted press, swallowed
        menu.pending = None;
        menu.single.inspected_slot = 0;
        left(&mut menu, &mut physical, true);
        assert_eq!(
            menu.single.inspected_slot, slot,
            "filtered release must release physical history"
        );
        left(&mut menu, &mut physical, false);
        menu.single.inspected_slot = 0;
        menu.pending = Some(0);
        left(&mut menu, &mut physical, true);
        menu.pending = None;
        assert!(!physical.sample(&WindowEvent::RedrawRequested));
        left(&mut menu, &mut physical, true);
        assert_eq!(
            menu.single.inspected_slot, 0,
            "pending-held press cannot become a new edge"
        );
        physical.sample(&WindowEvent::Focused(false));
        menu.cancel_input();
        left(&mut menu, &mut physical, true);
        assert_eq!(
            menu.single.inspected_slot, slot,
            "focus loss resets physical sample"
        );
        left(&mut menu, &mut physical, false);
        menu.single.inspected_slot = 0;
        // A generic child wins even with deliberately overlapping portrait art.
        let child = menu.single.character_panel.bounds[70]; // Start
        let original = menu.single.character_panel.bounds[35 + slot];
        menu.single.character_panel.bounds[35 + slot] = child;
        let child_point = center(child);
        assert!(menu.single.hit(child_point).is_some());
        menu.pointer(child_point);
        left(&mut menu, &mut physical, true);
        assert_eq!(menu.single.inspected_slot, 0);
        physical.sample(&WindowEvent::Focused(false));
        menu.cancel_input(); // cancel release so this child probe cannot launch
        menu.single.character_panel.bounds[35 + slot] = original;
        // Exact portrait edge rule using actual handler; generic-child-free edges.
        for (p, admitted) in [
            ([bounds[0], point[1]], false),
            ([bounds[2], point[1]], false),
            ([point[0], bounds[1]], true),
            ([point[0], bounds[3]], true),
        ] {
            if menu.single.hit(p).is_some() {
                continue;
            } // child priority is separately proved
            menu.single.inspected_slot = 0;
            menu.pointer(p);
            left(&mut menu, &mut physical, true);
            left(&mut menu, &mut physical, false);
            assert_eq!(menu.single.inspected_slot == slot, admitted);
        }
        // Controlled inactive-slot admission: same supplied geometry/character,
        // kind=-1 is the original inactive sentinel (not a natural script claim).
        let kind = menu
            .single
            .custom
            .as_ref()
            .unwrap()
            .roster
            .as_ref()
            .unwrap()
            .records[slot]
            .kind;
        menu.single
            .custom
            .as_mut()
            .unwrap()
            .roster
            .as_mut()
            .unwrap()
            .records[slot]
            .kind = -1;
        menu.single.inspected_slot = 0;
        menu.pointer(point);
        left(&mut menu, &mut physical, true);
        left(&mut menu, &mut physical, false);
        assert_eq!(menu.single.inspected_slot, 0);
        menu.single
            .custom
            .as_mut()
            .unwrap()
            .roster
            .as_mut()
            .unwrap()
            .records[slot]
            .kind = kind;
        menu.single.busy = true;
        left(&mut menu, &mut physical, true);
        left(&mut menu, &mut physical, false);
        assert_eq!(menu.single.inspected_slot, 0);
        menu.single.busy = false;
        menu.pointer(point);
        left(&mut menu, &mut physical, true);
        left(&mut menu, &mut physical, false);
        // Modern wheel convenience consumes inspected text; no original producer claim.
        menu.single.scroll(1);
        assert_eq!(menu.single.biography_character(), other);
        assert_eq!(
            menu.single
                .custom
                .as_ref()
                .unwrap()
                .roster
                .as_ref()
                .unwrap()
                .records
                .iter()
                .map(|r| (r.kind, r.character))
                .collect::<Vec<_>>(),
            roster_before
        );
        menu.select(Some(0), false);
        menu.activate(); // real Menu launch still uses human identity
        assert_eq!(menu.single.request.as_ref().unwrap().character_index, human);
        assert_eq!(
            menu.single.request.as_ref().unwrap().profile_name,
            b"Participant"
        );
        assert_eq!(menu.single.request.as_ref().unwrap().profile_path, profile);
        menu.select(Some(1), false);
        menu.activate(); // actual Menu Back to Custom
        menu.select(Some(0), false);
        menu.activate(); // reopen ordinary Character
        assert_eq!(menu.single.inspected_slot, 0);
        assert_eq!(menu.single.character, human);
        menu.single.inspected_slot = slot;
        menu.single.biography_first.set(1);
        menu.single.activate(3); // real preference write only in owned test root
        assert_eq!(
            menu.single.inspected_slot, 0,
            "arrows return inspection to human"
        );
        assert_eq!(menu.single.character, (human + 1) % 10);
        assert_eq!(
            menu.single.biography_first.get(),
            1,
            "arrow clear also retains first"
        );
    }
}

#[cfg(test)]
mod campaign_description_tests {
    use super::*;

    // Independent, deliberately unequal synthetic advances expose >= versus >,
    // gap0 versus -1, reserved-strip arithmetic, paint insets and clipping.
    fn synthetic_font() -> Vec<u8> {
        let mut tab = vec![0; 4 + 64 * 44];
        tab[..4].copy_from_slice(&64u32.to_le_bytes());
        for glyph in 0..64 {
            let at = 4 + glyph * 44;
            let width: i32 = match glyph {
                0 => 3,
                2 => 4,
                _ => 5,
            };
            tab[at + 32..at + 36].copy_from_slice(&width.to_le_bytes());
            tab[at + 36..at + 40].copy_from_slice(&6i32.to_le_bytes());
        }
        tab
    }
    fn vertices(pages: &[Surface]) -> Vec<[f32; 8]> {
        pages
            .iter()
            .flat_map(|page| page.vertices.iter().copied())
            .collect()
    }
    fn extent(points: &[[f32; 8]]) -> [f32; 4] {
        assert!(!points.is_empty());
        points.iter().fold(
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ],
            |b, p| {
                [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ]
            },
        )
    }
    #[test]
    fn campaign_description_reserves_strip_insets_and_native_break_boundary() {
        let tab = synthetic_font();
        // Advances 4+2+3 == 9: native >= breaks at the previous space.
        let exact = CampaignDescription::new(&tab, &[1, 0, 2], [0., 0., 31., 24.]).unwrap();
        assert_eq!(exact.offsets, [0, 2, 4]);
        assert_eq!(exact.rows().collect::<Vec<_>>(), [&[1][..], &[2][..]]);
        assert_eq!(exact.child, [0., 0., 15., 24.]);
        assert_eq!(exact.origin, [5., 3.]);
        assert_eq!(exact.visible, 2);
        let wider = CampaignDescription::new(&tab, &[1, 0, 2], [0., 0., 32., 24.]).unwrap();
        assert_eq!(wider.offsets, [0, 4]);
        // Preserve fractional edges until truncation: both budgets truncate9.
        let fractional =
            CampaignDescription::new(&tab, &[1, 0, 2], [0.8, 0.8, 32.2, 24.8]).unwrap();
        assert_eq!(fractional.offsets, exact.offsets);
        assert_eq!(fractional.origin, [5., 3.]);
        assert!(CampaignDescription::new(&tab, &[1], [0., 0., 22., 24.]).is_err());
        assert!(CampaignDescription::new(&tab, &[1], [0., f32::NAN, 31., 24.]).is_err());
        let forced = CampaignDescription::new(&tab, &[1, 28, 46, 2], [0., 0., 100., 24.]).unwrap();
        assert_eq!(forced.offsets, [0, 3, 5]);
        let empty = CampaignDescription::new(&tab, &[], [0., 0., 31., 24.]).unwrap();
        assert_eq!(empty.offsets, [0]);
        assert_eq!(empty.rows().count(), 0);
        assert_eq!(empty.limit(), 0);
    }
    #[test]
    fn campaign_description_paints_same_capacity_and_wheel_track_frontier() {
        let tab = synthetic_font();
        let text: Vec<u8> = (0..21)
            .flat_map(|i| {
                let glyph = if i % 2 == 0 { 1 } else { 2 };
                if i == 20 { vec![glyph] } else { vec![glyph, 0] }
            })
            .collect();
        let layout = CampaignDescription::new(&tab, &text, [0., 0., 31., 24.]).unwrap();
        assert_eq!(layout.rows().count(), 21);
        assert_eq!(layout.visible, 2);
        assert_eq!(layout.limit(), 19);
        assert_eq!(layout.wheel(0, -1), 0);
        assert_eq!(layout.wheel(0, i32::MAX), 19);
        assert_eq!(layout.wheel(19, i32::MIN), 0);
        let track = ui::slider_track(SCENARIO_INFO_SCROLL, true);
        assert_eq!(
            ui::slider_selection(track, track[3], layout.limit() as i32, true),
            19
        );
        let mut pages = vec![Surface::solid(), Surface::solid()];
        layout.paint(&mut pages, &tab, 0).unwrap();
        let p = vertices(&pages);
        assert_eq!(p.len(), 12); // two actual glyph quads, not all21 rows
        assert_eq!(extent(&p[..6]), [4.5, 2.5, 9.5, 8.5]);
        assert_eq!(extent(&p[6..]), [4.5, 14.5, 8.5, 20.5]);
        let mut pages = vec![Surface::solid(), Surface::solid()];
        layout.paint(&mut pages, &tab, 19).unwrap();
        let p = vertices(&pages);
        assert_eq!(p.len(), 12);
        assert_eq!(extent(&p[..6]), [4.5, 2.5, 8.5, 8.5]);
        assert_eq!(extent(&p[6..]), [4.5, 14.5, 9.5, 20.5]);
        // Height exactly divisible by12 retains its original capacity even
        // with the3-unit paint inset; the final row is not silently dropped.
        assert_eq!(
            CampaignDescription::new(&tab, &text, [0., 0., 31., 23.9])
                .unwrap()
                .visible,
            1
        );
    }

    // Original bytes remain external. These Linux-host ignored checks use
    // sha256sum on the bytes actually read, never shell interpolation.
    fn guard(bytes: &[u8], expected: &str) {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("supplied fixture checks require sha256sum");
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .split_whitespace()
                .next(),
            Some(expected)
        );
    }
    fn guarded(path: &Path, expected: &str) -> Vec<u8> {
        let bytes = fs::read(path).unwrap();
        guard(&bytes, expected);
        bytes
    }
    struct Supplied {
        tab: Vec<u8>,
        directory: Vec<u8>,
        archive: Vec<u8>,
        executable: Vec<u8>,
        installer: PathBuf,
        strings: Vec<Vec<u8>>,
    }
    impl Supplied {
        fn read() -> Self {
            let installer = openmt2001::fixtures::path("PARAMETERS")
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf();
            let tab = guarded(
                &installer.join("Standard/fonts/tahoma11noantip.tab"),
                "0bb10874f0ccac54cb6bfba360aecb713c870d9b74325beb36404a0b683f1976",
            );
            // Explicit patched language directory: the old installer row1961
            // is different. Reject it instead of blessing an unrelated string.
            let language = openmt2001::fixtures::path("SCENARIO_LANGUAGE");
            let off = guarded(
                &language.join("tycoon.off"),
                "54a2c8adf44e37138bcaf36bf9c9218e0543880e308a0b9cc9a883ee98265b4d",
            );
            let bin = guarded(
                &language.join("tycoon.bin"),
                "4d5e302035f3579d2c0cb8cd35a58abae5ea6d8159e6c3661bef091fedff6c38",
            );
            assert_eq!(off.len() % 2, 0);
            let mut at = 0;
            let strings = off
                .chunks_exact(2)
                .map(|n| {
                    let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
                    let row = bin.get(at..end).unwrap().to_vec();
                    at = end;
                    row
                })
                .collect::<Vec<_>>();
            assert_eq!(at, bin.len());
            let executable = guarded(
                &openmt2001::fixtures::path("EXE"),
                "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b",
            );
            let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
            let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
            let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            for (name, hash) in [
                (
                    "gamedata2\\brand_new_panels\\SP_Scenario_V2.tm",
                    "3eaa5eb0db10b0cd8ad7439818d7fda5219e25373bf615aea505d024ebf66792",
                ),
                (
                    "gamedata2\\font640\\textures.gtd",
                    "21ed8e328fc95d98eb8cf6f99927d2dc28bd7b3bd309b67bbfbbec52f9be5912",
                ),
                (
                    "gamedata2\\GenericTextures640\\textures.gtd",
                    "10a278bd4ed76076f5b517069f1c8a96ddc7a8b3bb282ff8998842fa8beb88f4",
                ),
            ] {
                let entry = index.find(name).unwrap();
                guard(
                    &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
                    hash,
                );
            }
            Self {
                tab,
                directory,
                archive,
                executable,
                installer,
                strings,
            }
        }
        fn panel(&self) -> SinglePlayer {
            let index = ArchiveIndex::parse(&self.directory, self.archive.len() as u64).unwrap();
            let refs = self.strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let mut panel = SinglePlayer::new(
                &index,
                &self.archive,
                &self.installer,
                &refs,
                &self.executable,
            )
            .unwrap();
            // Supplied-component render only: no catalog/script/profile IO,
            // launch or authored progress claim. Production draw uses real assets.
            panel.visible = true;
            panel.scenario_rows = vec![200003];
            panel.scenario_id = Some(200003);
            panel.difficulty = 1;
            panel
        }
    }
    #[test]
    #[ignore = "requires SHA-pinned original assets, patched language and sha256sum"]
    fn supplied_starting_out_description_offsets_paint_and_scroll() {
        let supplied = Supplied::read();
        let mut panel = supplied.panel();
        let text = &supplied.strings[1961];
        guard(
            text,
            "08cc9190c846393acc2558d0e414be60f05999d82269bd9ac2d020f95bb57cbf",
        );
        assert_eq!(text.len(), 256);
        assert_eq!(panel.scenario_record_text(false), *text);
        assert_eq!(
            level_labels(crate::scenarios::CAMPAIGN[0].id),
            Some((1960, 1961))
        );
        assert_eq!(panel.scenario_list_tab, supplied.tab);
        let image = PeImage::parse(&supplied.executable).unwrap();
        assert_eq!(image.at(0x49979b, 5).unwrap(), &[0xbb, 2, 0, 0, 0]);
        assert_eq!(
            image.at(0x4997d3, 5).unwrap(),
            &[0xe8, 0x98, 0x73, 0xfa, 0xff]
        );
        assert_eq!(
            image.at(0x54cac4, 31).unwrap(),
            b"gamedata\\fonts\\tahoma11noantip\0"
        );
        let original_bounds = panel.scenario.bounds[24];
        assert_eq!(original_bounds, [334.33594, 201.953, 604.44995, 396.0419]);
        let index =
            ArchiveIndex::parse(&supplied.directory, supplied.archive.len() as u64).unwrap();
        let payload = |name| {
            let e = index.find(name).unwrap();
            &supplied.archive[e.offset as usize..(e.offset + e.length) as usize]
        };
        let font = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")).unwrap();
        assert_eq!(panel.scenario_list_fonts.len(), 2);
        for (actual, original) in panel.scenario_list_fonts.iter().zip(&font.textures[4..6]) {
            let expected = Surface::new(original);
            assert_eq!(
                (actual.width, actual.height, &actual.pixels),
                (expected.width, expected.height, &expected.pixels)
            );
        }
        let generic =
            TextureSet::parse(payload("gamedata2\\GenericTextures640\\textures.gtd")).unwrap();
        assert_eq!(generic.textures[105].width, 16);
        assert_eq!(SCENARIO_INFO_SCROLL[2] - SCENARIO_INFO_SCROLL[0], 16.);
        let layout = panel.campaign_description().unwrap();
        assert_eq!(layout.offsets, [0, 31, 67, 108, 148, 181, 217, 257]);
        assert_eq!(
            layout.rows().map(<[u8]>::len).collect::<Vec<_>>(),
            [30, 35, 40, 39, 32, 35, 39]
        );
        assert_eq!(layout.origin, [339., 204.]);
        assert_eq!(layout.visible, 16);
        assert_eq!(layout.limit(), 0);
        assert_eq!(panel.scenario_description_limit(), 0);
        assert_eq!(super::scenario_scroll_hit(0, 1, [596., 210.]), None);
        let mut pages = panel.scenario_list_fonts.clone();
        layout.paint(&mut pages, &supplied.tab, 0).unwrap();
        let actual = vertices(&pages);
        assert_eq!(actual.len(), 1248);
        assert_eq!(extent(&actual), [338.5, 205.5, 585.5, 290.5]);
        let bounds = extent(&actual);
        assert!(bounds[0] >= 338.5 && bounds[2] <= layout.child[2]);
        assert!(bounds[1] >= 203.5 && bounds[3] <= layout.child[3]);
        // Compare the first row against the real production ui::text renderer,
        // independent of the layout helper's row iterator/origin calculation.
        let mut first = panel.scenario_list_fonts.clone();
        ui::text(
            &mut first,
            &supplied.tab,
            &text[..30],
            [339., 204., 588.44995, 396.0419],
            false,
            0xffffffff,
        )
        .unwrap();
        assert_eq!(vertices(&first).len(), 144);
        assert_eq!(extent(&vertices(&first)), [338.5, 205.5, 544.5, 218.5]);
        for (actual, expected) in pages.iter().zip(&first) {
            assert_eq!(
                &actual.vertices[..expected.vertices.len()],
                expected.vertices.as_slice()
            );
        }
        panel.scroll(i32::MAX);
        assert_eq!(panel.description_first, 0);
        // Full-height supplied descriptions naturally fit. Explicitly shrink
        // only this test component to24px: real rows, synthetic overflow viewport.
        panel.scenario.bounds[24][3] = original_bounds[1] + 24.;
        assert_eq!(panel.scenario_description_limit(), 5);
        panel.scroll(i32::MAX);
        assert_eq!(panel.description_first, 5);
        let tail = panel.campaign_description().unwrap();
        assert_eq!(
            tail.rows().skip(5).map(<[u8]>::len).collect::<Vec<_>>(),
            [35, 39]
        );
        let mut pages = panel.scenario_list_fonts.clone();
        tail.paint(&mut pages, &supplied.tab, 5).unwrap();
        assert_eq!(vertices(&pages).len(), 378);
        assert_eq!(
            extent(&vertices(&pages)),
            [338.5, 205.5, 576.5, tail.child[3] - 0.5]
        );
        panel.scroll(i32::MIN);
        assert_eq!(panel.description_first, 0);
        assert!(panel.drag_track(Some(8), [596., 377.])); // one row: base6+track2
        assert_eq!(panel.description_first, 5);
        panel.scenario.bounds[24] = original_bounds;
        panel.scroll(0);
        assert_eq!(panel.description_first, 0);
        assert!(panel.request.is_none() && panel.profile_path.is_none() && panel.catalog.is_none());
        assert_eq!(panel.scenario_rows, [200003]);
        assert_eq!(panel.difficulty, 1);
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires SHA-pinned supplied assets, offscreen GPU and output directory"]
    fn capture_starting_out_description_component() {
        let supplied = Supplied::read();
        let mut panel = supplied.panel();
        let output = PathBuf::from(std::env::var("OPENMT2001_DESCRIPTION_CAPTURE_DIR").unwrap());
        assert!(output.is_dir());
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let prepared = panel.draw(&renderer, None, false).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &prepared,
            output
                .join("starting-out-medium-description.png")
                .to_str()
                .unwrap(),
        );
        panel.scenario.bounds[24][3] = panel.scenario.bounds[24][1] + 24.;
        panel.scroll(i32::MAX);
        assert_eq!(panel.description_first, 5);
        let prepared = panel.draw(&renderer, None, false).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &prepared,
            output
                .join("starting-out-synthetic-viewport-tail.png")
                .to_str()
                .unwrap(),
        );
        assert!(panel.request.is_none() && panel.profile_path.is_none());
    }
}
