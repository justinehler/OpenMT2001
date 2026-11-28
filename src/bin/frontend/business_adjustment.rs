//! Original Business Adjustment views over saved or live business records.
use openmt2001::business_pool::BusinessCatalog;
#[derive(Debug, Clone)]
pub struct Row {
    pub commodity: u32,
    pub name: u32,
    pub cost: f32,
    pub price: f32,
    pub yesterday: u16,
    pub wanted: i16,
    pub stock: u16,
    pub today: u16,
}
pub fn rows(
    record: &[u8; 544],
    info: &BusinessCatalog,
    commodities: &[[u8; 28]; 32],
    products: &[u32],
) -> Result<Vec<Row>, String> {
    let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    if word(0) != 0 {
        return Err("retail adjustment requires retail business".into());
    }
    let entry = info
        .row(word(4) as usize)
        .ok_or("adjustment business kind")?;
    let count = usize::from(entry[0x4c]);
    if count > 6 || products.len() != count {
        return Err("adjustment ordered catalog count".into());
    }
    let mut mask = 0u32;
    for &id in products {
        if id >= 32 || mask & (1 << id) != 0 {
            return Err("adjustment commodity identity".into());
        }
        mask |= 1 << id;
    }
    if mask != u32::from_le_bytes(entry[0x70..0x74].try_into().unwrap()) {
        return Err("default catalog order disagrees with saved mask".into());
    }
    let bank = usize::from(record[0x209]);
    if bank >= 4 {
        return Err("adjustment sales history bank".into());
    }
    let short = |at| u16::from_le_bytes(record[at..at + 2].try_into().unwrap());
    products
        .iter()
        .enumerate()
        .map(|(slot, &id)| {
            let commodity = &commodities[id as usize];
            let cost = f32::from_le_bytes(commodity[0x14..0x18].try_into().unwrap());
            let price = f32::from_bits(word(0x1ec + slot * 4));
            if !cost.is_finite() || !price.is_finite() {
                return Err("nonfinite saved adjustment display value".into());
            }
            let stock = 0x80 + slot * 0x34;
            Ok(Row {
                commodity: id,
                name: u32::from_le_bytes(commodity[8..12].try_into().unwrap()),
                cost,
                price,
                yesterday: short(stock + 2 + ((bank + 3) & 3) * 2),
                wanted: short(0x1e0 + slot * 2) as i16,
                stock: short(stock),
                today: short(stock + 2 + bank * 2),
            })
        })
        .collect()
}

use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    render_scene::Scene,
    texture::TextureSet,
};
use std::time::{Duration, Instant};
use std::{fs, path::Path};
/// Original41903d..4192df ordered runtime fills. Textured panel parts alone
/// omit these colours; the title uses the business owner's original palette.
fn retail_palette(body: u32, owner: u32) -> Vec<(&'static str, [u32; 4])> {
    let mut out = Vec::new();
    for name in [
        "BAP_BOTTOM_AREA",
        "BAP_COMMODITY_LIST",
        "BAP_COST_LIST",
        "BAP_YESTERDAY_LIST",
        "BAP_STOCK_I_HAVE_LIST",
        "BAP_TODAY_LIST",
    ] {
        out.push((name, [body; 4]));
    }
    out.push((
        "BAP_TOP_TITLE_AREA",
        [
            owner.wrapping_sub(0x01000000),
            owner.wrapping_sub(0x01000000),
            owner.wrapping_add(0x7f000000),
            owner.wrapping_add(0x7f000000),
        ],
    ));
    for name in [
        "BAP_TOPBLUE_BACKING",
        "BAP_SELLING_TYPE_AREA",
        "BAP_YESTERDAY_TYPE_AREA",
        "BAP_TODAY_TYPE_AREA",
    ] {
        out.push((name, [0xff1f3070; 4]));
    }
    for name in [
        "BAP_COMMODIDTY_TITLE",
        "BAP_COST_TITLE",
        "BAP_SELLING_TITLE",
        "BAP_YESTERDAY_TITLE",
        "BAP_STOCK_I_WANT_TITLE",
        "BAP_STOCK_I_HAVE_TITLE",
        "BAP_TODAY_TITLE",
        "BAP_TOTAL_LABEL",
        "BAP_YESTERDAY_TOTAL",
        "BAP_STOCK_I_WANT_TOTAL",
        "BAP_STOCK_I_HAVE_TOTAL",
        "BAP_TODAY_TOTAL",
    ] {
        out.push((name, [0x6d4f94b0; 4]));
    }
    for name in ["BAP_SELLING_LIST", "BAP_STOCK_I_WANT_LIST"] {
        out.push((name, [0xff202a53; 4]));
    }
    out
}
/// Original40c5ef..40c7bf, including its repeated commodity-list blend.
fn apartment_palette(body: u32, owner: u32) -> Vec<(&'static str, [u32; 4])> {
    let mut out = vec![(
        "BAP_COM_TOP_TITLE_AREA",
        [
            owner.wrapping_sub(0x01000000),
            owner.wrapping_sub(0x01000000),
            owner.wrapping_add(0x7f000000),
            owner.wrapping_add(0x7f000000),
        ],
    )];
    for name in [
        "BAP_COM_BOTTOM_AREA",
        "BAP_COM_COMMODITY_LIST",
        "BAP_COM_COMMODITY_LIST",
        "BAP_COM_TENNANTS_LIST",
        "BAP_COM_TOTALRENT_LIST",
        "BAP_COM_POTENTIALRENT_LIST",
    ] {
        out.push((name, [body; 4]));
    }
    for name in [
        "BAP_COM_TOPBLUE_BACKING",
        "BAP_COM_TENNANTS_TYPE_AREA",
        "BAP_COM_TOTALRENT_TYPE_AREA",
    ] {
        out.push((name, [0xff1f3070; 4]));
    }
    for name in [
        "BAP_COM_COMMODIDTY_TITLE",
        "BAP_COM_RENT_TITLE",
        "BAP_COM_CURRENT_TITLE",
        "BAP_COM_TOTALRENT_TITLE",
        "BAP_COM_POTENTIALRENT_TITLE",
    ] {
        out.push((name, [0x6d4f94b0; 4]));
    }
    out.push(("BAP_COM_RENT_LIST", [0xff202a53; 4]));
    out
}
fn fill_regions(
    bounds: &[(String, [f32; 4])],
    palette: Vec<(&str, [u32; 4])>,
) -> Result<Surface, String> {
    let mut fills = Surface::solid();
    for (name, colors) in palette {
        if let Some((_, rect)) = bounds.iter().find(|(n, _)| n == name) {
            fills.rect(*rect, colors);
        } else if !matches!(
            name,
            "BAP_SELLING_TYPE_AREA" | "BAP_YESTERDAY_TYPE_AREA" | "BAP_TODAY_TYPE_AREA"
        ) {
            return Err(format!("missing adjustment fill region {name}"));
        }
        // Supplied retail mesh lacks these three named selectors. Original
        //43af30/43aca0 retains its zero rectangle for an absent part (-1),
        // so these submissions have no visible area. Do not invent bounds.
    }
    Ok(fills)
}
pub struct Assets {
    base: Vec<Surface>,
    retail_bounds: Vec<(String, [f32; 4])>,
    apartment_bounds: Vec<(String, [f32; 4])>,
    body_color: u32,
    owner_colors: [u32; 10],
    apartment_base: Vec<Surface>,
    apartment_regions: Vec<[f32; 4]>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    regions: Vec<[f32; 4]>,
    products: Vec<Vec<u32>>,
    buttons: Vec<Surface>,
    apply_buttons: Vec<Surface>,
    commodity_icons: Vec<Surface>,
    arrows: Vec<Surface>,
    retail_numeric: Vec<(crate::business_edit::Action, [f32; 4])>,
    apartment_numeric: Vec<(crate::business_edit::Action, [f32; 4])>,
    numeric_arrows: Vec<Surface>,
    block_names: Vec<u32>,
}
// 4162c0/443080 store normal, hover, down and disabled images separately.
const APPLY_TEXTURES: [usize; 4] = [85, 87, 86, 143];
fn apply_presentation(disabled: bool, hover: bool, pressed: bool) -> (usize, u32) {
    if disabled {
        (3, 0xffffffff)
    } else if hover {
        (if pressed { 2 } else { 1 }, 0xffffffff)
    } else {
        (0, 0xffffffff)
    }
}
// Submitted terms can await the delayed command with dirty already cleared.
// A retail edit that restores all applied terms has nothing left to submit.
fn pending_terms(record: &[u8; 544]) -> bool {
    record[0x208] != 0 && (record[..4] != [0; 4] || record[0x1b8..0x1dc] != record[0x1e0..0x204])
}
fn retail_capacity(lot: [u8; 4], height: u8, info: &[u8; 116]) -> i32 {
    openmt2001::economy::stock_capacity(
        i32::from(lot[2]) * i32::from(lot[3]),
        height,
        f32::from_le_bytes(info[0x58..0x5c].try_into().unwrap()),
    )
}
fn retail_totals(rows: &[Row], capacity: i32) -> [String; 4] {
    let mut totals = [0i32; 4];
    for row in rows {
        for (total, value) in totals.iter_mut().zip([
            i32::from(row.yesterday),
            i32::from(row.wanted),
            i32::from(row.stock),
            i32::from(row.today),
        ]) {
            *total += value;
        }
    }
    [
        totals[0].to_string(),
        format!("{}/{capacity}", totals[1]),
        format!("{}/{capacity}", totals[2]),
        totals[3].to_string(),
    ]
}
// 416bf9/416d61/41702e/4172c3/4173b6/417495 configure every
// numeric column as style 1. 440720..440747 rounds the centered X origin;
// 416b60 uses style 0 with a twenty-unit inset for commodity names.
fn retail_row_text(
    fonts: &mut [Surface],
    tab: &[u8],
    text: &[u8],
    bounds: [f32; 4],
    column: usize,
    row: usize,
) -> Result<(), String> {
    let [left, top, right, bottom] = bounds;
    let x = if column == 0 {
        left.trunc() + 22.
    } else {
        let width = f64::from(ui::text_width(tab, text, 1.)?);
        left.trunc() + ((f64::from(right) - f64::from(left) - width) * 0.5 + 0.5).trunc() as f32
    };
    ui::text(
        fonts,
        tab,
        text,
        [x, top.trunc() + 2. + row as f32 * 17., right, bottom],
        false,
        0xffffffff,
    )
}

// 41937a..4193f2 divides the commodity list into six horizontal hit regions,
// using the full BAP_BACKING width. 4193ff..419423 paints only the hovered row.
fn retail_row_bounds(regions: &[[f32; 4]], row: usize) -> [f32; 4] {
    let backing = regions[1];
    let list = regions[15];
    let height = (f64::from(list[3]) - f64::from(list[1])) * f64::from(1.0f32 / 6.);
    [
        backing[0],
        (f64::from(list[1]) + row as f64 * height) as f32,
        backing[2],
        (f64::from(list[1]) + (row + 1) as f64 * height) as f32,
    ]
}
fn retail_row_band(regions: &[[f32; 4]], row: Option<usize>) -> Surface {
    let mut band = Surface::solid();
    if let Some(row) = row {
        band.rect(retail_row_bounds(regions, row), [0xcc37478a; 4]);
    }
    band
}
const RETAIL_ARROW_TEXTURES: [usize; 8] = [118, 119, 120, 121, 122, 123, 124, 125];
fn retail_numeric_controls(
    regions: &[(String, [f32; 4])],
) -> Result<Vec<(crate::business_edit::Action, [f32; 4])>, String> {
    use crate::business_edit::Action;
    let mut controls = Vec::with_capacity(24);
    //417fa4..4183aa resolve actual arrow parts;416440 keeps their bounds.
    for stock in [false, true] {
        let family = if stock {
            "BAP_STOCKIWANT"
        } else {
            "BAP_SELLING"
        };
        for slot in 0..6 {
            for increase in [false, true] {
                let name = format!(
                    "{family}_ROW{}_{}",
                    slot + 1,
                    if increase { "RIGHT" } else { "LEFT" }
                );
                let bounds = regions
                    .iter()
                    .find(|(n, _)| n == &name)
                    .ok_or_else(|| format!("Missing native adjustment region {name}"))?
                    .1;
                controls.push((
                    if stock {
                        Action::Stock { slot, increase }
                    } else {
                        Action::Price { slot, increase }
                    },
                    bounds,
                ));
            }
        }
    }
    Ok(controls)
}
fn apartment_numeric_controls(
    regions: &[(String, [f32; 4])],
) -> Result<Vec<(crate::business_edit::Action, [f32; 4])>, String> {
    //40bc62/40bc8e ->40af10 preserves these two original rent-arrow parts.
    [false, true]
        .into_iter()
        .map(|increase| {
            let name = if increase {
                "BAP_COM_RENT_ROW1_RIGHT"
            } else {
                "BAP_COM_RENT_ROW1_LEFT"
            };
            let bounds = regions
                .iter()
                .find(|(n, _)| n == name)
                .ok_or_else(|| format!("Missing native adjustment region {name}"))?
                .1;
            Ok((crate::business_edit::Action::Rent { increase }, bounds))
        })
        .collect()
}
/// Borrowed authoritative refresh inputs; no renderer or retained owner is consumed.
#[derive(Clone, Copy)]
pub(crate) struct RefreshScene<'a> {
    pub buildings: &'a [openmt2001::render_scene::InspectionBuilding],
    pub catalog: Option<&'a openmt2001::business_pool::BusinessCatalog>,
    pub commodities: Option<&'a [[u8; 0x1c]; 32]>,
}
impl<'a> From<&'a Scene> for RefreshScene<'a> {
    fn from(scene: &'a Scene) -> Self {
        Self {
            buildings: &scene.inspection_buildings,
            catalog: scene.saved_business_catalog.as_ref(),
            commodities: scene.saved_commodity_rows.as_ref(),
        }
    }
}
/// All fallible decoding is complete. Commit preserves the actual State's
/// channels, pending edits and control owners unless admission changed.
pub(crate) struct PreparedRefresh {
    fresh: State,
    admission_changed: bool,
}
impl PreparedRefresh {
    pub(crate) fn commit(self, state: &mut State) {
        let Self {
            fresh,
            admission_changed,
        } = self;
        if admission_changed {
            state.cancel_press();
            state.edit_request = None;
            state.edit_pending = None;
            state.edit_inflight = None;
            state.close_after_apply = false;
            state.requested_building = None;
            state.tutorial_control = None;
            state.tutorial_producer = None;
        }
        state.building = fresh.building;
        state.edit_target = fresh.edit_target;
        state.rows = fresh.rows;
        state.row_hover = state.row_hover.filter(|&row| row < state.rows.len());
        state.stock_capacity = fresh.stock_capacity;
        state.apartment = fresh.apartment;
        state.name = fresh.name;
        state.block_name = fresh.block_name;
        state.owner_color = fresh.owner_color;
        state.initialized = fresh.initialized;
        state.navigation = fresh.navigation;
        state.dirty_terms = fresh.dirty_terms;
        state.prepared = None;
        state.dirty = true;
    }
}
pub struct State {
    pub edit_target: Option<crate::business_edit::Target>,
    pub edit_request: Option<crate::business_edit::Action>,
    pub edit_pending: Option<std::sync::mpsc::Receiver<Result<[u8; 544], String>>>,
    pub edit_inflight: Option<crate::business_edit::Action>,
    pub close: bool,
    close_after_apply: bool,
    dirty_terms: bool,
    last_refresh: Instant,
    edit_hover: Option<crate::business_edit::Action>,
    edit_pressed: Option<crate::business_edit::Action>,
    diagnostic: Option<String>,
    pub initialized: bool,
    pub building: usize,
    pub rows: Vec<Row>,
    stock_capacity: i32,
    row_hover: Option<usize>,
    pub apartment: Option<Apartment>,
    pub name: u32,
    block_name: u32,
    owner_color: u32,
    pub prepared: Option<PreparedScene>,
    navigation: Vec<usize>,
    pub requested_building: Option<usize>,
    dirty: bool,
    hover: bool,
    pressed: bool,
    arrow_hover: Option<usize>,
    arrow_pressed: Option<usize>,
    tutorial_control: Option<u32>,
    tutorial_producer: Option<(u32, u32)>,
}
impl State {
    pub fn tutorial_control_pressed(&self) -> bool {
        self.pressed || self.edit_pressed.is_some() || self.arrow_pressed.is_some()
    }
    pub fn take_tutorial_control(&mut self) -> Option<u32> {
        self.tutorial_control.take()
    }
    pub fn take_tutorial_producer(&mut self) -> Option<(u32, u32)> {
        let producer = self.tutorial_producer.take();
        if producer.is_some() && self.tutorial_control == Some(7) {
            self.tutorial_control = None;
        }
        producer
    }
    pub fn tutorial_control_at(&mut self, assets: &Assets, point: Option<[i32; 2]>) -> Option<u32> {
        self.pointer(assets, point);
        if self.hover {
            return Some(53);
        }
        match self.edit_hover? {
            crate::business_edit::Action::Price { slot: 0, increase } => {
                Some(if increase { 41 } else { 42 })
            }
            crate::business_edit::Action::Stock { slot: 0, increase } => {
                Some(if increase { 43 } else { 44 })
            }
            crate::business_edit::Action::Apply => Some(53),
            // Apartment controls have separate native registrations.
            _ => None,
        }
    }
    fn provenance(&self) -> &[u8] {
        if let Some(message) = &self.diagnostic {
            message.as_bytes()
        } else if self.edit_pending.is_some() {
            b"Updating business..."
        } else {
            b""
        }
    }
    pub fn pointer(&mut self, assets: &Assets, point: Option<[i32; 2]>) {
        let row = point.and_then(|p| {
            (0..self.rows.len()).find(|&row| {
                let b = retail_row_bounds(&assets.regions, row);
                let [x, y] = p.map(|v| v as f32 * 0.5);
                // Original 4342c0 excludes the left and right edges.
                x > b[0] && x < b[2] && y >= b[1] && y <= b[3]
            })
        });
        if self.row_hover != row {
            self.row_hover = row;
            self.dirty = true;
        }
        let edit = if self.edit_target.is_some() && self.edit_pending.is_none() {
            point.and_then(|p| {
                assets
                    .edit_controls(self)
                    .into_iter()
                    .find(|(action, bounds)| {
                        (*action != crate::business_edit::Action::Apply || self.dirty_terms)
                            && inside(p.map(|v| v as f32 * 0.5), *bounds)
                    })
                    .map(|(action, _)| action)
            })
        } else {
            None
        };
        if self.edit_hover != edit {
            self.edit_hover = edit;
            self.dirty = true;
        }
        let hit = point.is_some_and(|p| {
            inside(
                p.map(|v| v as f32 * 0.5),
                if self.apartment.is_some() {
                    assets.apartment_regions[19]
                } else {
                    assets.regions[28]
                },
            )
        });
        let arrow = point.and_then(|p| {
            if self.apartment.is_some() {
                [21, 23, 24, 26, 4, 5]
                    .iter()
                    .position(|&i| inside(p.map(|v| v as f32 * 0.5), assets.apartment_regions[i]))
            } else {
                [4, 5]
                    .iter()
                    .position(|&i| inside(p.map(|v| v as f32 * 0.5), assets.regions[i]))
                    .map(|i| i + 4)
            }
        });
        if arrow != self.arrow_hover {
            self.arrow_hover = arrow;
            self.dirty = true;
        }
        if hit != self.hover {
            self.hover = hit;
            self.dirty = true;
        }
    }
    pub fn cancel_press(&mut self) {
        self.edit_pressed = None;
        self.pressed = false;
        self.arrow_pressed = None;
        self.dirty = true;
    }
    pub fn mouse(&mut self, down: bool) -> bool {
        if !down
            && self.edit_pressed == self.edit_hover
            && let Some(action) = self.edit_hover
        {
            self.edit_request = Some(action);
            self.diagnostic = None;
            if action == crate::business_edit::Action::Apply {
                self.tutorial_control = Some(53);
            }
        }
        self.edit_pressed = if down { self.edit_hover } else { None };
        if !down
            && self.arrow_pressed == self.arrow_hover
            && let Some(arrow) = self.arrow_hover
        {
            if arrow < 4 {
                if let Some(apartment) = &mut self.apartment {
                    apartment.toggle(arrow / 2);
                }
            } else if let Some(position) = self.navigation.iter().position(|&i| i == self.building)
            {
                self.requested_building = Some(
                    self.navigation[navigation_index(position, self.navigation.len(), arrow == 5)],
                );
            }
        }
        self.arrow_pressed = if down { self.arrow_hover } else { None };
        let close = !down && self.pressed && self.hover;
        if close {
            self.tutorial_control = Some(53);
        }
        self.pressed = down && self.hover;
        self.dirty = true;
        close && self.request_close()
    }
    pub fn request_close(&mut self) -> bool {
        if self.edit_target.is_some() {
            self.close_after_apply = true;
            self.edit_request = Some(crate::business_edit::Action::Apply);
            false
        } else {
            true
        }
    }
    pub fn receive_edit(&mut self, result: Result<[u8; 544], String>) {
        self.dirty = true;
        self.last_refresh = Instant::now();
        let action = self.edit_inflight.take();
        let applied = action == Some(crate::business_edit::Action::Apply);
        match result {
            Err(error) => {
                self.diagnostic = Some(error);
                self.close_after_apply = false;
            }
            Ok(record) => {
                // Native419d99..419da7 emits7 after all four retail edits,
                // including a price decrement clamped at0.1. Deliver it only
                // after the authoritative worker admitted the edit.
                if matches!(
                    action,
                    Some(
                        crate::business_edit::Action::Price { .. }
                            | crate::business_edit::Action::Stock { .. }
                    )
                ) {
                    self.tutorial_control = Some(7);
                    //418a3d/418bd3 register only row0's price/stock arrows.
                    //Other rows emit7 too, but have no proven registered parent.
                    let control = match action.unwrap() {
                        crate::business_edit::Action::Price { slot: 0, increase } => {
                            Some(if increase { 41 } else { 42 })
                        }
                        crate::business_edit::Action::Stock { slot: 0, increase } => {
                            Some(if increase { 43 } else { 44 })
                        }
                        _ => None,
                    };
                    self.tutorial_producer = control.map(|control| (control, 7));
                }
                self.dirty_terms = pending_terms(&record);
                let refreshed = if let Some(apartment) = &self.apartment {
                    Apartment::from_record(&record, apartment.modes[0], apartment.modes[1])
                        .map(|value| self.apartment = Some(value))
                } else {
                    refresh_retail_rows(&mut self.rows, &record)
                };
                if let Err(error) = refreshed {
                    self.diagnostic = Some(error);
                }
                if self.close_after_apply && applied && !self.dirty_terms {
                    self.close = true;
                }
            }
        }
    }
    pub fn prepare(&mut self, assets: &Assets, renderer: &Renderer) -> Result<(), String> {
        // The original adjustment refresh reads the live record. Polling is a
        // UI adapter cadence; the original dispatch interval is not recovered.
        if self.edit_target.is_some()
            && !self.close_after_apply
            && self.diagnostic.is_none()
            && self.edit_request.is_none()
            && self.edit_pending.is_none()
            && self.last_refresh.elapsed() >= Duration::from_millis(250)
        {
            self.edit_request = Some(crate::business_edit::Action::Refresh);
            self.last_refresh = Instant::now();
        }
        if self.dirty || self.prepared.is_none() {
            self.prepared = Some(assets.prepare(renderer, self)?);
            self.dirty = false;
        }
        Ok(())
    }
}
fn refresh_retail_rows(rows: &mut [Row], record: &[u8; 544]) -> Result<(), String> {
    if record[..4] != [0; 4] || rows.len() > 6 {
        return Err("retail adjustment refresh changed business class".into());
    }
    let bank = usize::from(record[0x209]);
    if bank >= 4 {
        return Err("adjustment sales history bank".into());
    }
    let short = |at| u16::from_le_bytes(record[at..at + 2].try_into().unwrap());
    let mut updated = rows.to_vec();
    for (slot, row) in updated.iter_mut().enumerate() {
        row.price = f32::from_le_bytes(
            record[0x1ec + slot * 4..0x1f0 + slot * 4]
                .try_into()
                .unwrap(),
        );
        if !row.price.is_finite() {
            return Err("nonfinite adjustment refresh price".into());
        }
        let stock = 0x80 + slot * 0x34;
        row.yesterday = short(stock + 2 + ((bank + 3) & 3) * 2);
        row.wanted = short(0x1e0 + slot * 2) as i16;
        row.stock = short(stock);
        row.today = short(stock + 2 + bank * 2);
    }
    rows.clone_from_slice(&updated);
    Ok(())
}
fn navigation_index(current: usize, count: usize, next: bool) -> usize {
    if next {
        (current + 1) % count
    } else {
        (current + count - 1) % count
    }
}
fn business_order<'a>(
    records: impl Iterator<Item = (usize, &'a [u8; 544])>,
    owner: u8,
    kind: u8,
) -> Vec<usize> {
    let mut result: Vec<_> = records
        .filter(|(_, r)| r[..4] == [kind, 0, 0, 0] && r[0x21c] != 0 && r[0xa] == owner)
        .map(|(slot, _)| slot)
        .collect();
    result.sort_unstable_by(|a, b| b.cmp(a));
    result
}
fn inspection_record(
    building: &openmt2001::render_scene::InspectionBuilding,
) -> Option<(usize, &[u8; 544])> {
    building
        .business()
        .map(|b| (b.slot, &b.record))
        .or_else(|| {
            building
                .initialized_apartment
                .as_ref()
                .map(|b| (b.slot, &b.record))
        })
}
fn business_navigation(scene: RefreshScene<'_>, owner: u8, kind: u8) -> Vec<usize> {
    let records = scene.buildings.iter().filter_map(inspection_record);
    business_order(records, owner, kind)
        .iter()
        .map(|slot| {
            scene
                .buildings
                .iter()
                .position(|b| inspection_record(b).is_some_and(|(id, _)| id == *slot))
                .unwrap()
        })
        .collect()
}
fn inside(p: [f32; 2], b: [f32; 4]) -> bool {
    p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3]
}
impl Assets {
    /// Runtime editing uses the same native one-based owner byte as the worker.
    /// A missing current player keeps inspection available without mutation.
    pub fn open_for_owner(
        &self,
        scene: &Scene,
        building: usize,
        owner: Option<u8>,
    ) -> Result<Option<State>, String> {
        self.open_for_owner_view(scene.into(), building, owner)
    }
    pub(crate) fn open_for_owner_view(
        &self,
        scene: RefreshScene<'_>,
        building: usize,
        owner: Option<u8>,
    ) -> Result<Option<State>, String> {
        let mut state = self.open_view(scene, building)?;
        if let Some(state) = &mut state {
            let editable = scene
                .buildings
                .get(building)
                .and_then(inspection_record)
                .is_some_and(|(_, record)| owner == Some(record[10]));
            if !editable {
                state.edit_target = None;
                state.edit_request = None;
            }
        }
        Ok(state)
    }
    /// Preserve a displayed business across a structural scene refresh. Native
    /// pool identity, rather than the scene vector index, owns the panel.
    pub fn refresh(
        &self,
        scene: &Scene,
        previous: &Scene,
        state: &mut State,
        owner: Option<u8>,
    ) -> Result<bool, String> {
        let prepared =
            self.prepare_refresh(scene.into(), &previous.inspection_buildings, state, owner)?;
        if let Some(prepared) = prepared {
            prepared.commit(state);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub(crate) fn prepare_refresh(
        &self,
        scene: RefreshScene<'_>,
        previous: &[openmt2001::render_scene::InspectionBuilding],
        state: &State,
        owner: Option<u8>,
    ) -> Result<Option<PreparedRefresh>, String> {
        let Some(old) = previous.get(state.building) else {
            return Ok(None);
        };
        let Some((business, old_record)) = inspection_record(old) else {
            return Ok(None);
        };
        let Some(building) = scene.buildings.iter().position(|candidate| {
            let same_building = if let Some(slot) = old.runtime_slot {
                candidate.runtime_slot == Some(slot)
            } else if let Some(slot) = old.saved_slot {
                candidate.saved_slot == Some(slot)
            } else {
                false
            };
            same_building && inspection_record(candidate).is_some_and(|(slot, _)| slot == business)
        }) else {
            return Ok(None);
        };
        let Some(mut fresh) = self.open_for_owner_view(scene, building, owner)? else {
            return Ok(None);
        };
        let new_record = inspection_record(&scene.buildings[building]).unwrap().1;
        let admission_changed = old_record[..8] != new_record[..8]
            || old_record[10] != new_record[10]
            || state.edit_target.is_some() != fresh.edit_target.is_some();
        if let (Some(old), Some(new)) = (&state.apartment, &mut fresh.apartment) {
            *new = Apartment::from_record(new_record, old.modes[0], old.modes[1])?;
        }
        Ok(Some(PreparedRefresh {
            fresh,
            admission_changed,
        }))
    }
    fn edit_controls(&self, state: &State) -> Vec<(crate::business_edit::Action, [f32; 4])> {
        use crate::business_edit::Action;
        let mut controls = Vec::new();
        if state.apartment.is_some() {
            controls.extend(self.apartment_numeric.iter().copied());
            controls.push((Action::Apply, self.apartment_regions[20]));
        } else {
            controls.extend(self.retail_numeric.iter().copied().filter(|(action, _)| {
                let (Action::Price { slot, .. } | Action::Stock { slot, .. }) = *action else {
                    unreachable!()
                };
                slot < state.rows.len()
            }));
            controls.push((Action::Apply, self.regions[29]));
        }
        controls
    }
    fn prepare_edits(
        &self,
        state: &State,
        surfaces: &mut Vec<Surface>,
        fonts: &mut [Surface],
    ) -> Result<(), String> {
        use crate::business_edit::Action;
        if state.edit_target.is_none() {
            return Ok(());
        }
        for (action, b) in self.edit_controls(state) {
            let disabled =
                state.edit_pending.is_some() || (action == Action::Apply && !state.dirty_terms);
            let phase = if state.edit_hover == Some(action) && !disabled {
                if state.edit_pressed == Some(action) {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            let apply = apply_presentation(
                disabled,
                state.edit_hover == Some(action),
                state.edit_pressed == Some(action),
            );
            let mut icon = match action {
                Action::Apply => self.apply_buttons[apply.0].clone(),
                Action::Price { increase, .. }
                | Action::Stock { increase, .. }
                | Action::Rent { increase } => self.numeric_arrows
                    [usize::from(increase) * 4 + if disabled { 3 } else { phase }]
                .clone(),
                Action::Refresh => unreachable!(),
            };
            icon.quad(
                [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(icon);
            if action == Action::Apply {
                ui::text(
                    fonts,
                    &self.tab,
                    &b"Apply".map(|b| b - 32),
                    [b[0], b[1] + 7., b[2], b[3]],
                    true,
                    apply.1,
                )?;
            }
        }
        Ok(())
    }
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        let parts = [0, 1, 2, 3, 8, 9, 10, 11, 12, 13, 14, 22, 23, 24, 25, 26, 27];
        let (base, regions) = ui::panel_frame_base(
            index,
            archive,
            "gamedata2/brand_new_panels/bap_v2.tm",
            &parts,
            9,
        )?;
        let (apartment_base, apartment_regions) = ui::panel_frame_base(
            index,
            archive,
            "gamedata2/brand_new_panels/bap_apartments_v2.tm",
            &[0, 1, 2, 3, 8, 9, 10, 11, 12, 18],
            0,
        )?;
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or("Business Adjustment asset missing")?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("Business Adjustment asset range".into())
        };
        let mesh =
            openmt2001::mesh::CachedMesh::parse(payload("gamedata2/brand_new_panels/bap_v2.tm")?)?;
        let retail_bounds: Vec<_> = openmt2001::ui_mesh::regions(&mesh, 9)?
            .into_iter()
            .map(|r| (r.name, r.bounds))
            .collect();
        let retail_numeric = retail_numeric_controls(&retail_bounds)?;
        let mesh = openmt2001::mesh::CachedMesh::parse(payload(
            "gamedata2/brand_new_panels/bap_apartments_v2.tm",
        )?)?;
        let apartment_bounds: Vec<_> = openmt2001::ui_mesh::regions(&mesh, 0)?
            .into_iter()
            .map(|r| (r.name, r.bounds))
            .collect();
        let apartment_numeric = apartment_numeric_controls(&apartment_bounds)?;
        let fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[6..8]
            .iter()
            .map(Surface::new)
            .collect();
        let buttons = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[79..82]
            .iter()
            .map(Surface::new)
            .collect();
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let apply_buttons = APPLY_TEXTURES
            .iter()
            .map(|&i| Surface::new(&generic.textures[i]))
            .collect();
        let arrows = [97, 98, 99, 101, 102, 103]
            .iter()
            .map(|&i| Surface::new(&generic.textures[i]))
            .collect();
        let numeric_arrows = RETAIL_ARROW_TEXTURES
            .iter()
            .map(|&i| Surface::new(&generic.textures[i]))
            .collect();
        let tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let image = openmt2001::pe::PeImage::parse(exe)?;
        let body_color = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        let mut owner_colors = [0; 10];
        for (i, color) in owner_colors.iter_mut().enumerate() {
            *color = u32::from_le_bytes(image.at(0x541af0 + i as u32 * 40, 4)?.try_into().unwrap());
        }
        let block_names = openmt2001::board::Board::from_v14(&image)?
            .blocks
            .iter()
            .map(|b| b.text_id)
            .collect();
        let mut commodity = openmt2001::commodity_script::State::from_v14(&image, [0; 5])?;
        let mut state = openmt2001::business_script::State::from_v14(&image)?;
        let mut runtime = openmt2001::lua4::Runtime::new()?;
        let business = openmt2001::business_script::Bindings::register(&mut runtime, &image, 100)?;
        let commodities =
            openmt2001::commodity_script::Bindings::register(&mut runtime, &image, 200)?;
        runtime.execute(
            "Business Adjustment default commodity catalog",
            &fs::read(installer.join("scripts/DEFAULT/commoditysettings.lua"))
                .map_err(|e| e.to_string())?,
            &mut |id, args| commodities.handle(id, args, &mut commodity),
        )?;
        runtime.execute(
            "Business Adjustment default product order",
            &fs::read(installer.join("scripts/DEFAULT/businesssettings.lua"))
                .map_err(|e| e.to_string())?,
            &mut |id, args| business.handle(id, args, &mut state, &commodity),
        )?;
        let products = state
            .products
            .iter()
            .enumerate()
            .map(|(i, slots)| {
                slots[..usize::from(state.info.rows()[i][0x4c])]
                    .iter()
                    .map(|p| p[0])
                    .collect()
            })
            .collect();
        let commodity_icons = commodity
            .rows
            .iter()
            .map(|row| {
                let image = u32::from_le_bytes(row[12..16].try_into().unwrap()) as usize;
                generic
                    .textures
                    .get(image)
                    .map(Surface::new)
                    .ok_or("adjustment commodity icon texture".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let strings = crate::tooltip::localized_strings(installer, strings)?;
        Ok(Self {
            base,
            retail_bounds,
            apartment_bounds,
            body_color,
            owner_colors,
            apartment_base,
            apartment_regions,
            fonts,
            tab,
            strings,
            regions,
            products,
            buttons,
            apply_buttons,
            commodity_icons,
            arrows,
            retail_numeric,
            apartment_numeric,
            numeric_arrows,
            block_names,
        })
    }
    pub fn open(&self, scene: &Scene, building: usize) -> Result<Option<State>, String> {
        self.open_view(scene.into(), building)
    }
    fn open_view(&self, scene: RefreshScene<'_>, building: usize) -> Result<Option<State>, String> {
        let b = scene
            .buildings
            .get(building)
            .ok_or("adjustment building index")?;
        let Some((_, record)) = inspection_record(b) else {
            return Ok(None);
        };
        let edit_target = b.runtime_slot.map(|building| crate::business_edit::Target {
            building,
            business: inspection_record(b).unwrap().0,
        });
        let proprietor_character = b
            .business()
            .and_then(|business| business.owner_character)
            .or_else(|| {
                b.initialized_apartment
                    .as_ref()
                    .and_then(|business| business.owner_character)
            });
        let owner_color = match proprietor_character {
            Some(c) => *self
                .owner_colors
                .get(c as usize)
                .ok_or("adjustment owner character")?,
            None => 0x00909090,
        };
        if record[..4] == [1, 0, 0, 0] {
            return Ok(Some(State {
                edit_target,
                edit_request: edit_target.map(|_| crate::business_edit::Action::Refresh),
                edit_pending: None,
                edit_inflight: None,
                close: false,
                close_after_apply: false,
                dirty_terms: pending_terms(record),
                last_refresh: Instant::now(),
                edit_hover: None,
                edit_pressed: None,
                diagnostic: None,
                initialized: b.initialized_apartment.is_some(),
                building,
                rows: vec![],
                stock_capacity: 0,
                row_hover: None,
                apartment: Some(Apartment::from_record(record, false, false)?),
                name: b
                    .business()
                    .map(|b| b.name_text_id)
                    .or_else(|| b.initialized_apartment.as_ref().map(|b| b.name_text_id))
                    .ok_or("apartment name missing")?,
                block_name: self.block_names[b.category],
                owner_color,
                prepared: None,
                navigation: business_navigation(scene, record[0xa], 1),
                requested_building: None,
                dirty: true,
                hover: false,
                pressed: false,
                arrow_hover: None,
                arrow_pressed: None,
                tutorial_control: None,
                tutorial_producer: None,
            }));
        }
        let business = b
            .business()
            .ok_or("initialized non-apartment adjustment unsupported")?;
        if business.record[..4] != [0, 0, 0, 0] {
            return Ok(None);
        }
        let info = scene.catalog.ok_or("saved BusinessInfo missing")?;
        let commodities = scene.commodities.ok_or("saved commodities missing")?;
        let products = self
            .products
            .get(business.business_type as usize)
            .ok_or("business product order missing")?;
        Ok(Some(State {
            edit_target,
            edit_request: edit_target.map(|_| crate::business_edit::Action::Refresh),
            edit_pending: None,
            edit_inflight: None,
            close: false,
            close_after_apply: false,
            dirty_terms: pending_terms(record),
            last_refresh: Instant::now(),
            edit_hover: None,
            edit_pressed: None,
            diagnostic: None,
            initialized: b.runtime_slot.is_some(),
            building,
            rows: rows(&business.record, info, commodities, products)?,
            stock_capacity: retail_capacity(
                b.lot,
                b.height,
                info.row(business.business_type as usize)
                    .ok_or("adjustment business kind")?,
            ),
            row_hover: None,
            apartment: None,
            name: business.name_text_id,
            block_name: self.block_names[b.category],
            owner_color,
            prepared: None,
            navigation: business_navigation(scene, business.record[0xa], 0),
            requested_building: None,
            dirty: true,
            hover: false,
            pressed: false,
            arrow_hover: None,
            arrow_pressed: None,
            tutorial_control: None,
            tutorial_producer: None,
        }))
    }
    pub fn navigate(&self, scene: &Scene, state: &State, building: usize) -> Result<State, String> {
        if !state.navigation.contains(&building) {
            return Err("adjustment navigation target outside original list".into());
        }
        let mut next = self
            .open(scene, building)?
            .ok_or("adjustment navigation target missing")?;
        if state.edit_target.is_none() {
            next.edit_target = None;
            next.edit_request = None;
        }
        if let (Some(previous), Some(apartment)) = (&state.apartment, &mut next.apartment) {
            for column in 0..2 {
                if previous.modes[column] {
                    apartment.toggle(column);
                }
            }
        }
        Ok(next)
    }
    fn prepare_commodity_icons(
        &self,
        rows: &[Row],
        surfaces: &mut Vec<Surface>,
    ) -> Result<(), String> {
        for (slot, row) in rows.iter().enumerate() {
            let name = format!("BAP_COMM_ICON_ROW{}", slot + 1);
            let b = self
                .retail_bounds
                .iter()
                .find(|(n, _)| n == &name)
                .ok_or_else(|| format!("missing adjustment icon region {name}"))?
                .1;
            let mut icon = self
                .commodity_icons
                .get(row.commodity as usize)
                .ok_or("adjustment commodity icon identity")?
                .clone();
            icon.quad(
                [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(icon);
        }
        Ok(())
    }
    fn prepare(&self, renderer: &Renderer, state: &State) -> Result<PreparedScene, String> {
        if let Some(apartment) = &state.apartment {
            return self.prepare_apartment(renderer, state, apartment);
        }
        let mut surfaces = self.base.clone();
        surfaces.push(fill_regions(
            &self.retail_bounds,
            retail_palette(self.body_color, state.owner_color),
        )?);
        let mut fonts = self.fonts.clone();
        let label = |id: u32| {
            self.strings
                .get(id as usize)
                .map(Vec::as_slice)
                .ok_or("adjustment language ID")
        };
        let centered = |fonts: &mut [Surface], text: &[u8], b: [f32; 4], color: u32| {
            ui::text(
                fonts,
                &self.tab,
                text,
                [b[0] + 2., (b[1] + b[3] - 10.) * 0.5, b[2] - 2., b[3]],
                true,
                color,
            )
        };
        let mut title = label(state.name)?.to_vec();
        title.extend_from_slice(&[b':' - 32, 0]);
        title.extend_from_slice(label(state.block_name)?);
        ui::text_fit(
            &mut fonts,
            &self.tab,
            &title,
            self.regions[6],
            false,
            0xffffffff,
        )?;
        for (i, region) in [4, 5].into_iter().enumerate() {
            let state_index = if state.arrow_hover == Some(i + 4) {
                if state.arrow_pressed == Some(i + 4) {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            let mut arrow = self.arrows[i * 3 + state_index].clone();
            let b = self.regions[region];
            arrow.quad(
                [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(arrow);
        }
        let titles = [1421, 1470, 1471, 1472, 1473, 1474, 1475];
        for (i, id) in titles.into_iter().enumerate() {
            let b = self.regions[8 + i];
            let text = label(id)?;
            let (offsets, copy) =
                crate::prompt::wrap_with_gap(&self.tab, text, (b[2] - b[0] - 4.) as i32, -1)?;
            for (line, pair) in offsets.windows(2).enumerate() {
                let end = (pair[1] - 1).min(copy.len());
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &copy[pair[0].min(end)..end],
                    [b[0] + 2., b[1] + 2. + line as f32 * 10., b[2] - 2., b[3]],
                    true,
                    0xffffffff,
                )?;
            }
        }
        let currency = label(2243)?;
        let currency_suffix = label(2244)?.first() != Some(&0x4c);
        surfaces.push(retail_row_band(&self.regions, state.row_hover));
        self.prepare_commodity_icons(&state.rows, &mut surfaces)?;
        for (row, item) in state.rows.iter().enumerate() {
            let numbers = [
                Vec::new(),
                ui::formatted_float_money(item.cost, currency, currency_suffix)?,
                ui::formatted_float_money(item.price, currency, currency_suffix)?,
                item.yesterday.to_string().bytes().map(|b| b - 32).collect(),
                item.wanted.to_string().bytes().map(|b| b - 32).collect(),
                item.stock.to_string().bytes().map(|b| b - 32).collect(),
                item.today.to_string().bytes().map(|b| b - 32).collect(),
            ];
            for (column, text) in numbers.iter().enumerate() {
                let encoded = if column == 0 {
                    label(item.name)?
                } else {
                    text.as_slice()
                };
                retail_row_text(
                    &mut fonts,
                    &self.tab,
                    encoded,
                    self.regions[15 + column],
                    column,
                    row,
                )?;
            }
        }
        let b = self.regions[22];
        ui::text(
            &mut fonts,
            &self.tab,
            label(1476)?,
            [b[0] + 2., (b[1] + b[3] - 10.) * 0.5, b[2] - 2., b[3]],
            false,
            0xffffffff,
        )?;
        for (i, total) in retail_totals(&state.rows, state.stock_capacity)
            .into_iter()
            .enumerate()
        {
            // 417545/4175b9/4176a9/41775d use centered style 4 totals.
            ui::text_widget_style4(
                &mut fonts,
                &self.tab,
                &total.bytes().map(|b| b - 32).collect::<Vec<_>>(),
                self.regions[23 + i],
                0xffffffff,
            )?;
        }
        let mut button = self.buttons[if state.pressed && state.hover {
            2
        } else if state.hover {
            1
        } else {
            0
        }]
        .clone();
        let b = self.regions[28];
        button.quad(
            [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        surfaces.push(button);
        centered(&mut fonts, &[b'O' - 32, b'K' - 32], b, 0xffffffff)?;
        let note = state
            .provenance()
            .iter()
            .map(|b| b - 32)
            .collect::<Vec<_>>();
        ui::text_fit(
            &mut fonts,
            &self.tab,
            &note,
            [
                self.regions[27][0] + 6.,
                self.regions[27][1] + 10.,
                self.regions[29][0] - 4.,
                self.regions[27][3],
            ],
            false,
            0xffffd080,
        )?;
        self.prepare_edits(state, &mut surfaces, &mut fonts)?;
        surfaces.extend(fonts);
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

/// Four values emitted by original 40b1d0. Modes select retained record history.
#[derive(Debug, Clone)]
pub struct Apartment {
    pub rent: f32,
    pub tenants: u16,
    pub capacity: u16,
    pub total_rent: f32,
    pub potential_rent: i32,
    modes: [bool; 2],
    tenant_history: [u16; 2],
    rent_history: [f32; 2],
}
impl Apartment {
    fn toggle(&mut self, column: usize) {
        self.modes[column] = !self.modes[column];
        self.tenants = self.tenant_history[usize::from(self.modes[0])];
        self.total_rent = self.rent_history[usize::from(self.modes[1])];
    }
    pub fn from_record(
        record: &[u8; 544],
        previous_tenants: bool,
        previous_income: bool,
    ) -> Result<Self, String> {
        let f = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let u = |at| u16::from_le_bytes(record[at..at + 2].try_into().unwrap());
        if record[..4] != [1, 0, 0, 0] {
            return Err("apartment adjustment requires housing".into());
        }
        let rent = f(0x204);
        let total_rent = f(if previous_income { 0x14 } else { 0x50 });
        if !rent.is_finite() || !f(0x14).is_finite() || !f(0x50).is_finite() {
            return Err("nonfinite apartment display value".into());
        }
        let capacity = u(0xe);
        // f32 * u16 is exact in f64 (at most 40 significant bits). Original FTOL
        // truncates; its low word is displayed as signed 32-bit currency.
        let potential = (f64::from(rent) * f64::from(capacity)).trunc();
        if potential < i32::MIN as f64 || potential > i32::MAX as f64 {
            return Err("apartment potential rent display range".into());
        }
        Ok(Self {
            rent,
            tenants: u(if previous_tenants { 0x12 } else { 0x10 }),
            capacity,
            total_rent,
            potential_rent: potential as i32,
            modes: [previous_tenants, previous_income],
            tenant_history: [u(0x10), u(0x12)],
            rent_history: [f(0x50), f(0x14)],
        })
    }
}
impl Assets {
    fn prepare_apartment(
        &self,
        renderer: &Renderer,
        state: &State,
        apartment: &Apartment,
    ) -> Result<PreparedScene, String> {
        let mut surfaces = self.apartment_base.clone();
        surfaces.push(fill_regions(
            &self.apartment_bounds,
            apartment_palette(self.body_color, state.owner_color),
        )?);
        let mut fonts = self.fonts.clone();
        let label = |id: usize| {
            self.strings
                .get(id)
                .map(Vec::as_slice)
                .ok_or("apartment language ID")
        };
        let regions = &self.apartment_regions;
        let mut title = label(state.name as usize)?.to_vec();
        title.extend_from_slice(&[0, b'-' - 32, 0]);
        title.extend_from_slice(label(state.block_name as usize)?);
        ui::text_fit(&mut fonts, &self.tab, &title, regions[6], false, 0xffffffff)?;
        for (region, id) in [
            (8, 1421),
            (9, 1423),
            (10, 1424),
            (11, 1425),
            (12, 1426),
            (13, 1427),
        ] {
            let b = regions[region];
            ui::text(
                &mut fonts,
                &self.tab,
                label(id)?,
                [b[0] + 2., b[1] + 2., b[2] - 2., b[3]],
                true,
                0xffffffff,
            )?;
        }
        for (column, region) in [22, 25].into_iter().enumerate() {
            ui::text(
                &mut fonts,
                &self.tab,
                label(if apartment.modes[column] { 32 } else { 33 })?,
                regions[region],
                true,
                0xffffffff,
            )?;
        }
        let symbol = label(2243)?;
        let suffix = label(2244)?.first() != Some(&0x4c);
        let values = [
            (
                14,
                ui::formatted_float_money(apartment.rent, symbol, suffix)?,
            ),
            (
                15,
                format!("{}/{}", apartment.tenants, apartment.capacity)
                    .bytes()
                    .map(|b| b - 32)
                    .collect(),
            ),
            (
                16,
                ui::formatted_float_money(apartment.total_rent, symbol, suffix)?,
            ),
            (
                17,
                ui::integer_money(apartment.potential_rent, symbol, suffix),
            ),
        ];
        for (region, value) in values {
            let b = regions[region];
            let inset = if region == 14 && state.edit_target.is_some() {
                16.
            } else {
                4.
            };
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &value,
                [b[0] + inset, b[1] + 4., b[2] - inset, b[3]],
                true,
                0xffffffff,
            )?;
        }
        for (i, region) in [21, 23, 24, 26, 4, 5].into_iter().enumerate() {
            let state_index = if state.arrow_hover == Some(i) {
                if state.arrow_pressed == Some(i) { 2 } else { 1 }
            } else {
                0
            };
            let mut arrow = self.arrows[(i % 2) * 3 + state_index].clone();
            let b = regions[region];
            arrow.quad(
                [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(arrow);
        }
        let b = regions[19];
        let mut button = self.buttons[if state.pressed && state.hover {
            2
        } else if state.hover {
            1
        } else {
            0
        }]
        .clone();
        button.quad(
            [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        surfaces.push(button);
        ui::text(
            &mut fonts,
            &self.tab,
            &[b'O' - 32, b'K' - 32],
            [b[0], b[1] + 7., b[2], b[3]],
            true,
            0xffffffff,
        )?;
        ui::text_fit(
            &mut fonts,
            &self.tab,
            &state
                .provenance()
                .iter()
                .map(|b| b - 32)
                .collect::<Vec<_>>(),
            [
                regions[18][0] + 6.,
                regions[18][1] + 10.,
                regions[20][0] - 4.,
                regions[18][3],
            ],
            false,
            0xffffd080,
        )?;
        self.prepare_edits(state, &mut surfaces, &mut fonts)?;
        surfaces.extend(fonts);
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
pub fn verify_live_panels(scene: &Scene) {
    let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
    let executable = read("OPENMT2001_EXE");
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
        .as_chunks::<2>()
        .0
        .iter()
        .map(|n| {
            let end = at + u16::from_le_bytes(*n) as usize;
            let text = &bytes[at..end];
            at = end;
            text
        })
        .collect::<Vec<_>>();
    let assets = Assets::load(&index, &archive, installer, &strings, &executable).unwrap();
    for (_name, index) in [
        (
            "apartment",
            scene
                .inspection_buildings
                .iter()
                .position(|b| b.initialized_apartment.is_some()),
        ),
        (
            "retail",
            scene
                .inspection_buildings
                .iter()
                .position(|b| b.initialized_business.is_some()),
        ),
    ] {
        let Some(index) = index else {
            continue;
        };
        let mut state = assets.open(scene, index).unwrap().unwrap();
        // Scenario11's original card73 can transfer business+0xa owner1 to
        // owner4 while category0 land remains owner1. Keep their title palettes
        // distinct on first open and after the retained panel refresh.
        let mut transferred = scene.clone();
        let selected = &mut transferred.inspection_buildings[index];
        selected.saved_owner_character = Some(0);
        if let Some(apartment) = &mut selected.initialized_apartment {
            apartment.record[0xa] = 4;
            apartment.owner_character = Some(3);
        } else if let Some(business) = &mut selected.initialized_business {
            business.record[0xa] = 4;
            business.owner_character = Some(3);
        }
        let opened = assets.open(&transferred, index).unwrap().unwrap();
        assert_eq!(opened.owner_color, assets.owner_colors[3]);
        assert_ne!(opened.owner_color, assets.owner_colors[0]);
        let mut retained = assets.open(scene, index).unwrap().unwrap();
        assert!(
            assets
                .refresh(&transferred, scene, &mut retained, None)
                .unwrap()
        );
        assert_eq!(retained.owner_color, assets.owner_colors[3]);
        let mut unowned = transferred.clone();
        let selected = &mut unowned.inspection_buildings[index];
        if let Some(apartment) = &mut selected.initialized_apartment {
            apartment.record[0xa] = 0;
            apartment.owner_character = None;
        } else if let Some(business) = &mut selected.initialized_business {
            business.record[0xa] = 0;
            business.owner_character = None;
        }
        assert_eq!(
            assets.open(&unowned, index).unwrap().unwrap().owner_color,
            0x00909090
        );
        assert_eq!(
            state.edit_request.take(),
            Some(crate::business_edit::Action::Refresh)
        );
        let controls = assets.edit_controls(&state);
        for &(action, bounds) in controls
            .iter()
            .filter(|(action, _)| *action != crate::business_edit::Action::Apply)
        {
            state.pointer(
                &assets,
                Some([
                    (bounds[0] + bounds[2]) as i32,
                    (bounds[1] + bounds[3]) as i32,
                ]),
            );
            assert!(!state.mouse(true));
            assert!(!state.mouse(false));
            assert_eq!(state.edit_request.take(), Some(action));
        }
        // Controlled snapshot adapter checks: reordering render entries must
        // retain the actual pool target; ownership loss must cancel admission.
        let owner = inspection_record(&scene.inspection_buildings[index])
            .unwrap()
            .1[10];
        let mut tracked = assets
            .open_for_owner(scene, index, Some(owner))
            .unwrap()
            .unwrap();
        let target = tracked.edit_target.unwrap();
        let (_, bounds) = controls[0];
        tracked.pointer(
            &assets,
            Some([
                (bounds[0] + bounds[2]) as i32,
                (bounds[1] + bounds[3]) as i32,
            ]),
        );
        tracked.mouse(true);
        assert!(tracked.tutorial_control_pressed());
        let mut reordered = scene.clone();
        reordered.inspection_buildings.reverse();
        assert!(
            assets
                .refresh(&reordered, scene, &mut tracked, Some(owner))
                .unwrap()
        );
        assert_eq!(
            tracked.building,
            scene.inspection_buildings.len() - 1 - index
        );
        let retained = tracked.edit_target.unwrap();
        assert_eq!(
            (retained.building, retained.business),
            (target.building, target.business)
        );
        assert!(tracked.tutorial_control_pressed());
        let mut changed = reordered.clone();
        let building = &mut changed.inspection_buildings[tracked.building];
        let record = if let Some(apartment) = &mut building.initialized_apartment {
            &mut apartment.record
        } else {
            &mut building.initialized_business.as_mut().unwrap().record
        };
        record[10] = owner.wrapping_add(1);
        assert!(
            assets
                .refresh(&changed, &reordered, &mut tracked, Some(owner))
                .unwrap()
        );
        assert!(tracked.edit_target.is_none());
        assert!(tracked.edit_request.is_none());
        assert!(!tracked.tutorial_control_pressed());
        #[cfg(feature = "capture")]
        if let Ok(prefix) = std::env::var("OPENMT2001_BUSINESS_LIVE_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            state.prepare(&assets, &renderer).unwrap();
            crate::capture_test::capture(
                &renderer,
                state.prepared.as_ref(),
                scene,
                &format!("{prefix}-{_name}.png"),
            );
        }
    }
}

#[cfg(test)]
pub(crate) fn refresh_assets_fixture() -> Assets {
    // The refresh path reads only owner_colors, products and block_names.
    Assets {
        base: vec![],
        retail_bounds: vec![],
        apartment_bounds: vec![],
        body_color: 0,
        owner_colors: [0; 10],
        apartment_base: vec![],
        apartment_regions: vec![],
        fonts: vec![],
        tab: vec![],
        strings: vec![],
        regions: vec![],
        products: vec![],
        buttons: vec![],
        apply_buttons: vec![],
        commodity_icons: vec![],
        arrows: vec![],
        retail_numeric: vec![],
        apartment_numeric: vec![],
        numeric_arrows: vec![],
        block_names: vec![0; 52],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retail_fixture() -> (Vec<Row>, [u8; 544]) {
        let mut catalog = [[0; 116]; 45];
        catalog[0][0x4c] = 2;
        catalog[0][0x70..0x74].copy_from_slice(&5u32.to_le_bytes());
        let mut record = [0; 544];
        for (slot, (wanted, held, yesterday, today)) in [(12u16, 4u16, 3u16, 2u16), (8, 5, 7, 6)]
            .into_iter()
            .enumerate()
        {
            record[0x1e0 + slot * 2..0x1e2 + slot * 2].copy_from_slice(&wanted.to_le_bytes());
            record[0x1b8 + slot * 2..0x1ba + slot * 2].copy_from_slice(&wanted.to_le_bytes());
            let stock = 0x80 + slot * 0x34;
            record[stock..stock + 2].copy_from_slice(&held.to_le_bytes());
            record[stock + 2..stock + 4].copy_from_slice(&today.to_le_bytes());
            record[stock + 8..stock + 10].copy_from_slice(&yesterday.to_le_bytes());
        }
        let rows = rows(
            &record,
            &BusinessCatalog::from_rows(catalog),
            &[[0; 28]; 32],
            &[2, 0],
        )
        .unwrap();
        (rows, record)
    }

    fn retail_state_fixture() -> State {
        State {
            edit_target: Some(crate::business_edit::Target {
                building: 0,
                business: 0,
            }),
            edit_request: None,
            edit_pending: None,
            edit_inflight: None,
            close: false,
            close_after_apply: false,
            dirty_terms: false,
            last_refresh: Instant::now(),
            edit_hover: None,
            edit_pressed: None,
            diagnostic: None,
            initialized: true,
            building: 0,
            rows: retail_fixture().0,
            stock_capacity: 30,
            row_hover: None,
            apartment: None,
            name: 0,
            block_name: 0,
            owner_color: 0,
            prepared: None,
            navigation: vec![],
            requested_building: None,
            dirty: false,
            hover: false,
            pressed: false,
            arrow_hover: None,
            arrow_pressed: None,
            tutorial_control: None,
            tutorial_producer: None,
        }
    }

    #[test]
    fn retail_columns_center_values_without_moving_commodity_labels() {
        let mut tab = vec![0; 4 + 96 * 44];
        tab[..4].copy_from_slice(&96u32.to_le_bytes());
        for glyph in tab[4..].as_chunks_mut::<44>().0.iter_mut() {
            glyph[32..36].copy_from_slice(&8i32.to_le_bytes());
            glyph[36..40].copy_from_slice(&10i32.to_le_bytes());
        }
        // The seven-unit advance makes the centering exercise native rounding.
        for column in 0..7 {
            let mut fonts = [Surface::solid(), Surface::solid()];
            let bounds = [40.25, 140.25, 141.25, 242.25];
            retail_row_text(&mut fonts, &tab, &[0, 0], bounds, column, 2).unwrap();
            let expected_x = if column == 0 { 61.5 } else { 83.5 };
            assert_eq!(fonts[0].vertices[0][0], expected_x, "column {column}");
            assert_eq!(fonts[0].vertices[0][1], 185.5);
            // All four totals use native style 4: centered both horizontally
            // and vertically, with the same unscaled font advances.
            if column < 4 {
                let mut totals = [Surface::solid(), Surface::solid()];
                ui::text_widget_style4(&mut totals, &tab, &[0, 0], bounds, 0xffffffff).unwrap();
                assert_eq!(totals[0].vertices[0][0], 83.5);
                assert_eq!(totals[0].vertices[0][1], 195.5);
            }
        }
    }

    #[test]
    fn pointer_moves_full_width_translucent_band_between_occupied_rows() {
        let mut state = retail_state_fixture();
        let mut assets = refresh_assets_fixture();
        assets.regions = vec![[0.; 4]; 30];
        assets.regions[1] = [35., 100., 635., 300.];
        assets.regions[15] = [35., 145., 150., 247.];
        for (point, expected) in [
            (Some([80, 300]), Some(0)),
            (Some([1000, 335]), Some(1)),
            (Some([1000, 370]), None), // Empty third row.
            (Some([1300, 300]), None),
            (None, None),
        ] {
            let previous = state.row_hover;
            state.pointer(&assets, point);
            assert_eq!(state.row_hover, expected);
            assert_eq!(state.dirty, previous != expected);
            state.dirty = false;
            let band = retail_row_band(&assets.regions, state.row_hover);
            if let Some(row) = expected {
                let b = retail_row_bounds(&assets.regions, row);
                assert_eq!(band.vertices.len(), 6);
                assert_eq!(band.vertices[0][..2], [35., 145. + row as f32 * 17.]);
                assert_eq!(band.vertices[2][..2], [635., b[3]]);
                assert_eq!(band.alpha, [0xcc; 6]);
                assert_eq!(
                    band.vertices[0][3..6],
                    [55. / 255., 71. / 255., 138. / 255.]
                );
            } else {
                assert!(band.vertices.is_empty());
            }
        }
        // Hover works while a worker edit is pending and on read-only businesses.
        state.edit_target = None;
        state.pointer(&assets, Some([80, 335]));
        assert_eq!(state.row_hover, Some(1));
        let (_sender, pending) = std::sync::mpsc::channel();
        state.edit_pending = Some(pending);
        state.pointer(&assets, Some([80, 300]));
        assert_eq!(state.row_hover, Some(0));
    }

    #[test]
    fn stock_totals_share_building_capacity_and_refresh_after_apply() {
        let (mut rows, mut record) = retail_fixture();
        let mut info = [0; 116];
        info[0x58..0x5c].copy_from_slice(&1.25f32.to_le_bytes());
        let capacity = retail_capacity([9, 3, 4, 3], 2, &info);
        assert_eq!(capacity, 30);
        assert_eq!(retail_totals(&rows, capacity), ["10", "20/30", "9/30", "8"]);
        record[0x1e0..0x1e2].copy_from_slice(&15u16.to_le_bytes());
        record[0x80..0x82].copy_from_slice(&11u16.to_le_bytes());
        refresh_retail_rows(&mut rows, &record).unwrap();
        assert_eq!(
            retail_totals(&rows, capacity),
            ["10", "23/30", "16/30", "8"]
        );
    }

    #[test]
    fn apply_uses_original_disabled_and_active_images() {
        for (hover, pressed, phase) in [(false, false, 0), (true, false, 1), (true, true, 2)] {
            let (image, color) = apply_presentation(false, hover, pressed);
            assert_eq!(APPLY_TEXTURES[image], [85, 87, 86][phase]);
            assert_eq!(color, 0xffffffff);
            let (image, color) = apply_presentation(true, hover, pressed);
            assert_eq!(APPLY_TEXTURES[image], 143);
            assert_eq!(color, 0xffffffff);
        }
    }

    #[test]
    fn apply_disables_after_submission_or_reverting_stock_and_price_edits() {
        let (_, mut record) = retail_fixture();
        assert!(!pending_terms(&record));
        record[0x208] = 1;
        record[0x1e0..0x1e2].copy_from_slice(&13u16.to_le_bytes());
        assert!(pending_terms(&record));
        record[0x1e0..0x1e2].copy_from_slice(&12u16.to_le_bytes());
        assert!(!pending_terms(&record));
        record[0x1ec..0x1f0].copy_from_slice(&1.5f32.to_le_bytes());
        assert!(pending_terms(&record));
        record[0x1ec..0x1f0].copy_from_slice(&0f32.to_le_bytes());
        assert!(!pending_terms(&record));
        record[0x1e0..0x1e2].copy_from_slice(&13u16.to_le_bytes());
        record[0x208] = 0;
        assert!(!pending_terms(&record));
    }

    #[test]
    fn authoritative_edit_replies_refresh_totals_and_apply_admission() {
        use crate::business_edit::Action;
        let (_, mut record) = retail_fixture();
        let mut state = retail_state_fixture();
        let mut assets = refresh_assets_fixture();
        assets.regions = vec![[0.; 4]; 30];
        assets.regions[29] = [100., 100., 160., 124.];
        let apply = |state: &mut State| {
            state.pointer(&assets, Some([240, 220]));
            state.mouse(true);
            state.mouse(false);
            state.edit_request.take()
        };
        assert_eq!(apply(&mut state), None);
        record[0x208] = 1;
        record[0x1e0..0x1e2].copy_from_slice(&13u16.to_le_bytes());
        state.edit_inflight = Some(Action::Stock {
            slot: 0,
            increase: true,
        });
        state.receive_edit(Ok(record));
        assert_eq!(retail_totals(&state.rows, state.stock_capacity)[1], "21/30");
        assert_eq!(apply(&mut state), Some(Action::Apply));
        let (_sender, pending) = std::sync::mpsc::channel();
        state.edit_pending = Some(pending);
        assert_eq!(apply(&mut state), None);
        state.edit_pending = None;
        record[0x1e0..0x1e2].copy_from_slice(&12u16.to_le_bytes());
        state.edit_inflight = Some(Action::Stock {
            slot: 0,
            increase: false,
        });
        state.receive_edit(Ok(record));
        assert_eq!(apply(&mut state), None);
        record[0x1e0..0x1e2].copy_from_slice(&14u16.to_le_bytes());
        record[0x208] = 0;
        state.edit_inflight = Some(Action::Apply);
        state.receive_edit(Ok(record));
        assert_eq!(retail_totals(&state.rows, state.stock_capacity)[1], "22/30");
        assert_eq!(apply(&mut state), None);
    }

    #[test]
    fn every_commodity_row_submits_its_own_icon_in_the_mesh_region() {
        let (rows, _) = retail_fixture();
        let mut assets = refresh_assets_fixture();
        assets.commodity_icons = (0..3)
            .map(|i| {
                let mut surface = Surface::solid();
                surface.pixels[0] = i;
                surface
            })
            .collect();
        assets.retail_bounds = vec![
            ("BAP_COMM_ICON_ROW1".into(), [41., 140., 55., 154.]),
            ("BAP_COMM_ICON_ROW2".into(), [41., 157., 55., 171.]),
        ];
        let mut surfaces = Vec::new();
        assets
            .prepare_commodity_icons(&rows, &mut surfaces)
            .unwrap();
        assert_eq!(surfaces.len(), rows.len());
        for (slot, surface) in surfaces.iter().enumerate() {
            assert_eq!(surface.pixels[0], rows[slot].commodity as u8);
            assert_eq!(surface.vertices.len(), 6);
            assert_eq!(surface.vertices[0][..2], assets.retail_bounds[slot].1[..2]);
            assert_eq!(surface.vertices[2][..2], assets.retail_bounds[slot].1[2..]);
        }
    }

    #[test]
    fn panel_backings_preserve_translucent_stock_and_solid_edit_columns() {
        let palette = super::retail_palette(0xbb000a28, 0x0000ff00);
        for name in [
            "BAP_BOTTOM_AREA",
            "BAP_COMMODITY_LIST",
            "BAP_COST_LIST",
            "BAP_YESTERDAY_LIST",
            "BAP_STOCK_I_HAVE_LIST",
            "BAP_TODAY_LIST",
        ] {
            assert_eq!(
                palette.iter().find(|p| p.0 == name).unwrap().1,
                [0xbb000a28; 4]
            );
        }
        for name in ["BAP_SELLING_LIST", "BAP_STOCK_I_WANT_LIST"] {
            assert_eq!(
                palette.iter().find(|p| p.0 == name).unwrap().1,
                [0xff202a53; 4]
            );
        }
        assert_eq!(
            palette.iter().find(|p| p.0 == "BAP_TOTAL_LABEL").unwrap().1,
            [0x6d4f94b0; 4]
        );
    }

    #[test]
    #[ignore = "original43aca0/416440/44309c numeric geometry and texture oracle"]
    fn matches_native_retail_numeric_controls() {
        check_numeric_controls(false);
    }
    #[test]
    #[ignore = "original43aca0/40af10/44309c apartment geometry and texture oracle"]
    fn matches_native_apartment_numeric_controls() {
        check_numeric_controls(true);
    }
    fn check_numeric_controls(apartment: bool) {
        use crate::business_edit::Action;
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find(if apartment {
                "gamedata2/brand_new_panels/bap_apartments_v2.tm"
            } else {
                "gamedata2/brand_new_panels/bap_v2.tm"
            })
            .unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let regions = openmt2001::ui_mesh::regions(&mesh, if apartment { 0 } else { 9 })
            .unwrap()
            .into_iter()
            .map(|region| (region.name, region.bounds))
            .collect::<Vec<_>>();
        let controls = if apartment {
            super::apartment_numeric_controls(&regions)
        } else {
            super::retail_numeric_controls(&regions)
        }
        .unwrap();
        let bytes = std::fs::read(openmt2001::fixtures::path(if apartment {
            "NATIVE_APARTMENT_CONTROLS"
        } else {
            "NATIVE_ADJUSTMENT_CONTROLS"
        }))
        .unwrap();
        assert_eq!(&bytes[..8], b"MTBAPCTL");
        let mut words = bytes[8..]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        assert_eq!(words.next(), Some(0x037f));
        assert_eq!(words.next(), Some(if apartment { 2 } else { 24 }));
        for (action, bounds) in controls {
            let id = words.next().unwrap();
            let increase = id % 2 != 0;
            let native_action = if apartment {
                assert!(matches!(id, 8 | 9));
                Action::Rent { increase }
            } else if id < 22 {
                Action::Price {
                    slot: ((id - 10) / 2) as usize,
                    increase,
                }
            } else {
                Action::Stock {
                    slot: ((id - 22) / 2) as usize,
                    increase,
                }
            };
            assert_eq!(action, native_action);
            for value in bounds {
                assert_eq!(words.next(), Some(value.to_bits()));
            }
            assert_eq!(words.next(), Some(0.5f32.to_bits()));
            for &texture in &super::RETAIL_ARROW_TEXTURES[usize::from(increase) * 4..][..4] {
                assert_eq!(words.next(), Some(texture as u32));
            }
        }
        assert_eq!(words.next(), None);
    }
    #[test]
    #[ignore = "private original retail draw fixture"]
    fn matches_native_retail_palette() {
        check_palette("OPENMT2001_NATIVE_BAP_STYLE", false);
    }
    #[test]
    #[ignore = "private original apartment draw fixture"]
    fn matches_native_apartment_palette() {
        check_palette("OPENMT2001_NATIVE_APARTMENT_STYLE", true);
    }
    fn check_palette(key: &str, apartment: bool) {
        let bytes = std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        assert_eq!(&bytes[..8], b"MTBAPCLR");
        let mut at = 8;
        fn word(bytes: &[u8], at: &mut usize) -> u32 {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        }
        for _ in 0..word(&bytes, &mut at) {
            let owner = word(&bytes, &mut at);
            let actual = if apartment {
                super::apartment_palette(0xbb000a28, owner)
            } else {
                super::retail_palette(0xbb000a28, owner)
            };
            assert_eq!(word(&bytes, &mut at) as usize, actual.len());
            for (name, colors) in actual {
                let length = word(&bytes, &mut at) as usize;
                assert_eq!(name.as_bytes(), &bytes[at..at + length]);
                at += length;
                assert_eq!(colors, std::array::from_fn(|_| word(&bytes, &mut at)));
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "private original apartment navigation fixture"]
    fn matches_native_apartment_navigation() {
        check_navigation("OPENMT2001_NATIVE_APARTMENT_NAVIGATION", 1);
    }
    #[test]
    #[ignore = "private original retail navigation fixture"]
    fn matches_native_retail_navigation() {
        check_navigation("OPENMT2001_NATIVE_RETAIL_NAVIGATION", 0);
    }
    fn check_navigation(env: &str, kind: u8) {
        let bytes = std::fs::read(openmt2001::fixtures::path(env)).unwrap();
        assert_eq!(&bytes[..8], b"MTAPNAV2");
        let mut at = 8;
        fn word(b: &[u8], at: &mut usize) -> usize {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v as usize
        }
        for _ in 0..word(&bytes, &mut at) {
            let records = &bytes[at..at + 600 * 544];
            at += 600 * 544;
            for _ in 0..word(&bytes, &mut at) {
                let slot = word(&bytes, &mut at);
                let count = word(&bytes, &mut at);
                let selected = word(&bytes, &mut at);
                let actual = super::business_order(
                    records
                        .chunks_exact(544)
                        .enumerate()
                        .map(|(slot, r)| (slot, r.try_into().unwrap())),
                    records[slot * 544 + 0xa],
                    kind,
                );
                assert_eq!(actual.len(), count);
                for &value in &actual {
                    assert_eq!(value, word(&bytes, &mut at));
                }
                assert_eq!(actual[selected], slot);
                for next in [false, true] {
                    assert_eq!(
                        super::navigation_index(selected, count, next),
                        word(&bytes, &mut at)
                    );
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "private original apartment refresh fixture"]
    fn matches_native_apartment_adjustment() {
        check_apartment_fixture("OPENMT2001_NATIVE_APARTMENT_ADJUSTMENT");
    }
    #[test]
    #[ignore = "private original refresh applied to initialized apartment records"]
    fn matches_native_initialized_apartment_adjustment() {
        check_apartment_fixture("OPENMT2001_NATIVE_INITIALIZED_APARTMENT_ADJUSTMENT");
    }
    fn check_apartment_fixture(key: &str) {
        let bytes = std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        assert_eq!(&bytes[..8], b"MTAPAR02");
        let mut at = 8;
        fn word(b: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        for _ in 0..word(&bytes, &mut at) {
            let slot = word(&bytes, &mut at);
            let tenants = word(&bytes, &mut at) != 0;
            let income = word(&bytes, &mut at) != 0;
            let row = super::Apartment::from_record(
                bytes[at..at + 544].try_into().unwrap(),
                tenants,
                income,
            )
            .unwrap();
            at += 544;
            for (offset, format, a, b) in [
                (0x1644, 1, row.rent.to_bits(), 0),
                (0x17f8, 3, u32::from(row.tenants), u32::from(row.capacity)),
                (0x19ac, 1, row.total_rent.to_bits(), 0),
                (0x1b60, 2, row.potential_rent as u32, 0),
            ] {
                assert_eq!(
                    [
                        word(&bytes, &mut at),
                        word(&bytes, &mut at),
                        word(&bytes, &mut at),
                        word(&bytes, &mut at)
                    ],
                    [offset, format, a, b],
                    "apartment {slot}"
                );
            }
        }
        for _ in 0..word(&bytes, &mut at) {
            let arrow = word(&bytes, &mut at) as usize;
            let before = word(&bytes, &mut at) != 0;
            let expected = word(&bytes, &mut at) != 0;
            let mut record = [0u8; 544];
            record[0] = 1;
            let mut apartment = super::Apartment::from_record(&record, before, before).unwrap();
            apartment.toggle(arrow / 2);
            assert_eq!(apartment.modes[arrow / 2], expected);
            assert_eq!(apartment.modes[1 - arrow / 2], before);
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "private original Business Adjustment row fixture"]
    fn matches_native_business_adjustment() {
        let bytes =
            std::fs::read(openmt2001::fixtures::path("NATIVE_BUSINESS_ADJUSTMENT")).unwrap();
        assert_eq!(&bytes[..8], b"MTBAPR01");
        let mut at = 8;
        fn word(b: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        let ni = word(&bytes, &mut at) as usize;
        let nc = word(&bytes, &mut at) as usize;
        let info =
            openmt2001::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|i| {
                bytes[at + i * 116..at + (i + 1) * 116].try_into().unwrap()
            }));
        at += ni;
        let commodities =
            std::array::from_fn(|i| bytes[at + i * 28..at + (i + 1) * 28].try_into().unwrap());
        at += nc;
        for _ in 0..word(&bytes, &mut at) {
            let slot = word(&bytes, &mut at);
            let _kind = word(&bytes, &mut at);
            let product = word(&bytes, &mut at);
            let record = bytes[at..at + 544].try_into().unwrap();
            at += 544;
            let row = super::rows(record, &info, &commodities, &[product])
                .unwrap()
                .remove(0);
            let mut refreshed = vec![super::Row {
                price: 0.,
                yesterday: 0,
                wanted: 0,
                stock: 0,
                today: 0,
                ..row.clone()
            }];
            super::refresh_retail_rows(&mut refreshed, record).unwrap();
            assert_eq!(refreshed[0].price.to_bits(), row.price.to_bits());
            assert_eq!(refreshed[0].yesterday, row.yesterday);
            assert_eq!(refreshed[0].wanted, row.wanted);
            assert_eq!(refreshed[0].stock, row.stock);
            assert_eq!(refreshed[0].today, row.today);
            let values = [
                row.name,
                row.cost.to_bits(),
                row.price.to_bits(),
                u32::from(row.yesterday),
                row.wanted as u32,
                u32::from(row.stock),
                u32::from(row.today),
            ];
            assert_eq!(word(&bytes, &mut at), 7);
            for (column, value) in values.into_iter().enumerate() {
                assert_eq!(word(&bytes, &mut at), 0x2e64 + column as u32 * 6 * 0x1b4);
                assert_eq!(
                    word(&bytes, &mut at),
                    match column {
                        0 => 0,
                        1 | 2 => 1,
                        _ => 2,
                    },
                    "business{slot} column{column} native formatter",
                );
                assert_eq!(
                    word(&bytes, &mut at),
                    value,
                    "business{slot} column{column}"
                );
            }
        }
        assert_eq!(at, bytes.len());
    }
}
