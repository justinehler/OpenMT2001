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
    highlight: ui::LobbyHighlight,
}
/// Shared original roster art, with route-specific controls and per-row records.
pub struct Presentation<'a> {
    /// Connected lobby chat: the retained lines and the current input text.
    pub chat: Option<(&'a openmt2001::network::chat::View, &'a [u8])>,
    /// None for offline Skirmish; network authority determines the ready caption.
    pub network_host: Option<bool>,
    pub visible: bool,
    pub busy: bool,
    pub rows: &'a [Participant; 6],
    pub enabled: [bool; 26],
    pub start_enabled: bool,
    pub local_row: usize,
    pub name_focused: bool,
    pub closed: [bool; 6],
}
fn inside(b: [f32; 4], p: [f32; 2]) -> bool {
    p[0] > b[0] && p[0] < b[2] && p[1] >= b[1] && p[1] <= b[3]
}
impl RosterPanel {
    pub fn chat_bounds(&self) -> Result<[f32; 4], String> {
        self.bounds("CHAT_TYPEAREA")
    }
    pub(crate) fn apply_board(
        &mut self,
        board: &openmt2001::packed_language::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

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
            highlight: ui::LobbyHighlight::default(),
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
            highlight: ui::LobbyHighlight::default(),
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
                chat: None,
                network_host: None,
                visible: state.visible,
                busy: state.busy,
                rows: state.roster.slots(),
                enabled: std::array::from_fn(|id| state.enabled(id)),
                start_enabled: state.roster.start_enabled(),
                local_row: 0,
                name_focused: state.name_focused,
                closed: [false; 6],
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
            ui::button_label(
                &mut pages,
                &self.fonts[0].tab,
                &encode(label)?,
                *bounds,
                !*enabled,
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
    pub fn advance_highlight(&mut self) {
        self.highlight.advance();
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
        encode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
        view: Presentation<'_>,
        panel_alpha: u8,
    ) -> Result<PreparedScene, String> {
        let out = self.row_surfaces(selected, pressed, metadata, encode, view, panel_alpha)?;
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
    #[allow(clippy::too_many_arguments)]
    fn row_surfaces(
        &self,
        selected: Option<usize>,
        pressed: bool,
        metadata: [&[u8]; 2],
        mut encode: impl FnMut(&[u8]) -> Result<Vec<u8>, String>,
        view: Presentation<'_>,
        panel_alpha: u8,
    ) -> Result<Vec<Surface>, String> {
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
            if view
                .rows
                .get(view.local_row)
                .is_some_and(|p| p.kind == Kind::Human)
            {
                self.highlight.paint(
                    &mut shapes,
                    self.bounds(&format!("SELECTP{}", view.local_row + 1))?,
                );
            }
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
            ] {
                ui::text_widget_style9(
                    &mut pages[font],
                    &self.fonts[font].tab,
                    self.label(id)?,
                    self.bounds(name)?,
                    0xffffffff,
                )?;
            }
            // 4a08ed/4a08fa/4a0903 select the campaign's green caption style.
            for (name, id) in [
                ("SESS_LABEL1", 1615),
                ("SESS_LABEL2", 1879),
                ("SESS_LABEL3", 1616),
            ] {
                ui::section_header(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    self.label(id)?,
                    self.bounds(name)?,
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
                self.label(if !view.start_enabled {
                    1759
                } else if view.network_host == Some(false) {
                    1758 // 49ead0: ready clients wait for the host.
                } else {
                    1757
                })?
                .to_vec()
            };
            ui::text_widget_style9(
                &mut pages[0],
                &self.fonts[0].tab,
                &status_text,
                status,
                0xffffffff,
            )?;
            let chat_input = self.bounds("CHAT_TYPEAREA")?;
            shapes.rect(chat_input, [0xff1f2f70; 4]);
            if view.network_host.is_some() {
                // 4a0ff0 creates the lobby's active, left-aligned chat input.
                ui::text_entry(
                    &mut pages[0],
                    &mut shapes,
                    &self.fonts[0].tab,
                    &encode(view.chat.map_or(&[], |(_, input)| input))?,
                    chat_input,
                    false,
                    true,
                )?;
            }
            if let Some((chat, _)) = view.chat {
                // Lines stack upward from the input bar in the sender's colour.
                let area = self.bounds("CHAT_BACKING")?;
                let mut bottom = chat_input[1] - 2.;
                for line in chat.lines.iter().rev() {
                    let top = bottom - 14.;
                    if top < area[1] {
                        break;
                    }
                    let color = line.color(&std::array::from_fn(|i| {
                        self.characters.get(i).map_or(0xffffff, |c| c.color)
                    }));
                    ui::text_fit(
                        &mut pages[0],
                        &self.fonts[0].tab,
                        &encode(&line.text)?,
                        [area[0] + 4., top, area[2] - 4., bottom],
                        false,
                        color,
                    )?;
                    bottom = top;
                }
            }
            for (i, p) in view.rows.iter().enumerate() {
                let row = i + 1;
                let b = self.bounds(&format!("SLOT_{row}BACKING"))?;
                shapes.rect(b, [0xff000a4b; 4]);
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
                ui::text_entry(
                    &mut pages[1],
                    &mut shapes,
                    &self.fonts[1].tab,
                    &text,
                    name,
                    false,
                    view.name_focused && i == view.local_row,
                )?;
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
                        // 49f3a0 hides remote human selectors, rather than
                        // displaying their disabled textures. Keep the AI controls.
                        if view.network_host.is_some()
                            && p.kind == Kind::Human
                            && i != view.local_row
                        {
                            continue;
                        }
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
                ui::button_label(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    self.label(label)?,
                    b,
                    !view.enabled[id],
                )?;
            }
            out.push(shapes);
            out.extend(images);
            for p in pages {
                out.extend(p);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic_panel() -> RosterPanel {
        let mut panel = RosterPanel::inert(vec![vec![]; 1900]);
        panel.backing = 0xbb000a28;
        panel.generic = vec![Surface::solid(); 263];
        panel.characters = (0..10)
            .map(|_| Character {
                color: 0xffaa2222,
                token: 0,
                portrait: 1,
            })
            .collect();
        panel.fonts = (0..3)
            .map(|_| Font {
                pages: vec![Surface::solid(); 2],
                tab: ui::synthetic_text_font(5, 8),
            })
            .collect();
        panel.strings[1440] = crate::load::ascii("Start");
        let mut region = |name: &str, bounds| {
            panel.regions.push(Region {
                name: format!("NETWORK_CHAT_{name}"),
                part: 0,
                bounds,
            })
        };
        for name in [
            "SESS_BACKING",
            "CHAT_BACKING",
            "PLAYERS_TITLE_AREA",
            "SESS_TITLE_AREA",
            "CHAT_TITLE_AREA",
            "TITLE_AREA",
            "SESS_LABEL1",
            "SESS_LABEL2",
            "SESS_LABEL3",
            "SESS_INFO1",
            "SESS_INFO2",
            "SESS_INFO3",
            "CHAT_TYPEAREA",
            "CANCEL_BUTTON",
        ] {
            region(name, [10., 300., 100., 330.]);
        }
        region("START_BUTTON", [450., 400., 580., 435.]);
        region("PLAYER_BACKING", [10., 80., 610., 260.]);
        for row in 1..=6 {
            let x = 20. + (row - 1) as f32 * 96.;
            for (prefix, bounds) in [
                ("SELECTP", [x, 90., x + 88., 254.]),
                ("SLOT_", [x + 3., 194., x + 85., 251.]),
                ("NAMESLOT_P", [x + 3., 93., x + 85., 109.]),
                ("3DPIC_P", [x + 4., 112., x + 84., 191.]),
                ("TINT_SLOT", [x + 4., 195., x + 84., 223.]),
                ("TOKEN_P", [x + 30., 195., x + 58., 223.]),
                ("LEFT_BUTT_P", [x + 5., 200., x + 20., 215.]),
                ("RIGHT_BUTT_P", [x + 65., 200., x + 80., 215.]),
                ("SLOT_TOGGLE_P", [x + 5., 228., x + 20., 243.]),
                ("CHECKBOX_P", [x + 65., 228., x + 80., 243.]),
            ] {
                region(
                    &if prefix == "SLOT_" {
                        format!("SLOT_{row}BACKING")
                    } else {
                        format!("{prefix}{row}")
                    },
                    bounds,
                );
            }
        }
        panel
    }
    fn surfaces(
        panel: &RosterPanel,
        rows: &[Participant; 6],
        enabled: bool,
        local: usize,
    ) -> Vec<Surface> {
        panel
            .row_surfaces(
                Some(25),
                true,
                [&[], &[]],
                |b| Ok(crate::load::ascii(&String::from_utf8_lossy(b))),
                Presentation {
                    chat: None,
                    network_host: None,
                    visible: true,
                    busy: false,
                    rows,
                    enabled: std::array::from_fn(|id| id != 25 || enabled),
                    start_enabled: enabled,
                    local_row: local,
                    name_focused: false,
                    closed: [false; 6],
                },
                255,
            )
            .unwrap()
    }
    fn assert_start_style(panel: &RosterPanel, rows: &[Participant; 6], enabled: bool) {
        let out = surfaces(panel, rows, enabled, 0);
        // The Start label is the only nonempty font0 caption in this fixture.
        let label = &out[out.len() - 6];
        let mut expected = vec![Surface::solid(); 2];
        ui::text_widget_style9(
            &mut expected,
            &panel.fonts[0].tab,
            &panel.strings[1440],
            panel.action_bounds(25).unwrap(),
            if enabled { 0xffffffff } else { 0xff646464 },
        )
        .unwrap();
        assert_eq!(label.vertices, expected[0].vertices);
        let mut button = Surface::solid();
        ui::button(
            &mut button,
            panel.action_bounds(25).unwrap(),
            [80, 148, 34],
            true,
            true,
            !enabled,
        )
        .unwrap();
        // Shapes end with Cancel then Start; both label and frame use availability.
        assert!(out[0].vertices.ends_with(&button.vertices));
    }
    #[test]
    fn network_lobby_headers_status_tokens_and_chat_match_host_and_client() {
        use openmt2001::network::lobby::{Request, Settings, Snapshot};
        let mut panel = synthetic_panel();
        for (id, text) in [
            (1615, "Scenario"),
            (1879, "Difficulty"),
            (1616, "Status"),
            (1757, "Ready!"),
            (1758, "Waiting for Host"),
            (1759, "Waiting for Players"),
        ] {
            panel.strings[id] = crate::load::ascii(text);
        }
        for (name, bounds) in [
            ("SESS_LABEL1", [20., 280., 140., 300.]),
            ("SESS_LABEL2", [20., 310., 140., 330.]),
            ("SESS_LABEL3", [20., 340., 140., 360.]),
            ("SESS_INFO3", [145., 340., 320., 360.]),
            ("CHAT_BACKING", [350., 280., 600., 380.]),
            ("CHAT_TYPEAREA", [350., 390., 600., 415.]),
        ] {
            panel
                .regions
                .iter_mut()
                .find(|r| r.name == format!("NETWORK_CHAT_{name}"))
                .unwrap()
                .bounds = bounds;
        }
        let mut snapshot = Snapshot::host(
            Settings {
                name: b"Host session".to_vec(),
                level: 300001,
                parameter_bits: 2f32.to_bits(),
                difficulty: 0,
                map: b"default".to_vec(),
            },
            b"Host".to_vec(),
            0,
        )
        .unwrap();
        for phase in 0..3 {
            if phase == 1 {
                snapshot
                    .request(
                        1,
                        Request::Join {
                            name: b"Guest".to_vec(),
                            token: 1,
                        },
                    )
                    .unwrap();
            } else if phase == 2 {
                snapshot
                    .request(
                        1,
                        Request::Edit {
                            token: 1,
                            ready: true,
                        },
                    )
                    .unwrap();
            }
            let rows = std::array::from_fn(|i| {
                let p = &snapshot.rows[i].player;
                Participant {
                    kind: if p.kind == openmt2001::network::RowKind::Human {
                        Kind::Human
                    } else {
                        Kind::Empty
                    },
                    token: usize::try_from(p.token).ok(),
                    name: p.name.clone(),
                    ready: p.ready,
                    difficulty: 0,
                }
            });
            for host in [true, false] {
                if !host && phase == 0 {
                    continue;
                }
                let local = usize::from(!host);
                let enabled = std::array::from_fn(|id| match id {
                    0..=11 => id / 2 == local,
                    25 => host && snapshot.start_enabled(),
                    _ => true,
                });
                let out = panel
                    .row_surfaces(
                        None,
                        false,
                        [&[], &[]],
                        |b| Ok(crate::load::ascii(&String::from_utf8_lossy(b))),
                        Presentation {
                            chat: None,
                            network_host: Some(host),
                            visible: true,
                            busy: false,
                            rows: &rows,
                            enabled,
                            start_enabled: snapshot.start_enabled(),
                            local_row: local,
                            name_focused: false,
                            closed: [false; 6],
                        },
                        255,
                    )
                    .unwrap();
                let font = &out[out.len() - 6];
                for (name, id) in [
                    ("SESS_LABEL1", 1615),
                    ("SESS_LABEL2", 1879),
                    ("SESS_LABEL3", 1616),
                ] {
                    let mut expected = vec![Surface::solid(); 2];
                    ui::section_header(
                        &mut expected,
                        &panel.fonts[0].tab,
                        &panel.strings[id],
                        panel.bounds(name).unwrap(),
                    )
                    .unwrap();
                    assert!(
                        font.vertices
                            .windows(expected[0].vertices.len())
                            .any(|v| v == expected[0].vertices)
                    );
                }
                let status = if phase < 2 {
                    1759
                } else if host {
                    1757
                } else {
                    1758
                };
                let mut expected = vec![Surface::solid(); 2];
                ui::text_widget_style9(
                    &mut expected,
                    &panel.fonts[0].tab,
                    &panel.strings[status],
                    panel.bounds("SESS_INFO3").unwrap(),
                    0xffffffff,
                )
                .unwrap();
                let b = panel.bounds("SESS_INFO3").unwrap();
                let actual = font
                    .vertices
                    .iter()
                    .filter(|v| v[0] >= b[0] && v[0] <= b[2] && v[1] >= b[1] && v[1] <= b[3])
                    .copied()
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected[0].vertices, "host={host}, phase={phase}");
                let images = &out[1..out.len() - 6];
                let has_image = |b: [f32; 4]| {
                    images.iter().any(|s| {
                        s.vertices.first().is_some_and(|v| v[..2] == b[..2])
                            && s.vertices.get(2).is_some_and(|v| v[..2] == b[2..])
                    })
                };
                for (i, participant) in rows.iter().enumerate() {
                    for id in [2 * i, 2 * i + 1] {
                        assert_eq!(has_image(panel.action_bounds(id).unwrap()), i == local);
                    }
                    if participant.kind == Kind::Human {
                        let token = panel.bounds(&format!("TOKEN_P{}", i + 1)).unwrap();
                        let tint = panel.bounds(&format!("TINT_SLOT{}", i + 1)).unwrap();
                        assert!(has_image(token));
                        assert_eq!(token[0] + token[2], tint[0] + tint[2]);
                    }
                }
                let chat = panel.bounds("CHAT_BACKING").unwrap();
                assert!(
                    out[out.len() - 6..]
                        .iter()
                        .flat_map(|s| &s.vertices)
                        .all(|v| v[0] < chat[0]
                            || v[0] > chat[2]
                            || v[1] < chat[1]
                            || v[1] > chat[3])
                );
                let mut caret = Surface::solid();
                ui::text_entry(
                    &mut expected,
                    &mut caret,
                    &panel.fonts[0].tab,
                    &[],
                    panel.bounds("CHAT_TYPEAREA").unwrap(),
                    false,
                    true,
                )
                .unwrap();
                let actual = out[0]
                    .vertices
                    .iter()
                    .zip(&out[0].alpha)
                    .filter(|(_, a)| **a == 0xaf)
                    .map(|(v, _)| *v)
                    .collect::<Vec<_>>();
                assert_eq!(actual, caret.vertices);
                assert_eq!(actual.len(), 6);
            }
        }
        // The offline renderer keeps its passive chat input and AI token controls.
        let mut offline =
            Roster::new(b"Host", 0, std::array::from_fn(|_| b"AI".to_vec()), [0; 6]).unwrap();
        offline.toggle_ai(1).unwrap();
        let out = surfaces(&panel, offline.slots(), true, 0);
        assert!(!out[0].alpha.contains(&0xaf));
        for id in 0..4 {
            let b = panel.action_bounds(id).unwrap();
            assert!(
                out[1..out.len() - 6]
                    .iter()
                    .any(|s| s.vertices[0][..2] == b[..2])
            );
        }
    }
    #[test]
    fn lobby_start_label_and_frame_follow_offline_and_network_availability() {
        use openmt2001::network::lobby::{Request, Settings, Snapshot};
        let panel = synthetic_panel();
        let mut roster =
            Roster::new(b"Host", 0, std::array::from_fn(|_| b"AI".to_vec()), [0; 6]).unwrap();
        assert!(!roster.start_enabled());
        assert_start_style(&panel, roster.slots(), roster.start_enabled());
        roster.toggle_ai(1).unwrap();
        assert!(roster.start_enabled());
        assert_start_style(&panel, roster.slots(), roster.start_enabled());
        let mut snapshot = Snapshot::host(
            Settings {
                name: b"Session".to_vec(),
                level: 300001,
                parameter_bits: 2f32.to_bits(),
                difficulty: 0,
                map: b"default".to_vec(),
            },
            b"Host".to_vec(),
            0,
        )
        .unwrap();
        for phase in 0..3 {
            if phase == 1 {
                snapshot
                    .request(
                        1,
                        Request::Join {
                            name: b"Guest".to_vec(),
                            token: 1,
                        },
                    )
                    .unwrap();
            } else if phase == 2 {
                snapshot
                    .request(
                        1,
                        Request::Edit {
                            token: 1,
                            ready: true,
                        },
                    )
                    .unwrap();
            }
            assert_eq!(snapshot.start_enabled(), phase == 2);
            let rows = std::array::from_fn(|i| {
                let p = &snapshot.rows[i].player;
                Participant {
                    kind: if p.kind == openmt2001::network::RowKind::Human {
                        Kind::Human
                    } else {
                        Kind::Empty
                    },
                    token: usize::try_from(p.token).ok(),
                    name: p.name.clone(),
                    ready: p.ready,
                    difficulty: 0,
                }
            });
            for host in [true, false] {
                assert_start_style(&panel, &rows, host && snapshot.start_enabled());
            }
        }
    }
    #[test]
    fn lobby_has_one_dim_full_slot_frame_and_opaque_lower_interiors() {
        let mut panel = synthetic_panel();
        let mut rows = Roster::new(b"Host", 0, std::array::from_fn(|_| b"AI".to_vec()), [0; 6])
            .unwrap()
            .slots()
            .clone();
        rows[1] = Participant {
            kind: Kind::Human,
            token: Some(1),
            name: b"Guest".to_vec(),
            ready: true,
            difficulty: 0,
        };
        for local in [0, 1, 5] {
            for phase in 0..33 {
                let out = surfaces(&panel, &rows, false, local);
                let shapes = &out[0];
                // The green title gradient shares the marker colour; exclude it.
                let title = panel.bounds("TITLE_AREA").unwrap();
                let markers = shapes
                    .vertices
                    .as_chunks::<6>()
                    .0
                    .iter()
                    .filter(|q| q.iter().all(|v| v[3..6] == [0., 1., 0.]))
                    .filter(|q| !(q[0][..2] == title[..2] && q[2][..2] == title[2..]))
                    .collect::<Vec<_>>();
                assert_eq!(markers.len(), usize::from(local != 5));
                if local != 5 {
                    let bounds = panel.bounds(&format!("SELECTP{}", local + 1)).unwrap();
                    assert_eq!(markers[0][0][..2], bounds[..2]);
                    assert_eq!(markers[0][2][..2], bounds[2..]);
                    assert!(bounds[1] < panel.name_bounds(local).unwrap()[1]);
                    assert!(bounds[3] > panel.action_bounds(18 + local).unwrap()[3]);
                    assert_eq!(shapes.alpha[0], 255 - ((phase * 8) & 255) as u8);
                    // The very next primitive dims green with the native player backing.
                    assert_eq!(&shapes.alpha[6..12], &[0xbb; 6]);
                }
                for row in 1..=6 {
                    let b = panel.bounds(&format!("SLOT_{row}BACKING")).unwrap();
                    let at = shapes
                        .vertices
                        .as_chunks::<6>()
                        .0
                        .iter()
                        .position(|q| q[0][..2] == b[..2] && q[2][..2] == b[2..])
                        .unwrap()
                        * 6;
                    assert_eq!(&shapes.alpha[at..at + 6], &[255; 6]);
                    assert_eq!(shapes.vertices[at][3..6], [0., 10. / 255., 75. / 255.]);
                }
                panel.advance_highlight();
            }
            // Restart the pulse to check its exact wrapping alpha each time.
            panel.highlight = ui::LobbyHighlight::default();
        }
    }
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
