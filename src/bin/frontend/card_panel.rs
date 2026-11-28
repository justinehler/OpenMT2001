//! Original41b800 event-card presentation, independent of the retained card lifecycle.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

// The card button and panel state are library code (#99).
pub use openmt2001::card_panel::{Entry, EntryVisual, Panel};
#[cfg(test)]
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
#[derive(Default)]
pub struct Input {
    pressed: Option<u32>,
    entry_pressed: bool,
}
impl Input {
    pub fn captured(&self) -> bool {
        self.pressed.is_some() || self.entry_pressed
    }
    pub fn mouse(
        &mut self,
        assets: &Assets,
        panel: Option<Panel>,
        entry: Entry,
        pointer: Option<[f32; 2]>,
        down: bool,
    ) -> Option<openmt2001::session::cards::Action> {
        if let Some(panel) = panel {
            let hovered = (0..4)
                .find(|&i| panel.buttons[i] && pointer.is_some_and(|p| hit(assets.buttons[i], p)))
                .map(|i| i as u32);
            if down {
                self.pressed = hovered;
                None
            } else {
                let prior = self.pressed.take();
                (prior.is_some() && prior == hovered).then_some(
                    openmt2001::session::cards::Action::Button {
                        event: 12,
                        button: prior.unwrap_or(0),
                    },
                )
            }
        } else {
            let inside = pointer.is_some_and(|p| hit(assets.entry, p));
            if down {
                self.entry_pressed = inside && entry.can_open();
                None
            } else {
                let activate =
                    std::mem::take(&mut self.entry_pressed) && inside && entry.can_open();
                activate.then_some(openmt2001::session::cards::Action::Open)
            }
        }
    }
}
fn hit(b: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3]
}
const PROJECTION: [f32; 16] = [
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
];
pub struct Assets {
    main: [f32; 4],
    base: Vec<Surface>,
    textures: Vec<Surface>,
    fonts: [Vec<Surface>; 2],
    tabs: [Vec<u8>; 2],
    strings: Vec<Vec<u8>>,
    title: [f32; 4],
    body: [f32; 4],
    portrait: [f32; 4],
    pub entry: [f32; 4],
    pub buttons: [[f32; 4]; 4],
}
impl Assets {
    pub(crate) fn apply_board(
        &mut self,
        board: &openmt2001::packed_language::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("Card asset {name} missing"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("Card asset bounds".into())
        };
        let (mut base, regions) = ui::panel_base(
            index,
            archive,
            "gamedata2/brand_new_panels/random_card_panel_v2.tm",
            &[0, 1, 2],
        )?;
        let mut fills = Surface::solid();
        fills.rect(regions[0], [0xbb000a28; 4]);
        fills.rect(regions[2], [0xff808080, 0xff808080, 0x80808080, 0x80808080]);
        base.push(fills);
        // The four CARD_*_TINT names are absent in this supplied v1.4 mesh;
        // original43af30 returns its zero rectangle for these names.
        let mesh = openmt2001::mesh::CachedMesh::parse(payload(
            "gamedata2/brand_new_panels/top_right_panel_v2.tm",
        )?)?;
        let entry = openmt2001::ui_mesh::regions(&mesh, 0)?
            .into_iter()
            .find(|r| r.name == "TIME_CHANCE_ICON")
            .ok_or("Card entry region missing")?
            .bounds;
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let font = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let read =
            |n: &str| fs::read(installer.join("Standard/fonts").join(n)).map_err(|e| e.to_string());
        Ok(Self {
            main: regions[0],
            base,
            textures: generic.textures.iter().map(Surface::new).collect(),
            fonts: [
                font.textures[..2].iter().map(Surface::new).collect(),
                font.textures[6..8].iter().map(Surface::new).collect(),
            ],
            tabs: [read("cust640p.tab")?, read("tahoma10bldnoantip.tab")?],
            strings: crate::tooltip::localized_strings(installer, strings)?,
            title: regions[2],
            body: regions[3],
            portrait: regions[4],
            entry,
            buttons: [regions[6], regions[5], regions[6], regions[7]],
        })
    }
    pub fn contains(&self, p: Option<[f32; 2]>) -> bool {
        p.is_some_and(|p| hit(self.main, p))
    }
    fn texture(&self, id: usize, bounds: [f32; 4]) -> Result<Surface, String> {
        let mut s = self.textures.get(id).ok_or("Card texture index")?.clone();
        let [l, t, r, b] = bounds;
        s.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        Ok(s)
    }
    fn text(
        &self,
        surfaces: &mut Vec<Surface>,
        font: usize,
        text: &[u8],
        bounds: [f32; 4],
    ) -> Result<(), String> {
        let (offsets, copy) = openmt2001::prompt_queue::wrap(
            &self.tabs[font],
            text,
            (bounds[2] - bounds[0] - 8.).trunc() as i32,
        )?;
        let height = if font == 0 { 8. } else { 10. };
        let count = offsets.len().saturating_sub(1);
        let top = (bounds[1] + (bounds[3] - bounds[1] - count as f32 * height) * 0.5).trunc();
        let mut pages = self.fonts[font].clone();
        for (i, pair) in offsets.windows(2).enumerate() {
            let end = (pair[1] - 1).min(copy.len());
            let start = pair[0].min(end);
            ui::text(
                &mut pages,
                &self.tabs[font],
                &copy[start..end],
                [
                    bounds[0] + 4.,
                    top + i as f32 * height,
                    bounds[2] - 4.,
                    bounds[3],
                ],
                true,
                0xffffffff,
            )?;
        }
        surfaces.extend(pages);
        Ok(())
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        panel: Panel,
        input: &Input,
        pointer: Option<[f32; 2]>,
    ) -> Result<PreparedScene, String> {
        let mut surfaces = self.base.clone();
        surfaces.push(self.texture(300, self.portrait)?);
        self.text(
            &mut surfaces,
            1,
            self.strings.get(1134).ok_or("Card title")?,
            self.title,
        )?;
        let mut body = self
            .strings
            .get(panel.body as usize)
            .ok_or("Card body")?
            .clone();
        if let Some(amount) = panel.amount {
            let digits = ui::decimal2(f32::from_bits(amount))?
                .bytes()
                .map(|b| b - 32)
                .collect::<Vec<_>>();
            let symbol = self.strings.get(2243).ok_or("Card currency")?;
            let left = self.strings.get(2244).and_then(|s| s.first()) == Some(&0x4c);
            let value = if left {
                [symbol.as_slice(), digits.as_slice()].concat()
            } else {
                [digits.as_slice(), symbol.as_slice()].concat()
            };
            if let Some(at) = body.windows(2).position(|v| v == [b'%' - 32, b's' - 32]) {
                body.splice(at..at + 2, value);
            }
        }
        self.text(&mut surfaces, 0, &body, self.body)?;
        for (i, (&visible, &bounds)) in panel.buttons.iter().zip(&self.buttons).enumerate() {
            if !visible {
                continue;
            }
            let hover = pointer.is_some_and(|p| hit(bounds, p));
            let state = if hover && input.pressed == Some(i as u32) {
                2
            } else if hover {
                1
            } else {
                0
            };
            let texture = [79, 79, 82, 85][i] + state;
            surfaces.push(self.texture(texture, bounds)?);
            self.text(
                &mut surfaces,
                1,
                self.strings
                    .get([1127, 1487, 1485, 1486][i])
                    .ok_or("Card button text")?,
                bounds,
            )?;
        }
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            PROJECTION,
            None,
        )
    }
    pub fn entry_scene(
        &self,
        renderer: &Renderer,
        entry: Entry,
        visual: EntryVisual,
        input: &Input,
        pointer: Option<[f32; 2]>,
    ) -> Result<PreparedScene, String> {
        let hover = pointer.is_some_and(|p| hit(self.entry, p));
        let texture = visual.textures[if !entry.enabled {
            2
        } else if input.entry_pressed && hover {
            1
        } else if hover {
            3
        } else {
            0
        }];
        let surface = self.texture(texture, self.entry)?;
        renderer.prepare(&[surface.draw()], PROJECTION, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::{pe::PeImage, player_manager::Player};
    fn player(mode: u32, valid: bool) -> Player {
        let mut p = Player {
            reference: 1,
            bytes: vec![0; 0x228],
            auxiliaries: vec![openmt2001::player_manager::Auxiliary {
                reference: 2,
                bytes: [0; 0x30],
            }],
        };
        p.bytes[0x21c..0x220].copy_from_slice(&2u32.to_le_bytes());
        p.bytes[0x10] = 1;
        p.auxiliaries[0].bytes[0x18..0x1c].copy_from_slice(&mode.to_le_bytes());
        p.auxiliaries[0].bytes[8] = u8::from(valid);
        p
    }
    #[test]
    #[ignore = "original card presentation fixtures"]
    fn original_card_presentation() {
        let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let bytes = fs::read(openmt2001::fixtures::path("NATIVE_CARD_PANEL")).unwrap();
        assert_eq!(&bytes[..8], b"MTCARD01");
        assert_eq!(word(&bytes, 8), 0x37f);
        let mut at = 20;
        for case in 0..word(&bytes, 12) {
            let actual = Panel::present(
                word(&bytes, at),
                &player(word(&bytes, at + 4), word(&bytes, at + 8) != 0),
                &image,
            )
            .unwrap();
            assert_eq!(actual.body, word(&bytes, at + 12), "body {case}");
            assert_eq!(
                actual.amount,
                (word(&bytes, at + 16) != 0).then(|| word(&bytes, at + 20)),
                "amount {case}"
            );
            assert_eq!(
                actual.buttons.map(u8::from),
                bytes[at + 24..at + 28],
                "buttons {case}"
            );
            at += 28;
        }
        for case in 0..word(&bytes, 16) {
            let input: [u32; 13] = std::array::from_fn(|i| word(&bytes, at + i * 4));
            at += 52;
            let mut p = player(input[0], input[1] != 0);
            p.bytes[0x10] = input[2] as u8;
            let entry = Entry::player(&p, input[3] != 0).unwrap();
            let mut visual = EntryVisual {
                textures: [999; 4],
                reset: input[4] != 0,
                blink: input[5],
            };
            visual.tick(entry);
            assert_eq!(entry.enabled, input[6] != 0, "enabled {case}");
            assert_eq!(
                (visual.reset, visual.blink),
                (input[7] != 0, input[8]),
                "counter {case}"
            );
            assert_eq!(
                visual.textures.map(|i| i as u32),
                input[9..13],
                "textures {case}"
            );
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "original card panel resources and GPU"]
    fn original_card_resources() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let executable = read("OPENMT2001_EXE");
        let image = PeImage::parse(&executable).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe = openmt2001::fixtures::path("EXE");
        let language = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let bytes = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets
            .chunks_exact(2)
            .map(|n| {
                let end = at + u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let text = &bytes[at..end];
                at = end;
                text
            })
            .collect::<Vec<_>>();
        let assets = Assets::load(&index, &archive, installer, &strings).unwrap();
        for (card, mode, valid) in [(1, 1, false), (65, 1, false), (65, 2, true)] {
            let p = player(mode, valid);
            let panel = Panel::present(card, &p, &image).unwrap();
            let mut input = Input::default();
            let entry = Entry::player(&p, false).unwrap();
            for button in 0..4 {
                let b = assets.buttons[button];
                let pointer = Some([(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5]);
                assert!(
                    input
                        .mouse(&assets, Some(panel), entry, pointer, true)
                        .is_none()
                );
                let action = input.mouse(&assets, Some(panel), entry, pointer, false);
                if panel.buttons[button] {
                    assert!(
                        matches!(action,Some(openmt2001::session::cards::Action::Button{event:12,button:i}) if i==button as u32)
                    );
                }
            }
            #[cfg(feature = "capture")]
            if let Ok(path) = std::env::var("OPENMT2001_CARD_CAPTURE") {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                let prepared = assets.prepare(&renderer, panel, &input, None).unwrap();
                crate::capture_test::capture_standalone(
                    &renderer,
                    &prepared,
                    &format!("{path}-{card}-{mode}.png"),
                );
            }
        }
    }
}
