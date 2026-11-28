//! In-game Save Game panel: original 496c70/4981d0 control family.
//! File publication belongs to save_menu and the Host worker. This view emits
//! an action only after the appropriate name or selected-slot confirmation.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

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
const ROW_HEIGHT: f32 = 14.;
const VISIBLE_ROWS: usize = 20;
// Original 496c70 -> 43f510, with 43f510's sixteen-pixel arrow children.
const LIST_SCROLL: [f32; 4] = [305., 70., 321., 383.];
const INFO_SCROLL: [f32; 4] = [588., 287., 604., 378.];
// Supplied Mac frame270, normalized against the native Save mesh landmarks.
// The Save As singleton's v1.4 constructor remains unrecovered.
const NAME_MODAL: [f32; 4] = [192., 178., 452., 308.];
const NAME_HEADER: [f32; 4] = [192., 178., 452., 203.];
const NAME_INPUT: [f32; 4] = [218., 220., 428., 247.];
const NAME_BUTTONS: [[f32; 4]; 2] = [[218., 265., 313., 289.], [335., 265., 429., 289.]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    SaveAs,
    Save,
    Back,
    Delete,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    NewName(String),
    ExistingSlot(String),
    DeleteSlot(String),
    Back,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Modal {
    Name,
    Overwrite,
    Delete,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CueTarget {
    Button(Button),
    ScrollArrow(usize, usize),
    Prompt(usize),
}

pub struct Assets {
    base: Vec<Surface>,
    bounds: Vec<[f32; 4]>,
    font1: Vec<Surface>,
    font2: Vec<Surface>,
    font4: Vec<Surface>,
    tab1: Vec<u8>,
    tab2: Vec<u8>,
    tab4: Vec<u8>,
    strings: Vec<Vec<u8>>,
    level_text: crate::load::LevelTextCatalog,
    slider_textures: Vec<Surface>,
}
impl Assets {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
    ) -> Result<Self, String> {
        let (base, bounds) = ui::panel_base(
            index,
            archive,
            "gamedata2/brand_new_panels/save_menu_V2.tm",
            &[0, 1, 2, 3, 4, 6, 7],
        )?;
        if bounds.len() != 26 {
            return Err("original Save Game region count".into());
        }
        let texture = index
            .find("gamedata2/font640/textures.gtd")
            .ok_or("Save Game font atlas missing")?;
        let atlas = TextureSet::parse(
            archive
                .get(texture.offset as usize..(texture.offset + texture.length) as usize)
                .ok_or("Save Game font atlas range")?,
        )
        .map_err(|e| e.to_string())?;
        let generic_entry = index
            .find("gamedata2/GenericTextures640/textures.gtd")
            .ok_or("Save Game generic scrollbar atlas missing")?;
        let generic = TextureSet::parse(
            archive
                .get(
                    generic_entry.offset as usize
                        ..(generic_entry.offset + generic_entry.length) as usize,
                )
                .ok_or("Save Game generic scrollbar atlas range")?,
        )
        .map_err(|e| e.to_string())?;
        let mut slider_textures = vec![Surface::solid(); 109];
        for (side, first) in [105, 93].into_iter().enumerate() {
            for state in 0..4 {
                slider_textures[first + state] =
                    Surface::new(generic.textures.get(first + state).ok_or_else(|| {
                        format!("original Save Game scrollbar arrow {side}:{state} missing")
                    })?);
            }
        }
        let tab1 = fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let tab2 = fs::read(installer.join("Standard/fonts/tahoma11noantip.tab"))
            .map_err(|e| e.to_string())?;
        let tab4 = fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
            .map_err(|e| e.to_string())?;
        let off = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
        let bin = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
        let mut cursor = 0usize;
        let mut rows = Vec::new();
        for length in off.chunks_exact(2) {
            let end = cursor + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            rows.push(bin.get(cursor..end).ok_or("Save Game language row range")?);
            cursor = end;
        }
        let strings = crate::tooltip::localized_strings(installer, &rows)?;
        for id in [
            0x597, 0x598, 0x59a, 0x59c, 0x59f, 0x64c, 0x64d, 0x64f, 0x653, 0x65a, 0x65b, 0x65e,
            0x65f, 0x660, 0x661, 0x664, 0x665, 0x6a7,
        ] {
            strings
                .get(id)
                .ok_or("Save Game localized caption missing")?;
        }
        Ok(Self {
            base,
            bounds,
            font1: atlas.textures[2..4].iter().map(Surface::new).collect(),
            font2: atlas.textures[4..6].iter().map(Surface::new).collect(),
            font4: atlas.textures[8..10].iter().map(Surface::new).collect(),
            tab1,
            tab2,
            tab4,
            strings,
            level_text: crate::load::LevelTextCatalog::default(),
            slider_textures,
        })
    }
    /// Retain registered metadata using this panel's active language glyphs.
    pub fn set_level_catalog(&mut self, catalog: &openmt2001::level_catalog::Catalog) {
        self.level_text = crate::load::LevelTextCatalog::new(catalog, &self.strings);
    }
    pub fn bounds(&self, button: Button) -> [f32; 4] {
        self.bounds[match button {
            Button::SaveAs => 24,
            Button::Save => 23,
            Button::Back => 25,
            Button::Delete => 22,
        }]
    }
    fn hit(&self, p: [f32; 2]) -> Option<Button> {
        [Button::SaveAs, Button::Save, Button::Back, Button::Delete]
            .into_iter()
            .find(|&button| inside(self.bounds(button), p))
    }
    fn list_hit(&self, p: [f32; 2], first: usize, len: usize) -> Option<usize> {
        let b = self.bounds[8];
        if inside(b, p) {
            let visible = ((p[1] - b[1]) / ROW_HEIGHT) as usize;
            let i = first + visible;
            (visible < VISIBLE_ROWS && i < len).then_some(i)
        } else {
            None
        }
    }
    fn info_rows(&self, view: &View) -> usize {
        let Some(slot) = view.selected.and_then(|i| view.slots.get(i)) else {
            return 0;
        };
        let Ok(words) = slot.metadata else { return 0 };
        let Some(description) = self.level_text.description(words[0], &self.strings) else {
            return 0;
        };
        let info = self.bounds[21];
        crate::prompt::wrap_with_gap(
            &self.tab2,
            description,
            (info[2] - info[0] - 18.).trunc() as i32,
            -1,
        )
        .map_or(0, |(offsets, _)| offsets.len().saturating_sub(1))
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        view: &View,
        pointer: Option<[f32; 2]>,
    ) -> Result<PreparedScene, String> {
        let mut shapes = Surface::solid();
        let mut slider_shapes = Surface::solid();
        let mut headings = Surface::solid();
        let mut font1 = self.font1.clone();
        let mut font2 = self.font2.clone();
        let mut font4 = self.font4.clone();
        let mut controls = Vec::new();
        let mut info_rows = 0usize;
        let b = &self.bounds;
        // Native 497ab0 submits the 433370 panel rectangles before 4419f0.
        shapes.rect(b[5], [0xff0000c0, 0xff0000c0, 0x800000c0, 0x800000c0]);
        shapes.rect(b[6], [0xff0000ff, 0xff0000ff, 0x800000ff, 0x800000ff]);
        for part in [8, 11] {
            shapes.rect(b[part], [0xbb000a28; 4]);
        }
        for (part, id) in [(9, 0x65b), (10, 0x661)] {
            headings.rect(b[part], [0xff909090, 0xff909090, 0x80909090, 0x80909090]);
            ui::text_widget_style9(
                &mut font1,
                &self.tab1,
                &self.strings[id],
                b[part],
                0xffffffff,
            )?;
        }
        ui::text_widget_style9(
            &mut font4,
            &self.tab4,
            &self.strings[0x65a],
            b[5],
            0xffffffff,
        )?;
        for (part, id) in [
            (12, 0x64c),
            (14, 0x64f),
            (16, 0x64d),
            (18, 0x653),
            (20, 0x6a7),
        ] {
            shapes.rect(b[part], [0xff01762f; 4]);
            ui::text_widget_style4(
                &mut font1,
                &self.tab1,
                &self.strings[id],
                b[part],
                0xffffffff,
            )?;
        }
        let list = b[8];
        for (i, slot) in view
            .slots
            .iter()
            .enumerate()
            .skip(view.first)
            .take(VISIBLE_ROWS)
        {
            let top = list[1] + (i - view.first) as f32 * ROW_HEIGHT;
            let row = [list[0], top, list[2], top + ROW_HEIGHT];
            if view.selected == Some(i) {
                shapes.rect(row, [0xff0037a6; 4]);
            }
            ui::text_fit(
                &mut font1,
                &self.tab1,
                &crate::load::ascii(&slot.name),
                [row[0] + 3., row[1], row[0] + 151., row[3]],
                false,
                0xffffffff,
            )?;
            if let Some(modified) = crate::save_menu::format_modified_local(slot.modified) {
                ui::text_fit(
                    &mut font1,
                    &self.tab1,
                    &crate::load::ascii(&modified),
                    [row[0] + 155., row[1], row[2] - 16., row[3]],
                    false,
                    0xff6ed1e1,
                )?;
            }
        }
        if let Some(slot) = view.selected.and_then(|i| view.slots.get(i)) {
            ui::text_widget_style4(
                &mut font1,
                &self.tab1,
                &crate::load::ascii(&slot.name),
                b[13],
                0xffffffff,
            )?;
            if let Ok(words) = slot.metadata {
                let level = self.level_text.title(words[0], &self.strings);
                ui::text_widget_style4(&mut font1, &self.tab1, &level, b[15], 0xffffffff)?;
                ui::text_widget_style4(
                    &mut font1,
                    &self.tab1,
                    &crate::load::ascii(&words[1].to_string()),
                    b[17],
                    0xffffffff,
                )?;
                ui::text_widget_style4(
                    &mut font1,
                    &self.tab1,
                    &ui::formatted_float_money(
                        f32::from_bits(words[2]),
                        self.strings
                            .get(2243)
                            .ok_or("Save currency symbol missing")?,
                        self.strings.get(2244).and_then(|row| row.first()) != Some(&0x4c),
                    )?,
                    b[19],
                    0xffffffff,
                )?;
                if let Some(description) = self.level_text.description(words[0], &self.strings) {
                    let info = b[21];
                    let (offsets, wrapped) = crate::prompt::wrap_with_gap(
                        &self.tab2,
                        description,
                        (info[2] - info[0] - 18.).trunc() as i32,
                        -1,
                    )?;
                    info_rows = offsets.len().saturating_sub(1);
                    for (line, range) in
                        offsets.windows(2).skip(view.info_first).take(9).enumerate()
                    {
                        let end = range[1].saturating_sub(1).min(wrapped.len());
                        let row = &wrapped[range[0].min(end)..end];
                        let top = info[1] + 2. + line as f32 * 10.;
                        ui::text_widget_style0(
                            &mut font2,
                            &self.tab2,
                            row,
                            [info[0], top, info[2] - 18., top + 10.],
                            0xffffffff,
                        )?;
                    }
                }
            }
        }
        // Native 431ba0→4326d0 gives list-linked sliders a paint extent
        // independent of the actual scroll clamp. Save's second-column row
        // counts are not recovered yet; the filled visible count is inferred
        // from the supplied Save frame's active short information scrollbar.
        for (index, bounds, value, maximum, paint_extent) in [
            (
                0,
                LIST_SCROLL,
                view.first,
                view.slots.len().saturating_sub(VISIBLE_ROWS),
                if view.slots.is_empty() {
                    0
                } else {
                    view.slots.len().max(VISIBLE_ROWS) - VISIBLE_ROWS + 1
                },
            ),
            (
                1,
                INFO_SCROLL,
                view.info_first,
                info_rows.saturating_sub(9),
                if info_rows == 0 {
                    0
                } else {
                    info_rows.max(9) - 9 + 1
                },
            ),
        ] {
            ui::Slider {
                bounds,
                vertical: true,
                value: value as f32,
                range: [0., maximum as f32],
                pointer,
                pressed: view
                    .scroll_capture
                    .filter(|(which, _)| *which == index)
                    .map(|(_, point)| point),
            }
            .paint_with_extent(
                &mut slider_shapes,
                &mut controls,
                &self.slider_textures,
                paint_extent as i32,
            );
        }
        for button in [Button::SaveAs, Button::Save, Button::Back, Button::Delete] {
            let disabled = view.busy
                || (matches!(button, Button::Save | Button::Delete) && view.selected.is_none());
            let color = match button {
                Button::SaveAs => [166, 133, 0],
                Button::Save => [80, 148, 34],
                Button::Back | Button::Delete => [175, 6, 6],
            };
            ui::button(
                &mut shapes,
                self.bounds(button),
                color,
                !disabled && pointer.is_some_and(|p| self.hit(p) == Some(button)),
                !disabled && view.pressed == Some(button),
                disabled,
            )?;
            let id = match button {
                Button::SaveAs => 0x597,
                Button::Save => 0x598,
                Button::Back => 0x59f,
                Button::Delete => 0x59a,
            };
            ui::text_widget_style9(
                &mut font1,
                &self.tab1,
                &self.strings[id],
                self.bounds(button),
                if disabled { 0xff646464 } else { 0xffffffff },
            )?;
        }
        if let Some(message) = &view.message {
            ui::text_fit(
                &mut font1,
                &self.tab1,
                &crate::load::ascii(message),
                [32., 438., 608., 458.],
                false,
                0xffffffff,
            )?;
        }
        let mut surfaces = self.base.clone();
        surfaces.push(shapes);
        // Native 43e790 draws slider children before Save's 497ab0 heading
        // overlay. Its y70 list arrow is dimmed by the later y71..94 heading.
        surfaces.push(slider_shapes);
        surfaces.extend(controls);
        surfaces.push(headings);
        surfaces.extend(font1);
        surfaces.extend(font2);
        surfaces.extend(font4);
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            VIEW,
            None,
        )
    }
    pub fn prepare_name_modal(
        &self,
        renderer: &Renderer,
        view: &View,
        pointer: Option<[f32; 2]>,
    ) -> Result<Option<PreparedScene>, String> {
        if view.modal != Some(Modal::Name) {
            return Ok(None);
        }
        let mut shapes = Surface::solid();
        let mut font = self.font1.clone();
        self.paint_name_modal(&mut shapes, &mut font, view, pointer)?;
        let mut surfaces = vec![shapes];
        surfaces.extend(font);
        renderer
            .prepare(
                &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
                VIEW,
                None,
            )
            .map(Some)
    }
    fn paint_name_modal(
        &self,
        shapes: &mut Surface,
        font: &mut [Surface],
        view: &View,
        pointer: Option<[f32; 2]>,
    ) -> Result<(), String> {
        // The supplied named-save keyframe bounds this central modal;
        // its text row uses the shared native input-control painter.
        shapes.rect(NAME_MODAL, [0xcc000000; 4]);
        shapes.rect(
            NAME_HEADER,
            [0xff909090, 0xff909090, 0x80909090, 0x80909090],
        );
        ui::text_widget_style9(
            font,
            &self.tab1,
            &self.strings[0x598],
            NAME_HEADER,
            0xffffffff,
        )?;
        let body = crate::load::ascii(&view.name);
        shapes.rect(NAME_INPUT, [0xff0724a2; 4]);
        ui::text_entry(font, shapes, &self.tab1, &body, NAME_INPUT, true, true)?;
        for (part, id, color) in [(true, 0x598, [80, 148, 34]), (false, 0x59c, [175, 6, 6])] {
            let bounds = NAME_BUTTONS[usize::from(!part)];
            ui::button(
                shapes,
                bounds,
                color,
                pointer.is_some_and(|p| inside(bounds, p)),
                view.modal_pressed == Some(part),
                false,
            )?;
            ui::text_widget_style9(font, &self.tab1, &self.strings[id], bounds, 0xffffffff)?;
        }
        let [left, top, right, bottom] = NAME_MODAL;
        for (start, end) in [
            ([left, top], [right, top]),
            ([right, top], [right, bottom]),
            ([right, bottom], [left, bottom]),
            ([left, bottom], [left, top]),
        ] {
            shapes.line(start, end, 0xffaaaaaa);
        }
        Ok(())
    }
}

fn inside([l, t, r, b]: [f32; 4], [x, y]: [f32; 2]) -> bool {
    x >= l && x <= r && y >= t && y <= b
}

#[derive(Default)]
pub struct View {
    pub visible: bool,
    pub busy: bool,
    pub slots: Vec<crate::save_menu::Slot>,
    selected: Option<usize>,
    first: usize,
    info_first: usize,
    scroll_capture: Option<(usize, [f32; 2])>,
    pressed: Option<Button>,
    held: bool,
    cue_latch: Option<CueTarget>,
    pressed_row: Option<usize>,
    modal: Option<Modal>,
    modal_pressed: Option<bool>,
    name: String,
    pub message: Option<String>,
}
impl View {
    pub fn generic_prompt(&self) -> Option<(crate::human_dialog::Dialog, Option<usize>)> {
        let (title, body) = match self.modal.as_ref()? {
            Modal::Overwrite => (0x661, 0x65f),
            Modal::Delete => (0x65e, 0x660),
            Modal::Name => return None,
        };
        let selected_name = self.selected_name()?;
        let mut name = [0u8; 96];
        // The original prompt font accepts printable ASCII only. Imported V2
        // saves may have Unicode names; keep the opaque slot ID intact and
        // substitute unsupported glyphs in the confirmation text alone.
        for (at, character) in name.iter_mut().take(95).zip(selected_name.chars()) {
            *at = if (' '..='~').contains(&character) {
                character as u8
            } else {
                b'?'
            };
        }
        Some((
            crate::human_dialog::Dialog {
                title,
                body,
                arguments: [
                    crate::human_dialog::Argument::Name(name),
                    crate::human_dialog::Argument::Integer(0),
                    crate::human_dialog::Argument::Integer(0),
                ],
                argument_count: 1,
                buttons: [0x664, 0x665],
                button_count: 2,
                timeout: false,
                ..crate::human_dialog::Dialog::default()
            },
            self.modal_pressed.map(|yes| usize::from(!yes)),
        ))
    }
    pub fn mouse_generic(
        &mut self,
        pointer: Option<[f32; 2]>,
        pressed: bool,
        buttons: [[f32; 4]; 2],
    ) -> Option<Action> {
        if !self.visible
            || self.busy
            || !matches!(self.modal, Some(Modal::Overwrite | Modal::Delete))
        {
            return None;
        }
        self.held = pressed;
        if !pressed {
            self.cue_latch = None;
        }
        let hit = pointer.and_then(|point| {
            buttons
                .into_iter()
                .enumerate()
                .find(|(_, bounds)| inside(*bounds, point))
                .map(|(index, _)| index == 0)
        });
        if pressed {
            self.modal_pressed = hit;
            return None;
        }
        let activate = (hit == self.modal_pressed).then_some(hit).flatten();
        self.modal_pressed = None;
        match activate {
            Some(true) => self.confirm(),
            Some(false) => {
                self.modal = None;
                None
            }
            None => None,
        }
    }
    /// The two confirmation children are generic 443360 buttons too. Their
    /// latch resets on held leave, independently of Save's page controls.
    pub fn prompt_click_cue(&mut self, pointer: Option<[f32; 2]>, buttons: [[f32; 4]; 2]) -> bool {
        if !self.visible
            || !self.held
            || self.busy
            || !matches!(self.modal, Some(Modal::Overwrite | Modal::Delete))
        {
            self.cue_latch = None;
            return false;
        }
        let target = pointer.and_then(|point| {
            buttons
                .into_iter()
                .enumerate()
                .find(|(_, bounds)| inside(*bounds, point))
                .map(|(index, _)| CueTarget::Prompt(index))
        });
        let cue = target.is_some() && target != self.cue_latch;
        self.cue_latch = target;
        cue
    }
    pub fn open(&mut self, profile_root: &Path) {
        self.visible = true;
        self.busy = false;
        self.first = 0;
        self.info_first = 0;
        self.scroll_capture = None;
        self.pressed = None;
        self.held = false;
        self.cue_latch = None;
        self.pressed_row = None;
        self.modal = None;
        self.modal_pressed = None;
        self.name.clear();
        self.message = None;
        match crate::save_menu::list_slots(profile_root) {
            Ok(slots) => {
                self.slots = slots;
                self.selected = None;
            }
            Err(error) => {
                self.slots.clear();
                self.selected = None;
                self.message = Some(error);
            }
        }
    }
    pub fn close(&mut self) {
        self.visible = false;
        self.modal = None;
        self.busy = false;
        self.held = false;
        self.cue_latch = None;
        self.pressed_row = None;
        self.scroll_capture = None;
    }
    pub fn selected_name(&self) -> Option<&str> {
        self.selected
            .and_then(|i| self.slots.get(i))
            .map(|s| s.name.as_str())
    }
    pub fn scroll(&mut self, delta: i32) {
        self.first = (self.first as i32 + delta)
            .clamp(0, self.slots.len().saturating_sub(VISIBLE_ROWS) as i32)
            as usize;
    }
    pub fn input(&mut self, text: &str) {
        if self.modal != Some(Modal::Name) {
            return;
        }
        for c in text.chars() {
            if c.is_ascii()
                && !c.is_ascii_control()
                && !"/\\:*?\"<>|".contains(c)
                && self.name.len() < 120
            {
                self.name.push(c);
            }
        }
    }
    pub fn erase(&mut self) {
        if self.modal == Some(Modal::Name) {
            self.name.pop();
        }
    }
    /// Returns true only when the outer page closed. A modal Cancel leaves
    /// the Save Game page open, as in the supplied named-save frame.
    pub fn escape(&mut self) -> bool {
        if self.modal.take().is_some() {
            self.modal_pressed = None;
            false
        } else {
            self.close();
            true
        }
    }
    pub fn enter(&mut self) -> Option<Action> {
        self.confirm()
    }
    fn confirm(&mut self) -> Option<Action> {
        let action = match self.modal.as_ref()? {
            Modal::Name if !self.name.is_empty() => Action::NewName(self.name.clone()),
            Modal::Overwrite => Action::ExistingSlot(self.slots.get(self.selected?)?.id.clone()),
            Modal::Delete => Action::DeleteSlot(self.slots.get(self.selected?)?.id.clone()),
            Modal::Name => return None,
        };
        self.modal = None;
        self.modal_pressed = None;
        Some(action)
    }
    pub fn mouse(
        &mut self,
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        pressed: bool,
    ) -> Option<Action> {
        if !self.visible {
            return None;
        }
        self.held = pressed;
        if !pressed {
            self.cue_latch = None;
        }
        if self.busy {
            if !pressed {
                self.pressed = None;
                self.pressed_row = None;
                self.scroll_capture = None;
            }
            return None;
        }
        if self.modal == Some(Modal::Name) {
            let hit = pointer.and_then(|p| {
                if inside(NAME_BUTTONS[0], p) {
                    Some(true)
                } else if inside(NAME_BUTTONS[1], p) {
                    Some(false)
                } else {
                    None
                }
            });
            if pressed {
                self.modal_pressed = hit;
                return None;
            }
            let activate = (hit == self.modal_pressed).then_some(hit).flatten();
            self.modal_pressed = None;
            return match activate {
                Some(true) => self.confirm(),
                Some(false) => {
                    self.modal = None;
                    None
                }
                None => None,
            };
        }
        if pressed {
            self.cue_latch = None;
            if let Some((index, point)) = pointer.and_then(|point| {
                [LIST_SCROLL, INFO_SCROLL]
                    .into_iter()
                    .enumerate()
                    .find(|(_, bounds)| inside(*bounds, point))
                    .map(|(index, _)| (index, point))
            }) {
                self.scroll_capture = Some((index, point));
                self.pressed = None;
                self.pressed_row = None;
                return None;
            }
        } else if let Some((index, _)) = self.scroll_capture.take() {
            let bounds = [LIST_SCROLL, INFO_SCROLL][index];
            if let Some(point) = pointer.filter(|&point| inside(bounds, point)) {
                let maximum = if index == 0 {
                    self.slots.len().saturating_sub(VISIBLE_ROWS)
                } else {
                    assets.info_rows(self).saturating_sub(9)
                };
                let current = if index == 0 {
                    self.first
                } else {
                    self.info_first
                };
                let arrows = ui::slider_arrows(bounds, true);
                let selected = if inside(arrows[0], point) {
                    current.saturating_sub(1)
                } else if inside(arrows[1], point) {
                    current.saturating_add(1).min(maximum)
                } else if inside(ui::slider_track(bounds, true), point) {
                    ui::slider_selection(
                        ui::slider_track(bounds, true),
                        point[1],
                        maximum as i32,
                        true,
                    ) as usize
                } else {
                    current
                };
                if index == 0 {
                    self.first = selected;
                } else {
                    self.info_first = selected;
                }
            }
            return None;
        }
        let hit = pointer.and_then(|p| assets.hit(p));
        if pressed {
            self.cue_latch = None;
            self.pressed = hit;
            self.pressed_row =
                pointer.and_then(|p| assets.list_hit(p, self.first, self.slots.len()));
            return None;
        }
        let activation = (hit == self.pressed).then_some(hit).flatten();
        self.pressed = None;
        if let Some(button) = activation {
            match button {
                Button::SaveAs => {
                    self.name.clear();
                    self.modal = Some(Modal::Name);
                }
                Button::Save if self.selected.is_some() => self.modal = Some(Modal::Overwrite),
                Button::Delete if self.selected.is_some() => self.modal = Some(Modal::Delete),
                Button::Back => {
                    self.close();
                    return Some(Action::Back);
                }
                _ => (),
            }
        } else if let Some(i) = pointer
            .and_then(|p| assets.list_hit(p, self.first, self.slots.len()))
            .filter(|&i| self.pressed_row == Some(i))
        {
            self.selected = Some(i);
            self.info_first = 0;
            self.message = None;
        }
        self.pressed_row = None;
        None
    }
    /// Native generic 443360 buttons sound on first held entry. Save list
    /// rows and slider tracks are separate, silent widget classes.
    pub fn pointer_click_cue(&mut self, assets: &Assets, pointer: Option<[f32; 2]>) -> bool {
        if !self.visible || !self.held || self.busy || self.modal.is_some() {
            self.cue_latch = None;
            return false;
        }
        let target = pointer.and_then(|point| {
            if let Some(button) = assets.hit(point)
                && (!matches!(button, Button::Save | Button::Delete) || self.selected.is_some())
            {
                return Some(CueTarget::Button(button));
            }
            for (index, (bounds, maximum)) in [
                (LIST_SCROLL, self.slots.len().saturating_sub(VISIBLE_ROWS)),
                (INFO_SCROLL, assets.info_rows(self).saturating_sub(9)),
            ]
            .into_iter()
            .enumerate()
            {
                if maximum == 0 {
                    continue;
                }
                for (arrow, region) in ui::slider_arrows(bounds, true).into_iter().enumerate() {
                    if inside(region, point) {
                        return Some(CueTarget::ScrollArrow(index, arrow));
                    }
                }
            }
            None
        });
        let cue = target.is_some() && target != self.cue_latch;
        self.cue_latch = target;
        cue
    }
    pub fn saved(&mut self, result: Result<std::path::PathBuf, String>, profile_root: &Path) {
        self.busy = false;
        match result {
            Ok(path) => {
                self.message = Some(format!(
                    "Saved: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                if let Ok(slots) = crate::save_menu::list_slots(profile_root) {
                    self.selected = slots.iter().position(|s| {
                        s.id == path.file_name().unwrap_or_default().to_string_lossy()
                    });
                    self.slots = slots;
                }
            }
            Err(error) => self.message = Some(format!("Save failed: {error}")),
        }
    }
    pub fn deleted(&mut self, result: Result<(), String>, profile_root: &Path) {
        self.busy = false;
        match result {
            Ok(()) => {
                self.message = Some("Saved game deleted".into());
                if let Ok(slots) = crate::save_menu::list_slots(profile_root) {
                    self.slots = slots;
                    self.selected = None;
                }
            }
            Err(error) => self.message = Some(format!("Delete failed: {error}")),
        }
    }
}

#[cfg(test)]
#[path = "save_list_order_tests.rs"]
mod save_list_order_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_dialog_requires_confirmed_name_or_selected_slot() {
        let mut bounds = vec![[0.; 4]; 26];
        bounds[8] = [20., 40., 120., 327.];
        bounds[24] = [10., 360., 100., 390.];
        bounds[23] = [110., 360., 200., 390.];
        bounds[25] = [210., 360., 300., 390.];
        bounds[22] = [310., 360., 400., 390.];
        let assets = Assets {
            base: Vec::new(),
            bounds,
            font1: Vec::new(),
            font2: Vec::new(),
            font4: Vec::new(),
            tab1: Vec::new(),
            tab2: Vec::new(),
            tab4: Vec::new(),
            strings: Vec::new(),
            level_text: crate::load::LevelTextCatalog::default(),
            slider_textures: Vec::new(),
        };
        assert_eq!(
            assets.list_hit([30., 326.], 0, 21),
            None,
            "bottom seven pixels are outside the twenty painted rows"
        );
        let mut view = View {
            visible: true,
            ..View::default()
        };
        view.slots.push(crate::save_menu::Slot {
            id: "prior.sav".into(),
            name: "prior".into(),
            modified: None,
            metadata: Err("fixture".into()),
        });
        for i in 1..21 {
            view.slots.push(crate::save_menu::Slot {
                id: format!("slot{i}.sav"),
                name: format!("slot{i}"),
                modified: None,
                metadata: Err("fixture".into()),
            });
        }
        view.mouse(&assets, Some([50., 375.]), true);
        assert!(view.pointer_click_cue(&assets, Some([50., 375.])));
        assert!(!view.pointer_click_cue(&assets, Some([50., 375.])));
        assert!(!view.pointer_click_cue(&assets, None));
        assert!(view.pointer_click_cue(&assets, Some([50., 375.])));
        view.mouse(&assets, None, false);
        assert!(!view.pointer_click_cue(&assets, Some([50., 375.])));
        view.mouse(&assets, Some([30., 326.]), true);
        assert!(!view.pointer_click_cue(&assets, Some([30., 326.])));
        view.mouse(&assets, Some([30., 326.]), false);
        assert_eq!(view.selected, None, "unpainted 21st row is not selectable");
        view.mouse(&assets, Some([310., 375.]), true);
        assert!(view.pointer_click_cue(&assets, Some([310., 375.])));
        assert!(!view.pointer_click_cue(&assets, Some([310., 375.])));
        view.mouse(&assets, Some([310., 375.]), false);
        assert_eq!(view.first, 1, "native bottom arrow scrolls one row");
        view.mouse(&assets, Some([310., 75.]), true);
        view.mouse(&assets, Some([310., 75.]), false);
        assert_eq!(view.first, 0, "native top arrow scrolls one row");
        view.slots.truncate(1);
        // Save without a selected row neither writes nor opens a modal.
        assert!(view.mouse(&assets, Some([150., 375.]), true).is_none());
        assert!(!view.pointer_click_cue(&assets, Some([150., 375.])));
        assert!(view.mouse(&assets, Some([150., 375.]), false).is_none());
        assert_eq!(view.modal, None);
        view.mouse(&assets, Some([50., 375.]), true);
        view.mouse(&assets, Some([50., 375.]), false);
        assert_eq!(view.modal, Some(Modal::Name));
        view.input("test1-inprogressgame");
        assert_eq!(view.name, "test1-inprogressgame");
        assert!(view.mouse(&assets, Some([350., 250.]), true).is_none());
        assert!(view.mouse(&assets, Some([350., 250.]), false).is_none());
        assert_eq!(
            view.modal,
            Some(Modal::Name),
            "old button location is inert"
        );
        assert!(view.mouse(&assets, Some([380., 277.]), true).is_none());
        assert!(view.mouse(&assets, Some([380., 277.]), false).is_none());
        assert!(view.visible && view.modal.is_none());
        view.mouse(&assets, Some([50., 375.]), true);
        view.mouse(&assets, Some([50., 375.]), false);
        view.input("new-save");
        assert_eq!(view.enter(), Some(Action::NewName("new-save".into())));
        view.mouse(&assets, Some([10., 10.]), true);
        view.mouse(&assets, Some([30., 45.]), false);
        assert_eq!(
            view.selected, None,
            "release over row must follow a list press"
        );
        view.mouse(&assets, Some([30., 45.]), true);
        view.mouse(&assets, Some([30., 45.]), false);
        assert_eq!(view.selected, Some(0));
        view.mouse(&assets, Some([150., 375.]), true);
        view.mouse(&assets, Some([150., 375.]), false);
        assert_eq!(view.modal, Some(Modal::Overwrite));
        view.slots[0].name = "caf\u{e9}".into();
        let (unicode_dialog, _) = view.generic_prompt().unwrap();
        assert!(matches!(unicode_dialog.arguments[0],
            crate::human_dialog::Argument::Name(name) if &name[..5] == b"caf?\0"));
        assert_eq!(
            view.slots[0].id, "prior.sav",
            "display fallback cannot change the target"
        );
        view.slots[0].name = "prior".into();
        let generic_buttons = [[278., 274., 338., 299.], [392., 274., 452., 299.]];
        let (dialog, _) = view.generic_prompt().unwrap();
        assert_eq!((dialog.title, dialog.body), (0x661, 0x65f));
        assert_eq!(dialog.buttons, [0x664, 0x665]);
        // The screenshot-backed Save As hitboxes do not activate this distinct
        // original generic confirmation object.
        view.mouse_generic(Some([260., 250.]), true, generic_buttons);
        assert_eq!(
            view.mouse_generic(Some([260., 250.]), false, generic_buttons),
            None
        );
        assert_eq!(view.modal, Some(Modal::Overwrite));
        view.mouse_generic(Some([420., 285.]), true, generic_buttons);
        assert_eq!(
            view.mouse_generic(Some([420., 285.]), false, generic_buttons),
            None
        );
        assert_eq!(view.modal, None, "native second button cancels");
        view.mouse(&assets, Some([150., 375.]), true);
        view.mouse(&assets, Some([150., 375.]), false);
        view.mouse_generic(Some([300., 285.]), true, generic_buttons);
        assert!(view.prompt_click_cue(Some([300., 285.]), generic_buttons));
        assert!(!view.prompt_click_cue(Some([300., 285.]), generic_buttons));
        assert!(!view.prompt_click_cue(None, generic_buttons));
        assert!(view.prompt_click_cue(Some([300., 285.]), generic_buttons));
        assert_eq!(
            view.mouse_generic(Some([300., 285.]), false, generic_buttons),
            Some(Action::ExistingSlot("prior.sav".into()))
        );
        view.mouse(&assets, Some([350., 375.]), true);
        view.mouse(&assets, Some([350., 375.]), false);
        assert_eq!(view.modal, Some(Modal::Delete));
        let (dialog, _) = view.generic_prompt().unwrap();
        assert_eq!((dialog.title, dialog.body), (0x65e, 0x660));
        view.mouse_generic(Some([300., 285.]), true, generic_buttons);
        assert_eq!(
            view.mouse_generic(Some([300., 285.]), false, generic_buttons),
            Some(Action::DeleteSlot("prior.sav".into()))
        );
        view.mouse(&assets, Some([250., 375.]), true);
        assert_eq!(
            view.mouse(&assets, Some([250., 375.]), false),
            Some(Action::Back)
        );
        assert!(!view.visible);
    }
    #[test]
    #[ignore = "original v1.4 Save Game mesh/button fixture"]
    fn original_save_panel_bounds_and_buttons() {
        let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameters.parent().unwrap().parent().unwrap();
        let executable = openmt2001::fixtures::path("EXE");
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language).unwrap();
        let fixture = fs::read(openmt2001::fixtures::path("NATIVE_SAVE_DIALOG")).unwrap();
        assert_eq!(&fixture[..8], b"MTSAVE01");
        let mut at = 8;
        fn word(fixture: &[u8], at: &mut usize) -> u32 {
            let value = u32::from_le_bytes(fixture[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        }
        assert_eq!(word(&fixture, &mut at), 26);
        assert_eq!(word(&fixture, &mut at), 4);
        let geometry: [[f32; 4]; 26] = std::array::from_fn(|_| {
            std::array::from_fn(|_| f32::from_bits(word(&fixture, &mut at)))
        });
        assert_eq!(assets.bounds, geometry);
        for (part, id, rgb) in [
            (24, 0x597, [166, 133, 0]),
            (23, 0x598, [80, 148, 34]),
            (25, 0x59f, [175, 6, 6]),
            (22, 0x59a, [175, 6, 6]),
        ] {
            assert_eq!(word(&fixture, &mut at), part);
            assert_eq!(word(&fixture, &mut at), id);
            assert_eq!(&fixture[at..at + 3], &rgb);
            at += 3;
            assert!(geometry[part as usize][2] > geometry[part as usize][0]);
        }
        for (button, part) in [
            (Button::SaveAs, 24),
            (Button::Save, 23),
            (Button::Back, 25),
            (Button::Delete, 22),
        ] {
            assert_eq!(assets.bounds(button), geometry[part]);
            let b = geometry[part];
            assert_eq!(
                assets.hit([(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5]),
                Some(button)
            );
        }
        assert_eq!(at, fixture.len());
        let scroll_fixture =
            fs::read(openmt2001::fixtures::path("NATIVE_SAVE_SCROLLBARS")).unwrap();
        assert_eq!(&scroll_fixture[..8], b"MTSVSL01");
        let mut scroll_at = 8;
        assert_eq!(word(&scroll_fixture, &mut scroll_at), 0x037f);
        assert_eq!(word(&scroll_fixture, &mut scroll_at), 2);
        for (expected_id, bounds) in [(18, LIST_SCROLL), (20, INFO_SCROLL)] {
            assert_eq!(word(&scroll_fixture, &mut scroll_at), expected_id);
            assert_eq!(word(&scroll_fixture, &mut scroll_at), bounds[0] as u32);
            assert_eq!(word(&scroll_fixture, &mut scroll_at), bounds[1] as u32);
            assert_eq!(
                word(&scroll_fixture, &mut scroll_at),
                (bounds[3] - bounds[1]) as u32
            );
            assert_eq!(word(&scroll_fixture, &mut scroll_at), 1);
            let native = std::array::from_fn::<_, 17, _>(|_| {
                f32::from_bits(word(&scroll_fixture, &mut scroll_at))
            });
            assert_eq!(native[0..4], ui::slider_track(bounds, true));
            assert_eq!(native[4..8], ui::slider_track(bounds, true));
            assert_eq!(native[8], 0.5);
            let arrows = ui::slider_arrows(bounds, true);
            assert_eq!(native[9..13], arrows[0]);
            assert_eq!(native[13..17], arrows[1]);
        }
        assert_eq!(scroll_at, scroll_fixture.len());
        #[cfg(feature = "capture")]
        {
            use openmt2001::{
                flyby::CameraSample,
                render_scene::{self, OriginalAssets, Request, Source},
            };
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let build_file = read("OPENMT2001_BUILD_FILE");
            let suncols = read("OPENMT2001_SUNCOLS");
            let map = read("OPENMT2001_INTRO_MAP");
            let exe = read("OPENMT2001_EXE");
            let camera = CameraSample {
                eye: [26., 586., -695.],
                target: [26., 463., -595.],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            };
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
                camera: Some(camera),
                width: 1280,
                height: 960,
                animated: true,
            })
            .unwrap();
            let mut view = View {
                visible: true,
                ..View::default()
            };
            let panel = assets.prepare(&renderer, &view, None).unwrap();
            crate::capture_test::capture_scene_overlays(
                &renderer,
                &scene,
                &[&panel],
                "work/frontend/save-game-empty-city.png",
            );
            let saved = read("OPENMT2001_SAVE_ONE");
            let metadata = openmt2001::save::SaveFile::parse(&saved)
                .unwrap()
                .level_metadata();
            view.slots.push(crate::save_menu::Slot {
                id: "test1-inprogressgame.sav".into(),
                name: "test1-inprogressgame".into(),
                modified: Some(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_782_073_260),
                ),
                metadata,
            });
            view.selected = Some(0);
            let panel = assets.prepare(&renderer, &view, None).unwrap();
            crate::capture_test::capture_scene_overlays(
                &renderer,
                &scene,
                &[&panel],
                "work/frontend/save-game-selected-city.png",
            );
            let strings = assets.strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let generic = crate::prompt::Assets::load(
                &index,
                &archive,
                installer,
                &strings,
                &openmt2001::pe::PeImage::parse(&exe).unwrap(),
            )
            .unwrap();
            let buttons = generic.dialog_buttons(2);
            assert_eq!(buttons.len(), 2);
            view.modal = Some(Modal::Overwrite);
            let yes = [
                (buttons[0][0] + buttons[0][2]) * 0.5,
                (buttons[0][1] + buttons[0][3]) * 0.5,
            ];
            view.mouse_generic(Some(yes), true, [buttons[0], buttons[1]]);
            assert_eq!(
                view.mouse_generic(Some(yes), false, [buttons[0], buttons[1]]),
                Some(Action::ExistingSlot("test1-inprogressgame.sav".into()))
            );
            for (modal, output) in [
                (
                    Modal::Overwrite,
                    "work/frontend/save-game-overwrite-modal-city.png",
                ),
                (
                    Modal::Delete,
                    "work/frontend/save-game-delete-modal-city.png",
                ),
            ] {
                view.modal = Some(modal);
                let (dialog, pressed_button) = view.generic_prompt().unwrap();
                let page = assets.prepare(&renderer, &view, None).unwrap();
                let popup = generic
                    .prepare_dialog(&renderer, dialog, [-1.; 2], pressed_button)
                    .unwrap();
                crate::capture_test::capture_scene_overlays(
                    &renderer,
                    &scene,
                    &[&page, &popup],
                    output,
                );
            }
            view.modal = None;
            view.slots.clear();
            view.selected = None;
            view.modal = Some(Modal::Name);
            view.name = "test1-".into();
            let panel = assets.prepare(&renderer, &view, None).unwrap();
            let modal = assets
                .prepare_name_modal(&renderer, &view, None)
                .unwrap()
                .unwrap();
            crate::capture_test::capture_scene_overlays(
                &renderer,
                &scene,
                &[&panel, &modal],
                "work/frontend/save-game-named-modal-city.png",
            );
        }
    }
}
