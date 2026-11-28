//! Private offline setup component. No menu route, roster construction or launch.
use crate::{
    load::{ascii, wrap_lines},
    ui::{self, Surface},
};
use openmt2001::{
    archive::ArchiveIndex,
    level_catalog::{Catalog, Labels, Level},
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    skirmish::{self, Display, Modifier, Options},
    texture::TextureSet,
    ui_mesh::Region,
};
use std::{fs, path::Path};

const PANEL: &str = "gamedata2\\brand_new_panels\\network_hostnew_v5.tm";
#[derive(Clone, Debug, PartialEq)]
pub struct Handoff {
    pub level_id: u32,
    pub directory: Vec<u8>,
    /// Host-resolved original map identity, never an arbitrary browser path.
    pub map: Vec<u8>,
    pub objective_kind: i32,
    pub objective_target: f32,
    pub difficulty: u32,
    pub session_name: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Ok,
    Back,
    Minus,
    Plus,
    Difficulty(u32),
    Row(usize),
    MapRow(usize),
    MapScroll { part: usize },
    Name,
    Scroll { description: bool, part: usize },
}
#[derive(Debug, PartialEq)]
pub enum Outcome {
    None,
    Back,
    Roster(Handoff),
}
/// Load the selected hub and dispatch SetObjective, as48d740(id,1) does.
/// Return its actual objective kind; do not run the city Level initializer.
pub struct State {
    pub visible: bool,
    pub catalog: Catalog,
    rows: Vec<usize>,
    selected: Option<usize>,
    kind: i32,
    modifier: Modifier,
    name: Vec<u8>,
    map: Option<Vec<u8>>,
    maps: Vec<Vec<u8>>,
    map_selected: Option<usize>,
    map_first: usize,
    pub name_focused: bool,
    first: usize,
    description_first: usize,
}
impl State {
    pub fn open(
        mut catalog: Catalog,
        name: &[u8],
        map: Option<Vec<u8>>,
        options: &Options,
        mut initialize: impl FnMut(&Level) -> Result<i32, String>,
    ) -> Result<Self, String> {
        let rows = skirmish::scenario_rows(&catalog);
        catalog.difficulty = 0;
        catalog.selected = None;
        let mut state = Self {
            visible: true,
            catalog,
            rows,
            selected: None,
            kind: 0,
            modifier: Modifier::default(),
            name: name
                .iter()
                .copied()
                .take_while(|&b| b != 0)
                .take(16)
                .collect(),
            map: None,
            maps: vec![],
            map_selected: None,
            map_first: 0,
            name_focused: false,
            first: 0,
            description_first: 0,
        };
        // Compatibility: old callers still supply one resolved identity.
        state.set_map(map);
        if !state.rows.is_empty() {
            state.select(0, options, &mut initialize)?;
        }
        Ok(state)
    }
    pub fn select(
        &mut self,
        row: usize,
        options: &Options,
        initialize: &mut impl FnMut(&Level) -> Result<i32, String>,
    ) -> Result<(), String> {
        let Some(&index) = self.rows.get(row) else {
            return Ok(());
        };
        // Native48d740 selects by associated ID, including first-match behavior.
        let id = self.catalog.levels[index].id();
        let selected = self
            .catalog
            .levels
            .iter()
            .position(|l| !l.single_player() && l.id() == id)
            .ok_or("Skirmish selection disappeared")?;
        let kind = initialize(&self.catalog.levels[selected])?;
        self.catalog.selected = Some(selected);
        self.selected = Some(row);
        self.kind = kind;
        self.modifier.select(options, kind);
        self.description_first = 0;
        Ok(())
    }
    /// Legacy single-map adapter; None/empty means genuinely no selectable map.
    pub fn set_map(&mut self, original_identity: Option<Vec<u8>>) {
        self.maps = original_identity
            .filter(|m| !m.is_empty())
            .into_iter()
            .collect();
        self.map_selected = (!self.maps.is_empty()).then_some(0);
        self.map = self.maps.first().cloned();
        self.map_first = 0;
    }
    /// Host-validated bare basenames in host order; no discovery, sorting or I/O.
    /// Preserve the current identity if retained, otherwise default/first. Invalid
    /// default/empty identity refuses atomically rather than inventing a map.
    pub fn set_maps(&mut self, rows: Vec<Vec<u8>>, default: Option<usize>) -> Result<(), String> {
        if default.is_some_and(|i| i >= rows.len()) || rows.iter().any(Vec::is_empty) {
            return Err("invalid map default or empty map identity".into());
        }
        let selected = self
            .map
            .as_ref()
            .and_then(|m| rows.iter().position(|r| r == m))
            .or(default)
            .or_else(|| (!rows.is_empty()).then_some(0));
        self.map = selected.map(|i| rows[i].clone());
        self.maps = rows;
        self.map_selected = selected;
        // The panel clamps this anchor to its actual viewport capacity. A newly
        // supplied selected row must be visible even if the default is near EOF.
        self.map_first = selected.unwrap_or(0);
        Ok(())
    }
    fn select_map(&mut self, row: usize) {
        if let Some(identity) = self.maps.get(row) {
            self.map = Some(identity.clone());
            self.map_selected = Some(row);
        }
    }
    pub fn text(&mut self, text: &str) {
        if !self.visible || !self.name_focused {
            return;
        }
        for b in text.bytes().filter(|b| (32..=126).contains(b)) {
            if self.name.len() == 16 {
                break;
            }
            self.name.push(b);
        }
    }
    pub fn backspace(&mut self) {
        if self.visible && self.name_focused {
            self.name.pop();
        }
    }
    fn map_ready(&self) -> bool {
        self.map_selected
            .and_then(|i| self.maps.get(i))
            .is_some_and(|m| !m.is_empty() && self.map.as_ref() == Some(m))
    }
    pub fn confirm(&mut self) -> Outcome {
        if !self.visible {
            return Outcome::None;
        }
        let Some(level) = self.catalog.selected_level() else {
            return Outcome::None;
        };
        // Native refuses the empty string, not whitespace. Missing map is a
        // modern host boundary; never fabricate an empty.lua map identity.
        let Some(map) = self
            .map_selected
            .and_then(|i| self.maps.get(i))
            .filter(|m| !m.is_empty() && self.map.as_ref() == Some(*m))
        else {
            return Outcome::None;
        };
        if self.name.is_empty() {
            return Outcome::None;
        }
        let request = Handoff {
            level_id: level.id(),
            directory: level.directory().to_vec(),
            map: map.clone(),
            objective_kind: self.kind,
            objective_target: self.modifier.target(),
            difficulty: self.catalog.difficulty,
            session_name: self.name.clone(),
        };
        Outcome::Roster(request)
    }
    pub fn activate(
        &mut self,
        action: Action,
        options: &Options,
        initialize: &mut impl FnMut(&Level) -> Result<i32, String>,
    ) -> Result<Outcome, String> {
        if !self.visible {
            return Ok(Outcome::None);
        }
        if action == Action::Ok && !self.map_ready() {
            return Ok(Outcome::None);
        }
        self.name_focused = action == Action::Name;
        match action {
            Action::Ok => return Ok(self.confirm()),
            Action::Back => {
                self.visible = false;
                return Ok(Outcome::Back);
            }
            Action::Minus | Action::Plus if self.modifier.controls_visible() => {
                self.modifier.adjust(options, action == Action::Plus)
            }
            Action::Difficulty(d) if d < 3 => self.catalog.difficulty = d,
            Action::Row(row) => self.select(row, options, initialize)?,
            Action::MapRow(row) => self.select_map(row),
            _ => (),
        }
        Ok(Outcome::None)
    }
}
struct Typeface {
    pages: Vec<Surface>,
    tab: Vec<u8>,
}
pub struct SetupPanel {
    headings: [usize; 2],
    base: Vec<Surface>,
    regions: Vec<Region>,
    generic: Vec<Surface>,
    fonts: Vec<Typeface>,
    strings: Vec<Vec<u8>>,
    options: Options,
    /// Native global5362c0 is a runtime theme color, supplied by the host.
    backing: u32,
}
fn inside(b: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3]
}
fn moved(value: usize, delta: i32, max: usize) -> usize {
    (value as i64 + i64::from(delta)).clamp(0, max as i64) as usize
}
impl SetupPanel {
    #[cfg(test)]
    pub(crate) fn inert(options: Options) -> Self {
        Self {
            base: vec![],
            regions: vec![],
            generic: vec![],
            fonts: vec![],
            strings: vec![],
            options,
            headings: [2226, 2228],
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
                .ok_or_else(|| format!("missing Skirmish asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("Skirmish asset range".into())
        };
        let mesh = CachedMesh::parse(payload(PANEL)?)?;
        let regions = openmt2001::ui_mesh::regions(&mesh, 0)?;
        let art = [
            "BLUE_TRIM",
            "BORDER01",
            "BORDER02",
            "BACKING",
            "BORDER03",
            "BORDER04",
            "BORDER05",
            "BACKING3",
        ];
        let parts = art
            .iter()
            .map(|name| {
                regions
                    .iter()
                    .find(|r| r.name == format!("NETWORK_HOST_{name}"))
                    .map(|r| r.part)
                    .ok_or_else(|| format!("missing host art {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (base, _) = ui::panel_base(index, archive, PANEL, &parts)?;
        let texture = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let fonts = [
            (1, "tahoma12bldnoantip.tab"),
            (2, "tahoma11noantip.tab"),
            (3, "tahoma10bldnoantip.tab"),
            (4, "coppgoth23p.tab"),
        ]
        .into_iter()
        .map(|(id, name)| {
            Ok(Typeface {
                pages: texture
                    .textures
                    .get(id * 2..id * 2 + 2)
                    .ok_or("missing font pages")?
                    .iter()
                    .map(Surface::new)
                    .collect(),
                tab: fs::read(installer.join("Standard/fonts").join(name))
                    .map_err(|e| e.to_string())?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
        let generic = TextureSet::parse(payload("gamedata2\\GenericTextures640\\textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let panel = Self {
            headings: [2226, 2228],
            base,
            regions,
            generic,
            fonts,
            strings: strings
                .iter()
                .map(|s| crate::scenarios::currency_text(s, strings))
                .collect::<Result<_, _>>()?,
            options: Options::from_pe(&PeImage::parse(executable)?)?,
            backing,
        };
        for name in [
            "TITLE_AREA",
            "SESS_TITLE_AREA",
            "SESS_INFO_TITLE",
            "BACKING2",
            "LABEL1",
            "LABEL2",
            "LABEL4",
            "LABEL7",
            "INFO1",
            "INFO4",
            "INFO7",
            "SESS_LIST",
            "OK_BUTTON",
            "BACK_BUTTON",
            "MOD01_MINUS",
            "MOD01_PLUS",
            "TOGGLE1",
            "TOGGLE2",
            "TOGGLE3",
            "EASY_LABEL",
            "MEDIUM_LABEL",
            "HARD_LABEL",
            "EASY_CUP",
            "MEDIUM_CUP",
            "HARD_CUP",
        ] {
            panel.bounds(name)?;
        }
        Ok(panel)
    }
    pub fn keyboard_actions(&self, state: &State) -> Vec<Action> {
        let mut actions = (0..state.rows.len()).map(Action::Row).collect::<Vec<_>>();
        actions.extend((0..state.maps.len()).map(Action::MapRow));
        actions.push(Action::Name);
        actions.extend((0..3).map(Action::Difficulty));
        if state.modifier.controls_visible() {
            actions.extend([Action::Minus, Action::Plus]);
        }
        if state.map_ready() {
            actions.push(Action::Ok);
        }
        actions.push(Action::Back);
        actions
    }
    pub fn keyboard_activate(
        &self,
        state: &mut State,
        action: Action,
        initialize: &mut impl FnMut(&Level) -> Result<i32, String>,
    ) -> Result<Outcome, String> {
        let outcome = state.activate(action, &self.options, initialize)?;
        if let Action::Row(row) = action {
            let capacity = self.capacity(false)?.max(1);
            if row < state.first {
                state.first = row;
            }
            if row >= state.first + capacity {
                state.first = row + 1 - capacity;
            }
        }
        if let Action::MapRow(row) = action {
            if row < state.maps.len() {
                let capacity = self.map_capacity()?.max(1);
                let first = self.map_first(state)?;
                state.map_first = if row < first {
                    row
                } else if row >= first + capacity {
                    row + 1 - capacity
                } else {
                    first
                };
            }
        }
        Ok(outcome)
    }
    pub fn network_mode(&mut self) {
        self.headings = [1683, 1692]; // Network Game / New Network Session.
    }
    pub fn options(&self) -> &Options {
        &self.options
    }
    fn bounds(&self, name: &str) -> Result<[f32; 4], String> {
        self.regions
            .iter()
            .find(|r| r.name == format!("NETWORK_HOST_{name}"))
            .map(|r| r.bounds)
            .ok_or_else(|| format!("missing host region {name}"))
    }
    fn label(&self, id: usize) -> Result<&[u8], String> {
        self.strings
            .get(id)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("missing host label {id}"))
    }
    fn encoded(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let map = self
            .strings
            .first()
            .ok_or("missing active language mapping")?;
        bytes
            .iter()
            .map(|b| {
                b.checked_sub(32)
                    .and_then(|i| map.get(i as usize))
                    .copied()
                    .ok_or("unmapped setup text".into())
            })
            .collect()
    }
    fn record_text(&self, level: &Level, title: bool) -> Result<Vec<u8>, String> {
        match level.labels() {
            Labels::Localized {
                title: t,
                description: d,
            } => Ok(self.label(if title { t } else { d } as usize)?.to_vec()),
            Labels::Literal {
                title: t,
                description: d,
            } => self.encoded(if title { &t } else { &d }),
        }
    }
    fn viewport(&self, description: bool) -> Result<[f32; 4], String> {
        let mut b = self.bounds(if description { "INFO7" } else { "SESS_LIST" })?;
        if !description {
            b[3] -= 130.;
        } // native conditions/map split
        b[2] -= 16.; // existing original generic vertical scrollbar width
        Ok(b)
    }
    fn map_title(&self) -> Result<[f32; 4], String> {
        let mut b = self.bounds("SESS_TITLE_AREA")?;
        b[1] += 241.;
        b[3] += 241.;
        Ok(b)
    }
    fn map_viewport(&self) -> Result<[f32; 4], String> {
        let mut b = self.map_title()?;
        b[1] += 25.;
        b[3] += 105.;
        b[2] -= 16.;
        Ok(b)
    }
    fn row_height(&self) -> f32 {
        let at = 4 + usize::from(b'W' - 32) * 44 + 36;
        i32::from_le_bytes(self.fonts[1].tab[at..at + 4].try_into().unwrap()).max(1) as f32
    }
    fn map_capacity(&self) -> Result<usize, String> {
        let b = self.map_viewport()?;
        Ok(((b[3] - b[1]) / self.row_height()).floor() as usize)
    }
    fn map_limit(&self, state: &State) -> Result<usize, String> {
        Ok(state.maps.len().saturating_sub(self.map_capacity()?))
    }
    fn map_first(&self, state: &State) -> Result<usize, String> {
        Ok(state.map_first.min(self.map_limit(state)?))
    }
    fn map_slider(&self) -> Result<[f32; 4], String> {
        let b = self.map_viewport()?;
        Ok([b[2], b[1], b[2] + 16., b[3]])
    }
    fn lines(&self, state: &State) -> Result<Vec<Vec<u8>>, String> {
        let Some(level) = state.catalog.selected_level() else {
            return Ok(vec![]);
        };
        let b = self.viewport(true)?;
        wrap_lines(
            &self.fonts[1].tab,
            &self.record_text(level, false)?,
            b[2] - b[0],
        )
    }
    fn capacity(&self, description: bool) -> Result<usize, String> {
        let b = self.viewport(description)?;
        // Font2's W height supplies the list row advance.
        let tab = &self.fonts[1].tab;
        let at = 4 + usize::from(b'W' - 32) * 44 + 36;
        let h = i32::from_le_bytes(tab[at..at + 4].try_into().unwrap()).max(1) as f32;
        Ok(((b[3] - b[1]) / h).floor() as usize)
    }
    fn limit(&self, state: &State, description: bool) -> Result<usize, String> {
        Ok(if description {
            self.lines(state)?.len()
        } else {
            state.rows.len()
        }
        .saturating_sub(self.capacity(description)?))
    }
    pub fn scroll(&self, state: &mut State, point: [f32; 2], delta: i32) -> Result<(), String> {
        if !state.visible {
            return Ok(());
        }
        for description in [false, true] {
            if inside(
                {
                    let mut b = self.viewport(description)?;
                    b[2] += 16.;
                    b
                },
                point,
            ) {
                let maximum = self.limit(state, description)?;
                let value = if description {
                    &mut state.description_first
                } else {
                    &mut state.first
                };
                *value = moved(*value, delta, maximum);
            }
        }
        let mut b = self.map_viewport()?;
        b[2] += 16.;
        if inside(b, point) {
            state.map_first = moved(self.map_first(state)?, delta, self.map_limit(state)?);
        }
        Ok(())
    }
    fn slider(&self, description: bool) -> Result<[f32; 4], String> {
        let b = self.viewport(description)?;
        Ok([b[2], b[1], b[2] + 16., b[3]])
    }
    pub fn hit(&self, state: &State, point: [f32; 2]) -> Result<Option<Action>, String> {
        if !state.visible {
            return Ok(None);
        }
        for (name, action) in [
            ("OK_BUTTON", Action::Ok),
            ("BACK_BUTTON", Action::Back),
            ("INFO1", Action::Name),
            ("TOGGLE1", Action::Difficulty(0)),
            ("TOGGLE2", Action::Difficulty(1)),
            ("TOGGLE3", Action::Difficulty(2)),
        ] {
            if action == Action::Ok && !state.map_ready() {
                continue;
            }
            if inside(self.bounds(name)?, point) {
                return Ok(Some(action));
            }
        }
        if state.modifier.controls_visible() {
            for (name, action) in [("MOD01_MINUS", Action::Minus), ("MOD01_PLUS", Action::Plus)] {
                if inside(self.bounds(name)?, point) {
                    return Ok(Some(action));
                }
            }
        }
        for description in [false, true] {
            if self.limit(state, description)? > 0 {
                let b = self.slider(description)?;
                for (part, bounds) in ui::slider_arrows(b, true)
                    .into_iter()
                    .chain([ui::slider_track(b, true)])
                    .enumerate()
                {
                    if inside(bounds, point) {
                        return Ok(Some(Action::Scroll { description, part }));
                    }
                }
            }
        }
        if self.map_limit(state)? > 0 {
            let b = self.map_slider()?;
            for (part, bounds) in ui::slider_arrows(b, true)
                .into_iter()
                .chain([ui::slider_track(b, true)])
                .enumerate()
            {
                if inside(bounds, point) {
                    return Ok(Some(Action::MapScroll { part }));
                }
            }
        }
        let b = self.map_viewport()?;
        if inside(b, point) {
            let row = self.map_first(state)? + ((point[1] - b[1]) / self.row_height()) as usize;
            if row < state.maps.len()
                && point[1] < b[1] + self.map_capacity()? as f32 * self.row_height()
            {
                return Ok(Some(Action::MapRow(row)));
            }
        }
        let b = self.viewport(false)?;
        if inside(b, point) {
            let h = (self.fonts[1].tab[4 + usize::from(b'W' - 32) * 44 + 36..][..4])
                .try_into()
                .unwrap();
            let row =
                state.first + ((point[1] - b[1]) / i32::from_le_bytes(h).max(1) as f32) as usize;
            if row < state.rows.len()
                && point[1] < b[1] + self.capacity(false)? as f32 * self.row_height()
            {
                return Ok(Some(Action::Row(row)));
            }
        }
        Ok(None)
    }
    pub fn activate(
        &self,
        state: &mut State,
        action: Action,
        point: [f32; 2],
        initialize: &mut impl FnMut(&Level) -> Result<i32, String>,
    ) -> Result<Outcome, String> {
        if !state.visible {
            return Ok(Outcome::None);
        }
        if let Action::MapScroll { part } = action {
            let max = self.map_limit(state)?;
            let b = self.map_slider()?;
            let first = self.map_first(state)?;
            state.map_first = match part {
                0 => moved(first, -1, max),
                1 => moved(first, 1, max),
                2 => ui::slider_selection(ui::slider_track(b, true), point[1], max as i32, true)
                    as usize,
                _ => first,
            };
            return Ok(Outcome::None);
        }
        if let Action::Scroll { description, part } = action {
            let max = self.limit(state, description)?;
            let b = self.slider(description)?;
            let value = if description {
                &mut state.description_first
            } else {
                &mut state.first
            };
            *value = match part {
                0 => moved(*value, -1, max),
                1 => moved(*value, 1, max),
                2 => ui::slider_selection(ui::slider_track(b, true), point[1], max as i32, true)
                    as usize,
                _ => *value,
            };
            return Ok(Outcome::None);
        }
        state.activate(action, &self.options, initialize)
    }
    fn image(&self, out: &mut Vec<Surface>, id: usize, b: [f32; 4]) -> Result<(), String> {
        let mut s = self
            .generic
            .get(id)
            .ok_or("Skirmish generic texture")?
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
    fn surfaces(
        &self,
        state: &State,
        pointer: Option<[f32; 2]>,
        pressed: bool,
        keyboard: Option<Action>,
    ) -> Result<Vec<Surface>, String> {
        let mut out = if state.visible {
            self.base.clone()
        } else {
            vec![]
        };
        if state.visible {
            let selected = keyboard.or(pointer.map(|p| self.hit(state, p)).transpose()?.flatten());
            let mut shapes = Surface::solid();
            let mut images = vec![];
            let mut pages = self
                .fonts
                .iter()
                .map(|f| f.pages.clone())
                .collect::<Vec<_>>();
            for name in ["BACKING2", "INFO7"] {
                shapes.rect(self.bounds(name)?, [self.backing; 4]);
            }
            shapes.rect(
                self.map_title()?,
                [0xff909090, 0xff909090, 0x7f909090, 0x7f909090],
            );
            let mut conditions = self.viewport(false)?;
            conditions[2] += 16.;
            shapes.rect(conditions, [self.backing; 4]);
            // Native draw's lower backing uses the original list extent,
            // distinct from the inner map row viewport synthesized from title.
            let mut map_backing = self.bounds("SESS_LIST")?;
            map_backing[1] += 241.;
            shapes.rect(map_backing, [self.backing; 4]);
            ui::text_widget_style9(
                &mut pages[0],
                &self.fonts[0].tab,
                self.label(2237)?,
                self.map_title()?,
                0xffffffff,
            )?;
            for name in ["SESS_TITLE_AREA", "SESS_INFO_TITLE"] {
                shapes.rect(
                    self.bounds(name)?,
                    [0xff909090, 0xff909090, 0x7f909090, 0x7f909090],
                );
            }
            shapes.rect(
                self.bounds("TITLE_AREA")?,
                [0xff00ff00, 0xff00ff00, 0x8000ff00, 0x8000ff00],
            );
            for name in ["LABEL1", "LABEL2", "LABEL4", "LABEL7"] {
                shapes.rect(self.bounds(name)?, [0xff01762f; 4]);
            }
            for name in ["INFO1", "INFO4"] {
                shapes.rect(self.bounds(name)?, [0xff1b254b; 4]);
            }
            for (font, name, id) in [
                (3, "TITLE_AREA", self.headings[0]),
                (0, "SESS_TITLE_AREA", self.headings[1]),
                (0, "LABEL1", 1880),
                (0, "LABEL2", 1879),
                (0, "LABEL7", 1883),
            ] {
                ui::text_widget_style9(
                    &mut pages[font],
                    &self.fonts[font].tab,
                    self.label(id)?,
                    self.bounds(name)?,
                    0xffffffff,
                )?;
            }
            if let Some(level) = state.catalog.selected_level() {
                ui::text_widget_style9(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    &self.record_text(level, true)?,
                    self.bounds("SESS_INFO_TITLE")?,
                    0xffffffff,
                )?;
            }
            ui::text_widget_style3(
                &mut pages[2],
                &self.fonts[2].tab,
                &self.encoded(&state.name)?,
                self.bounds("INFO1")?,
                0xffffffff,
            )?;
            for (i, name) in ["EASY", "MEDIUM", "HARD"].iter().enumerate() {
                self.image(&mut images, 293 + i, self.bounds(&format!("{name}_CUP"))?)?;
                self.image(
                    &mut images,
                    if state.catalog.difficulty == i as u32 {
                        174
                    } else {
                        175
                    },
                    self.bounds(&format!("TOGGLE{}", i + 1))?,
                )?;
                ui::text_widget_style3(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    self.label(1876 - i)?,
                    self.bounds(&format!("{name}_LABEL"))?,
                    0xffffffff,
                )?;
            }
            if let Some(label) = state.modifier.label(&self.options) {
                ui::text_widget_style9(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    self.label(label as usize)?,
                    self.bounds("LABEL4")?,
                    0xffffffff,
                )?;
            }
            let value = match state.modifier.display(&self.options) {
                Display::Hidden => vec![],
                Display::Integer(n) => self.encoded(n.to_string().as_bytes())?,
                Display::Currency(n) => ui::integer_money(
                    n as i32,
                    self.label(2243)?,
                    self.label(2244)?.first() != Some(&0x4c),
                ),
            };
            ui::text_widget_style9(
                &mut pages[2],
                &self.fonts[2].tab,
                &value,
                self.bounds("INFO4")?,
                0xffffffff,
            )?;
            if state.modifier.controls_visible() {
                for (name, action, id) in [
                    ("MOD01_MINUS", Action::Minus, 118),
                    ("MOD01_PLUS", Action::Plus, 122),
                ] {
                    self.image(
                        &mut images,
                        id + if selected == Some(action) {
                            if pressed { 2 } else { 1 }
                        } else {
                            0
                        },
                        self.bounds(name)?,
                    )?;
                }
            }
            let tab = &self.fonts[1].tab;
            let at = 4 + usize::from(b'W' - 32) * 44 + 36;
            let height = i32::from_le_bytes(tab[at..at + 4].try_into().unwrap()).max(1) as f32;
            for description in [false, true] {
                let b = self.viewport(description)?;
                let first = if description {
                    state.description_first
                } else {
                    state.first
                };
                let lines = if description {
                    self.lines(state)?
                } else {
                    state
                        .rows
                        .iter()
                        .map(|&i| self.record_text(&state.catalog.levels[i], true))
                        .collect::<Result<Vec<_>, _>>()?
                };
                for (i, text) in lines
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take(self.capacity(description)?)
                {
                    let row = [
                        b[0],
                        b[1] + (i - first) as f32 * height,
                        b[2],
                        b[1] + (i - first + 1) as f32 * height,
                    ];
                    if !description && state.selected == Some(i) {
                        shapes.rect(row, [0xff008ed0; 4]);
                    }
                    ui::text_widget_style0(&mut pages[1], tab, text, row, 0xffffffff)?;
                }
                let maximum = self.limit(state, description)?;
                ui::Slider {bounds:self.slider(description)?,vertical:true,value:first as f32,
                    range:[0.,maximum as f32],pointer,pressed:if pressed && matches!(selected,Some(Action::Scroll {description:d,..}) if d==description) {pointer}else{None}}
                    .paint(&mut shapes,&mut images,&self.generic);
            }
            let b = self.map_viewport()?;
            let first = self.map_first(state)?;
            for (i, identity) in state
                .maps
                .iter()
                .enumerate()
                .skip(first)
                .take(self.map_capacity()?)
            {
                let row = [
                    b[0],
                    b[1] + (i - first) as f32 * height,
                    b[2],
                    b[1] + (i - first + 1) as f32 * height,
                ];
                if state.map_selected == Some(i) {
                    shapes.rect(row, [0xff008ed0; 4]);
                }
                ui::text_widget_style0(
                    &mut pages[1],
                    tab,
                    &self.encoded(identity)?,
                    row,
                    0xffffffff,
                )?;
            }
            let maximum = self.map_limit(state)?;
            ui::Slider {
                bounds: self.map_slider()?,
                vertical: true,
                value: first as f32,
                range: [0., maximum as f32],
                pointer,
                pressed: if pressed && matches!(selected, Some(Action::MapScroll { .. })) {
                    pointer
                } else {
                    None
                },
            }
            .paint(&mut shapes, &mut images, &self.generic);
            for (name, action, id, color) in [
                ("OK_BUTTON", Action::Ok, 1435, [80, 148, 34]),
                ("BACK_BUTTON", Action::Back, 1439, [175, 6, 6]),
            ] {
                let b = self.bounds(name)?;
                let disabled = action == Action::Ok && !state.map_ready();
                ui::button(
                    &mut shapes,
                    b,
                    color,
                    selected == Some(action),
                    pressed && selected == Some(action),
                    disabled,
                )?;
                ui::text_widget_style9(
                    &mut pages[0],
                    &self.fonts[0].tab,
                    self.label(id)?,
                    b,
                    if disabled { 0xff909090 } else { 0xffffffff },
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
    pub fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        pointer: Option<[f32; 2]>,
        pressed: bool,
        keyboard: Option<Action>,
    ) -> Result<PreparedScene, String> {
        let out = self.surfaces(state, pointer, pressed, keyboard)?;
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
    fn original() -> (Options, Catalog) {
        let exe = fs::read(std::env::var_os("OPENMT2001_EXE").expect("original PE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let scripts = std::env::var_os("OPENMT2001_SCRIPTS").expect("original scripts root");
        (
            Options::from_pe(&image).unwrap(),
            crate::campaign_store::load_catalog(&image, Path::new(&scripts)).unwrap(),
        )
    }
    #[test]
    #[ignore = "player-supplied original PE and scripts; integration setup required"]
    fn original_catalog_setup_handoff_and_refusals() {
        let (options, catalog) = original();
        let rows = skirmish::scenario_rows(&catalog);
        assert!(!rows.is_empty());
        assert!(rows.windows(2).all(|w| w[0] < w[1]));
        assert!(rows.iter().all(|&i| !catalog.levels[i].single_player()));
        let net1 = rows
            .iter()
            .position(|&i| catalog.levels[i].id() == 300001)
            .expect("supplied NET1");
        let map = b"empty.lua".to_vec();
        let mut state =
            State::open(catalog, b"Justin2", Some(map.clone()), &options, |_| Ok(6)).unwrap();
        state.select(net1, &options, &mut |_| Ok(6)).unwrap();
        assert_eq!(state.catalog.selected_level().unwrap().directory(), b"net1");
        assert_eq!(state.catalog.difficulty, 0);
        let rule = options.rules().iter().find(|r| r.kind == 6).unwrap();
        assert_eq!(
            state.modifier.value(),
            rule.initial.clamp(rule.minimum, rule.maximum)
        );
        state
            .activate(Action::Plus, &options, &mut |_| Ok(6))
            .unwrap();
        assert_eq!(state.modifier.value(), rule.initial + rule.step);
        state
            .activate(Action::Difficulty(2), &options, &mut |_| Ok(6))
            .unwrap();
        state.name.clear();
        assert_eq!(state.confirm(), Outcome::None);
        state.name_focused = true;
        state.text("A1234567890123456789");
        assert_eq!(state.name.len(), 16);
        state.backspace();
        assert_eq!(state.name.len(), 15);
        state.set_map(None);
        assert_eq!(state.confirm(), Outcome::None);
        state.set_map(Some(map.clone()));
        let Outcome::Roster(request) = state.confirm() else {
            panic!("expected roster handoff")
        };
        assert_eq!(request.level_id, 300001);
        assert_eq!(request.directory, b"net1");
        assert_eq!(request.map, map);
        assert_eq!(request.difficulty, 2);
        assert_eq!(request.objective_kind, 6);
        assert_eq!(request.objective_target, (rule.initial + rule.step) as f32);
        assert!(state.visible); // Route hides setup only after roster construction commits.
        let mut empty = State::open(
            Catalog::default(),
            b" ",
            Some(b"empty.lua".to_vec()),
            &options,
            |_| panic!("empty list must not initialize"),
        )
        .unwrap();
        assert_eq!(empty.confirm(), Outcome::None);
        assert_eq!(
            empty
                .activate(Action::Back, &options, &mut |_| Ok(0))
                .unwrap(),
            Outcome::Back
        );
    }
    #[test]
    #[ignore = "player-supplied original archive, installer, PE, language and scripts"]
    fn original_v5_regions_labels_and_scroll_geometry() {
        let read = |name| fs::read(std::env::var_os(name).expect(name)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let offsets = read("OPENMT2001_LANGUAGE_OFF");
        let glyphs = read("OPENMT2001_LANGUAGE_BIN");
        let mut at = 0;
        let strings = offsets
            .chunks_exact(2)
            .map(|b| {
                let start = at;
                at += u16::from_le_bytes(b.try_into().unwrap()) as usize;
                &glyphs[start..at]
            })
            .collect::<Vec<_>>();
        let installer = std::path::PathBuf::from(std::env::var_os("OPENMT2001_INSTALLER").unwrap());
        let localized = crate::tooltip::localized_strings(&installer, &strings).unwrap();
        let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_eq!(strings[2243], crate::load::ascii("$"));
        let panel = SetupPanel::new(
            &index,
            &archive,
            Path::new(&std::env::var_os("OPENMT2001_INSTALLER").unwrap()),
            &strings,
            &read("OPENMT2001_EXE"),
            0xbb000a28,
        )
        .unwrap();
        assert_eq!(panel.bounds("TITLE_AREA").unwrap(), [25., 25., 615., 56.]);
        assert_eq!(
            panel.bounds("TOGGLE1").unwrap()[2] - panel.bounds("TOGGLE1").unwrap()[0],
            16.
        );
        assert_eq!(panel.label(2226).unwrap(), ascii("Skirmish Mode"));
        let (options, catalog) = original();
        let mut state = State::open(
            catalog,
            b"Justin2",
            Some(b"empty.lua".to_vec()),
            &options,
            |_| Ok(3),
        )
        .unwrap();
        for &i in &state.rows {
            assert!(
                !panel
                    .record_text(&state.catalog.levels[i], true)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !panel
                    .record_text(&state.catalog.levels[i], false)
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(panel.capacity(false).unwrap() >= state.rows.len());
        // Native dynamic rectangles derive from existing v5 regions, never INFO3/MOD02.
        let original_list = panel.bounds("SESS_LIST").unwrap();
        assert_eq!(
            panel.viewport(false).unwrap(),
            [
                original_list[0],
                original_list[1],
                original_list[2] - 16.,
                original_list[3] - 130.
            ]
        );
        let mut title = panel.bounds("SESS_TITLE_AREA").unwrap();
        title[1] += 241.;
        title[3] += 241.;
        assert_eq!(panel.map_title().unwrap(), title);
        assert_eq!(
            panel.map_viewport().unwrap(),
            [title[0], title[1] + 25., title[2] - 16., title[3] + 105.]
        );
        assert!(!panel.label(2237).unwrap().is_empty());
        assert!(panel.bounds("INFO3").is_err());
        assert!(panel.bounds("MOD02_MINUS").is_err());
        let conditions_before = (
            state.selected,
            state.catalog.selected,
            state.catalog.difficulty,
            state.modifier.value(),
            state.name.clone(),
        );
        // UI-only trusted fixture names, not discovery/start acceptance of these maps.
        let maps = (0..25)
            .map(|i| {
                if i == 0 {
                    b"empty.lua".to_vec()
                } else {
                    format!("fixture{i:02}.lua").into_bytes()
                }
            })
            .collect::<Vec<_>>();
        state.set_maps(maps.clone(), Some(24)).unwrap();
        assert_eq!(
            state.map_selected,
            Some(0),
            "current map wins over requested default"
        );
        let actions = panel.keyboard_actions(&state);
        assert!(actions.contains(&Action::MapRow(24)) && actions.contains(&Action::Row(0)));
        let mv = panel.map_viewport().unwrap();
        let h = panel.row_height();
        let first_point = [mv[0] + 2., mv[1] + h * 0.5];
        assert_eq!(
            panel.hit(&state, first_point).unwrap(),
            Some(Action::MapRow(0))
        );
        assert_eq!(
            panel
                .activate(&mut state, Action::MapRow(1), first_point, &mut |_| panic!(
                    "map selection must not initialize a level"
                ))
                .unwrap(),
            Outcome::None
        );
        assert_eq!(state.map_selected, Some(1));
        assert_eq!(
            (
                state.selected,
                state.catalog.selected,
                state.catalog.difficulty,
                state.modifier.value(),
                state.name.clone()
            ),
            conditions_before
        );
        assert!(matches!(state.confirm(),Outcome::Roster(ref h) if h.map==maps[1]));
        // Actual surface builder used by draw must move exactly one map highlight
        // with selection, independent of condition highlight and keyboard focus.
        let map_highlights = |state: &State| {
            panel
                .surfaces(state, None, false, None)
                .unwrap()
                .iter()
                .flat_map(|s| s.vertices.chunks_exact(6))
                .filter(|q| {
                    q.iter().all(|v| v[3..6] == [0., 142. / 255., 208. / 255.])
                        && q[0][1] >= mv[1]
                        && q[0][1] < mv[3]
                })
                .map(|q| [q[0][0], q[0][1], q[2][0], q[2][1]])
                .collect::<Vec<_>>()
        };
        assert_eq!(
            map_highlights(&state),
            vec![[mv[0], mv[1] + h, mv[2], mv[1] + 2. * h]]
        );
        panel
            .keyboard_activate(&mut state, Action::MapRow(24), &mut |_| {
                panic!("keyboard map row must not initialize level")
            })
            .unwrap();
        assert_eq!(
            panel.map_first(&state).unwrap(),
            25 - panel.map_capacity().unwrap()
        );
        let last_y = mv[1] + (panel.map_capacity().unwrap() - 1) as f32 * h;
        assert_eq!(
            map_highlights(&state),
            vec![[mv[0], last_y, mv[2], last_y + h]]
        );
        assert!(matches!(state.confirm(),Outcome::Roster(ref h) if h.map==maps[24]));
        let condition_first = state.first;
        panel.scroll(&mut state, first_point, -1).unwrap();
        assert_eq!(state.first, condition_first);
        let map_first = state.map_first;
        panel
            .activate(
                &mut state,
                Action::MapScroll { part: 0 },
                first_point,
                &mut |_| Ok(3),
            )
            .unwrap();
        assert_eq!(state.map_first, map_first.saturating_sub(1));
        let slider = panel.map_slider().unwrap();
        let track = ui::slider_track(slider, true);
        let point = [(track[0] + track[2]) * 0.5, track[3]];
        panel
            .activate(
                &mut state,
                Action::MapScroll { part: 2 },
                point,
                &mut |_| Ok(3),
            )
            .unwrap();
        assert_eq!(state.map_first, panel.map_limit(&state).unwrap());
        assert_eq!(state.first, condition_first);
        // Controlled long condition list proves its independent scroll state.
        let actual_rows = state.rows.clone();
        let index = state.rows[0];
        state.rows.resize(panel.capacity(false).unwrap() + 4, index);
        let condition_point = panel.viewport(false).unwrap();
        let map_first = state.map_first;
        panel
            .scroll(
                &mut state,
                [condition_point[0] + 2., condition_point[1] + 2.],
                1,
            )
            .unwrap();
        assert_eq!(state.first, condition_first + 1);
        assert_eq!(state.map_first, map_first);
        state.rows = actual_rows;
        state.first = 0;
        state.set_maps(vec![], None).unwrap();
        assert_eq!(state.confirm(), Outcome::None);
        assert!(map_highlights(&state).is_empty());
        assert!(!panel.keyboard_actions(&state).contains(&Action::Ok));
        assert!(panel.keyboard_actions(&state).contains(&Action::Back));
        let ok = panel.bounds("OK_BUTTON").unwrap();
        let ok_point = [(ok[0] + ok[2]) * 0.5, (ok[1] + ok[3]) * 0.5];
        assert_eq!(panel.hit(&state, ok_point).unwrap(), None);
        state.name_focused = true;
        assert_eq!(
            panel
                .keyboard_activate(&mut state, Action::Ok, &mut |_| Ok(3))
                .unwrap(),
            Outcome::None
        );
        assert!(
            state.name_focused,
            "empty-map refusal must not steal name focus"
        );
        let empty_scene = panel.surfaces(&state, None, false, None).unwrap();
        assert!(
            !empty_scene.is_empty(),
            "empty catalog still renders title/list/Back"
        );
        assert!(!matches!(
            panel.hit(&state, first_point).unwrap(),
            Some(Action::MapRow(_))
        ));
        state.set_maps(maps, Some(0)).unwrap();
        let b = panel.bounds("MOD01_PLUS").unwrap();
        assert_eq!(
            panel
                .hit(&state, [(b[0] + b[2]) / 2., (b[1] + b[3]) / 2.])
                .unwrap(),
            Some(Action::Plus)
        );
        assert_eq!(moved(0, -1, 5), 0);
        assert_eq!(moved(0, 100, 5), 5);
        #[cfg(feature = "capture")]
        {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let scene = panel.draw(&state, &renderer, None, false, None).unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                &scene,
                "work/frontend/skirmish-setup.png",
            );
            // Component captures: synthetic trusted fixture names, actual supplied
            // panel/fonts/colors/renderer. No map startup or compatibility claim.
            panel
                .keyboard_activate(&mut state, Action::MapRow(24), &mut |_| Ok(3))
                .unwrap();
            let selected_scene = panel
                .draw(&state, &renderer, None, false, Some(Action::MapRow(24)))
                .unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                &selected_scene,
                "work/frontend/skirmish-map-selected.png",
            );
            state.set_maps(vec![], None).unwrap();
            let empty_scene = panel.draw(&state, &renderer, None, false, None).unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                &empty_scene,
                "work/frontend/skirmish-map-empty.png",
            );
        }
    }
}

#[cfg(test)]
mod map_catalog_state_tests {
    use super::*;
    fn state() -> State {
        State {
            visible: true,
            catalog: Catalog::default(),
            rows: vec![],
            selected: None,
            kind: 0,
            modifier: Modifier::default(),
            name: b"Player".to_vec(),
            map: None,
            maps: vec![],
            map_selected: None,
            map_first: 0,
            name_focused: false,
            first: 7,
            description_first: 9,
        }
    }
    #[test]
    fn host_map_replacement_retains_identity_defaults_atomically_and_never_fabricates() {
        let mut s = state();
        let a = b"empty.lua".to_vec();
        let b = b"apartments2.lua".to_vec();
        let c = b"goodsparsemap.lua".to_vec();
        s.set_maps(vec![a.clone(), b.clone()], Some(1)).unwrap();
        assert_eq!((s.map_selected, s.map.clone()), (Some(1), Some(b.clone())));
        s.set_maps(vec![b.clone(), a.clone()], Some(1)).unwrap();
        assert_eq!((s.map_selected, s.map.clone()), (Some(0), Some(b.clone())));
        s.set_maps(vec![a.clone(), c.clone()], Some(1)).unwrap();
        assert_eq!(s.map, Some(c));
        s.set_maps(vec![a.clone(), b.clone()], None).unwrap();
        assert_eq!(s.map, Some(a.clone()));
        let before = (s.maps.clone(), s.map_selected, s.map.clone(), s.map_first);
        assert!(s.set_maps(vec![b.clone()], Some(1)).is_err());
        assert_eq!(
            (s.maps.clone(), s.map_selected, s.map.clone(), s.map_first),
            before
        );
        assert!(s.set_maps(vec![vec![]], None).is_err());
        assert_eq!(
            (s.maps.clone(), s.map_selected, s.map.clone(), s.map_first),
            before
        );
        s.select_map(99);
        assert_eq!(s.map, Some(a));
        s.set_maps(vec![], None).unwrap();
        assert!(s.maps.is_empty() && s.map_selected.is_none() && s.map.is_none());
        assert_eq!(s.confirm(), Outcome::None);
        assert_eq!((s.first, s.description_first), (7, 9));
        s.set_map(Some(b.clone()));
        assert_eq!(s.maps, vec![b]);
        assert_eq!(s.map_selected, Some(0));
        s.set_map(None);
        assert!(s.maps.is_empty() && s.map.is_none());
        s.set_map(Some(vec![]));
        assert!(s.maps.is_empty() && s.map.is_none());
    }
}
