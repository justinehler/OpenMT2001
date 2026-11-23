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
use std::{fs, path::{Path, PathBuf},
};

#[path = "singleplayer/custom.rs"]
mod custom;

const TUTORIAL_TITLE_COLORS: [u32; 4] =
    [0xffffb400, 0xffffb400, 0x80ffb400, 0x80ffb400];
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
        let fixture = std::fs::read(std::env::var("OPENMT2001_NATIVE_TUTORIAL_TITLE")
            .expect("set OPENMT2001_NATIVE_TUTORIAL_TITLE"),
        ).unwrap();
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
        let directory = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
        let archive = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let (_, regions) = ui::panel_base(&index, &archive,
            "gamedata2\\brand_new_panels\\tutorial_panel_v2.tm", &[part],
        ).unwrap();
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
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let executable = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let installer = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap().parent().unwrap().to_path_buf();
        let language = executable.parent().unwrap().parent().unwrap().join("US/language/usa");
        let offsets = std::fs::read(language.join("tycoon.off")).unwrap();
        let glyphs = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0usize;
        let rows = offsets.chunks_exact(2).map(|length| {
            let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let row = &glyphs[at..end];
            at = end;
            row
        }).collect::<Vec<_>>();
        let strings = crate::tooltip::localized_strings(&installer, &rows).unwrap();
        let refs = strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let exe = read("OPENMT2001_EXE");
        let mut panel = SinglePlayer::new(&index, &archive, &installer, &refs, &exe).unwrap();
        // Open the original registration catalog with a fresh V2 profile.
        // This exercises the same progression filter as the visible menu.
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let map_path = std::path::PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
        let scripts = map_path.parent().unwrap().parent().unwrap();
        let mut catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
        let fresh = std::path::PathBuf::from("work/frontend/capture-fresh-profile-missing.lua");
        assert!(!fresh.exists());
        crate::campaign_store::load_progress(&mut catalog, &fresh).unwrap();
        assert_eq!(super::scenario_rows_for(&catalog).len(), 1);
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets {
                executable: &exe, directory: &directory, archive: &archive,
            }),
            suncols: Some(&suncols), phase: Some(12),
            camera: Some(CameraSample {
                eye: [26., 586., -695.], target: [26., 463., -595.],
                up: [0., 1., 0.], vertical_fov: 45.,
            }),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let overlay = panel.draw_tutorial(&renderer, None, false).unwrap();
        crate::capture_test::capture_scene_overlays(&renderer, &scene, &[&overlay],
            "work/frontend/tutorial-caption-city.png",
        );
        panel.open(false, "Capture", catalog, fresh);
        let scenario = panel.draw(&renderer, None, false).unwrap();
        crate::capture_test::capture_scene_overlays(&renderer, &scene, &[&scenario],
            "work/frontend/scenario-scrollbars-city.png",
        );
        panel.activate(0); // the unlocked Starting Out row opens Character Selection
        assert!(panel.character_screen);
        panel.activate(3);
        panel.activate(3); // right arrow reaches Dog from the new-profile Racecar
        assert_eq!(panel.characters[panel.character].name, 1746);
        let dog = panel.draw(&renderer, None, false).unwrap();
        crate::capture_test::capture_scene_overlays(&renderer, &scene, &[&dog],
            "work/frontend/character-dog-biography-city.png",
        );
    }
}
#[cfg(test)]
mod scenario_scroll_tests {
    use super::{SCENARIO_INFO_SCROLL, SCENARIO_LIST_SCROLL, scenario_rows_for, scenario_scroll_hit, ui,
    };

    #[test]
    #[ignore = "requires original v1.4 Scenario completion-cup fixture"]
    fn scenario_completion_cups_match_original_painter() {
        let fixture = std::fs::read("work/frontend/native-scenario-cups.bin").unwrap();
        assert_eq!(&fixture[..8], b"MTSCCP01");
        assert_eq!(fixture.len(), 8 + 32 + 12 * 4);
        let values: Vec<u32> = fixture[40..].chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap())).collect();
        assert_eq!(&values[..7], &[242, 20, 98, 12, 17, 14, 0x874]);
        assert_eq!(&values[7..11], &[293, 294, 295, super::SCENARIO_CUP_EMPTY as u32]);
        assert_eq!(values[11], 2);
        for row in [0, 1, 5] {
            for cup in 0..3 {
                let left = values[0] + cup * values[1];
                let top = values[2] + row * values[3];
                assert_eq!(super::scenario_cup_rect(row as usize, cup as usize),
                    [left as f32, top as f32, (left + values[4]) as f32,
                        (top + values[5]) as f32]);
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
            bytes[0x44c..0x450].copy_from_slice(&id.to_le_bytes());
            bytes[0x86c] = u8::from(unlocked);
            bytes[0x85c] = u8::from(tutorial);
            bytes[2] = u8::from(custom);
            catalog.levels.push(Level { bytes });
        }
        // Original 49b1da/49b1e4/49b1eb rejects tutorial, locked and
        // custom records before appending a native list row.
        assert_eq!(scenario_rows_for(&catalog), vec![0]);
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
        let fixture = std::fs::read("work/frontend/native-scenario-scrollbars.bin").unwrap();
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
            assert_eq!(args, [total[0] as i32, total[1] as i32,
                (total[3] - total[1]) as i32, 1]);
            let outer: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word()));
            let track: [f32; 5] = std::array::from_fn(|_| f32::from_bits(word()));
            assert_eq!(outer, ui::slider_track(total, true));
            assert_eq!(track[..4], outer);
            assert_eq!(track[4], 0.5);
            for (expected, arrow) in [105, 93].into_iter()
                .zip(ui::slider_arrows(total, true)) {
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
        let fixture = std::fs::read("work/frontend/native-character-biography.bin").unwrap();
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

        let installer = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap().parent().unwrap().to_path_buf();
        let font = std::fs::read(installer.join("Standard/fonts/tahoma11noantip.tab")).unwrap();
        let lines = character_biography_lines(&font, dog, bounds[2] - bounds[0]).unwrap();
        assert_eq!(lines.len(), 11);
        assert_eq!(lines[0], ascii("Dog gets her name from her favorite pet. She will"));
        assert_eq!(lines[10], ascii("shows true compassion for."));
        assert!(lines.len() as f32 * line_height as f32 <= bounds[3] - bounds[1]);

        let directory = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
        let archive = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let (_, regions) = ui::panel_base(&index, &archive,
            "gamedata2\\brand_new_panels\\sp_token_V2.tm", &[68],
        ).unwrap();
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
    scenario_index: usize,
    scenario_rows: Vec<usize>,
    tutorial_index: usize,
    character: usize,
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
    if maximum == 0 { return None; }
    let inside = |[l, t, r, b]: [f32; 4]| point[0] >= l && point[0] <= r && point[1] >= t && point[1] <= b;
    let base = 5 + rows;
    if let Some(side) = ui::slider_arrows(SCENARIO_INFO_SCROLL, true)
        .into_iter().position(inside) {
        return Some(base + side);
    }
    inside(ui::slider_track(SCENARIO_INFO_SCROLL, true)).then_some(base + 2)
}
fn scenario_rows_for(catalog: &openmt2001::level_catalog::Catalog) -> Vec<usize> {
    catalog.levels.iter().filter(|level| !level.tutorial()
        && level.unlocked() && level.bytes[2] == 0)
        .filter_map(|level| {
            crate::scenarios::CAMPAIGN.iter()
            .position(|scenario| scenario.id == level.id())
        })
        .collect()
}
fn scenario_cup_rect(row: usize, cup: usize) -> [f32; 4] {
    let left = 242. + 20. * cup as f32;
    let top = 98. + SCENARIO_ROW_HEIGHT * row as f32;
    [left, top, left + 17., top + 14.]
}
fn character_biography_lines(tab: &[u8], encoded: &[u8], width: f32,
)
    -> Result<Vec<Vec<u8>>, String> {
    let (offsets, wrapped) = crate::prompt::wrap_with_gap(tab, encoded,
        width.trunc() as i32, -1)?;
    Ok(offsets.windows(2).map(|range| {
        let end = range[1].saturating_sub(1).min(wrapped.len());
        wrapped[range[0].min(end)..end].to_vec()
    }).collect())
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
            scenario_index: 0,
            scenario_rows: vec![],
            tutorial_index: 0,
            character: 0,
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
    pub fn open(&mut self, sandbox: bool, name: &str,
        catalog: openmt2001::level_catalog::Catalog, profile_path: PathBuf,
    ) {
        self.custom = None;
        self.visible = true;
        self.tutorial_screen = false;
        self.sandbox = sandbox;
        self.character_screen = sandbox;
        self.difficulty = 0;
        // Fail closed before loading another profile's selection/catalog.
        self.request = None;
        self.busy = true;
        self.catalog = None;
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
        self.scenario_index = self.scenario_rows.first().copied().unwrap_or(0);
        self.catalog = Some(catalog);
        self.profile_path = Some(profile_path);
        self.request = None;
        self.busy = false;
        self.message = None;
        self.description_first = 0;
        self.pointer_position = None;
    }
    pub fn open_tutorial(&mut self, name: &str,
        catalog: openmt2001::level_catalog::Catalog, profile_path: PathBuf,
    ) {
        self.custom = None;
        self.visible = true;
        self.tutorial_screen = true;
        self.character_screen = false;
        self.sandbox = false;
        self.difficulty = 0;
        self.character = 0;
        self.name = name.as_bytes().to_vec();
        self.tutorial_index = 0;
        self.catalog = Some(catalog);
        self.profile_path = Some(profile_path);
        self.request = None;
        self.busy = false;
        self.message = None;
        self.description_first = 0;
        self.pointer_position = None;
    }
    pub fn reload_progress(&mut self) -> Result<(), String> {
        let Some(path) = &self.profile_path else { return Ok(()); };
        let catalog = self.catalog.as_mut().ok_or("Campaign catalog missing")?;
        crate::campaign_store::load_progress(catalog, path)?;
        if self.custom.is_none() {
            self.scenario_rows = scenario_rows_for(catalog);
            if !self.scenario_rows.contains(&self.scenario_index) {
                self.scenario_index = self.scenario_rows.first().copied().unwrap_or(0);
            }
        }
        self.message = None;
        Ok(())
    }
    pub fn count(&self) -> usize {
        if self.custom.is_some() && !self.character_screen { return self.custom_count(); }
        if self.tutorial_screen {
            2 + crate::scenarios::TUTORIALS.len()
        } else if self.character_screen {
            4
        } else {
            5 + self.scenario_rows.len()
                + if self.scenario_description_limit() > 0 { 3 } else { 0 }
        }
    }
    /// The Scenario and Tutorial bottom children and the Character bottom
    /// children/arrows are original 43ed20 widgets with 443360 press audio.
    /// Scenario difficulty radios and both list families use other widgets.
    pub fn generic_click_enabled(&self, i: usize) -> bool {
        if !self.keyboard_enabled(i) { return false; }
        native_generic_click(self.visible, self.busy, self.character_screen, i)
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        self.pointer_position = Some(p);
    }
    pub fn is_scroll_track(&self, origin: Option<usize>) -> bool {
        if self.custom.is_some() { return !self.character_screen && matches!(origin, Some(7 | 10)); }
        self.visible && !self.tutorial_screen && !self.character_screen
            && self.scenario_description_limit() > 0
            && origin == Some(7 + self.scenario_rows.len())
    }
    pub fn drag_track(&mut self, origin: Option<usize>, p: [f32; 2]) -> bool {
        if self.custom.is_some() && self.is_scroll_track(origin) { self.pointer_position = Some(p); self.custom_scroll_control(7, origin == Some(10)); return true; }
        if !self.is_scroll_track(origin) { return false; }
        let next = ui::slider_selection(ui::slider_track(SCENARIO_INFO_SCROLL, true),
            p[1], self.scenario_description_limit() as i32, true,
        ) as usize;
        let changed = next != self.description_first;
        self.description_first = next;
        changed
    }
    fn scenario_description_limit(&self) -> usize {
        let text = self.scenario_record_text(false);
        let b = self.scenario.bounds[24];
        let visible = ((b[3] - b[1]) / SCENARIO_ROW_HEIGHT) as usize;
        wrap_lines(&self.scenario_list_tab, &text, b[2] - b[0])
            .map(|lines| lines.len().saturating_sub(visible))
            .unwrap_or(0)
    }
    fn scenario_record_text(&self, title: bool) -> Vec<u8> {
        if self.custom.is_some() { return self.custom_text(title); }
        self.scenario_record_text_at(self.scenario_index, title)
    }
    fn scenario_record_text_at(&self, scenario_index: usize, title: bool) -> Vec<u8> {
        let id = crate::scenarios::CAMPAIGN[scenario_index].id;
        let fallback = level_labels(id).unwrap();
        let fallback = self.strings[if title { fallback.0 } else { fallback.1 }].clone();
        let Some(level) = self.catalog.as_ref().and_then(|c| c.find(id).map(|i| &c.levels[i]))
            else { return fallback; };
        match level.labels() {
            openmt2001::level_catalog::Labels::Localized { title: name, description,
            } => self.strings.get((if title { name } else { description }) as usize)
                    .cloned().unwrap_or(fallback),
            openmt2001::level_catalog::Labels::Literal { title: name, description,
            } => (if title { name } else { description }).into_iter().map(|byte| {
                    if (32..=126).contains(&byte) { byte - 32 } else { b'?' - 32 }
                }).collect(),
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
        if self.custom.is_some() { self.custom_roster()?; }
        Ok(())
    }
    pub fn activate(&mut self, i: usize) {
        if self.busy && i != 1 {
            return;
        }
        if self.custom.is_some() && !self.character_screen { self.custom_activate(i); return; }
        if self.tutorial_screen {
            match i {
                0 => {
                    let tutorial = crate::scenarios::TUTORIALS[self.tutorial_index];
                    if !crate::scenarios::live_tutorial(tutorial.id) {
                        self.message = Some("This tutorial's live session is not yet available.".into());
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
                    self.tutorial_index = i - 2;
                    self.description_first = 0;
                    self.message = None;
                }
                _ => (),
            }
        } else if self.character_screen {
            match i {
                0 => {
                    if self.custom.is_some() { if let Err(e) = self.custom_roster() { self.message = Some(e); return; } }
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
                            Preview::Scenario(c.selected.and_then(|i| self.catalog.as_ref()?.levels.get(i)).map(|l| l.id()).unwrap())
                        } else if self.sandbox {
                            Preview::Sandbox
                        } else {
                            Preview::Scenario(crate::scenarios::CAMPAIGN[self.scenario_index].id)
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
                    let id = crate::scenarios::CAMPAIGN[self.scenario_index].id;
                    let unlocked = self.catalog.as_ref().and_then(|c| c.find(id).map(|i| c.levels[i].unlocked())).unwrap_or(false);
                    if !unlocked {
                        self.message = Some("This scenario is locked in the selected profile.".into());
                        return;
                    }
                    if !(200001..=200016).contains(&id) && id != 400001 {
                        self.message = Some("Scenario initialization is not yet available.".into());
                        return;
                    }
                    self.character_screen = true;
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
                    self.scenario_index = self.scenario_rows[i - 5];
                    self.description_first = 0;
                    self.message = None;
                }
                _ => (),
            }
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.custom.is_some() && !self.character_screen && self.pointer_position.is_some_and(|p| p[0] < 320.) {
            let max = self.custom_list_limit(); let c = self.custom.as_mut().unwrap(); c.first = (c.first as i32 + delta).clamp(0,max as i32) as usize; return;
        }
        let scenario_text = if !self.tutorial_screen && !self.character_screen {
            self.scenario_record_text(false)
        } else { vec![] };
        let (text, b) = if self.tutorial_screen {
            (&self.strings[level_labels(crate::scenarios::TUTORIALS[self.tutorial_index].id)
                .unwrap().1], self.tutorial_panel.bounds[11],
            )
        } else if self.character_screen {
            (
                &self.strings[self.characters[self.character].description],
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
            self.description_first = (self.description_first as i32 + delta)
                .clamp(0, lines.len().saturating_sub(visible) as i32)
                as usize;
        }
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        if self.custom.is_some() && !self.character_screen { return self.custom_hit(p); }
        let inside = |[l, t, r, b]: [f32; 4]| p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b;
        if self.tutorial_screen {
            for (i, part) in [13, 12].iter().enumerate() {
                if inside(self.tutorial_panel.bounds[*part]) { return Some(i); }
            }
            if inside(self.tutorial_panel.bounds[8]) {
                let row = ((p[1] - self.tutorial_panel.bounds[8][1]) / 13.) as usize;
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
                self.scenario_description_limit(), self.scenario_rows.len(), p,
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
    fn draw_tutorial(&self, renderer: &Renderer, selected: Option<usize>,
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
        // 49c3b6/49c4a1 construct font4 and font1 style9 caption widgets.
        ui::text_widget_style9(&mut title_fonts, &self.title_tab,
            &self.strings[1667], b[6], 0xffffffff,
        )?;
        ui::text_widget_style9(&mut fonts, &self.tab,
            &self.strings[1668], b[9], 0xffffffff)?;
        let chosen = crate::scenarios::TUTORIALS[self.tutorial_index];
        let (title, description) = level_labels(chosen.id).unwrap();
        ui::text(&mut fonts, &self.tab, &self.strings[title],
            b[10], true, 0xffffffff,
        )?;
        let [l, t, r, _] = b[8];
        for (row, tutorial) in crate::scenarios::TUTORIALS.iter().enumerate() {
            let top = t + row as f32 * 13.;
            if row == self.tutorial_index || selected == Some(2 + row) {
                shapes.rect([l, top, r, top + 13.],
                    [if row == self.tutorial_index { 0xff008ed0 } else { 0x99505050 }; 4],
                );
            }
            ui::text(&mut fonts, &self.tab,
                &self.strings[level_labels(tutorial.id).unwrap().0],
                [l + 3., top, r, top + 13.], false, 0xffffffff,
            )?;
        }
        wrapped(&mut fonts, &self.tab, &self.strings[description],
            b[11], self.description_first,
        )?;
        for (i, part, label, color) in
            [(0, 13, 1435, [80, 148, 34]), (1, 12, 1439, [175, 6, 6])] {
            ui::button(&mut shapes, b[part], color, selected == Some(i),
                pressed && selected == Some(i), self.busy && i == 0,
            )?;
            ui::text(&mut fonts, &self.tab, &self.strings[label],
                b[part], true, 0xffffffff,
            )?;
        }
        wrapped(&mut fonts, &self.tab,
            &ascii(self.message.as_deref().unwrap_or("")),
            [32., 449., 609., 466.], 0,
        )?;
        surfaces.push(shapes);
        surfaces.extend(fonts);
        surfaces.extend(title_fonts);
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            [2. / 640., 0., 0., 0., 0., -2. / 480., 0., 0.,
                0., 0., 1., 0., -1., 1., 0., 1.,
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
            &self.strings[if self.character_screen { 0x756 } else if self.custom.is_some() {2240} else { 0x755 }],
            title,
            true,
            0xffffffff,
        )?;
        let level = if self.sandbox {
            400002
        } else {
            crate::scenarios::CAMPAIGN[self.scenario_index].id
        };
        let (level_title, _) = level_labels(level).unwrap();
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
                        self.custom.as_ref().and_then(|c| c.roster.as_ref()).and_then(|r| { let record = r.records[slot]; (record.kind != -1).then(|| self.strings[self.characters[record.character as usize].name].as_slice()) }).unwrap_or(&self.strings[0x6db])
                    },
                    r,
                    false,
                    0xffffffff,
                )?;
                let mut tint = b[41 + slot];
                let authored = self.custom.as_ref().and_then(|c| c.roster.as_ref()).and_then(|r| { let v=r.records[slot]; (v.kind != -1).then_some(v.character as usize) });
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
                let difficulty_icon = self.custom.as_ref().and_then(|c| c.roster.as_ref()).map(|r| {
                    let record = r.records[slot];
                    if record.kind == 0 {259} else if record.kind == 1 {258} else {257}
                }).unwrap_or(if slot == 0 {259} else {257});
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
            let c = &self.characters[self.character];
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
            for (line, text) in character_biography_lines(&self.scenario_list_tab,
                &self.strings[c.description], biography[2] - biography[0],
            )?
                .iter().skip(self.description_first).enumerate() {
                let top = biography[1].trunc() + line as f32 * 10.;
                if top + 10. > biography[3] { break; }
                ui::text_widget_style0(&mut scenario_list_fonts,
                    &self.scenario_list_tab, text,
                    [biography[0], top, biography[2], biography[3]], 0xffffffff,
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
                &if self.custom.is_some() { self.custom_text(true) } else {self.strings[level_title].clone()},
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
            for (part, label) in [(9, self.strings[0x66d].as_slice()),
                (10, selected_title.as_slice()),
            ] {
                shapes.rect(b[part], [0xff909090, 0xff909090, 0x80909090, 0x80909090]);
                let mut r = b[part];
                r[1] += 4.;
                ui::text(
                    &mut fonts,
                    &self.tab,
                    label,
                    r,
                    true,
                    0xffffffff)?;
            }
            let [l, t, r, _] = b[8];
            if self.custom.is_some() {
                self.custom_list_draw(&mut shapes, &mut scenario_list_fonts)?;
                self.custom_cups_draw(&mut completion_cups)?;
            }
            for (row, &scenario_index) in self.scenario_rows.iter().enumerate().filter(|_| self.custom.is_none()) {
                let top = t + row as f32 * SCENARIO_ROW_HEIGHT;
                if self.scenario_index == scenario_index || selected == Some(5 + row) {
                    shapes.rect(
                        [l, top, r, top + SCENARIO_ROW_HEIGHT],
                        [if self.scenario_index == scenario_index {
                            0xff008ed0
                        } else {
                            0x99505050
                        }; 4],
                    );
                }
                ui::text(
                    &mut scenario_list_fonts,
                    &self.scenario_list_tab,
                    &self.scenario_record_text_at(scenario_index, true),
                    [l + 3., top, r, top + SCENARIO_ROW_HEIGHT],
                    false,
                    0xffffffff,
                )?;
                let id = crate::scenarios::CAMPAIGN[scenario_index].id;
                let completed = self.catalog.as_ref().and_then(|catalog| {
                        catalog.find(id)
                    .map(|index| catalog.levels[index].completed())
                    }).unwrap_or([false; 3]);
                for (cup, earned) in completed.into_iter().enumerate() {
                    self.image(&mut completion_cups,
                        if earned { 293 + cup } else { SCENARIO_CUP_EMPTY },
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
            for (line, text) in wrap_lines(&self.scenario_list_tab,
                &selected_description, b[24][2] - b[24][0],
            )?
                .iter().skip(self.description_first).enumerate() {
                let top = b[24][1] + line as f32 * SCENARIO_ROW_HEIGHT;
                if top + SCENARIO_ROW_HEIGHT > b[24][3] { break; }
                ui::text(&mut scenario_list_fonts, &self.scenario_list_tab, text,
                    [b[24][0], top, b[24][2], b[24][3]], false, 0xffffffff,
                )?;
            }
            // 499817/49994e attach original 43f510 children beside the
            // description and list. The 17 scenario rows fit the native
            // list viewport, so its arrows remain visibly disabled.
            for (bounds, value, maximum, target) in [
                (SCENARIO_INFO_SCROLL, self.description_first,
                    self.scenario_description_limit(),
                    if self.custom.is_some() {5} else {5 + self.scenario_rows.len()},
                ),
                (SCENARIO_LIST_SCROLL, self.custom.as_ref().map_or(0, |c| c.first), self.custom.as_ref().map_or(0, |_| self.custom_list_limit()), if self.custom.is_some() {8} else {usize::MAX}),
            ] {
                ui::Slider {
                    bounds,
                    vertical: true,
                    value: value as f32,
                    range: [0., maximum as f32],
                    pointer: self.pointer_position,
                    pressed: if pressed && target != usize::MAX
                        && selected.is_some_and(|i| i >= target && i <= target + 2) {
                        self.pointer_position
                    } else { None },
                }.paint(&mut shapes, &mut slider_controls, &self.generic);
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
                i == 0 && (self.busy || self.custom.as_ref().is_some_and(|c| c.selected.is_none())),
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
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let installer = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        let language = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
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
        let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
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
