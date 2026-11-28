//! Original auction announcement41d3d0/41d700/41d870, using its two mesh frames.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub category: u32,
    pub character: u32,
    pub actor: [u8; 96],
    pub amount: u32,
    /// The original text starts empty and changes only at whole-second callbacks.
    pub seconds: Option<i32>,
}
impl Notice {
    pub fn from_session(
        session: &openmt2001::session::Session<'_>,
        seconds: Option<i32>,
    ) -> Result<Option<Self>, String> {
        let presentation = &session.world.city.auction.presentation;
        if !presentation.visible {
            return Ok(None);
        }
        let [category, participant, amount] = presentation.announcement;
        let player = session
            .roster
            .manager
            .players
            .get(participant as usize)
            .ok_or("Auction announcement participant outside roster")?;
        Ok(Some(Self {
            category,
            character: u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()),
            actor: player.bytes[0x158..0x1b8].try_into().unwrap(),
            amount,
            seconds,
        }))
    }
}

/// Retained announcement panel state. Native41d700 always opens frame1
/// (compact); message12 toggles its mode, while message4 selects the category.
#[derive(Default)]
pub struct State {
    notice: Option<Notice>,
    expanded: bool,
    pressed: bool,
    over_button: bool,
    dirty: bool,
    pub prepared: Option<PreparedScene>,
}
impl State {
    pub fn update(&mut self, notice: Option<Notice>) {
        if self.notice != notice {
            if self.notice.is_none() || notice.is_none() {
                self.expanded = false;
                self.pressed = false;
            }
            self.notice = notice;
            self.dirty = true;
        }
    }
    /// Return whether the panel consumed input and an optional category view.
    pub fn mouse(
        &mut self,
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        down: bool,
    ) -> (bool, Option<u32>) {
        let Some(notice) = &self.notice else {
            return (false, None);
        };
        let hit = |bounds: [f32; 4]| {
            pointer.is_some_and(|[x, y]| {
                x >= bounds[0] && x <= bounds[2] && y >= bounds[1] && y <= bounds[3]
            })
        };
        let button = hit(assets.button_bounds(self.expanded));
        if self.pressed || button {
            if down {
                self.pressed = button;
            } else {
                if self.pressed && button {
                    self.expanded = !self.expanded;
                }
                self.pressed = false;
            }
            self.over_button = button;
            self.dirty = true;
            return (true, None);
        }
        let backing = hit(assets.bounds(self.expanded));
        (backing, (backing && down).then_some(notice.category))
    }
    pub fn prepare(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        pointer: Option<[f32; 2]>,
    ) -> Result<(), String> {
        let bounds = assets.button_bounds(self.expanded);
        let over = pointer.is_some_and(|[x, y]| {
            x >= bounds[0] && x <= bounds[2] && y >= bounds[1] && y <= bounds[3]
        });
        if self.pressed && self.over_button != over {
            self.dirty = true;
        }
        self.over_button = over;
        if self.dirty {
            self.prepared = self
                .notice
                .as_ref()
                .map(|notice| {
                    assets.prepare_state(renderer, notice, self.expanded, self.pressed && over)
                })
                .transpose()?;
            self.dirty = false;
        }
        Ok(())
    }
}
struct Layout {
    base: Vec<Surface>,
    backing: [f32; 4],
    title: [f32; 4],
    button: [f32; 4],
    timer: [f32; 4],
    portrait: [f32; 4],
    label: [f32; 4],
    bid: [f32; 4],
}
pub struct Assets {
    layouts: Vec<Layout>,
    portraits: Vec<Surface>,
    buttons: [Surface; 2],
    fonts: [Vec<Surface>; 2],
    tabs: [Vec<u8>; 2],
    strings: Vec<Vec<u8>>,
    category_titles: [u32; 52],
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
        exe: &[u8],
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let entry = index
                .find(name)
                .ok_or_else(|| format!("Missing auction asset {name}"))?;
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Auction asset range".into())
        };
        let image = PeImage::parse(exe)?;
        let name = "gamedata2/brand_new_panels/new_auction_external_timer3.tm";
        let mesh = CachedMesh::parse(payload(name)?)?;
        if mesh.frames != 2 {
            return Err("Auction announcement needs its two original frames".into());
        }
        let color = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        let mut layouts = Vec::new();
        for frame in 0..2 {
            let regions = openmt2001::ui_mesh::regions(&mesh, frame)?;
            let region = |name: &str| -> Result<[f32; 4], String> {
                regions
                    .iter()
                    .find(|r| r.name == name)
                    .map(|r| r.bounds)
                    .ok_or_else(|| format!("Missing auction region {name}"))
            };
            let backing = region("AUCTION_TIMER_BACKING")?;
            let title = region("AUCTION_TIMER_TITLE_LABEL")?;
            let button = region("AUCTION_TIMER_OPEN_BUTTON")?;
            let mut fill = Surface::solid();
            fill.rect(backing, [color; 4]);
            if frame == 1 {
                fill.rect(title, [0xff909090, 0xff909090, 0x7f909090, 0x7f909090]);
            }
            //43c3a0 draws the mesh before41d870 fills its named regions.
            let mut base = ui::panel_frame_base(index, archive, name, &[0], frame)?.0;
            base.push(fill);
            layouts.push(Layout {
                base,
                backing,
                title,
                button,
                timer: region("AUCTION_TIMER_MINI_TIMER")?,
                portrait: region("AUCTION_TIMER_MINI_BIDDER")?,
                label: region("AUCTION_TIMER_MINI_TIMER_LABEL")?,
                bid: region("AUCTION_TIMER_MINI_BID_LABEL")?,
            });
        }
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut portraits = Vec::new();
        for character in 0..10 {
            let texture =
                u32::from_le_bytes(image.at(0x541af8 + character * 40, 4)?.try_into().unwrap());
            portraits.push(Surface::new(
                generic
                    .textures
                    .get(texture as usize)
                    .ok_or("Auction portrait index")?,
            ));
        }
        let font = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut category_titles = [0; 52];
        for (i, title) in category_titles.iter_mut().enumerate() {
            *title = u32::from_le_bytes(image.at(0x5385a0 + i as u32 * 88, 4)?.try_into().unwrap())
                .wrapping_add(1);
        }
        Ok(Self {
            layouts,
            portraits,
            buttons: [278, 280].map(|index| Surface::new(&generic.textures[index])),
            fonts: [
                font.textures[..2].iter().map(Surface::new).collect(),
                font.textures[6..8].iter().map(Surface::new).collect(),
            ],
            tabs: [
                fs::read(installer.join("Standard/fonts/cust640p.tab"))
                    .map_err(|e| e.to_string())?,
                fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
                    .map_err(|e| e.to_string())?,
            ],
            strings: crate::tooltip::localized_strings(installer, strings)?,
            category_titles,
        })
    }
    pub fn bounds(&self, expanded: bool) -> [f32; 4] {
        self.layouts[usize::from(expanded)].backing
    }
    pub fn button_bounds(&self, expanded: bool) -> [f32; 4] {
        self.layouts[usize::from(expanded)].button
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        notice: &Notice,
        expanded: bool,
    ) -> Result<PreparedScene, String> {
        self.prepare_state(renderer, notice, expanded, false)
    }
    fn prepare_state(
        &self,
        renderer: &Renderer,
        notice: &Notice,
        expanded: bool,
        pressed: bool,
    ) -> Result<PreparedScene, String> {
        let layout = &self.layouts[usize::from(expanded)];
        let mut surfaces = layout.base.clone();
        let mut portrait = self
            .portraits
            .get(notice.character as usize)
            .ok_or("Auction character outside registry")?
            .clone();
        rect(&mut portrait, layout.portrait);
        surfaces.push(portrait);
        let mut button = self.buttons[usize::from(pressed)].clone();
        rect(&mut button, layout.button);
        surfaces.push(button);
        if let Some(seconds) = notice.seconds {
            self.text(
                &mut surfaces,
                1,
                &ascii(&seconds.to_string())?,
                layout.timer,
                true,
                10.,
            )?;
        }
        if expanded {
            let title = *self
                .category_titles
                .get(notice.category as usize)
                .ok_or("Auction category outside registry")?;
            let mut bounds = layout.title;
            let half = (layout.button[2] - layout.button[0]) * 0.5;
            bounds[0] -= half;
            bounds[2] -= half;
            self.text(&mut surfaces, 1, self.language(title)?, bounds, true, 10.)?;
            self.text(
                &mut surfaces,
                1,
                self.language(1135)?,
                layout.label,
                true,
                10.,
            )?;
            let end = notice
                .actor
                .iter()
                .position(|b| *b == 0)
                .ok_or("Unterminated auction bidder")?;
            let actor =
                ascii(std::str::from_utf8(&notice.actor[..end]).map_err(|e| e.to_string())?)?;
            let value = notice.amount as i32;
            let amount = ui::integer_money(
                value,
                self.language(2243)?,
                self.language(2244)?.first() != Some(&0x4c),
            );
            let mut text = self.language(1567)?.to_vec();
            for value in [&actor, &amount] {
                let at = text
                    .windows(2)
                    .position(|p| p == [b'%' - 32, b's' - 32])
                    .ok_or("Auction bid substitution missing")?;
                text.splice(at..at + 2, value.iter().copied());
            }
            self.text(&mut surfaces, 0, &text, layout.bid, false, 8.)?;
        }
        let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
        renderer.prepare(
            &draws,
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
    fn language(&self, id: u32) -> Result<&[u8], String> {
        self.strings
            .get(id as usize)
            .map(Vec::as_slice)
            .ok_or("Auction language index".into())
    }
    fn text(
        &self,
        surfaces: &mut Vec<Surface>,
        font: usize,
        text: &[u8],
        bounds: [f32; 4],
        center: bool,
        height: f32,
    ) -> Result<(), String> {
        let (offsets, copy) = crate::prompt::wrap(
            &self.tabs[font],
            text,
            (bounds[2] - bounds[0]).trunc() as i32,
        )?;
        let top = (bounds[1] + (bounds[3] - bounds[1] - (offsets.len() - 1) as f32 * height) * 0.5)
            .trunc();
        let mut pages = self.fonts[font].clone();
        for (i, pair) in offsets.windows(2).enumerate() {
            let end = (pair[1] - 1).min(copy.len());
            ui::text(
                &mut pages,
                &self.tabs[font],
                &copy[pair[0].min(end)..end],
                [bounds[0], top + i as f32 * height, bounds[2], bounds[3]],
                center,
                0xffffffff,
            )?;
        }
        surfaces.extend(pages);
        Ok(())
    }
}
fn ascii(s: &str) -> Result<Vec<u8>, String> {
    s.bytes()
        .map(|b| {
            if (32..=126).contains(&b) {
                Ok(b - 32)
            } else {
                Err("Auction text needs an unrecovered glyph mapping".into())
            }
        })
        .collect()
}
fn rect(s: &mut Surface, b: [f32; 4]) {
    s.quad(
        [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
        [0xffffffff; 4],
    );
}

#[cfg(test)]
pub fn verify_panel(scene: &openmt2001::render_scene::Scene, player: &[u8; 440]) {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let exe = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
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
    let assets = Assets::load(&index, &archive, installer, &strings, &exe).unwrap();
    let notice = Notice {
        category: 33,
        character: u32::from_le_bytes(player[0x14c..0x150].try_into().unwrap()),
        actor: player[0x154..0x1b4].try_into().unwrap(),
        amount: 12500,
        seconds: Some(120),
    };
    assert!(assets.bounds(true)[0] < assets.bounds(false)[0]);
    assert_eq!(assets.button_bounds(true), assets.button_bounds(false));
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_AUCTION_NOTICE_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        for expanded in [false, true] {
            let panel = assets.prepare(&renderer, &notice, expanded).unwrap();
            crate::capture_test::capture(
                &renderer,
                Some(&panel),
                scene,
                &format!(
                    "{path}-{}.png",
                    if expanded { "expanded" } else { "compact" }
                ),
            );
        }
    }
    let _ = (scene, notice);
}
