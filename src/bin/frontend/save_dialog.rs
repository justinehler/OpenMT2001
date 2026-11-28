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
// Original 496c70 -> 43f510, with 43f510's sixteen-pixel arrow children.
const LIST_SCROLL: [f32; 4] = [305., 70., 321., 383.];
const INFO_SCROLL: [f32; 4] = [588., 287., 604., 378.];
// Supplied Mac frame270, normalized against the native Save mesh landmarks.
// The Save As singleton's v1.4 constructor remains unrecovered.
/// Caption region and language row of Name, Scenario, Time, Cash and
/// Scenario Information (0x542f40); each value region follows its caption.
const FIELD_CAPTIONS: [(usize, usize); 5] = [
    (12, 0x64c),
    (14, 0x64f),
    (16, 0x64d),
    (18, 0x653),
    (20, 0x6a7),
];
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
    list_layout: openmt2001::frontend_list::Layout,
    info_layout: openmt2001::frontend_list::Layout,
    font1: Vec<Surface>,
    font2: Vec<Surface>,
    font4: Vec<Surface>,
    tab1: Vec<u8>,
    tab2: Vec<u8>,
    tab4: Vec<u8>,
    strings: Vec<Vec<u8>>,
    level_text: openmt2001::level_text::LevelTextCatalog,
    slider_textures: Vec<Surface>,
}
impl Assets {
    /// The Save As field's font (496250 configures font 1); language row 0
    /// maps the name's code-page bytes to glyphs.
    fn name_font(&self) -> ui::text_field::FieldFont<'_> {
        self.field_font(&self.tab1)
    }
    fn field_font<'a>(&'a self, tab: &'a [u8]) -> ui::text_field::FieldFont<'a> {
        ui::text_field::FieldFont {
            tab,
            map: self.strings.first().map_or(&[], Vec::as_slice),
        }
    }
    pub fn apply_board(
        &mut self,
        board: &openmt2001::packed_language::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

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
        // 496c70 selects font2, gap2, inset3, and shortens the list by25.
        let mut list_bounds = bounds[8];
        list_bounds[3] -= 25.;
        list_bounds[2] -= 16.;
        let list_layout = openmt2001::frontend_list::Layout::from_font(list_bounds, &tab2)?;
        let mut info_bounds = bounds[21];
        info_bounds[2] -= 16.;
        let info_layout = openmt2001::frontend_list::Layout::from_font(info_bounds, &tab2)?;
        Ok(Self {
            base,
            bounds,
            list_layout,
            info_layout,
            font1: atlas.textures[2..4].iter().map(Surface::new).collect(),
            font2: atlas.textures[4..6].iter().map(Surface::new).collect(),
            font4: atlas.textures[8..10].iter().map(Surface::new).collect(),
            tab1,
            tab2,
            tab4,
            strings,
            level_text: openmt2001::level_text::LevelTextCatalog::default(),
            slider_textures,
        })
    }
    /// Retain registered metadata using this panel's active language glyphs.
    pub fn set_level_catalog(&mut self, catalog: &openmt2001::level_catalog::Catalog) {
        self.level_text = openmt2001::level_text::LevelTextCatalog::new(catalog, &self.strings);
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
        self.list_layout.hit(p, first, len)
    }
    fn info_rows(&self, view: &View) -> usize {
        let Some(slot) = view.selected_slot() else {
            return 0;
        };
        let Ok(words) = slot.metadata else { return 0 };
        let Some(description) = self.level_text.description(words[0], &self.strings) else {
            return 0;
        };
        let info = self.bounds[21];
        openmt2001::prompt_queue::wrap_with_gap(
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
        panel_alpha: u8,
    ) -> Result<PreparedScene, String> {
        let surfaces = self.surfaces(view, pointer, panel_alpha)?;
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            VIEW,
            None,
        )
    }
    /// The panel in draw order: mesh, 497ab0's rectangles, sliders,
    /// headings, then the three fonts. `panel_alpha` is the secondary
    /// group's fade on the bound save_menu_V2.tm model; 497ab0's rectangles
    /// and the text keep their own colours.
    fn surfaces(
        &self,
        view: &View,
        pointer: Option<[f32; 2]>,
        panel_alpha: u8,
    ) -> Result<Vec<Surface>, String> {
        let mut shapes = Surface::solid();
        let mut slider_shapes = Surface::solid();
        let mut headings = Surface::solid();
        let mut font1 = self.font1.clone();
        let mut font2 = self.font2.clone();
        let mut font4 = self.font4.clone();
        let mut controls = Vec::new();
        let mut info_rows = 0usize;
        let b = &self.bounds;
        // Native 497ab0 submits the 433370 panel rectangles before 4419f0:
        // the SAVE_TITLE_AREA gradient, then the 0x5362c0 list and
        // information backings. Region 6 is SAVE_BORDER04, drawn only by the
        // mesh; Load's region 6 is its title area.
        shapes.rect(b[5], [0xff0000c0, 0xff0000c0, 0x800000c0, 0x800000c0]);
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
        // 497ab0 fills each SAVE_LABELn caption bar, then its SAVE_INFOn
        // value band; 496c70 gives the captions text attribute 16.
        for (part, id) in FIELD_CAPTIONS {
            shapes.rect(b[part], [ui::FIELD_HEADER_BAR; 4]);
            shapes.rect(b[part + 1], [ui::FIELD_VALUE_BAND; 4]);
            ui::text_widget_style4(
                &mut font1,
                &self.tab1,
                &self.strings[id],
                b[part],
                ui::FIELD_CAPTION_COLOR,
            )?;
        }
        for (i, slot) in view
            .slots
            .iter()
            .enumerate()
            .skip(view.first)
            .take(self.list_layout.visible_rows())
        {
            let row = self.list_layout.text_bounds(i - view.first);
            if view.selected == Some(i) {
                shapes.rect(self.list_layout.highlight(i - view.first), [0xff0037a6; 4]);
            }
            ui::text_fit(
                &mut font2,
                &self.tab2,
                &self.field_font(&self.tab2).glyphs(&slot.name),
                [row[0] + 3., row[1], row[0] + 151., row[3]],
                false,
                0xffffffff,
            )?;
            let suffixes = crate::save_menu::DaySuffixes::from_strings(&self.strings);
            if let Some(modified) = crate::save_menu::format_modified_local(slot.modified, suffixes)
            {
                ui::text_fit(
                    &mut font2,
                    &self.tab2,
                    &modified,
                    [row[0] + 155., row[1], row[2], row[3]],
                    false,
                    0xff6ed1e1,
                )?;
            }
        }
        if let Some(slot) = view.selected_slot() {
            ui::text_widget_style4(
                &mut font1,
                &self.tab1,
                &self.name_font().glyphs(&slot.name),
                b[13],
                0xffffffff,
            )?;
            if let Ok(words) = slot.metadata {
                let level = self.level_text.title(words[0], &self.strings);
                ui::text_widget_style4(&mut font1, &self.tab1, &level, b[15], 0xffffffff)?;
                ui::text_widget_style4(
                    &mut font1,
                    &self.tab1,
                    &openmt2001::level_text::ascii(&words[1].to_string()),
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
                    let (offsets, wrapped) = openmt2001::prompt_queue::wrap_with_gap(
                        &self.tab2,
                        description,
                        (info[2] - info[0] - 18.).trunc() as i32,
                        -1,
                    )?;
                    info_rows = offsets.len().saturating_sub(1);
                    for (line, range) in offsets
                        .windows(2)
                        .skip(view.info_first)
                        .take(self.info_layout.visible_rows())
                        .enumerate()
                    {
                        let end = range[1].saturating_sub(1).min(wrapped.len());
                        let row = &wrapped[range[0].min(end)..end];
                        ui::text_widget_style0(
                            &mut font2,
                            &self.tab2,
                            row,
                            self.info_layout.text_bounds(line),
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
                view.slots
                    .len()
                    .saturating_sub(self.list_layout.visible_rows()),
                if view.slots.is_empty() {
                    0
                } else {
                    view.slots.len().max(self.list_layout.visible_rows())
                        - self.list_layout.visible_rows()
                        + 1
                },
            ),
            (
                1,
                INFO_SCROLL,
                view.info_first,
                info_rows.saturating_sub(self.info_layout.visible_rows()),
                if info_rows == 0 {
                    0
                } else {
                    info_rows.max(self.info_layout.visible_rows()) - self.info_layout.visible_rows()
                        + 1
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
            let disabled = !view.button_enabled(button);
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
        let mut surfaces = self.base.clone();
        crate::modal_frame::set_alpha(&mut surfaces, panel_alpha);
        surfaces.push(shapes);
        // Native 43e790 draws slider children before Save's 497ab0 heading
        // overlay. Its y70 list arrow is dimmed by the later y71..94 heading.
        surfaces.push(slider_shapes);
        surfaces.extend(controls);
        surfaces.push(headings);
        surfaces.extend(font1);
        surfaces.extend(font2);
        surfaces.extend(font4);
        Ok(surfaces)
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
        let body = self.name_font().glyphs(&view.name);
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
                !view.name_button_enabled(part),
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
    /// The last engine-side failure. The original has no failure text for
    /// saving or deleting, so it is logged rather than drawn.
    pub message: Option<String>,
}
impl View {
    pub fn fail(&mut self, error: String) {
        eprintln!("Save Game: {error}");
        self.message = Some(error);
    }
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
                // 497f60 and 4981d0 never set a portrait on the 88c0d4 owner.
                portrait: crate::human_dialog::NO_PORTRAIT,
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
                self.fail(error);
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
        self.selected_slot().map(|s| s.name.as_str())
    }
    /// The row whose details fill the Overwrite panel, if any.
    fn selected_slot(&self) -> Option<&crate::save_menu::Slot> {
        self.selected.and_then(|i| self.slots.get(i))
    }
    /// Save and Delete act on the selected row; every button is disabled
    /// while a write or delete is in flight.
    fn button_enabled(&self, button: Button) -> bool {
        !self.busy && (!matches!(button, Button::Save | Button::Delete) || self.selected.is_some())
    }
    pub fn scroll(&mut self, assets: &Assets, delta: i32) {
        self.first = (self.first as i32 + delta).clamp(
            0,
            self.slots
                .len()
                .saturating_sub(assets.list_layout.visible_rows()) as i32,
        ) as usize;
    }
    /// Typed text for the Save As name (496250): filename-safe, 155 px.
    pub fn input(&mut self, assets: &Assets, text: &str) {
        if self.modal != Some(Modal::Name) {
            return;
        }
        ui::text_field::Rule::SAVE_NAME.type_text(&mut self.name, text, assets.name_font());
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
    /// The Save As pop-up's Save is disabled while the name is empty and
    /// enabled once a character is typed; Cancel is always enabled (0x4965c0,
    /// text-change event 1).
    fn name_button_enabled(&self, save: bool) -> bool {
        !save || !self.name.is_empty()
    }
    pub fn enter(&mut self) -> Option<Action> {
        if self.modal == Some(Modal::Name) {
            self.submit_name(true)
        } else {
            self.confirm()
        }
    }
    /// Commits the Save As name (0x4965c0). Both paths ignore an empty name;
    /// Return leaves the field focused. The Save button writes `name.sav`
    /// straight away, overwriting a save of that name without asking. Only
    /// Return checks for a save with exactly that filename (0x4978a0): it
    /// selects that row and opens the Overwrite prompt (0x497f60), whose Yes
    /// rewrites the selected save (0x497d80). Otherwise Return writes too.
    fn submit_name(&mut self, by_return: bool) -> Option<Action> {
        if self.name.is_empty() {
            return None;
        }
        self.modal = None;
        self.modal_pressed = None;
        let file = format!("{}.sav", self.name);
        if by_return && let Some(index) = self.slots.iter().position(|slot| slot.id == file) {
            self.selected = Some(index);
            self.info_first = 0;
            self.message = None;
            self.modal = Some(Modal::Overwrite);
            return None;
        }
        // The original's direct write replaces an existing file whose name
        // differs only in case, since Windows filenames ignore case.
        Some(
            match self
                .slots
                .iter()
                .find(|slot| slot.id.eq_ignore_ascii_case(&file))
            {
                Some(slot) => Action::ExistingSlot(slot.id.clone()),
                None => Action::NewName(self.name.clone()),
            },
        )
    }
    fn confirm(&mut self) -> Option<Action> {
        let action = match self.modal.as_ref()? {
            Modal::Name => return self.submit_name(false),
            Modal::Overwrite => Action::ExistingSlot(self.slots.get(self.selected?)?.id.clone()),
            Modal::Delete => Action::DeleteSlot(self.slots.get(self.selected?)?.id.clone()),
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
            let hit = pointer
                .and_then(|p| {
                    if inside(NAME_BUTTONS[0], p) {
                        Some(true)
                    } else if inside(NAME_BUTTONS[1], p) {
                        Some(false)
                    } else {
                        None
                    }
                })
                .filter(|&save| self.name_button_enabled(save));
            if pressed {
                self.modal_pressed = hit;
                return None;
            }
            let activate = (hit == self.modal_pressed).then_some(hit).flatten();
            self.modal_pressed = None;
            return match activate {
                Some(true) => self.submit_name(false),
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
                    self.slots
                        .len()
                        .saturating_sub(assets.list_layout.visible_rows())
                } else {
                    assets
                        .info_rows(self)
                        .saturating_sub(assets.info_layout.visible_rows())
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
                && self.button_enabled(button)
            {
                return Some(CueTarget::Button(button));
            }
            for (index, (bounds, maximum)) in [
                (
                    LIST_SCROLL,
                    self.slots
                        .len()
                        .saturating_sub(assets.list_layout.visible_rows()),
                ),
                (
                    INFO_SCROLL,
                    assets
                        .info_rows(self)
                        .saturating_sub(assets.info_layout.visible_rows()),
                ),
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
    /// A finished Save As or Save. The original rebuilds the rows after every
    /// write and leaves nothing selected, so the Overwrite panel is empty and
    /// Save/Delete are disabled; it shows no confirmation text (Save As:
    /// 0x4965c0 then 0x497660, which resets the selection to -1; Save:
    /// 0x498110, 0x497d80, 0x497e70).
    pub fn saved(&mut self, result: Result<std::path::PathBuf, String>, profile_root: &Path) {
        self.busy = false;
        match result {
            Ok(_) => self.refresh_rows(profile_root),
            Err(error) => self.fail(format!("Save failed: {error}")),
        }
    }
    /// A finished Delete. Like a write, it rebuilds the rows, clears the
    /// selection and disables Save/Delete without a status line (0x498030,
    /// 0x497c70).
    pub fn deleted(&mut self, result: Result<(), String>, profile_root: &Path) {
        self.busy = false;
        match result {
            Ok(()) => self.refresh_rows(profile_root),
            Err(error) => self.fail(format!("Delete failed: {error}")),
        }
    }
    /// Re-enumerates the profile's saves with no row selected (0x497660).
    fn refresh_rows(&mut self, profile_root: &Path) {
        self.selected = None;
        self.info_first = 0;
        self.message = None;
        match crate::save_menu::list_slots(profile_root) {
            Ok(slots) => self.slots = slots,
            Err(error) => {
                self.slots.clear();
                self.fail(error);
            }
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
    fn confirmation_fade_reaches_the_panel_model_only() {
        // 497f60/4981d0 raise the prompt with 43cb80, so 43bce0 fades the
        // bound save_menu_V2.tm model; 497ab0's rectangles keep their colours.
        let (tab1, glyph_map) = ui::text_field::synthetic_font();
        let mut part = Surface::solid();
        part.rect([0., 0., 640., 480.], [0xff00_24c6; 4]);
        let assets = Assets {
            base: vec![part.clone(), part],
            info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
                .unwrap(),
            bounds: vec![[20., 40., 300., 80.]; 26],
            list_layout: openmt2001::frontend_list::Layout::new([20., 40., 104., 302.], 12., 3.)
                .unwrap(),
            font1: vec![Surface::solid(); 2],
            font2: vec![Surface::solid(); 2],
            font4: vec![Surface::solid(); 2],
            tab2: tab1.clone(),
            tab4: tab1.clone(),
            tab1,
            strings: vec![glyph_map; 0x700],
            level_text: openmt2001::level_text::LevelTextCatalog::default(),
            slider_textures: vec![Surface::solid(); 128],
        };
        let view = View {
            visible: true,
            modal: Some(Modal::Overwrite),
            ..View::default()
        };
        let open = assets.surfaces(&view, None, 255).unwrap();
        let faded = assets
            .surfaces(&view, None, crate::modal_frame::SETTLED_ALPHA)
            .unwrap();
        for (open, faded) in open.iter().zip(&faded).take(assets.base.len()) {
            assert_eq!(open.alpha, [255; 6]);
            assert_eq!(faded.alpha, [125; 6]);
            assert_eq!(faded.vertices, open.vertices);
        }
        for (open, faded) in open.iter().zip(&faded).skip(assets.base.len()) {
            assert_eq!(faded.alpha, open.alpha);
            assert_eq!(faded.vertices, open.vertices);
        }
    }
    #[test]
    fn save_dialog_requires_confirmed_name_or_selected_slot() {
        let mut bounds = vec![[0.; 4]; 26];
        bounds[8] = [20., 40., 120., 327.];
        bounds[24] = [10., 360., 100., 390.];
        bounds[23] = [110., 360., 200., 390.];
        bounds[25] = [210., 360., 300., 390.];
        bounds[22] = [310., 360., 400., 390.];
        let (tab1, glyph_map) = ui::text_field::synthetic_font();
        let assets = Assets {
            base: Vec::new(),
            info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
                .unwrap(),
            bounds,
            list_layout: openmt2001::frontend_list::Layout::new([20., 40., 104., 302.], 12., 3.)
                .unwrap(),
            font1: Vec::new(),
            font2: Vec::new(),
            font4: Vec::new(),
            tab1,
            tab2: Vec::new(),
            tab4: Vec::new(),
            strings: vec![glyph_map],
            level_text: openmt2001::level_text::LevelTextCatalog::default(),
            slider_textures: Vec::new(),
        };
        assert_eq!(
            assets.list_hit([30., 326.], 0, 21),
            None,
            "lower strip is outside the painted viewport"
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
        for i in 1..25 {
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
        assert_eq!(
            view.selected, None,
            "unpainted lower strip is not selectable"
        );
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
        view.input(&assets, "test1-inprogressgame");
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
        // 496250's field is filename-safe and stops at 155 px of font 1.
        view.input(&assets, &"W".repeat(20));
        assert_eq!(view.name, "W".repeat(12));
        view.name.clear();
        view.input(&assets, "new?-s\u{e9}ve");
        assert_eq!(view.enter(), Some(Action::NewName("new-s\u{e9}ve".into())));
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
    /// #560 and 0x4965c0: Save is drawn disabled and ignores clicks until a
    /// character is typed; the button overwrites an existing save without
    /// asking, while Return opens the Overwrite prompt for it.
    #[test]
    fn save_as_button_state_and_overwrite_rules_follow_the_original() {
        let (tab1, glyph_map) = ui::text_field::synthetic_font();
        let mut strings = vec![Vec::new(); 0x5a0];
        strings[0] = glyph_map;
        let assets = Assets {
            base: Vec::new(),
            info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
                .unwrap(),
            bounds: vec![[0.; 4]; 26],
            list_layout: openmt2001::frontend_list::Layout::new([20., 40., 104., 302.], 12., 3.)
                .unwrap(),
            font1: Vec::new(),
            font2: Vec::new(),
            font4: Vec::new(),
            tab1,
            tab2: Vec::new(),
            tab4: Vec::new(),
            strings,
            level_text: openmt2001::level_text::LevelTextCatalog::default(),
            slider_textures: Vec::new(),
        };
        let button_rects = |disabled: bool| {
            let mut surface = Surface::solid();
            ui::button(
                &mut surface,
                NAME_BUTTONS[0],
                [80, 148, 34],
                false,
                false,
                disabled,
            )
            .unwrap();
            solid_rects(&surface)
        };
        let painted_save = |view: &View| {
            let mut shapes = Surface::solid();
            let mut font = vec![Surface::solid(); 4];
            assets
                .paint_name_modal(&mut shapes, &mut font, view, None)
                .unwrap();
            // Outline lines straddle the bounds by half a pixel.
            let [l, t, r, b] = NAME_BUTTONS[0];
            solid_rects(&shapes)
                .into_iter()
                .filter(|([x0, y0, x1, y1], _)| {
                    *x0 >= l - 1. && *y0 >= t - 1. && *x1 <= r + 1. && *y1 <= b + 1.
                })
                .collect::<Vec<_>>()
        };
        assert_ne!(button_rects(true), button_rects(false));
        let save = [260., 277.];
        let click = |view: &mut View, point| {
            assert!(view.mouse(&assets, Some(point), true).is_none());
            view.mouse(&assets, Some(point), false)
        };
        let mut view = View {
            visible: true,
            modal: Some(Modal::Name),
            ..View::default()
        };
        view.slots.push(crate::save_menu::Slot {
            id: "prior.sav".into(),
            name: "prior".into(),
            modified: None,
            metadata: Err("fixture".into()),
        });

        // Empty name: Save is drawn disabled, ignores a click, and Return
        // leaves the pop-up open on the field.
        assert_eq!(painted_save(&view), button_rects(true));
        assert_eq!(click(&mut view, save), None);
        assert_eq!(view.modal_pressed, None);
        assert_eq!(view.enter(), None);
        assert_eq!(view.modal, Some(Modal::Name));

        // One character enables it.
        view.input(&assets, "p");
        assert_eq!(painted_save(&view), button_rects(false));
        view.erase();
        assert_eq!(painted_save(&view), button_rects(true));

        // The button writes an existing name straight over it, no prompt.
        view.input(&assets, "prior");
        assert_eq!(
            click(&mut view, save),
            Some(Action::ExistingSlot("prior.sav".into()))
        );
        assert_eq!(view.modal, None);
        assert_eq!(view.selected, None);

        // Return with the same name selects that save and opens Overwrite.
        view.modal = Some(Modal::Name);
        assert_eq!(view.enter(), None);
        assert_eq!(view.modal, Some(Modal::Overwrite));
        assert_eq!(view.selected, Some(0));
        let (dialog, _) = view.generic_prompt().unwrap();
        assert_eq!((dialog.title, dialog.body), (0x661, 0x65f));
        assert_eq!(view.enter(), Some(Action::ExistingSlot("prior.sav".into())));

        // A new name saves directly from either path.
        for by_return in [true, false] {
            view.modal = Some(Modal::Name);
            view.name = "fresh".into();
            let action = if by_return {
                view.enter()
            } else {
                click(&mut view, save)
            };
            assert_eq!(action, Some(Action::NewName("fresh".into())));
            assert_eq!(view.modal, None);
        }

        // Return's existence test compares the filename exactly; a name that
        // differs only in case is written over the existing file directly.
        view.modal = Some(Modal::Name);
        view.selected = None;
        view.name = "PRIOR".into();
        assert_eq!(view.enter(), Some(Action::ExistingSlot("prior.sav".into())));
        assert_eq!(view.modal, None);
    }
    #[test]
    #[ignore = "requires original v1.4 Save Game mesh, fonts, USA language and a supplied save"]
    fn original_save_panel_field_bars_bands_and_mint_captions() {
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
        let saved = fs::read(openmt2001::fixtures::path("SAVE_ONE")).unwrap();
        let metadata = openmt2001::save::SaveFile::parse(&saved)
            .unwrap()
            .level_metadata();
        let mut view = View {
            visible: true,
            ..View::default()
        };
        view.slots.push(crate::save_menu::Slot {
            id: "s9tmp.sav".into(),
            name: "s9tmp".into(),
            modified: None,
            metadata,
        });
        view.selected = Some(0);
        let surfaces = assets.surfaces(&view, None, 255).unwrap();
        let rects = solid_rects(&surfaces[assets.base.len()]);
        // 497ab0: the title gradient and the two 0x5362c0 backings only; no
        // full-panel blue fill brightens the Overwrite panel.
        assert_eq!(
            rects[..3],
            [
                (assets.bounds[5], 0xff0000c0),
                (assets.bounds[8], 0xbb000a28),
                (assets.bounds[11], 0xbb000a28),
            ]
        );
        let mut fields = Vec::new();
        for (part, _) in FIELD_CAPTIONS {
            let bar = rects
                .iter()
                .position(|r| *r == (assets.bounds[part], ui::FIELD_HEADER_BAR))
                .unwrap();
            assert_eq!(
                rects[bar + 1],
                (assets.bounds[part + 1], ui::FIELD_VALUE_BAND)
            );
            fields.push((assets.bounds[part], assets.bounds[part + 1]));
            let fonts = &surfaces[surfaces.len() - 6..];
            assert!(glyphs_in(fonts, assets.bounds[part], ui::FIELD_CAPTION_COLOR) > 0);
            assert_eq!(glyphs_in(fonts, assets.bounds[part], 0xffffffff), 0);
        }
        assert_eq!(fields, ORIGINAL_SAVE_FIELDS);
    }
    /// SAVE_LABELn caption bars and their SAVE_INFOn value bands in the
    /// original save_menu_V2.tm (640x480 panel units).
    const ORIGINAL_SAVE_FIELDS: [([f32; 4], [f32; 4]); 5] = [
        (
            [337.91446, 98.10548, 604.47546, 116.04093],
            [337.91446, 119.04093, 604.47546, 136.97638],
        ),
        (
            [337.91446, 140.07507, 604.4219, 158.1092],
            [337.91446, 161.15662, 604.48016, 179.19075],
        ),
        (
            [337.91446, 182.25648, 604.4485, 200.23598],
            [337.91446, 203.2561, 604.46356, 221.2356],
        ),
        (
            [337.91446, 224.22992, 604.4485, 242.20941],
            [337.91446, 245.22954, 604.46356, 263.20905],
        ),
        (
            [337.91446, 266.19623, 604.4575, 284.09625],
            [337.91446, 287.08127, 604.495, 378.99414],
        ),
    ];
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
            let panel = assets.prepare(&renderer, &view, None, 255).unwrap();
            crate::capture_test::capture_scene_overlays(
                &renderer,
                &scene,
                &[&panel],
                "frontend/save-game-empty-city.png",
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
            let panel = assets.prepare(&renderer, &view, None, 255).unwrap();
            crate::capture_test::capture_scene_overlays(
                &renderer,
                &scene,
                &[&panel],
                "frontend/save-game-selected-city.png",
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
            // The portrait-free 88c0d4 owner that 497f60 and 4981d0 raise,
            // shifted by (-40, 0) like the in-city Quit confirmation.
            let buttons = generic.frontend_dialog_buttons(2);
            assert_original_confirmation_buttons(&buttons);
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
                    "frontend/save-game-overwrite-modal-city.png",
                ),
                (Modal::Delete, "frontend/save-game-delete-modal-city.png"),
            ] {
                view.modal = Some(modal);
                let (dialog, pressed_button) = view.generic_prompt().unwrap();
                assert_eq!(dialog.portrait, crate::human_dialog::NO_PORTRAIT);
                // 43cb80 fades the page behind the raised prompt.
                let page = assets
                    .prepare(&renderer, &view, None, crate::modal_frame::SETTLED_ALPHA)
                    .unwrap();
                let popup = generic
                    .prepare_screen_confirmation(&renderer, dialog, [-1.; 2], pressed_button)
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
            let panel = assets.prepare(&renderer, &view, None, 255).unwrap();
            let modal = assets
                .prepare_name_modal(&renderer, &view, None)
                .unwrap()
                .unwrap();
            crate::capture_test::capture_scene_overlays(
                &renderer,
                &scene,
                &[&panel, &modal],
                "frontend/save-game-named-modal-city.png",
            );
        }
    }
}
/// The original's Overwrite and Delete Saved Game Yes/No buttons, measured at
/// 1024x768 under Wine (#531): Yes about (382-477, 441-478), No about
/// (565-658, 441-478), the same on Save Game and Load Game. 429630 makes each
/// button 60 units (96 pixels) wide, so the measurements are good to a few
/// pixels.
#[cfg(any(test, feature = "test-support"))]
pub fn assert_original_confirmation_buttons(buttons: &[[f32; 4]]) {
    let expected = [[382., 441., 477., 478.], [565., 441., 658., 478.]];
    assert_eq!(buttons.len(), 2);
    for (bounds, expected) in buttons.iter().zip(expected) {
        let at_1024 = bounds.map(|v| v * 1.6);
        for (actual, expected) in at_1024.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= 3.,
                "{at_1024:?} differs from the original {expected:?}"
            );
        }
    }
}
/// Solid rectangles of a shapes surface in submission order, with their
/// ARGB colour (each `Surface::rect` is one six-vertex quad).
#[cfg(any(test, feature = "test-support"))]
pub fn solid_rects(surface: &Surface) -> Vec<([f32; 4], u32)> {
    surface
        .vertices
        .as_chunks::<6>()
        .0
        .iter()
        .zip(surface.alpha.as_chunks::<6>().0)
        .map(|(quad, alpha)| {
            (
                [quad[0][0], quad[0][1], quad[2][0], quad[2][1]],
                vertex_color(&quad[0], alpha[0]),
            )
        })
        .collect()
}
#[cfg(any(test, feature = "test-support"))]
fn vertex_color(vertex: &[f32; 8], alpha: u8) -> u32 {
    let channel = |v: f32| (v * 255.).round() as u32;
    u32::from(alpha) << 24 | channel(vertex[3]) << 16 | channel(vertex[4]) << 8 | channel(vertex[5])
}
/// Number of glyph quads inside `bounds` drawn in `color` across `pages`.
#[cfg(any(test, feature = "test-support"))]
pub fn glyphs_in(pages: &[Surface], bounds: [f32; 4], color: u32) -> usize {
    pages
        .iter()
        .flat_map(|page| {
            page.vertices
                .as_chunks::<6>()
                .0
                .iter()
                .zip(page.alpha.as_chunks::<6>().0)
        })
        .filter(|(quad, alpha)| {
            vertex_color(&quad[0], alpha[0]) == color && inside(bounds, [quad[0][0], quad[0][1]])
        })
        .count()
}
