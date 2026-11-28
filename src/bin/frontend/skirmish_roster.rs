//! Private offline network_chat_V2 component. No routing or city launch.
use crate::{
    skirmish_setup,
    ui::{self, Surface},
};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    skirmish::roster::{Kind, Participant, Roster},
    texture::TextureSet,
    ui_mesh::Region,
};
use std::{fs, path::Path};
const PANEL: &str = "gamedata2\\brand_new_panels\\network_chat_V2.tm";

pub enum Outcome {
    None,
    Back(skirmish_setup::Handoff),
    Start {
        setup: skirmish_setup::Handoff,
        participants: Vec<Participant>,
    },
}
/// Name bytes are original code-page bytes. Encoding them for display belongs
/// to the host's active language/font mapping, never UTF-8 lossy conversion.
pub struct State {
    pub visible: bool,
    pub busy: bool,
    pub roster: Roster,
    setup: skirmish_setup::Handoff,
    profile_name: Vec<u8>,
    pub name_focused: bool,
}
impl State {
    pub fn open(setup: skirmish_setup::Handoff, roster: Roster) -> Result<Self, String> {
        if setup.map.is_empty()
            || setup.directory.is_empty()
            || setup.session_name.is_empty()
            || setup.difficulty > 2
            || !setup.objective_target.is_finite()
        {
            return Err("incomplete offline setup handoff".into());
        }
        let p = roster.slots();
        if p[0].kind != Kind::Human || p[1..].iter().any(|p| p.kind != Kind::Empty) {
            return Err(
                "offline roster must enter with one profile human and five empty slots".into(),
            );
        }
        Ok(Self {
            visible: true,
            busy: false,
            profile_name: p[0].name.clone(),
            roster,
            setup,
            name_focused: false,
        })
    }
    pub fn enabled(&self, id: usize) -> bool {
        self.visible
            && !self.busy
            && match id {
                0..=11 => self.roster.slots()[id / 2].kind != Kind::Empty,
                12..=17 => id != 12,
                18..=23 => id == 18,
                24 => true,
                25 => self.roster.start_enabled(),
                _ => false,
            }
    }
    pub fn focus_name(&mut self) {
        self.name_focused = self.visible;
    }
    pub fn text_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.visible && self.name_focused {
            let mut name = self.roster.slots()[0].name.clone();
            name.extend(
                bytes
                    .iter()
                    .copied()
                    .filter(|&b| b >= 32 && b != 127)
                    .take(31 - name.len()),
            );
            self.roster.rename(0, &name)?;
        }
        Ok(())
    }
    pub fn backspace(&mut self) -> Result<(), String> {
        if self.visible && self.name_focused {
            let mut name = self.roster.slots()[0].name.clone();
            name.pop();
            self.roster.rename(0, &name)?;
        }
        Ok(())
    }
    pub fn activate(&mut self, id: usize) -> Result<Outcome, String> {
        if !self.enabled(id) {
            return Ok(Outcome::None);
        }
        self.name_focused = false;
        match id {
            0..=11 => {
                self.roster
                    .cycle_token(id / 2, if id % 2 == 0 { -1 } else { 1 })?;
                //49edc0 restores the profile name when the human input is not active.
                if id / 2 == 0 {
                    self.roster.rename(0, &self.profile_name)?;
                }
            }
            12..=17 => self.roster.toggle_ai(id - 12)?,
            18 => self.roster.toggle_ready(),
            24 => {
                self.visible = false;
                return Ok(Outcome::Back(self.setup.clone()));
            }
            25 => {
                //4a214f..4a21b1 refuses incomplete rows without leaving the panel.
                if self.roster.slots().iter().any(|p| {
                    p.kind != Kind::Empty && (p.token.is_none() || p.name.is_empty() || !p.ready)
                }) {
                    return Ok(Outcome::None);
                }
                let participants = self.roster.compact_ready()?;
                self.visible = false;
                return Ok(Outcome::Start {
                    setup: self.setup.clone(),
                    participants,
                });
            }
            _ => (),
        }
        Ok(Outcome::None)
    }
}
struct Font {
    pages: Vec<Surface>,
    tab: Vec<u8>,
}
struct Character {
    color: u32,
    token: usize,
    portrait: usize,
}
/// Session metadata is supplied as already localized glyphs by setup/host:
/// selected level title and formatted difficulty/objective value respectively.
/// Native4a1c20 uses labels1654/1655/1658 and the authored target; the
/// host supplies that formatted string. INFO3 is computed readiness below.
pub struct RosterPanel {
    base: Vec<Surface>,
    regions: Vec<Region>,
    generic: Vec<Surface>,
    fonts: Vec<Font>,
    strings: Vec<Vec<u8>>,
    characters: Vec<Character>,
    backing: u32,
}
/// Shared original roster art, with route-specific controls and per-row records.
pub struct Presentation<'a> {
    pub visible: bool,
    pub busy: bool,
    pub rows: &'a [Participant; 6],
    pub enabled: [bool; 26],
    pub start_enabled: bool,
    pub local_row: usize,
    pub closed: [bool; 6],
    pub details: &'a [Vec<u8>],
}
fn inside(b: [f32; 4], p: [f32; 2]) -> bool {
    p[0] > b[0] && p[0] < b[2] && p[1] >= b[1] && p[1] <= b[3]
}
impl RosterPanel {
    #[cfg(test)]
    pub(crate) fn inert(strings: Vec<Vec<u8>>) -> Self {
        Self {
            base: vec![],
            regions: vec![],
            generic: vec![],
            fonts: vec![],
            strings,
            characters: vec![],
            backing: 0,
        }
    }

    pub fn new(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        executable: &[u8],
        backing: u32,
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing roster asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("roster asset range".into())
        };
        let mesh = CachedMesh::parse(payload(PANEL)?)?;
        let regions = openmt2001::ui_mesh::regions(&mesh, 0)?;
        // Actual named original art parts0..7 and9..14;8 is dynamic backing.
        let art = [
            "BLUE_TRIM",
            "BORDER01",
            "BORDER02",
            "BACKING",
            "BORDER03",
            "BORDER04",
            "BORDER05",
            "BORDER06",
            "PIC_BORDER01",
            "PIC_BORDER02",
            "PIC_BORDER03",
            "PIC_BORDER04",
            "PIC_BORDER05",
            "PIC_BORDER06",
        ];
        let parts = art
            .iter()
            .map(|name| {
                regions
                    .iter()
                    .find(|r| r.name == format!("NETWORK_CHAT_{name}"))
                    .map(|r| r.part)
                    .ok_or_else(|| format!("missing roster art {name}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (base, _) = ui::panel_base(index, archive, PANEL, &parts)?;
        let texture = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut fonts = vec![];
        for (id, name) in [
            (1, "tahoma12bldnoantip.tab"),
            (3, "tahoma10bldnoantip.tab"),
            (4, "coppgoth23p.tab"),
        ] {
            fonts.push(Font {
                pages: texture
                    .textures
                    .get(id * 2..id * 2 + 2)
                    .ok_or("missing roster font pages")?
                    .iter()
                    .map(Surface::new)
                    .collect(),
                tab: fs::read(installer.join("Standard/fonts").join(name))
                    .map_err(|e| e.to_string())?,
            });
        }
        let generic = TextureSet::parse(payload("gamedata2\\GenericTextures640\\textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let pe = PeImage::parse(executable)?;
        let mut characters = vec![];
        for i in 0..10 {
            let raw = pe.at(0x541ae8 + i * 40, 24)?;
            let w = |at| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
            characters.push(Character {
                color: w(8),
                token: w(16) as usize,
                portrait: w(20) as usize,
            });
        }
        let panel = Self {
            base,
            regions,
            generic,
            fonts,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            characters,
            backing,
        };
        for id in 1439..=1440 {
            panel.label(id)?;
        }
        for id in 1744..=1759 {
            panel.label(id)?;
        }
        for id in 0..26 {
            panel.action_bounds(id)?;
        }
        for i in 1..=6 {
            for name in [
                format!("NAMESLOT_P{i}"),
                format!("3DPIC_P{i}"),
                format!("TOKEN_P{i}"),
                format!("TINT_SLOT{i}"),
                format!("SLOT_{i}BACKING"),
            ] {
                panel.bounds(&name)?;
            }
        }
        Ok(panel)
    }
    /// Build domain names with the host's inverse language mapping, keeping
    /// names as code-page bytes and text rendering as font glyph indices.
    pub fn initial_roster(
        &self,
        name: &[u8],
        token: usize,
        retained: [i32; 6],
        mut decode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
    ) -> Result<Roster, String> {
        let names = (1744..=1753)
            .map(|id| decode(self.label(id)?))
            .collect::<Result<Vec<_>, String>>()?;
        Roster::new(
            name,
            token,
            names.try_into().map_err(|_| "ten token names")?,
            retained,
        )
    }
    fn label(&self, id: usize) -> Result<&[u8], String> {
        self.strings
            .get(id)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("missing roster label {id}"))
    }
    fn bounds(&self, name: &str) -> Result<[f32; 4], String> {
        self.regions
            .iter()
            .find(|r| r.name == format!("NETWORK_CHAT_{name}"))
            .map(|r| r.bounds)
            .ok_or_else(|| format!("missing roster region {name}"))
    }
    pub fn action_bounds(&self, id: usize) -> Result<[f32; 4], String> {
        self.bounds(&match id {
            0..=11 => format!(
                "{}_BUTT_P{}",
                if id % 2 == 0 { "LEFT" } else { "RIGHT" },
                id / 2 + 1
            ),
            12..=17 => format!("SLOT_TOGGLE_P{}", id - 11),
            18..=23 => format!("CHECKBOX_P{}", id - 17),
            24 => "CANCEL_BUTTON".into(),
            25 => "START_BUTTON".into(),
            _ => return Err("unknown roster action".into()),
        })
    }
    pub fn hit(&self, state: &State, p: [f32; 2]) -> Result<Option<usize>, String> {
        for id in 0..26 {
            if state.enabled(id) && inside(self.action_bounds(id)?, p) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
    /// Name inputs use event22/index0, separate from selector event12 IDs.
    pub fn hit_name(&self, state: &State, p: [f32; 2]) -> Result<bool, String> {
        Ok(state.visible && !state.busy && inside(self.bounds("NAMESLOT_P1")?, p))
    }
    fn image(&self, out: &mut Vec<Surface>, id: usize, b: [f32; 4]) -> Result<(), String> {
        let mut s = self
            .generic
            .get(id)
            .ok_or_else(|| format!("missing roster texture {id}"))?
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
    #[allow(clippy::too_many_arguments)] // Shared roster presentation plus parent descriptor alpha.
    pub fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        selected: Option<usize>,
        pressed: bool,
        metadata: [&[u8]; 2],
        encode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
        panel_alpha: u8,
    ) -> Result<PreparedScene, String> {
        self.draw_rows(
            renderer,
            selected,
            pressed,
            metadata,
            encode,
            Presentation {
                visible: state.visible,
                busy: state.busy,
                rows: state.roster.slots(),
                enabled: std::array::from_fn(|id| state.enabled(id)),
                start_enabled: state.roster.start_enabled(),
                local_row: 0,
                closed: [false; 6],
                details: &[],
            },
            panel_alpha,
        )
    }
    #[allow(clippy::too_many_arguments)] // Shared font assets and independent browser presentation.
    pub fn browser(
        &self,
        renderer: &Renderer,
        heading: &[u8],
        lines: &[Vec<u8>],
        controls: &[(usize, [f32; 4], Vec<u8>, bool)],
        selected: Option<usize>,
        pressed: bool,
        mut encode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
    ) -> Result<PreparedScene, String> {
        let mut shapes = Surface::solid();
        shapes.rect([35., 35., 605., 445.], [0xf0000a28; 4]);
        let mut pages = self.fonts[0].pages.clone();
        ui::text_widget_style9(
            &mut pages,
            &self.fonts[0].tab,
            &encode(heading)?,
            [45., 45., 595., 75.],
            0xffffffff,
        )?;
        for (i, line) in lines.iter().enumerate() {
            ui::text_widget_style3(
                &mut pages,
                &self.fonts[0].tab,
                &encode(line)?,
                [55., 285. + i as f32 * 22., 585., 307. + i as f32 * 22.],
                0xffffffff,
            )?;
        }
        for (id, bounds, label, enabled) in controls {
            ui::button(
                &mut shapes,
                *bounds,
                [0, 115, 85],
                selected == Some(*id),
                pressed && selected == Some(*id),
                !*enabled,
            )?;
            ui::text_widget_style9(
                &mut pages,
                &self.fonts[0].tab,
                &encode(label)?,
                *bounds,
                if *enabled { 0xffffffff } else { 0xff909090 },
            )?;
        }
        let mut out = vec![shapes];
        out.extend(pages);
        renderer.prepare(
            &out.iter().map(Surface::draw).collect::<Vec<_>>(),
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
    pub fn name_bounds(&self, row: usize) -> Result<[f32; 4], String> {
        self.bounds(&format!("NAMESLOT_P{}", row + 1))
    }
    pub fn token_names(
        &self,
        mut decode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
    ) -> Result<[Vec<u8>; 10], String> {
        (1744..1754)
            .map(|id| decode(self.label(id)?))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "ten token names required".into())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rows(
        &self,
        renderer: &Renderer,
        selected: Option<usize>,
        pressed: bool,
        metadata: [&[u8]; 2],
        mut encode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
        view: Presentation<'_>,
        panel_alpha: u8,
    ) -> Result<PreparedScene, String> {
        let mut out = vec![];
        if view.visible {
            out = self.base.clone();
            crate::menu::modal_frame::set_alpha(&mut out, panel_alpha);
            let mut shapes = Surface::solid();
            let mut images = vec![];
            let mut pages = self
                .fonts
                .iter()
                .map(|f| f.pages.clone())
                .collect::<Vec<_>>();
            for name in ["PLAYER_BACKING", "SESS_BACKING", "CHAT_BACKING"] {
                shapes.rect(self.bounds(name)?, [self.backing; 4]);
            }
            for name in ["PLAYERS_TITLE_AREA", "SESS_TITLE_AREA", "CHAT_TITLE_AREA"] {
                shapes.rect(
                    self.bounds(name)?,
                    [0xff909090, 0xff909090, 0x80909090, 0x80909090],
                );
            }
            shapes.rect(
                self.bounds("TITLE_AREA")?,
                [0xff00ff00, 0xff00ff00, 0x8000ff00, 0x8000ff00],
            );
            shapes.rect(self.bounds("CHAT_BACKING")?, [0xff000a4b; 4]);
            for name in ["SESS_LABEL1", "SESS_LABEL2", "SESS_LABEL3"] {
                shapes.rect(self.bounds(name)?, [0xff01762f; 4]);
            }
            for (name, id, font) in [
                ("TITLE_AREA", 1878, 2),
                ("PLAYERS_TITLE_AREA", 1684, 0),
                ("SESS_TITLE_AREA", 1685, 0),
                ("CHAT_TITLE_AREA", 1686, 0),
                ("SESS_LABEL1", 1615, 0),
                ("SESS_LABEL2", 1879, 0),
                ("SESS_LABEL3", 1616, 0),
            ] {
                ui::text_widget_style9(
                    &mut pages[font],
                    &self.fonts[font].tab,
                    self.label(id)?,
                    self.bounds(name)?,
                    0xffffffff,
                )?;
            }
            for (i, text) in metadata.iter().enumerate() {
                let b = self.bounds(&format!("SESS_INFO{}", i + 1))?;
                shapes.rect(b, [0xff1b254b; 4]);
                ui::text_widget_style9(&mut pages[0], &self.fonts[0].tab, text, b, 0xffffffff)?;
            }
            let status = self.bounds("SESS_INFO3")?;
            shapes.rect(status, [0xff1b254b; 4]);
            let status_text = if view.busy {
                encode(b"Loading...")?
            } else {
                self.label(if view.start_enabled { 1757 } else { 1759 })?
                    .to_vec()
            };
            ui::text_widget_style9(
                &mut pages[0],
                &self.fonts[0].tab,
                &status_text,
                status,
                0xffffffff,
            )?;
            // Offline component retains the authored chat area as passive art;
            // no invented network/chat action is exposed.
            shapes.rect(self.bounds("CHAT_TYPEAREA")?, [0xff1f2f70; 4]);
            shapes.rect(
                self.bounds(&format!("SELECTP{}", view.local_row + 1))?,
                [0xff00ff00; 4],
            );
            for (i, p) in view.rows.iter().enumerate() {
                let row = i + 1;
                let b = self.bounds(&format!("SLOT_{row}BACKING"))?;
                shapes.rect(b, [self.backing; 4]);
                let name = self.bounds(&format!("NAMESLOT_P{row}"))?;
                shapes.rect(name, [0xff000530; 4]);
                let text = if view.closed[i] {
                    encode(b"Closed")?
                } else if p.kind == Kind::Empty {
                    self.label(1754)?.to_vec()
                } else if p.kind == Kind::Ai {
                    self.label(1744 + p.token.ok_or("AI token missing")?)?
                        .to_vec()
                } else {
                    encode(&p.name)?
                };
                ui::text_widget_style3(&mut pages[1], &self.fonts[1].tab, &text, name, 0xffffffff)?;
                let tint = self.bounds(&format!("TINT_SLOT{row}"))?;
                if let Some(token) = p.token {
                    let c = self
                        .characters
                        .get(token)
                        .ok_or("roster token out of range")?;
                    shapes.rect(
                        tint,
                        [
                            c.color,
                            c.color,
                            (c.color & 0xffffff) | 0x7f000000,
                            (c.color & 0xffffff) | 0x7f000000,
                        ],
                    );
                    self.image(&mut images, c.token, self.bounds(&format!("TOKEN_P{row}"))?)?;
                    self.image(
                        &mut images,
                        c.portrait,
                        self.bounds(&format!("3DPIC_P{row}"))?,
                    )?;
                    for id in [2 * i, 2 * i + 1] {
                        self.image(
                            &mut images,
                            if id % 2 == 0 { 97 } else { 101 }
                                + if !view.enabled[id] {
                                    3
                                } else if selected == Some(id) {
                                    if pressed { 2 } else { 1 }
                                } else {
                                    0
                                },
                            self.action_bounds(id)?,
                        )?;
                    }
                } else {
                    shapes.rect(tint, [0xff909090, 0xff909090, 0x7f909090, 0x7f909090]);
                    shapes.rect(self.bounds(&format!("3DPIC_P{row}"))?, [0x7f000000; 4]);
                }
                //4a15e0 selector textures [human259, AI258, empty259, locked257].
                self.image(
                    &mut images,
                    match p.kind {
                        Kind::Human => 259,
                        Kind::Ai => 258,
                        Kind::Empty => {
                            if view.closed[i] {
                                257
                            } else {
                                259
                            }
                        }
                    },
                    self.action_bounds(12 + i)?,
                )?;
                self.image(
                    &mut images,
                    if p.ready { 262 } else { 261 },
                    self.action_bounds(18 + i)?,
                )?;
            }
            for (id, label, color) in [(24, 1439, [175, 6, 6]), (25, 1440, [80, 148, 34])] {
                let b = self.action_bounds(id)?;
                ui::button(
                    &mut shapes,
                    b,
                    color,
                    selected == Some(id),
                    pressed && selected == Some(id),
                    !view.enabled[id],
                )?;
                ui::text_widget_style9(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    self.label(label)?,
                    b,
                    0xffffffff,
                )?;
            }
            let chat = self.bounds("CHAT_BACKING")?;
            for (i, line) in view.details.iter().enumerate() {
                ui::text_widget_style3(
                    &mut pages[1],
                    &self.fonts[1].tab,
                    &encode(line)?,
                    [
                        chat[0] + 5.,
                        chat[1] + 5. + i as f32 * 12.,
                        chat[2] - 5.,
                        chat[1] + 17. + i as f32 * 12.,
                    ],
                    0xffffffff,
                )?;
            }
            out.push(shapes);
            out.extend(images);
            for p in pages {
                out.extend(p);
            }
        }
        renderer.prepare(
            &out.iter().map(Surface::draw).collect::<Vec<_>>(),
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
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires supplied original assets, English language and offscreen graphics"]
    fn original_offline_roster_controls_and_rendered_panels() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let executable = read("OPENMT2001_EXE");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let offsets = read("OPENMT2001_LANGUAGE_OFF");
        let glyphs = read("OPENMT2001_LANGUAGE_BIN");
        let mut at = 0usize;
        let strings = offsets
            .chunks_exact(2)
            .map(|b| {
                let start = at;
                at += u16::from_le_bytes(b.try_into().unwrap()) as usize;
                &glyphs[start..at]
            })
            .collect::<Vec<_>>();
        let installer = openmt2001::fixtures::path("INSTALLER");
        let localized = crate::tooltip::localized_strings(&installer, &strings).unwrap();
        let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let panel = RosterPanel::new(
            &index,
            &archive,
            &installer,
            &strings,
            &executable,
            0xbb000a28,
        )
        .unwrap();
        let roster = panel
            .initial_roster(b"Justin2", 4, [0; 6], |glyphs| {
                glyphs
                    .iter()
                    .map(|b| b.checked_add(32).ok_or("non-English token glyph".into()))
                    .collect()
            })
            .unwrap();
        let handoff = skirmish_setup::Handoff {
            level_id: 300001,
            directory: b"net1".to_vec(),
            map: b"empty.lua".to_vec(),
            objective_kind: 6,
            objective_target: 2000.,
            difficulty: 0,
            session_name: b"Justin2".to_vec(),
        };
        let mut state = State::open(handoff, roster).unwrap();
        assert!(!state.enabled(25));
        assert!(matches!(state.activate(25).unwrap(), Outcome::None));
        assert!(!state.enabled(2));
        assert!(!state.enabled(12));
        assert!(state.enabled(0));
        let center = |b: [f32; 4]| [(b[0] + b[2]) / 2., (b[1] + b[3]) / 2.];
        assert_eq!(
            panel
                .hit(&state, center(panel.action_bounds(2).unwrap()))
                .unwrap(),
            None
        );
        assert_eq!(
            panel
                .hit(&state, center(panel.action_bounds(13).unwrap()))
                .unwrap(),
            Some(13)
        );
        #[cfg(feature = "capture")]
        let renderer = {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            pollster::block_on(Renderer::new(descriptor)).unwrap()
        };
        #[cfg(feature = "capture")]
        let metadata = [strings[1885], strings[1654]]; // Actual labels; host value formatting is a separate boundary.
        #[cfg(feature = "capture")]
        let encode = |bytes: &[u8]| {
            Ok(crate::load::ascii(
                std::str::from_utf8(bytes).map_err(|e| e.to_string())?,
            ))
        };
        #[cfg(feature = "capture")]
        {
            let draw = panel
                .draw(&state, &renderer, None, false, metadata, encode, 255)
                .unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                &draw,
                "work/frontend/skirmish-roster-empty.png",
            );
        }
        state.activate(13).unwrap();
        assert!(state.enabled(25));
        state.busy = true;
        assert!((0..26).all(|id| !state.enabled(id)));
        assert!(
            !panel
                .hit_name(&state, center(panel.bounds("NAMESLOT_P1").unwrap()))
                .unwrap()
        );
        assert!(matches!(state.activate(25).unwrap(), Outcome::None));
        state.busy = false;
        assert!(state.enabled(2));
        assert_eq!(
            panel
                .hit(&state, center(panel.action_bounds(2).unwrap()))
                .unwrap(),
            Some(2)
        );
        assert_ne!(state.roster.slots()[0].token, state.roster.slots()[1].token);
        state.roster.rename(0, b"").unwrap();
        assert!(matches!(state.activate(25).unwrap(), Outcome::None));
        assert!(state.visible);
        state.roster.rename(0, b"Justin2").unwrap();
        state.activate(18).unwrap();
        assert!(!state.enabled(25));
        state.activate(18).unwrap();
        assert!(state.enabled(25));
        #[cfg(feature = "capture")]
        {
            let draw = panel
                .draw(&state, &renderer, Some(25), false, metadata, encode, 255)
                .unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                &draw,
                "work/frontend/skirmish-roster-one-ai.png",
            );
        }
        let Outcome::Start {
            setup,
            participants,
        } = state.activate(25).unwrap()
        else {
            panic!("expected Start")
        };
        assert_eq!(setup.level_id, 300001);
        assert_eq!(participants.len(), 2);
        assert_eq!(participants[0].name, b"Justin2");
        assert_eq!(participants[1].kind, Kind::Ai);
        assert!(participants.iter().all(|p| p.difficulty == 0));
        assert!(!state.visible);
    }
}
