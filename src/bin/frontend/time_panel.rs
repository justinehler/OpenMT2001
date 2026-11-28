//! Original top-right clock bounds and retained time controls in the live city.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

const PANEL: &str = "gamedata2/brand_new_panels/top_right_panel_v2.tm";
const VIEW: [f32; 16] = [
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

fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0] && point[0] <= rect[2] && point[1] >= rect[1] && point[1] <= rect[3]
}

/// Original425120 and4254f0 use supplied textured overlays at full bounds.
fn bottom_controls(manager: [f32; 4], speed: [f32; 4], fast: bool) -> [(usize, [f32; 4]); 4] {
    bottom_controls_with_state(manager, speed, fast, None, None)
}
fn bottom_controls_with_state(
    manager: [f32; 4],
    speed: [f32; 4],
    fast: bool,
    hover: Option<Button>,
    pressed: Option<Button>,
) -> [(usize, [f32; 4]); 4] {
    let speed_texture = if pressed == Some(Button::Speed) {
        if fast { 191 } else { 193 }
    } else if hover == Some(Button::Speed) {
        if fast { 194 } else { 192 }
    } else if fast {
        193
    } else {
        191
    };
    let manager_texture = if pressed == Some(Button::Manager) {
        193
    } else if hover == Some(Button::Manager) {
        192
    } else {
        191
    };
    [
        (speed_texture, speed),
        (303, speed),
        (manager_texture, manager),
        (65, manager),
    ]
}

fn options_texture(hover: Option<Button>, pressed: Option<Button>) -> usize {
    if pressed == Some(Button::Options) {
        84
    } else if hover == Some(Button::Options) {
        83
    } else {
        82
    }
}

/// Original42881d..428b1c. Each explicit f32 conversion is a native FST;
/// the remaining arithmetic and trig retain precision until the vertex store.
fn clock_hands(bounds: [f32; 4], hour: u32, fraction: u32) -> [[[f32; 2]; 4]; 2] {
    let store = |v: f64| f64::from(v as f32);
    let [left, top, right, bottom] = bounds.map(f64::from);
    let cx = store((left + right) * 0.5);
    let cy = store((top + bottom) * 0.5);
    let radius = store(cx - left);
    let revolution = f64::from(f32::from_bits(0x40c9_0fda)); // Original525200.
    let minute = store(f64::from(fraction) * revolution * 0.001_599_999_959_580_600_3);
    let angle = (f64::from(hour) * revolution + minute) * 0.083_333_335_816_860_2;
    let (sine, cosine) = (angle.sin(), store(angle.cos()));
    let cw = store(cosine * 2.);
    let sw = store(sine * 2.);
    let front = radius * 0.199_367_083_192_463_9;
    let back = store(radius * 0.75);
    let sf = store(sine * front);
    let cb = store(cosine * back);
    let x = store(cx - cw);
    let y = store(cy - sw);
    let hours = [
        [x - sf, y + cosine * front],
        [cx + cw - sf, sw + cosine * front + cy],
        [cw + sine * back + cx, sw + cy - cb],
        [x + sine * back, y - cb],
    ]
    .map(|p| p.map(|v| v as f32));
    let (sine, cosine) = (minute.sin(), minute.cos());
    let cw = store(cosine * 1.5);
    let sw = store(sine * 1.5);
    let front = radius * 0.298_701_316_118_240_36;
    let sf = store(sine * front);
    let x = store(cx - cw);
    let y = store(cy - sw);
    let minutes = [
        [x - sf, y + cosine * front],
        [cw + cx - sf, sw + cosine * front + cy],
        [sine * radius + cw + cx, sw + cy - cosine * radius],
        [sine * radius + x, y - cosine * radius],
    ]
    .map(|p| p.map(|v| v as f32));
    [hours, minutes]
}

fn clock_text(hour: u32, fraction: u32, year: u32, am: &[u8], pm: &[u8]) -> Vec<u8> {
    let hour12 = if hour > 12 {
        hour - 12
    } else if hour == 0 {
        12
    } else {
        hour
    };
    let mut text = format!("{hour12}:{:02}", fraction * 60 / 625)
        .bytes()
        .map(|c| c - 32)
        .collect::<Vec<_>>();
    text.extend_from_slice(if hour >= 12 { pm } else { am });
    text.push(0);
    text.extend(year.to_string().bytes().map(|c| c - 32));
    text
}

/// Native4406a0 alignment4 includes +198 in the centered X expression.
/// Both rectangle origin components truncate before adding integer offsets.
fn text_quads(
    tab: &[u8],
    value: &[u8],
    bounds: [f32; 4],
    center: bool,
) -> Result<Vec<openmt2001::font::GlyphQuad>, String> {
    text_quads_with_offset(tab, value, bounds, center, 4)
}
fn text_quads_with_offset(
    tab: &[u8],
    value: &[u8],
    bounds: [f32; 4],
    center: bool,
    x_offset: i32,
) -> Result<Vec<openmt2001::font::GlyphQuad>, String> {
    let font = openmt2001::font::Font::parse(tab)?;
    let (offsets, value) = crate::prompt::wrap_with_gap(
        tab,
        value,
        (f64::from(bounds[2]) - f64::from(bounds[0]) - f64::from(x_offset) * 2.).trunc() as i32,
        -1,
    )?;
    // Native4e7f60 measures ASCII W for the retained line height.
    let height = i32::from_le_bytes(
        tab[4 + usize::from(b'W' - 32) * 44 + 36..][..4]
            .try_into()
            .unwrap(),
    );
    let top = bounds[1].trunc()
        + ((f64::from(bounds[3])
            - f64::from(bounds[1])
            - (offsets.len() - 1) as f64 * f64::from(height))
            * 0.5)
            .trunc() as f32;
    let mut quads = vec![];
    for (line, range) in offsets.windows(2).enumerate() {
        let row = &value[range[0]..range[1].min(value.len())];
        let row = row.strip_suffix(&[0]).unwrap_or(row);
        let width = ui::text_width(tab, row, 1.)?;
        let left = bounds[0].trunc()
            + if center {
                ((f64::from(bounds[2]) - f64::from(bounds[0]) - f64::from(width)
                    + f64::from(x_offset))
                    * 0.5
                    + 0.5)
                    .trunc() as f32
            } else {
                (x_offset + 2) as f32
            };
        quads.extend(font.layout(
            row,
            openmt2001::font::Layout {
                origin: [left, top + line as f32 * height as f32],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )?);
    }
    Ok(quads)
}

/// App-owned native560a1c/560a20 globals, retained across city loads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Runtime {
    pub amount: f32,
    pub remaining: i32,
    /// Normal58fb40 and auction565bb0 retain independent show/update caches.
    /// Their model-bind initializer resets those caches on each city load.
    pub widgets: [Widget; 2],
    pub chance_visuals: [crate::card_panel::EntryVisual; 2],
}
/// Original565bb0 widget state, initialized independently of app globals.
#[derive(Clone, Copy, Debug)]
pub struct Widget {
    pub last_amount: f32,
    pub cash_icon_phase: i32,
    pub visible: bool,
    text_amount: Option<f32>,
}
impl Default for Widget {
    fn default() -> Self {
        Self {
            last_amount: -1.,
            cash_icon_phase: 0,
            visible: false,
            text_amount: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DisplayState {
    pub cash: Option<f32>,
    pub cash_icon_phase: i32,
    pub visible: bool,
}
impl Widget {
    pub fn display(&self) -> DisplayState {
        DisplayState {
            cash: self.text_amount,
            visible: self.visible,
            cash_icon_phase: self.cash_icon_phase,
        }
    }
}
impl Runtime {
    ///43b770 bind tail invokes4281e0 separately for both Time instances.
    /// It clears text, comparison9ac, phase9a0, visibility345 and reset9a4;
    /// shared560a1c/560a20 and Chance blink9a8 are not written.
    pub fn bind_panels(&mut self) {
        for (widget, chance) in self.widgets.iter_mut().zip(&mut self.chance_visuals) {
            widget.last_amount = 0.;
            widget.cash_icon_phase = 0;
            widget.visible = false;
            widget.text_amount = None;
            chance.textures = [285; 4];
            chance.reset = false;
        }
    }
    ///43be20→vtable+34=4285f0 on a newly shown instance. The blink phase
    /// survives; only the cached cash comparison resets before428060.
    pub fn activate(&mut self, actual: f32, instance: usize) -> Result<(), String> {
        let mut widget = *self
            .widgets
            .get(instance)
            .ok_or("Time instance outside two widgets")?;
        widget.last_amount = -1.;
        self.initialize(actual, &mut widget)?;
        self.widgets[instance] = widget;
        Ok(())
    }
    pub fn show(&mut self, actual: f32, instance: usize) -> Result<(), String> {
        let widget = self
            .widgets
            .get_mut(instance)
            .ok_or("Time instance outside two widgets")?;
        if widget.visible {
            return Ok(());
        }
        widget.visible = true;
        self.activate(actual, instance)
    }
    pub fn hide(&mut self, instance: usize) -> Result<(), String> {
        self.widgets
            .get_mut(instance)
            .ok_or("Time instance outside two widgets")?
            .visible = false;
        Ok(())
    }
    pub fn tick(&mut self, actual: f32, debt_status: bool, instance: usize) -> Result<(), String> {
        let mut widget = *self
            .widgets
            .get(instance)
            .ok_or("Time instance outside two widgets")?;
        self.step(actual, debt_status, &mut widget)?;
        self.widgets[instance] = widget;
        Ok(())
    }
    /// Native428099..42813f globals560a1c/560a20 and widget9ac.
    pub fn initialize(&mut self, actual: f32, widget: &mut Widget) -> Result<Option<u32>, String> {
        if self.amount != actual {
            if self.remaining == 0 {
                self.remaining = 50;
            } else {
                let mut amount = openmt2001::x87::Sum::from_f32(actual)?;
                amount.add_f32(-self.amount)?;
                amount.div(openmt2001::x87::Sum::from_i32(self.remaining))?;
                self.remaining = self.remaining.wrapping_sub(1);
                amount.add_f32(self.amount)?;
                self.amount = amount.to_f32();
            }
        }
        if widget.last_amount == self.amount {
            Ok(None)
        } else {
            widget.last_amount = self.amount;
            widget.text_amount = Some(self.amount);
            Ok(Some(if self.amount < 0. { 6 } else { 0 }))
        }
    }
    /// Native428faf/428fbd uses the retained midnight debt-status byte.
    /// Caller supplies the already-proven native widget dispatch gate.
    pub fn step(
        &mut self,
        actual: f32,
        debt_status: bool,
        widget: &mut Widget,
    ) -> Result<Option<u32>, String> {
        widget.cash_icon_phase = if debt_status {
            widget.cash_icon_phase.wrapping_sub(1)
        } else {
            0
        };
        self.initialize(actual, widget)
    }
}

/// Native4b5370 manager activation, then4ba580 dispatch. The caller applies
/// suppression and signed pause gates before these per-instance pass counts.
pub fn instance_passes(gui_mode: u32, iterations: u32) -> [u32; 2] {
    match gui_mode {
        1 => [0, 1],
        4 => [0, 0],
        _ => [iterations, 0],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Speed,
    Options,
    Manager,
    ScenarioReminder,
}
impl Button {
    pub(crate) fn tooltip(self, speed: u32, speed_disabled: bool) -> Option<u32> {
        match self {
            Self::ScenarioReminder => Some(1141),
            // 425490/481940: the caption describes the next action, with
            // 1144 at normal speed and 1145 after switching to four passes.
            Self::Speed if !speed_disabled => Some(if speed == 1 { 1144 } else { 1145 }),
            _ => None,
        }
    }
}

pub(crate) struct Display<'a> {
    pub year: u32,
    pub hour: u32,
    pub fraction: u32,
    pub speed: u32,
    pub name: &'a str,
    pub cash: Option<f32>,
    pub character: Option<usize>,
    pub cash_icon_phase: i32,
}

fn button_at(
    regions: &[[f32; 4]; 14],
    manager: [f32; 4],
    speed: [f32; 4],
    pointer: [f32; 2],
    auction: bool,
) -> Option<Button> {
    if inside(regions[9], pointer) {
        Some(Button::Options)
    } else if inside(regions[6], pointer) {
        Some(Button::ScenarioReminder)
    } else if auction {
        None
    } else if inside(manager, pointer) {
        Some(Button::Manager)
    } else {
        inside(speed, pointer).then_some(Button::Speed)
    }
}

fn player(status: &crate::live_host::Status) -> (String, Option<usize>) {
    let Some(record) = status
        .financial
        .as_ref()
        .and_then(|snapshot| snapshot.current.and_then(|i| snapshot.records.get(i)))
    else {
        return (String::new(), None);
    };
    let bytes = &record[0x154..];
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(0);
    let name = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let character = u32::from_le_bytes(record[0x14c..0x150].try_into().unwrap()) as usize;
    (name, Some(character))
}

pub struct Assets {
    regions: [[f32; 4]; 14],
    manager_bounds: [f32; 4],
    speed_bounds: [f32; 4],
    options_button: Surface,
    generic: Vec<Surface>,
    font: Vec<Surface>,
    tab: Vec<u8>,
    button_font: Vec<Surface>,
    button_tab: Vec<u8>,
    am_pm: [Vec<u8>; 2],
    options: Vec<u8>,
    currency: Vec<u8>,
    currency_left: bool,
    palettes: [u32; 18],
    character_colors: [u32; 10],
    backing_color: u32,
}
impl Assets {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
        executable: &[u8],
    ) -> Result<Self, String> {
        Self::load_impl(index, archive, installer, language, executable, None)
    }
    pub(crate) fn load_shared(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
        executable: &[u8],
        shared: &crate::intro::camera_live::ui_text_resources::UiTextResources,
    ) -> Result<Self, String> {
        Self::load_impl(
            index,
            archive,
            installer,
            language,
            executable,
            Some(shared),
        )
    }
    fn load_impl(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
        executable: &[u8],
        shared: Option<&crate::intro::camera_live::ui_text_resources::UiTextResources>,
    ) -> Result<Self, String> {
        let entry = index
            .find(PANEL)
            .ok_or("Original top-right panel missing")?;
        let mesh = openmt2001::mesh::CachedMesh::parse(
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Original top-right panel outside archive")?,
        )?;
        let regions = openmt2001::ui_mesh::regions(&mesh, 0)?;
        let names = [
            "TIME_BACKING",
            "TIME_TOP_GREY_FILL",
            "TIME_PLAYER_BACKING",
            "TIME_PLAYER_BACKING_GREY",
            "TIME_BORDER01",
            "TIME_BORDER02",
            "TIME_PLAYER_ICON",
            "TIME_CLOCK_ICON",
            "TIME_AREA",
            "TIME_OPTIONS_ICON",
            "TIME_CASH_ICON",
            "TIME_NAME_AREA",
            "TIME_CASH_AREA",
            "TIME_CHANCE_ICON",
        ];
        let regions = names
            .map(|name| {
                regions
                    .iter()
                    .find(|region| region.name == name)
                    .map(|region| region.bounds)
                    .ok_or_else(|| format!("Original top-right region {name} missing"))
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Original top-right region count")?;
        let nav_entry = index
            .find("gamedata2/brand_new_panels/new_nav_bottom.tm")
            .ok_or("Original bottom navigation panel missing")?;
        let nav = openmt2001::mesh::CachedMesh::parse(
            archive
                .get(nav_entry.offset as usize..(nav_entry.offset + nav_entry.length) as usize)
                .ok_or("Original bottom navigation panel outside archive")?,
        )?;
        let compass = openmt2001::ui_mesh::regions(&nav, 0)?
            .into_iter()
            .find(|region| region.name == "NAV_TOP_VIEW_ICON")
            .ok_or("Original bottom compass region missing")?
            .bounds;
        // 4251b2 adds 75px for index1 (4x); 425269 subtracts 37px
        // from that copy for index2 (the graph/manager widget).
        let shift = |dx: f32| [compass[0] + dx, compass[1], compass[2] + dx, compass[3]];
        let speed_bounds = shift(75.);
        let manager_bounds = shift(38.);
        let generic_entry = index
            .find("gamedata2/GenericTextures640/textures.gtd")
            .ok_or("Original generic UI texture set missing")?;
        let generic = TextureSet::parse(
            archive
                .get(
                    generic_entry.offset as usize
                        ..(generic_entry.offset + generic_entry.length) as usize,
                )
                .ok_or("Original generic UI texture set outside archive")?,
        )
        .map_err(|e| e.to_string())?;
        //42851d supplies normal texture82 for TIME_OPTIONS_ICON.
        let options_button = Surface::new(
            generic
                .textures
                .get(82)
                .ok_or("Original Options button texture missing")?,
        );
        let (font_pages, tab, button_pages, button_tab) = if let Some(shared) = shared {
            (
                shared.surfaces(2)?,
                shared.table(2)?.to_vec(),
                shared.surfaces(3)?,
                shared.table(3)?.to_vec(),
            )
        } else {
            let font_entry = index
                .find("gamedata2/font640/textures.gtd")
                .ok_or("Original UI font texture missing")?;
            let font = TextureSet::parse(
                archive
                    .get(
                        font_entry.offset as usize
                            ..(font_entry.offset + font_entry.length) as usize,
                    )
                    .ok_or("Original UI font texture outside archive")?,
            )
            .map_err(|e| e.to_string())?;
            let tab = fs::read(installer.join("Standard/fonts/tahoma11noantip.tab"))
                .map_err(|e| e.to_string())?;
            let button_tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
                .map_err(|e| e.to_string())?;
            (
                font.textures
                    .get(4..6)
                    .ok_or("shared font2 pages missing")?
                    .iter()
                    .map(Surface::new)
                    .collect(),
                tab,
                font.textures
                    .get(6..8)
                    .ok_or("shared font3 pages missing")?
                    .iter()
                    .map(Surface::new)
                    .collect(),
                button_tab,
            )
        };
        let loaded;
        let packed = match shared {
            Some(text) => &text.fonts().language,
            None => {
                loaded = crate::intro::camera_live::ui_text_resources::PackedLanguage::load(
                    &language.join("tycoon"),
                )?;
                &loaded
            }
        };
        let rows = packed.rows();
        let localized = if shared.is_some() {
            rows.iter().map(|s| s.to_vec()).collect()
        } else {
            crate::tooltip::localized_strings(installer, &rows)?
        };
        let row = |i| {
            localized
                .get(i)
                .cloned()
                .ok_or("Time localized entry missing".to_owned())
        };
        let image = openmt2001::pe::PeImage::parse(executable)?;
        let word = |at| {
            image
                .at(at, 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let palettes = (0..18)
            .map(|i| word(0x536288 + i * 4))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Time palette count")?;
        let character_colors = (0..10)
            .map(|i| word(0x541af0 + i * 40))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Time character count")?;
        Ok(Self {
            regions,
            manager_bounds,
            speed_bounds,
            options_button,
            generic: generic.textures.iter().map(Surface::new).collect(),
            font: font_pages,
            tab,
            button_font: button_pages,
            button_tab,
            am_pm: [row(1374)?, row(1375)?],
            options: row(1115)?,
            currency: row(2243)?,
            currency_left: localized.get(2244).and_then(|r| r.first()) == Some(&0x4c),
            palettes,
            character_colors,
            backing_color: word(0x5362c0)?,
        })
    }
    pub fn button(&self, pointer: [f32; 2]) -> Option<Button> {
        self.button_in_mode(pointer, false)
    }
    pub fn button_in_mode(&self, pointer: [f32; 2], auction: bool) -> Option<Button> {
        button_at(
            &self.regions,
            self.manager_bounds,
            self.speed_bounds,
            pointer,
            auction,
        )
    }

    pub fn contains(&self, pointer: [f32; 2]) -> bool {
        self.contains_in_mode(pointer, false)
    }
    pub fn contains_in_mode(&self, pointer: [f32; 2], auction: bool) -> bool {
        inside(self.regions[0], pointer)
            || !auction
                && (inside(self.manager_bounds, pointer) || inside(self.speed_bounds, pointer))
    }
    #[cfg(test)]
    pub(crate) fn prepare(
        &self,
        renderer: &Renderer,
        display: Display<'_>,
    ) -> Result<PreparedScene, String> {
        self.prepare_with_controls(renderer, display, true, None, None)
    }
    #[cfg(test)]
    fn prepare_with_controls(
        &self,
        renderer: &Renderer,
        display: Display<'_>,
        normal: bool,
        hover: Option<Button>,
        pressed: Option<Button>,
    ) -> Result<PreparedScene, String> {
        let surfaces = self.surfaces_with_controls(display, normal, hover, pressed)?;
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            VIEW,
            None,
        )
    }
    #[cfg(test)]
    pub(crate) fn refresh_for_test(
        &self,
        renderer: &Renderer,
        prepared: &mut PreparedScene,
        display: Display<'_>,
    ) -> Result<(), String> {
        let surfaces = self.surfaces_with_controls(display, true, None, None)?;
        renderer.refresh_prepared(
            prepared,
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            VIEW,
            None,
        )
    }
    fn split_surfaces(
        &self,
        display: Display<'_>,
        normal: bool,
        hover: Option<Button>,
        pressed: Option<Button>,
    ) -> Result<(Vec<Surface>, Vec<Surface>), String> {
        let speed = display.speed;
        let clock = self.surfaces_with_controls(display, false, hover, pressed)?;
        let mut bottom = Vec::new();
        if normal {
            for (texture, bounds) in bottom_controls_with_state(
                self.manager_bounds,
                self.speed_bounds,
                speed == 4,
                hover,
                pressed,
            ) {
                let mut surface = self
                    .generic
                    .get(texture)
                    .ok_or("Bottom texture missing")?
                    .clone();
                let [l, t, r, b] = bounds;
                surface.quad(
                    [[l, t], [l, b], [r, b], [r, t]],
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    [0xffffffff; 4],
                );
                bottom.push(surface);
            }
        }
        Ok((clock, bottom))
    }
    fn surfaces_with_controls(
        &self,
        display: Display<'_>,
        normal: bool,
        hover: Option<Button>,
        pressed: Option<Button>,
    ) -> Result<Vec<Surface>, String> {
        let Display {
            year,
            hour,
            fraction,
            speed,
            name,
            cash,
            character,
            cash_icon_phase,
        } = display;
        let mut backdrop = Surface::solid();
        backdrop.rect(self.regions[0], [self.backing_color; 4]);
        backdrop.rect(
            self.regions[1],
            [0xff909090, 0xff909090, 0x7f909090, 0x7f909090],
        );
        if let Some(character) = character {
            let color = *self
                .character_colors
                .get(character)
                .ok_or("Time player character outside palette")?;
            backdrop.rect(
                self.regions[2],
                [
                    color.wrapping_sub(0x01000000),
                    color.wrapping_sub(0x01000000),
                    color.wrapping_add(0x7f000000),
                    color.wrapping_add(0x7f000000),
                ],
            );
        }
        backdrop.rect(
            self.regions[3],
            [0x4c909090, 0x4c909090, 0x7f909090, 0x7f909090],
        );
        // Original43c3a0 submits all14 model parts after4286b0's backing.
        // Their transformed depth is0.600..1.000, behind the backing at0.5
        // under native state1's depth-write/LESS_EQUAL. The named border
        // parts therefore contribute no visible pixels (native mesh gate).
        let icon = |source: &Surface, bounds: [f32; 4]| {
            let mut surface = source.clone();
            let [l, t, r, b] = bounds;
            surface.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surface
        };
        let options = self.regions[9];
        let mut font = self.font.clone();
        let mut label = |encoded: &[u8], bounds, center, palette: usize| -> Result<(), String> {
            for quad in text_quads(&self.tab, encoded, bounds, center)? {
                font.get_mut(quad.page)
                    .ok_or("Time glyph font page missing")?
                    .quad(
                        quad.positions.map(|p| [p[0], p[1]]),
                        quad.uv,
                        [self.palettes[palette]; 4],
                    );
            }
            Ok(())
        };
        label(
            &clock_text(hour, fraction, year, &self.am_pm[0], &self.am_pm[1]),
            self.regions[8],
            true,
            0,
        )?;
        let name = name.bytes().map(|b| b.wrapping_sub(32)).collect::<Vec<_>>();
        label(&name, self.regions[11], false, 0)?;
        if let Some(cash) = cash {
            let mut text = ui::decimal2(cash.abs())?
                .bytes()
                .map(|b| b.wrapping_sub(32))
                .collect::<Vec<_>>();
            if self.currency_left {
                text.splice(0..0, self.currency.iter().copied());
            } else {
                text.extend_from_slice(&self.currency);
            }
            if cash < 0. {
                text.insert(0, b'-' - 32);
            }
            label(
                &text,
                self.regions[12],
                false,
                if cash < 0. { 6 } else { 0 },
            )?;
        }
        let mut buttons = self.button_font.clone();
        for quad in text_quads_with_offset(&self.button_tab, &self.options, options, true, 0)? {
            buttons
                .get_mut(quad.page)
                .ok_or("Options glyph font page missing")?
                .quad(
                    quad.positions.map(|p| [p[0], p[1]]),
                    quad.uv,
                    [self.palettes[0]; 4],
                );
        }
        //42851d assigns82/83/84 to the Options textured child.
        let options_texture = options_texture(hover, pressed);
        let options_source = if options_texture == 82 {
            &self.options_button
        } else {
            &self.generic[options_texture]
        };
        let mut surfaces = vec![backdrop, icon(options_source, options)];
        let bottom = if hover.is_none() && pressed.is_none() {
            bottom_controls(self.manager_bounds, self.speed_bounds, speed == 4)
        } else {
            bottom_controls_with_state(
                self.manager_bounds,
                self.speed_bounds,
                speed == 4,
                hover,
                pressed,
            )
        };
        for (texture, bounds) in bottom.into_iter().filter(|_| normal) {
            surfaces.push(icon(
                self.generic
                    .get(texture)
                    .ok_or("Original bottom control texture missing")?,
                bounds,
            ));
        }
        if let Some(character) = character {
            let texture = *[147, 144, 150, 153, 156, 159, 162, 165, 168, 171]
                .get(character)
                .ok_or("Time player character outside icons")?;
            surfaces.push(icon(&self.generic[texture], self.regions[6]));
        }
        surfaces.push(icon(&self.generic[11], self.regions[7]));
        surfaces.push(icon(
            &self.generic[if cash_icon_phase & 31 > 16 { 180 } else { 10 }],
            self.regions[10],
        ));
        for points in clock_hands(self.regions[7], hour, fraction) {
            let mut hand = self.generic[127].clone();
            hand.quad(
                points,
                [[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(hand);
        }
        surfaces.extend(font);
        surfaces.extend(buttons);
        Ok(surfaces)
    }
}

type ViewKey = (u32, u32, u32, u32, String, Option<u32>, Option<usize>, i32);
/// Authoritative worker clock only; no host-time prediction. The interface
/// stamp continues advancing while logic is paused and orders Status replies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockSample {
    pub interface_ticks: u64,
    /// Delivered normal modal-group updates, after pause and suppression gates.
    pub prompt_updates: u64,
    pub ticks: u32,
    pub year: u32,
    pub hour: u32,
    pub fraction: u32,
    pub speed: u32,
}
impl ClockSample {
    fn from_status(status: &crate::live_host::Status) -> Self {
        Self {
            interface_ticks: status.interface_ticks,
            prompt_updates: status.prompt_updates,
            ticks: status.ticks,
            year: status.year,
            hour: status.hour,
            fraction: status.hour_fraction,
            speed: status.speed,
        }
    }
    fn stamp(self) -> (u64, u32) {
        (self.interface_ticks, self.ticks)
    }
}
#[derive(Default)]
struct ClockState {
    latest: Option<ClockSample>,
    lightweight: bool,
}
impl ClockState {
    fn observe(&mut self, sample: ClockSample, lightweight: bool) -> ClockSample {
        // A lightweight reply wins equal stamps: speed can change between
        // commands while paused at the same logic/interface tick. An older
        // full Status must not restore the clock already sampled afterward.
        if self.latest.is_none_or(|old| {
            sample.stamp() > old.stamp()
                || (lightweight || !self.lightweight) && sample.stamp() == old.stamp()
        }) {
            self.latest = Some(sample);
            self.lightweight = lightweight;
        }
        self.latest.unwrap_or(sample)
    }
}
#[derive(Default)]
pub struct View {
    key: Option<ViewKey>,
    clock: ClockState,
    pub prepared: Option<PreparedScene>,
    pub(crate) bottom_prepared: Option<PreparedScene>,
    graph_bottom_visible: bool,
    graph_clock_visible: bool,
    graph_speed_disabled: bool,
    graph_speed_blink: u8,
    hover: Option<Button>,
    hover_started: Option<std::time::Instant>,
    pressed: Option<Button>,
    click_latched: Option<Button>,
    normal: bool,
}
impl View {
    pub(crate) fn displayed_speed(&self) -> Option<u32> {
        self.clock.latest.map(|sample| sample.speed)
    }
    #[cfg(test)]
    pub fn displayed_name(&self) -> Option<&str> {
        self.key.as_ref().map(|key| key.4.as_str())
    }
    ///Actual generic child phase3: clear hover and one-shot, retain down334.
    pub(crate) fn leave_child(&mut self, button: Button) {
        if self.hover == Some(button) {
            self.hover = None;
            self.key = None;
        }
        if self.click_latched == Some(button) {
            self.click_latched = None;
            self.key = None;
        }
    }
    pub fn cancel_input(&mut self) {
        self.hover_started = None;
        if self.hover.take().is_some()
            || self.pressed.take().is_some()
            || self.click_latched.take().is_some()
        {
            self.key = None;
        }
        self.pressed = None;
        self.click_latched = None;
    }
    ///Clock Options/portrait are57e77c; Speed/Manager are E57e764.
    pub(crate) fn graph_button(
        &self,
        assets: &Assets,
        point: [f32; 2],
        auction: bool,
    ) -> Option<Button> {
        assets
            .button_in_mode(point, auction)
            .filter(|button| match button {
                Button::Options | Button::ScenarioReminder => self.graph_clock_visible,
                Button::Speed => self.graph_bottom_visible && !self.graph_speed_disabled,
                Button::Manager => self.graph_bottom_visible,
            })
    }
    pub(crate) fn graph_cursor(&mut self, hit: Option<Button>, held: bool) -> Option<Button> {
        self.hover_held(hit, held)
    }
    pub(crate) fn graph_gates(
        &mut self,
        clock: bool,
        bottom: bool,
        speed_disabled: bool,
        blink: u8,
    ) {
        self.graph_clock_visible = clock;
        self.graph_bottom_visible = bottom;
        self.graph_speed_disabled = speed_disabled;
        self.graph_speed_blink = blink;
    }
    /// Legacy inspection can replace its scene between interface draws.
    /// Keep the original25/50-second dwell with the retained HUD child.
    pub(crate) fn tooltip_ready(&self) -> bool {
        self.hover == Some(Button::ScenarioReminder)
            && self
                .hover_started
                .is_some_and(|start| start.elapsed().as_secs_f64() > 0.5)
    }
    pub fn cursor(&mut self, assets: &Assets, point: Option<[f32; 2]>, auction: bool) {
        let hit = point.and_then(|point| assets.button_in_mode(point, auction));
        self.hover_held(hit, false);
    }
    fn hover_held(&mut self, hit: Option<Button>, held: bool) -> Option<Button> {
        let previous = self.hover;
        if previous == hit {
            return None;
        }
        self.hover = hit;
        self.hover_started = hit.map(|_| std::time::Instant::now());
        self.key = None;
        // Event3 clears +330 but leaves the visual pressed flag +334. Keep
        // those states separate so a held re-entry can click without erasing
        // the pressed appearance captured from the original widget.
        if self.click_latched == previous {
            self.click_latched = None;
        }
        if held && hit.is_some() && self.click_latched != hit {
            self.click_latched = hit;
            return hit;
        }
        None
    }
    /// Native 43c8e0 sends event3 to a child left behind, then event2 and a
    /// held event5 to the newly hit child. Event3 clears 443360's one-shot
    /// flag, so dragging out and back can request the click cue once more.
    pub fn cursor_held(
        &mut self,
        assets: &Assets,
        point: Option<[f32; 2]>,
        auction: bool,
        held: bool,
    ) -> Option<Button> {
        let hit = point.and_then(|point| assets.button_in_mode(point, auction));
        self.hover_held(hit, held)
    }
    /// Original443360 emits the light-click cue on the first press event4/5
    /// while its retained pressed flag is clear. A held button does not replay.
    pub fn mouse(&mut self, pressed: bool) -> Option<Button> {
        let first_press = if pressed && self.click_latched != self.hover {
            self.hover
        } else {
            None
        };
        self.pressed = if pressed { self.hover } else { None };
        self.click_latched = if pressed { self.hover } else { None };
        self.key = None;
        first_press
    }
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        status: &crate::live_host::Status,
    ) -> Result<(), String> {
        if !status.time_display.visible {
            self.prepared = None;
            self.key = None;
            self.cancel_input();
            return Ok(());
        }
        let normal = status.auction.is_none();
        if self.normal != normal {
            self.normal = normal;
            self.key = None;
            self.cancel_input();
        }
        let clock = self.clock.observe(ClockSample::from_status(status), false);
        let (name, character) = player(status);
        let cash = status.time_display.cash;
        let key = (
            clock.year,
            clock.hour,
            clock.fraction,
            clock.speed,
            name,
            cash.map(f32::to_bits),
            character,
            status.time_display.cash_icon_phase,
        );
        self.render_key(assets, renderer, key)
    }
    pub fn update_clock(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        sample: ClockSample,
    ) -> Result<(), String> {
        let clock = self.clock.observe(sample, true);
        // Visibility, mode, identity and cash still come from full Status.
        // A hidden Time panel is not made visible by lightweight polling.
        let Some(current) = &self.key else {
            return Ok(());
        };
        if (current.0, current.1, current.2, current.3)
            == (clock.year, clock.hour, clock.fraction, clock.speed)
        {
            return Ok(());
        }
        let mut key = current.clone();
        key.0 = clock.year;
        key.1 = clock.hour;
        key.2 = clock.fraction;
        key.3 = clock.speed;
        self.render_key(assets, renderer, key)
    }
    pub(crate) fn prepare_graph(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
    ) -> Result<(), String> {
        let Some(key) = self.key.as_ref() else {
            self.prepared = None;
            self.bottom_prepared = None;
            return Ok(());
        };
        let (clock, bottom) = assets.split_surfaces(
            Display {
                year: key.0,
                hour: key.1,
                fraction: key.2,
                speed: key.3,
                name: &key.4,
                cash: key.5.map(f32::from_bits),
                character: key.6,
                cash_icon_phase: key.7,
            },
            self.normal,
            self.hover,
            self.pressed.filter(|button| self.hover == Some(*button)),
        )?;
        self.prepared = if self.graph_clock_visible {
            Some(renderer.prepare(
                &clock.iter().map(Surface::draw).collect::<Vec<_>>(),
                VIEW,
                None,
            )?)
        } else {
            None
        };
        self.bottom_prepared = if self.graph_bottom_visible {
            Some(renderer.prepare(
                &bottom.iter().map(Surface::draw).collect::<Vec<_>>(),
                VIEW,
                None,
            )?)
        } else {
            None
        };
        Ok(())
    }
    fn render_key(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        key: ViewKey,
    ) -> Result<(), String> {
        if self.key.as_ref() != Some(&key) {
            let surfaces = assets.surfaces_with_controls(
                Display {
                    year: key.0,
                    hour: key.1,
                    speed: key.3,
                    fraction: key.2,
                    name: &key.4,
                    cash: key.5.map(f32::from_bits),
                    character: key.6,
                    cash_icon_phase: key.7,
                },
                self.normal,
                self.hover,
                self.pressed.filter(|pressed| self.hover == Some(*pressed)),
            )?;
            let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
            if let Some(prepared) = &mut self.prepared {
                renderer.refresh_prepared(prepared, &draws, VIEW, None)?;
            } else {
                self.prepared = Some(renderer.prepare(&draws, VIEW, None)?);
            }
            self.key = Some(key);
        }
        Ok(())
    }
}

/// Captures only the production HUD from a real fresh/loaded Host status.
/// The natural auction fixture owns admission and activation; this helper
/// does not change the Session, invoke show(), or claim an Intro window draw.
#[cfg(all(test, feature = "capture"))]
pub(crate) fn capture_auction_status(status: &crate::live_host::Status, label: &str) {
    assert!(status.auction.is_some());
    assert!(
        status.time_display.visible,
        "{label}: native phase-two widget group must show the selected Time instance"
    );
    let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
    let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let executable = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let executable_path = openmt2001::fixtures::path("EXE");
    let language = executable_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let assets = Assets::load(&index, &archive, installer, &language, &executable).unwrap();
    let center = |[l, t, r, b]: [f32; 4]| [(l + r) * 0.5, (t + b) * 0.5];
    for (bounds, button) in [
        (assets.manager_bounds, Button::Manager),
        (assets.speed_bounds, Button::Speed),
    ] {
        assert_eq!(assets.button_in_mode(center(bounds), false), Some(button));
        assert_eq!(assets.button_in_mode(center(bounds), true), None);
        assert!(!assets.contains_in_mode(center(bounds), true));
    }
    let option = center(assets.regions[9]);
    assert_eq!(assets.button_in_mode(option, true), Some(Button::Options));
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let mut view = View::default();
    view.update(&assets, &renderer, status).unwrap();
    crate::capture_test::capture_standalone(
        &renderer,
        view.prepared.as_ref().unwrap(),
        &format!("work/frontend/time-auction-{label}.png"),
    );
    view.cursor(&assets, Some(option), true);
    view.mouse(true);
    view.update(&assets, &renderer, status).unwrap();
    crate::capture_test::capture_standalone(
        &renderer,
        view.prepared.as_ref().unwrap(),
        &format!("work/frontend/time-auction-{label}-options-pressed.png"),
    );
    view.cursor(&assets, Some(center(assets.manager_bounds)), true);
    assert!(view.hover.is_none());
    view.update(&assets, &renderer, status).unwrap();
    assert!(view.pressed.is_some()); // capture stays with Options until release/cancel
    view.cancel_input();
    assert!(view.pressed.is_none());
    assert!(status.time_display.visible); // input never modifies original visibility
}

#[cfg(test)]
mod tests {
    #[test]
    fn speed_tooltip_follows_live_toggle_with_original_dwell_while_hovered() {
        use super::Button;
        use crate::intro::camera_live::focus::{ChildKey, Focus};
        let mut host = crate::live_host::Host::new([0.; 3]);
        let mut hud = super::View::default();
        let mut focus = Focus::default();
        let mut legacy = crate::tooltip::IconDwell::default();
        let mut sample = super::ClockSample {
            interface_ticks: 0,
            prompt_updates: 0,
            ticks: 0,
            year: 1930,
            hour: 9,
            fraction: 0,
            speed: 1,
        };
        for (speed, expected) in [(1, 1144), (4, 1145), (1, 1144)] {
            assert_eq!(host.policy.scheduler.iterations, speed);
            let stale = sample;
            sample.speed = speed;
            hud.clock.observe(sample, true);
            hud.clock.observe(stale, false);
            let id = Button::Speed
                .tooltip(
                    hud.displayed_speed().unwrap(),
                    host.policy.scheduler.network_enabled,
                )
                .unwrap();
            assert_eq!(id, expected);
            // Keep the same hovered widget across each speed change. Both
            // existing HUD paths refresh the caption and restart its dwell.
            focus
                .show_child(ChildKey::Time(Button::Speed), id, false)
                .unwrap();
            assert_eq!(focus.retained_text_identifier(), Some(expected));
            for _ in 0..25 {
                focus.advance();
                assert_eq!(focus.caption(), None);
                assert_eq!(legacy.advance(Some(id)), None);
            }
            focus.advance();
            assert_eq!(focus.caption(), Some(expected));
            assert_eq!(legacy.advance(Some(id)), Some(expected));
            focus
                .show_child(ChildKey::Time(Button::Speed), id, false)
                .unwrap();
            assert_eq!(
                focus.caption(),
                Some(expected),
                "unchanged hover retains dwell"
            );
            host.toggle_speed();
        }
        focus.clear_child();
        assert_eq!(focus.caption(), None);
        assert_eq!(legacy.advance(None), None);
        assert_eq!(legacy.advance(Some(1144)), None);
    }

    #[test]
    fn network_speed_has_neither_help_nor_toggle() {
        let mut host = crate::live_host::Host::new([0.; 3]);
        host.policy.scheduler.network_enabled = true;
        for speed in [1, 4] {
            host.policy.scheduler.iterations = speed;
            assert_eq!(super::Button::Speed.tooltip(speed, true), None);
            host.toggle_speed();
            assert_eq!(host.policy.scheduler.iterations, speed);
        }
    }

    #[test]
    fn portrait_hit_target_opens_scenario_reminder_separately_from_manager() {
        let mut regions = [[-10.; 4]; 14];
        regions[6] = [550., 0., 580., 30.];
        regions[9] = [580., 0., 610., 30.];
        let manager = [570., 450., 590., 470.];
        let speed = [600., 450., 620., 470.];
        let hit = |point, auction| super::button_at(&regions, manager, speed, point, auction);
        for auction in [false, true] {
            assert_eq!(
                hit([565., 15.], auction),
                Some(super::Button::ScenarioReminder)
            );
            assert_eq!(hit([595., 15.], auction), Some(super::Button::Options));
            assert_eq!(hit([540., 15.], auction), None);
        }
        assert_eq!(hit([580., 460.], false), Some(super::Button::Manager));
        assert_eq!(hit([610., 460.], false), Some(super::Button::Speed));
        assert_eq!(hit([580., 460.], true), None);
    }

    #[test]
    #[ignore = "requires original Time button vtable and sound string"]
    fn original_time_press_audio() {
        let executable = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        // 43ed20 constructs each Time child as a textured button; its final
        // vtable routes input through443360, which pushes the original cue.
        assert_eq!(
            image.at(0x43edc7, 6).unwrap(),
            &[0xc7, 0x06, 0xa4, 0x59, 0x52, 0x00]
        );
        assert_eq!(
            u32::from_le_bytes(image.at(0x5259a4 + 24, 4).unwrap().try_into().unwrap()),
            0x443360
        );
        assert_eq!(
            image.at(0x4433df, 2).unwrap(),
            &[0x6a, 0x64],
            "native button press requests 100-point playback"
        );
        assert_eq!(
            image.at(0x4433e8, 5).unwrap(),
            &[0x68, 0x70, 0x58, 0x53, 0x00]
        );
        assert_eq!(
            image.at(0x535870, 27).unwrap(),
            b"spot_any_ui_lightclick.wav\0"
        );
        let mut view = super::View::default();
        for button in [
            super::Button::Options,
            super::Button::Speed,
            super::Button::Manager,
        ] {
            view.hover = Some(button);
            assert_eq!(view.mouse(true), Some(button));
            assert_eq!(view.mouse(true), None, "held button is silent");
            assert_eq!(view.mouse(false), None);
        }
        view.hover = None;
        assert_eq!(view.mouse(true), None);
        view.mouse(false);
        view.hover_held(Some(super::Button::Options), false);
        assert_eq!(view.mouse(true), Some(super::Button::Options));
        assert_eq!(
            view.hover_held(Some(super::Button::Options), true),
            None,
            "held movement within the child stays silent"
        );
        assert_eq!(view.hover_held(None, true), None);
        assert_eq!(
            view.pressed,
            Some(super::Button::Options),
            "native event3 clears its click latch, not its pressed appearance"
        );
        assert_eq!(
            view.hover_held(Some(super::Button::Options), true),
            Some(super::Button::Options),
            "held re-entry gets native event5"
        );
        assert_eq!(view.hover_held(Some(super::Button::Options), true), None);
        assert_eq!(view.mouse(false), None);
    }

    #[test]
    fn clock_samples_preserve_1x_4x_pause_and_reject_stale_status() {
        use super::{ClockSample, ClockState};
        let mut state = ClockState::default();
        let mut clock = ClockSample {
            interface_ticks: 0,
            prompt_updates: 0,
            ticks: 0,
            year: 1930,
            hour: 9,
            fraction: 0,
            speed: 1,
        };
        for speed in [1, 4] {
            clock.speed = speed;
            // Feed the native scheduler's authoritative values: one logic
            // callback per two interface callbacks, with1/4 iterations.
            for _ in 0..25 {
                let delayed_status = clock;
                clock.interface_ticks += 2;
                clock.ticks += speed;
                clock.fraction += speed;
                assert_eq!(state.observe(clock, true), clock);
                assert_eq!(
                    state.observe(delayed_status, false),
                    clock,
                    "250ms Status delivery cannot regress a newer40ms sample"
                );
            }
        }
        assert_eq!(clock.ticks, 125);
        assert_eq!(clock.fraction, 125);
        let paused = clock;
        for _ in 0..50 {
            clock.interface_ticks += 1;
            assert_eq!(
                state.observe(clock, true).fraction,
                paused.fraction,
                "Options or script pause cannot create predicted clock ticks"
            );
        }
        let old_speed = clock;
        clock.speed = 1;
        assert_eq!(state.observe(clock, true), clock);
        assert_eq!(
            state.observe(old_speed, false),
            clock,
            "paused same-stamp Status must not undo a sampled speed change"
        );
        for (fraction, minute) in [(0, 0), (10, 0), (11, 1), (20, 1), (21, 2), (624, 59)] {
            let encoded = super::clock_text(9, fraction, 1930, b"AM", b"PM");
            assert_eq!(
                &encoded[..4],
                format!("9:{minute:02}")
                    .bytes()
                    .map(|b| b - 32)
                    .collect::<Vec<_>>()
            );
        }
    }
    #[cfg(feature = "capture")]
    use super::*;
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original Time assets and offscreen GPU"]
    fn retained_time_surfaces_match_fresh_gpu_across_clock_and_controls() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameters.parent().unwrap().parent().unwrap();
        let language = openmt2001::fixtures::path("EXE")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language, &executable).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut retained = None;
        for case in 0..6 {
            let normal = case != 4;
            let hover = (case == 1 || case == 2).then_some(Button::Options);
            let pressed = (case == 2).then_some(Button::Options);
            let display = Display {
                year: 1930 + case,
                hour: 9 + case,
                fraction: case * 103,
                speed: if case == 3 { 4 } else { 1 },
                name: if case == 3 {
                    "JUSTIN TEST LONG NAME"
                } else {
                    "JUSTIN"
                },
                cash: if case == 4 {
                    None
                } else {
                    Some(15000. + case as f32 * 12001.)
                },
                character: if case == 5 { None } else { Some(4) },
                cash_icon_phase: if case == 3 { 17 } else { 0 },
            };
            let surfaces = assets
                .surfaces_with_controls(display, normal, hover, pressed)
                .unwrap();
            let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
            let fresh = renderer.prepare(&draws, VIEW, None).unwrap();
            if let Some(prepared) = &mut retained {
                renderer
                    .refresh_prepared(prepared, &draws, VIEW, None)
                    .unwrap();
            } else {
                retained = Some(renderer.prepare(&draws, VIEW, None).unwrap());
            }
            let reference = format!("work/frontend/time-retained-{case}-fresh.png");
            let actual = format!("work/frontend/time-retained-{case}-updated.png");
            crate::capture_test::capture_standalone(&renderer, &fresh, &reference);
            crate::capture_test::capture_standalone(&renderer, retained.as_ref().unwrap(), &actual);
            assert!(
                fs::read(reference).unwrap() == fs::read(actual).unwrap(),
                "Time GPU pixels differ for case{case}"
            );
        }
        let mut view = View {
            normal: true,
            ..Default::default()
        };
        view.render_key(
            &assets,
            &renderer,
            (
                1930,
                9,
                0,
                1,
                "JUSTIN".into(),
                Some(15000f32.to_bits()),
                Some(4),
                0,
            ),
        )
        .unwrap();
        view.update_clock(
            &assets,
            &renderer,
            ClockSample {
                interface_ticks: 800,
                prompt_updates: 800,
                ticks: 125,
                year: 1930,
                hour: 9,
                fraction: 125,
                speed: 4,
            },
        )
        .unwrap();
        assert_eq!(view.key.as_ref().unwrap().4, "JUSTIN");
        assert_eq!(view.key.as_ref().unwrap().5, Some(15000f32.to_bits()));
        let fresh = assets
            .prepare_with_controls(
                &renderer,
                Display {
                    year: 1930,
                    hour: 9,
                    fraction: 125,
                    speed: 4,
                    name: "JUSTIN",
                    cash: Some(15000.),
                    character: Some(4),
                    cash_icon_phase: 0,
                },
                true,
                None,
                None,
            )
            .unwrap();
        let reference = "work/frontend/time-lightweight-fresh.png";
        let actual = "work/frontend/time-lightweight-updated.png";
        crate::capture_test::capture_standalone(&renderer, &fresh, reference);
        crate::capture_test::capture_standalone(&renderer, view.prepared.as_ref().unwrap(), actual);
        assert_eq!(
            fs::read(reference).unwrap(),
            fs::read(actual).unwrap(),
            "authoritative lightweight clock must retain identity/cash and match full HUD pixels"
        );
    }
    #[test]
    #[ignore = "requires original full textured-widget state streams"]
    fn original_time_button_states() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_BOTTOM_HUD_STATES")).unwrap();
        assert_eq!(&data[..8], b"MTBHST01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(12)), (0x37f, 16));
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/brand_new_panels/new_nav_bottom.tm")
            .unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let bounds = openmt2001::ui_mesh::regions(&mesh, 0)
            .unwrap()
            .into_iter()
            .find(|r| r.name == "NAV_TOP_VIEW_ICON")
            .unwrap()
            .bounds;
        let shifted = |offset: f32| [bounds[0] + offset, bounds[1], bounds[2] + offset, bounds[3]];
        let mut at = 16;
        for case in 0..16 {
            let fast = word(at) != 0;
            let button = if word(at + 4) == 1 {
                super::Button::Speed
            } else {
                super::Button::Manager
            };
            let hover = (word(at + 16) != 0).then_some(button);
            let pressed = (word(at + 8) != 0).then_some(button);
            assert_eq!(word(at + 20), 2);
            let draws =
                super::bottom_controls_with_state(shifted(38.), shifted(75.), fast, hover, pressed);
            let selected = if button == super::Button::Speed {
                &draws[..2]
            } else {
                &draws[2..]
            };
            at += 24;
            for &(texture, [l, t, r, b]) in selected {
                assert_eq!(word(at), texture as u32, "native state{case}");
                let points = [[l, t, 0.5], [l, b, 0.5], [r, b, 0.5], [r, t, 0.5]];
                for (i, value) in points.into_iter().flatten().enumerate() {
                    assert_eq!(word(at + 4 + i * 4), value.to_bits());
                }
                for (i, value) in [[0f32, 0.], [0., 1.], [1., 1.], [1., 0.]]
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    assert_eq!(word(at + 52 + i * 4), value.to_bits());
                }
                at += 84;
            }
        }
        assert_eq!(at, data.len());
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_BIND_OPTIONS")).unwrap();
        assert_eq!(&data[..8], b"MTOPST01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(12)), (0x37f, 4));
        for case in 0..4 {
            let at = 16 + case * 16;
            let hover = (word(at + 8) != 0).then_some(super::Button::Options);
            let pressed = (word(at) != 0).then_some(super::Button::Options);
            assert_eq!(super::options_texture(hover, pressed) as u32, word(at + 12));
        }
        assert_eq!(data.len(), 80);
        let executable = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        // Native phase-two setter shows the five members in this order;
        // V2 visible-cache restoration reconstructs the Time member only.
        for (at, address) in [
            (0x464f0fu32, 0x565b9cu32),
            (0x464f1a, 0x565ba4),
            (0x464f25, 0x565ba0),
            (0x464f30, 0x565b94),
            (0x464f3b, 0x565ba8),
        ] {
            let code = image.at(at, 11).unwrap();
            assert_eq!(&code[..2], &[0x8b, 0x0d]);
            assert_eq!(u32::from_le_bytes(code[2..6].try_into().unwrap()), address);
            assert_eq!(code[6], 0xe8);
            assert_eq!(
                (at + 11).wrapping_add_signed(i32::from_le_bytes(code[7..11].try_into().unwrap())),
                0x43be20
            );
        }
        // Native parent leave clears330 while retaining334. The view must
        // suppress the pressed image outside without losing capture identity.
        let mut view = super::View {
            hover: Some(super::Button::Options),
            ..Default::default()
        };
        view.mouse(true);
        view.hover = None;
        assert_eq!(
            view.pressed.filter(|pressed| view.hover == Some(*pressed)),
            None
        );
        view.hover = Some(super::Button::Options);
        assert_eq!(
            view.pressed.filter(|pressed| view.hover == Some(*pressed)),
            Some(super::Button::Options)
        );
        view.cancel_input();
        assert!(view.pressed.is_none() && view.hover.is_none());
    }
    #[test]
    #[ignore = "requires original complete analog-clock draw oracle"]
    fn original_clock_hand_draw() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_PANEL")).unwrap();
        assert_eq!(&data[..8], b"MTTIME01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(word(12), 0x37f);
        let mut at = 16;
        for case in 0..word(8) {
            let hour = word(at);
            let fraction = word(at + 4);
            let pulse = word(at + 8);
            let bounds = std::array::from_fn(|i| float(at + 12 + i * 4));
            assert_eq!(word(at + 28), 4);
            let hands = super::clock_hands(bounds, hour, fraction);
            at += 32;
            for (draw, expected_texture) in [11, if pulse > 16 { 180 } else { 10 }, 127, 127]
                .into_iter()
                .enumerate()
            {
                assert_eq!(word(at), u32::from(draw >= 2));
                assert_eq!(word(at + 4), expected_texture);
                let size = word(at + 8) as usize;
                at += 12;
                if draw >= 2 {
                    assert_eq!(size, 84);
                    for (corner, point) in hands[draw - 2].iter().enumerate() {
                        for (axis, value) in point.iter().enumerate() {
                            assert_eq!(
                                value.to_bits(),
                                word(at + corner * 12 + axis * 4),
                                "native clock case{case}, hand{}, corner{corner}, axis{axis}",
                                draw - 2
                            );
                        }
                        assert_eq!(float(at + corner * 12 + 8), 0.5);
                    }
                    assert_eq!(word(at + 80), 0xffffffff);
                } else {
                    assert_eq!(size, 20);
                }
                at += size;
            }
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original clock text and font2 oracle"]
    fn original_clock_text_draw() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_TEXT")).unwrap();
        assert_eq!(&data[..8], b"MTTTXT01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(word(12), 0x37f);
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let tab = std::fs::read(installer.join("Standard/fonts/tahoma11noantip.tab")).unwrap();
        let mut at = 16;
        for case in 0..word(8) {
            let hour = word(at);
            let fraction = word(at + 4);
            assert_eq!(word(at + 12), fraction * 60 / 625);
            assert_eq!(word(at + 16), if hour >= 12 { 1375 } else { 1374 });
            let value = super::clock_text(hour, fraction, word(at + 20), b"AM", b"PM");
            let size = word(at + 24) as usize;
            at += 28;
            assert_eq!(
                value,
                data[at..at + size - 1],
                "native formatted clock case{case}"
            );
            at += size;
            let lines = word(at) as usize;
            assert_eq!(lines, 2);
            assert_eq!(word(at + 4), 10);
            at += 12 + lines * 4;
            let quads = super::text_quads(
                &tab,
                &value,
                [519.5762, 5.817965, 573.7616, 29.725885],
                true,
            )
            .unwrap();
            for page in 0..2 {
                let count = word(at) as usize;
                at += 4;
                let quads = quads.iter().filter(|q| q.page == page).collect::<Vec<_>>();
                assert_eq!(count, quads.len(), "native glyph count case{case}");
                for (glyph, quad) in quads.iter().enumerate() {
                    for (i, v) in quad.positions.into_iter().flatten().enumerate() {
                        assert_eq!(
                            v.to_bits(),
                            word(at + glyph * 48 + i * 4),
                            "native clock text case{case} glyph{glyph} position{i}"
                        );
                    }
                    for (i, v) in quad.uv.into_iter().flatten().enumerate() {
                        assert_eq!(
                            v.to_bits(),
                            float(at + count * 48 + glyph * 32 + i * 4).to_bits()
                        );
                    }
                }
                at += count * 80;
            }
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original cash interpolation oracle"]
    fn original_cash_display_update() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_CASH")).unwrap();
        assert_eq!(&data[..8], b"MTCASH01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x37f);
        assert_eq!(data.len(), 16 + word(8) as usize * 32);
        for case in 0..word(8) as usize {
            let at = 16 + case * 32;
            let mut cash = super::Runtime {
                amount: f32::from_bits(word(at + 4)),
                remaining: word(at + 8) as i32,
                ..Default::default()
            };
            let mut widget = super::Widget {
                last_amount: f32::from_bits(word(at + 12)),
                ..Default::default()
            };
            assert_eq!(
                cash.initialize(f32::from_bits(word(at)), &mut widget)
                    .unwrap()
                    .unwrap_or(0xffffffff),
                word(at + 28),
                "cash palette case{case}"
            );
            assert_eq!(
                cash.amount.to_bits(),
                word(at + 16),
                "cash amount case{case}"
            );
            assert_eq!(
                cash.remaining as u32,
                word(at + 20),
                "cash divisor case{case}"
            );
            assert_eq!(
                widget.last_amount.to_bits(),
                word(at + 24),
                "cash widget case{case}"
            );
        }
    }
    #[test]
    #[ignore = "requires original two-instance UI dispatcher oracle"]
    fn original_time_widget_dispatch() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_DISPATCH")).unwrap();
        assert_eq!(&data[..8], b"MTTUPD01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x37f);
        let mut at = 16;
        for _ in 0..word(8) {
            let skipped = word(at + 4) != 0 || word(at + 8) as u8 as i8 > 0;
            let visible = word(at + 16) != 0;
            let active = word(at + 24) != 0;
            let count = word(at + 28) as usize;
            let mut time_events = 0;
            for i in 0..count {
                if word(at + 32 + i * 8) == 2 {
                    time_events += 1;
                }
            }
            assert_eq!(time_events, u32::from(!skipped && visible && active));
            at += 32 + count * 8;
        }
        let count = word(at);
        at += 4;
        for _ in 0..count {
            let mut runtime = super::Runtime::default();
            runtime.widgets[0].cash_icon_phase = word(at + 20) as i32;
            runtime.tick(0., word(at + 16) != 0, 0).unwrap();
            assert_eq!(runtime.widgets[0].cash_icon_phase as u32, word(at + 24));
            assert_eq!(word(at + 28), u32::from(word(at) != 0 && word(at + 4) == 0));
            at += 32;
        }
        let count = word(at);
        at += 4;
        for _ in 0..count {
            let passes = super::instance_passes(word(at + 4), word(at + 8));
            let count = word(at + 24) as usize;
            let actual = (0..count)
                .map(|i| word(at + 28 + i * 4))
                .collect::<Vec<_>>();
            let expected = std::iter::repeat_n(0x58fb40, passes[0] as usize)
                .chain(std::iter::repeat_n(0x565bb0, passes[1] as usize))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            at += 28 + count * 4;
        }
        // Reset+30 leaves both cache values untouched; show+34 sets−1.
        assert_eq!(word(at + 4), 0);
        assert_eq!(word(at + 12), 0);
        at += 16;
        assert_eq!(word(at + 4), (-1f32).to_bits());
        assert_eq!(word(at + 12), (-1f32).to_bits());
        at += 16;
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original Time model-bind initializer oracle"]
    fn original_time_widget_bind() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_BIND")).unwrap();
        assert_eq!(&data[..8], b"MTTBND01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x37f);
        for case in 0..word(8) as usize {
            let at = 16 + case * 96;
            let instance = word(at) as usize;
            let mut runtime = super::Runtime {
                amount: f32::from_bits(word(at + 4)),
                remaining: word(at + 8) as i32,
                ..Default::default()
            };
            runtime.widgets[instance] = super::Widget {
                last_amount: f32::from_bits(word(at + 12)),
                cash_icon_phase: word(at + 16) as i32,
                visible: word(at + 20) != 0,
                text_amount: Some(9876.),
            };
            runtime.chance_visuals[instance] = crate::card_panel::EntryVisual {
                reset: word(at + 24) != 0,
                blink: word(at + 28),
                textures: std::array::from_fn(|i| word(at + 32 + i * 4) as usize),
            };
            runtime.bind_panels();
            let widget = runtime.widgets[instance];
            assert_eq!(widget.display().visible, word(at + 64) != 0);
            let chance = runtime.chance_visuals[instance];
            let actual = [
                runtime.amount.to_bits(),
                runtime.remaining as u32,
                widget.last_amount.to_bits(),
                widget.cash_icon_phase as u32,
                u32::from(widget.visible),
                u32::from(chance.reset),
                chance.blink,
                chance.textures[0] as u32,
                chance.textures[1] as u32,
                chance.textures[2] as u32,
                chance.textures[3] as u32,
                u32::from(widget.display().cash.is_none()),
            ];
            for (i, value) in actual.into_iter().enumerate() {
                assert_eq!(
                    value,
                    word(at + 48 + i * 4),
                    "Time bind case{case} field{i}"
                );
            }
        }
        assert_eq!(data.len(), 16 + word(8) as usize * 96);
    }
    #[test]
    #[ignore = "requires original Time show/cache oracle"]
    fn original_time_widget_activation() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_ACTIVATION")).unwrap();
        assert_eq!(&data[..8], b"MTACT001");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x37f);
        for case in 0..word(8) as usize {
            let at = 16 + case * 44;
            let mut runtime = super::Runtime {
                amount: f32::from_bits(word(at + 4)),
                remaining: word(at + 8) as i32,
                ..Default::default()
            };
            runtime.widgets[0] = super::Widget {
                last_amount: f32::from_bits(word(at + 16)),
                cash_icon_phase: word(at + 20) as i32,
                visible: word(at + 12) != 0,
                text_amount: Some(9876.),
            };
            runtime.show(f32::from_bits(word(at)), 0).unwrap();
            assert_eq!(
                runtime.amount.to_bits(),
                word(at + 24),
                "show amount case{case}"
            );
            assert_eq!(
                runtime.remaining as u32,
                word(at + 28),
                "show divisor case{case}"
            );
            assert_eq!(
                runtime.widgets[0].last_amount.to_bits(),
                word(at + 32),
                "show cache case{case}"
            );
            assert_eq!(runtime.widgets[0].cash_icon_phase as u32, word(at + 36));
            let cash = if word(at + 40) == 0 {
                9876.
            } else {
                runtime.amount
            };
            assert_eq!(
                runtime.widgets[0].display().cash,
                Some(cash),
                "retained text case{case}"
            );
            assert!(runtime.widgets[0].display().visible);
            runtime.hide(0).unwrap();
            assert!(!runtime.widgets[0].display().visible);
            assert!(!runtime.widgets[0].visible);
            assert_eq!(runtime.widgets[0].display().cash, Some(cash));
        }
        assert_eq!(data.len(), 16 + word(8) as usize * 44);
    }
    #[test]
    #[ignore = "requires original font3 Options caption oracle"]
    fn original_options_caption() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_HUD_CAPTION")).unwrap();
        assert_eq!(&data[..8], b"MTHCPT01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x37f);
        let length = word(12) as usize;
        let bounds = std::array::from_fn(|i| f32::from_bits(word(16 + i * 4)));
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let tab = std::fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab")).unwrap();
        let quads =
            super::text_quads_with_offset(&tab, &data[36..36 + length], bounds, true, 0).unwrap();
        let mut at = 36 + length;
        for page in 0..2 {
            let count = word(at) as usize;
            at += 4;
            let quads = quads.iter().filter(|q| q.page == page).collect::<Vec<_>>();
            assert_eq!(count, quads.len());
            for (glyph, quad) in quads.iter().enumerate() {
                for (i, value) in quad.positions.into_iter().flatten().enumerate() {
                    assert_eq!(
                        value.to_bits(),
                        word(at + glyph * 48 + i * 4),
                        "Options glyph{glyph} position{i}"
                    );
                }
                for (i, value) in quad.uv.into_iter().flatten().enumerate() {
                    assert_eq!(value.to_bits(), word(at + count * 48 + glyph * 32 + i * 4));
                }
            }
            at += count * 80;
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original base/USA currency refresh oracle"]
    fn original_time_currency_refresh() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_CURRENCY")).unwrap();
        assert_eq!(&data[..8], b"MTTCUR01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 2);
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe_path = openmt2001::fixtures::path("EXE");
        let executable = std::fs::read(&exe_path).unwrap();
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let assets =
            super::Assets::load(&index, &archive, installer, &language, &executable).unwrap();
        let mut at = 12;
        let base_length = word(at) as usize + word(at + 4) as usize;
        assert_eq!(word(at + 8), 1);
        at += 12 + base_length;
        let symbol_length = word(at) as usize;
        let marker_length = word(at + 4) as usize;
        assert_eq!(word(at + 8), 0);
        assert_eq!(assets.currency_left, word(at + 8) == 0);
        assert_eq!(assets.currency, data[at + 12..at + 12 + symbol_length]);
        assert_eq!(data.len(), at + 12 + symbol_length + marker_length);
    }
    #[test]
    #[ignore = "requires original bottom HUD constructor/draw oracle"]
    fn original_bottom_control_draw() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_BOTTOM_HUD")).unwrap();
        assert_eq!(&data[..8], b"MTBHUD01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x37f);
        assert_eq!(word(16), 2);
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/brand_new_panels/new_nav_bottom.tm")
            .unwrap();
        let raw = &archive[entry.offset as usize..(entry.offset + entry.length) as usize];
        assert_eq!(&data[20..20 + word(12) as usize], raw);
        let mesh = openmt2001::mesh::CachedMesh::parse(raw).unwrap();
        let bounds = openmt2001::ui_mesh::regions(&mesh, 0)
            .unwrap()
            .into_iter()
            .find(|r| r.name == "NAV_TOP_VIEW_ICON")
            .unwrap()
            .bounds;
        let shifted = |delta| [bounds[0] + delta, bounds[1], bounds[2] + delta, bounds[3]];
        let mut at = 20 + raw.len();
        let backing = openmt2001::ui_mesh::regions(&mesh, 0)
            .unwrap()
            .into_iter()
            .find(|r| r.name == "NAV_BACKING_MAIN")
            .unwrap()
            .bounds;
        for (i, value) in backing.into_iter().enumerate() {
            assert_eq!(word(at + i * 4), value.to_bits());
        }
        assert_eq!(word(at + 16), 0.5f32.to_bits());
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let color = u32::from_le_bytes(image.at(0x5362c0, 4).unwrap().try_into().unwrap());
        for i in 0..4 {
            assert_eq!(word(at + 20 + i * 4), color);
        }
        assert_eq!(word(at + 36), 0.5f32.to_bits());
        at += 40;
        for _ in 0..2 {
            let fast = word(at) != 0;
            assert_eq!(word(at + 4), 4);
            at += 8;
            for (texture, [l, t, r, b]) in super::bottom_controls(shifted(38.), shifted(75.), fast)
            {
                assert_eq!(word(at), texture as u32);
                let points: [[f32; 3]; 4] = [[l, t, 0.5], [l, b, 0.5], [r, b, 0.5], [r, t, 0.5]];
                for (i, value) in points.into_iter().flatten().enumerate() {
                    assert_eq!(word(at + 4 + i * 4), value.to_bits());
                }
                let uv: [[f32; 2]; 4] = [[0., 0.], [0., 1.], [1., 1.], [1., 0.]];
                for (i, value) in uv.into_iter().flatten().enumerate() {
                    assert_eq!(word(at + 52 + i * 4), value.to_bits());
                }
                at += 84;
            }
        }
        assert_eq!(at, data.len());
        // Original640 texture loader writes ordinal handles at68ae28+i*4.
        assert_eq!(image.at(0x4c0f9f, 5).unwrap(), &[0xbf, 0x28, 0xae, 0x68, 0]);
        assert_eq!(image.at(0x4c0fc5, 3).unwrap(), &[0x83, 0xc7, 4]);
    }
    #[test]
    #[ignore = "requires original v1.4 executable"]
    fn original_bottom_navigation_dispatch() {
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        // Native 425490 dispatches group 12 option 0 to the compass,
        // option 1 to 1x/4x, and option 2 to the middle graph widget.
        assert_eq!(
            image.at(0x425490, 5).unwrap(),
            &[0x83, 0x7c, 0x24, 0x04, 0x0c]
        );
        let call_target = |at: u32| {
            let bytes = image.at(at, 5).unwrap();
            assert_eq!(bytes[0], 0xe8);
            (at + 5).wrapping_add_signed(i32::from_le_bytes(bytes[1..5].try_into().unwrap()))
        };
        assert_eq!(call_target(0x4254e0), 0x4baf10);
        assert_eq!(call_target(0x4254b3), 0x481940);
        assert_eq!(call_target(0x4254ab), 0x43be20);
        assert_eq!(image.f32_at(0x5251c4).unwrap(), 75.);
        assert_eq!(image.f32_at(0x5251c0).unwrap(), 37.);
        // 425120 clones NAV_TOP_VIEW_ICON at +75 and then at +75-37.
    }
    #[test]
    #[ignore = "requires original Time controller camera/depth oracle"]
    fn original_time_mesh_occlusion() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_TIME_MESH")).unwrap();
        assert_eq!(&data[..8], b"MTTMSH01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let scalar = |at| f32::from_bits(word(at));
        assert_eq!(word(8), 0x37f);
        let raw_length = word(12) as usize;
        let count = word(16) as usize;
        assert_eq!((count, word(20)), (88, 3));
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index.find(super::PANEL).unwrap();
        let raw = &archive[entry.offset as usize..(entry.offset + entry.length) as usize];
        assert_eq!(raw, &data[24..24 + raw_length]);
        let mesh = openmt2001::mesh::CachedMesh::parse(raw).unwrap();
        assert_eq!(mesh.parts.len(), 14);
        let bounds = openmt2001::ui_mesh::regions(&mesh, 0)
            .unwrap()
            .into_iter()
            .find(|r| r.name == "TIME_BACKING")
            .unwrap()
            .bounds;
        let mut at = 24 + raw_length + 48;
        let vertices = (0..count)
            .map(|i| std::array::from_fn::<_, 3, _>(|axis| scalar(at + i * 12 + axis * 4)))
            .collect::<Vec<_>>();
        at += count * 12;
        let [left, top, right, bottom] = bounds;
        for (i, point) in [[left, top], [left, bottom], [right, bottom], [right, top]]
            .into_iter()
            .enumerate()
        {
            assert_eq!(scalar(at + i * 12).to_bits(), point[0].to_bits());
            assert_eq!(scalar(at + i * 12 + 4).to_bits(), point[1].to_bits());
            assert_eq!(scalar(at + i * 12 + 8), 0.5);
        }
        at += 3 * 48;
        assert_eq!(word(at), 14);
        at += 4;
        for i in 0..14 {
            assert_eq!(word(at + i * 4), i as u32);
        }
        at += 14 * 4;
        let state_count = word(at) as usize;
        at += 4;
        let states = (0..state_count)
            .map(|i| (word(at + i * 8), word(at + i * 8 + 4)))
            .collect::<Vec<_>>();
        for contract in [(14, 1), (7, 1), (23, 4), (27, 1)] {
            assert!(states.contains(&contract), "native UI state {contract:?}");
        }
        at += state_count * 8;
        let transform_count = word(at) as usize;
        at += 4;
        let mut view = None;
        let mut projection = None;
        for i in 0..transform_count {
            let matrix = std::array::from_fn::<_, 16, _>(|j| scalar(at + i * 68 + 4 + j * 4));
            match word(at + i * 68) {
                2 => view = Some(matrix),
                3 => projection = Some(matrix),
                _ => {}
            }
        }
        at += transform_count * 68;
        assert_eq!(at, data.len());
        let view = view.unwrap();
        let projection = projection.unwrap();
        assert_eq!((view[10], view[14], view[15]), (1., 15., 1.));
        assert_eq!(projection[10], 0.0001);
        assert_eq!(projection[11], 0.);
        assert_eq!(projection[15], 1.);
        // Convex backing covers every triangle, and the common camera has
        // positive Z scale and constant W. All model fragments fail LEQUAL.
        for [x, y, z] in vertices {
            assert!(x >= left && x <= right && y >= top && y <= bottom);
            assert!(z > 0.5);
        }
    }
    #[test]
    #[ignore = "requires original top-right panel mesh"]
    fn inspect_original_regions() {
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/brand_new_panels/top_right_panel_v2.tm")
            .unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        for (i, part) in mesh.parts.iter().enumerate() {
            eprintln!("part {i}: {}", mesh.texture_names[part.texture as usize]);
        }
        for frame in 0..mesh.frames {
            let regions = openmt2001::ui_mesh::regions(&mesh, frame).unwrap();
            eprintln!("frame {frame}: {} regions", regions.len());
            for region in regions {
                eprintln!("{}: {:?}", region.name, region.bounds);
            }
        }
        let nav = index
            .find("gamedata2/brand_new_panels/new_nav_bottom.tm")
            .unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[nav.offset as usize..(nav.offset + nav.length) as usize],
        )
        .unwrap();
        for region in openmt2001::ui_mesh::regions(&mesh, 0).unwrap() {
            eprintln!("nav {}: {:?}", region.name, region.bounds);
        }
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original top-right panel mesh and GPU"]
    fn capture_original_mesh_parts() {
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let (surfaces, _) = crate::ui::panel_base(
            &index,
            &archive,
            "gamedata2/brand_new_panels/top_right_panel_v2.tm",
            &(0..14).collect::<Vec<_>>(),
        )
        .unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        let prepared = renderer
            .prepare(
                &surfaces
                    .iter()
                    .map(crate::ui::Surface::draw)
                    .collect::<Vec<_>>(),
                VIEW,
                None,
            )
            .unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &prepared,
            "work/frontend/time-panel-all-parts.png",
        );
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let executable = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let language = openmt2001::fixtures::path("EXE")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language, &executable).unwrap();
        let live = assets
            .prepare(
                &renderer,
                Display {
                    year: 1930,
                    hour: 12,
                    speed: 1,
                    name: "Player",
                    cash: Some(8000.),
                    fraction: 312,
                    character: Some(4),
                    cash_icon_phase: 0,
                },
            )
            .unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &live,
            "work/frontend/time-panel-live.png",
        );
    }
}
