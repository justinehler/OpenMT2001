//! Original403b10 utility revenue projections, read-only over saved business pools.
use openmt2001::{save::SaveFile, x87::Sum};
#[derive(Clone, Debug)]
pub struct Report {
    pub groups: [Vec<f32>; 4],
    pub total: [f32; 4],
    pub projections: [[f32; 4]; 4],
}
impl Report {
    pub fn from_save(
        save: &SaveFile<'_>,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        let economy = save.economy()?;
        let table = save
            .sections
            .iter()
            .find(|s| s.tag == 4)
            .ok_or("utility building table missing")?
            .payload;
        if table.len() != 0xb270 {
            return Err("utility building table size".into());
        }
        let businesses: Vec<_> = economy.businesses().map(|b| b.bytes.as_slice()).collect();
        let buildings: Vec<_> = table[..0x9600].chunks_exact(64).collect();
        let rates = std::array::from_fn(|i| economy.utility_rates(i as u32).unwrap());
        Self::from_records(
            save.players()?.len(),
            &businesses,
            &buildings,
            &rates,
            image,
        )
    }
    pub fn from_runtime(
        world: &openmt2001::level_startup::World,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        let businesses: Vec<_> = world
            .ledgers
            .businesses
            .iter()
            .map(|b| b.as_slice())
            .collect();
        let buildings: Vec<_> = world.buildings.iter().map(|b| b.as_slice()).collect();
        let rates = std::array::from_fn(|kind| {
            std::array::from_fn(|i| {
                f32::from_le_bytes(
                    world.catalog.info.rows()[kind][0x60 + i * 4..0x64 + i * 4]
                        .try_into()
                        .unwrap(),
                )
            })
        });
        Self::from_records(
            world.ledgers.players.len(),
            &businesses,
            &buildings,
            &rates,
            image,
        )
    }
    fn from_records(
        player_count: usize,
        businesses: &[&[u8]],
        buildings: &[&[u8]],
        rates: &[[f32; 4]; 45],
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        if player_count > 6 {
            return Err("utility report exceeds original roster".into());
        }
        let mut groups: [Vec<f32>; 4] = std::array::from_fn(|_| vec![0.; player_count + 1]);
        for business in businesses {
            if business[0x21c] != 1 {
                continue;
            }
            let owner = business[0xa] as usize;
            if owner > player_count {
                return Err("utility business owner outside roster".into());
            }
            let slot = i16::from_le_bytes(business[8..10].try_into().unwrap());
            if !(0..600).contains(&slot) {
                return Err("utility business building outside pool".into());
            }
            let b = buildings
                .get(slot as usize)
                .ok_or("utility building missing")?;
            let scalar = (i32::from(b[7]) - i32::from(b[5]) + 1)
                .wrapping_mul(i32::from(b[6]) - i32::from(b[4]) + 1)
                .wrapping_mul(i32::from(b[8]));
            let kind = u32::from_le_bytes(business[4..8].try_into().unwrap()) as usize;
            let rates = rates.get(kind).ok_or("utility report business type")?;
            for (i, rate) in [rates[2], rates[3], rates[1], rates[0]]
                .into_iter()
                .enumerate()
            {
                let mut amount = Sum::from_f32(Sum::from_i32(scalar).to_f32())?;
                amount.mul_f32(rate)?;
                amount.add_f32(groups[i][owner])?;
                groups[i][owner] = amount.to_f32();
            }
        }
        let mut total = [0.; 4];
        let mut projections = [[0.; 4]; 4];
        let factors = [
            image.f32_at(0x525048)?,
            image.f32_at(0x52502c)?,
            image.f32_at(0x525044)?,
            1.,
        ];
        for i in 0..4 {
            let mut amount = Sum::from_f32(groups[i][0])?;
            for &v in &groups[i][1..] {
                amount.add_f32(v)?;
            }
            total[i] = amount.to_f32();
            for (j, factor) in factors.iter().enumerate() {
                let mut v = Sum::from_f32(total[i])?;
                v.mul_f32(*factor)?;
                projections[i][j] = v.to_f32();
            }
        }
        Ok(Self {
            groups,
            total,
            projections,
        })
    }
    pub fn amounts(&self, utility: usize, details: bool, owner: usize) -> Result<Vec<f32>, String> {
        let groups = self.groups.get(utility).ok_or("utility index")?;
        let own = *groups.get(owner).ok_or("utility landlord outside roster")?;
        if !details {
            return Ok(self.projections[utility].to_vec());
        }
        let mut result = groups
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(i, _)| *i != owner)
            .map(|(_, v)| *v)
            .collect::<Vec<_>>();
        if owner != 0 {
            result.push(groups[0]);
        }
        let mut other = Sum::from_f32(self.total[utility])?;
        other.add(Sum::from_f32(own)?.negated());
        result.extend([other.to_f32(), own]);
        Ok(result)
    }
}
#[derive(Clone)]
pub struct Railroads {
    pub totals: [f32; 4],
    pub freight: f32,
}
impl Railroads {
    pub fn amounts(&self, station: usize, details: bool) -> Result<Vec<f32>, String> {
        let total = *self.totals.get(station).ok_or("railroad report station")?;
        let rounded = Sum::from_i32(Sum::from_f32(total)?.ftol_low_word()).to_f32();
        if details {
            let mut tickets = Sum::from_f32(total)?;
            tickets.add(Sum::from_f32(self.freight)?.negated());
            Ok(vec![self.freight, tickets.to_f32(), rounded])
        } else {
            [0.25, 0.5, 0.75, 1.]
                .into_iter()
                .map(|factor| {
                    let mut value = Sum::from_f32(rounded)?;
                    value.mul_f32(factor)?;
                    Ok(value.to_f32())
                })
                .collect()
        }
    }
}
/// Original405620 utility list: font3 in both columns, one/three-pixel
/// horizontal insets, three-pixel vertical inset, and ten-pixel line steps.
pub struct Layout {
    pub bounds: [f32; 4],
}
impl Layout {
    pub fn new([l, t, r, b]: [f32; 4]) -> Self {
        Self {
            bounds: [l, t + 38., r - 2., b],
        }
    }
    pub fn line(&self, row: usize, right: bool) -> [f32; 4] {
        let [l, t, r, _] = self.bounds;
        let inset = if right { 3. } else { 1. };
        let y = t + 3. + row as f32 * 10.;
        [l + inset, y, r - inset, y + 10.]
    }
    pub fn heading(&self, row: usize) -> [f32; 4] {
        self.line(row, false)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyRequest {
    pub category: u32,
    pub panel: openmt2001::property_offer::Panel,
    pub submit: bool,
    pub initialize_amount: bool,
}
/// Original Rights mode2 inspection /4050d0 activation handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RightsRequest {
    pub category: u32,
    pub panel_mode: u8,
    pub activate: bool,
}
#[derive(Default)]
pub struct View {
    category: Option<usize>,
    pub details: bool,
    pub scroll: usize,
    pub row_hover: Option<usize>,
    row_pressed: Option<usize>,
    pub requested_slot: Option<usize>,
    pub hover: Option<usize>,
    pub pressed: Option<usize>,
    pub dirty: bool,
    tutorial_activated: Option<u32>,
    pub property_tab: usize,
    pub property_tab_count: usize,
    pub previous_profit: bool,
    pub previous_efficiency: bool,
    property_rights_open: bool,
    property_rights_mode: u8,
    rights_inspection: Option<openmt2001::property_rights::Inspection>,
    rights_request: Option<RightsRequest>,
    rights_open_category: Option<usize>,
    property_offer_open: bool,
    property_inspection: Option<crate::fresh_city::property::Inspection>,
    property_panel: openmt2001::property_offer::Panel,
    property_request: Option<PropertyRequest>,
    property_error: Option<String>,
    pub property_control_hover: Option<u32>,
    property_control_pressed: Option<u32>,
    property_initialize_amount: bool,
}
impl View {
    pub fn select(&mut self, category: Option<usize>) {
        if self.category != category {
            self.category = category;
            self.details = false;
            self.scroll = 0;
            self.row_hover = None;
            self.row_pressed = None;
            self.requested_slot = None;
            self.hover = None;
            self.pressed = None;
            self.tutorial_activated = None;
            self.property_tab = 0;
            self.property_rights_open = false;
            self.rights_inspection = None;
            self.rights_request = None;
            self.rights_open_category = None;
            self.property_offer_open = false;
            self.property_inspection = None;
            self.property_request = None;
            self.property_error = None;
            self.property_initialize_amount = true;
            self.property_control_hover = None;
            self.property_control_pressed = None;
            self.dirty = true;
        }
    }
    pub fn pointer(&mut self, point: Option<[i32; 2]>, buttons: &[[f32; 4]]) {
        let hover = point.and_then(|[x, y]| {
            let (x, y) = (x as f32 * 0.5, y as f32 * 0.5);
            buttons
                .iter()
                .take(if self.property_tab_count == 0 {
                    2
                } else {
                    self.property_tab_count
                })
                .position(|&[l, t, r, b]| x >= l && x <= r && y >= t && y <= b)
        });
        self.dirty |= hover != self.hover;
        self.hover = hover;
    }
    pub fn mouse(&mut self, down: bool) -> bool {
        self.mouse_with_rights_admission(down, true)
    }
    /// Native 43e800 excludes disabled children from hit testing. Tutorial
    /// control 91 owns the Activate Building Rights child; a visible message
    /// alone does not enable its click after DisableAllControls.
    pub fn mouse_with_rights_admission(&mut self, down: bool, rights_enabled: bool) -> bool {
        if !rights_enabled {
            if self.property_control_hover == Some(102) {
                self.property_control_hover = None;
            }
            if self.property_control_pressed == Some(102) {
                self.property_control_pressed = None;
            }
        }
        if self.property_control_hover.is_some() || self.property_control_pressed.is_some() {
            if down {
                self.property_control_pressed = self.property_control_hover;
            } else if let Some(control) = self.property_control_pressed.take()
                && self.property_control_hover == Some(control)
            {
                match control {
                    9 => {
                        self.close_property_offer();
                        self.close_property_rights();
                    }
                    11 => {
                        if self.property_rights_open {
                            self.close_property_rights();
                        } else if let Some(category) = self.category {
                            self.open_property_rights(category, 0);
                        }
                    }
                    102 => self.activate_property_rights(),
                    10 => {
                        if self.property_offer_open {
                            self.close_property_offer();
                        } else if let Some(category) = self.category {
                            self.open_property_offer(category);
                        }
                    }
                    33 | 34 => {
                        if self.property_tab == 2 {
                            self.previous_profit = !self.previous_profit;
                        }
                        if self.property_tab == 3 {
                            self.previous_efficiency = !self.previous_efficiency;
                        }
                        self.scroll = 0;
                    }
                    35 => self.submit_property_offer(),
                    100 | 101 => self.adjust_property_amount(control == 101),
                    _ => (),
                }
                if control <= 35 {
                    self.tutorial_activated = Some(control);
                }
            }
            self.dirty = true;
            return true;
        }
        if self.row_hover.is_some() || self.row_pressed.is_some() {
            if down {
                self.row_pressed = self.row_hover;
            } else if self.row_pressed.take() == self.row_hover {
                self.requested_slot = self.row_hover;
                if self.requested_slot.is_some() {
                    self.tutorial_activated = Some(6);
                }
            }
            self.dirty = true;
            return true;
        }
        let consumed = self.hover.is_some() || self.pressed.is_some();
        if down {
            self.pressed = self.hover;
        } else if let Some(button) = self.pressed.take()
            && self.hover == Some(button)
        {
            self.details = button == 1;
            self.property_tab = button;
            self.scroll = 0;
            // Original41ffc4 dispatches MPP_INFO1 + the selected tab.
            self.tutorial_activated = Some(15 + button as u32);
        }
        self.dirty |= consumed;
        consumed
    }
    pub fn tutorial_control_pressed(&self) -> bool {
        self.pressed.is_some()
            || self.row_pressed.is_some()
            || self.property_control_pressed.is_some()
    }
    pub fn take_tutorial_control(&mut self) -> Option<u32> {
        self.tutorial_activated.take()
    }
    /// Original402390 toggles the Lease/Sell mode without reconstructing the
    /// numeric widget. Native404d90 then refreshes its retained amount/ranges.
    pub fn open_property_offer(&mut self, category: usize) {
        self.select(Some(category));
        self.close_property_rights();
        self.property_offer_open = true;
        self.property_error = None;
        // Original402d10 hides both children before404d90 and401300 display.
        self.property_panel.visible = 0;
        self.property_panel.amount_visible = 0;
        self.property_panel.action_visible = 0;
        self.property_request = Some(PropertyRequest {
            category: category as u32,
            panel: self.property_panel,
            submit: false,
            initialize_amount: self.property_initialize_amount,
        });
        self.dirty = true;
    }
    pub fn close_property_offer(&mut self) {
        self.property_offer_open = false;
        self.property_request = None;
        self.dirty = true;
    }
    pub fn open_property_rights(&mut self, category: usize, panel_mode: u8) {
        self.select(Some(category));
        self.close_property_offer();
        self.property_rights_open = true;
        self.property_rights_mode = panel_mode;
        self.rights_inspection = None;
        self.rights_open_category = None;
        self.property_error = None;
        self.rights_request = Some(RightsRequest {
            category: category as u32,
            panel_mode,
            activate: false,
        });
        self.dirty = true;
    }
    pub fn close_property_rights(&mut self) {
        self.property_rights_open = false;
        self.rights_request = None;
        self.rights_open_category = None;
        self.dirty = true;
    }
    pub fn property_rights_open(&self) -> bool {
        self.property_rights_open
    }
    pub fn property_rights_state(&self) -> Option<&openmt2001::property_rights::Inspection> {
        self.rights_inspection.as_ref()
    }
    pub fn take_rights_request(&mut self) -> Option<RightsRequest> {
        self.rights_request.take()
    }
    pub fn take_rights_open(&mut self) -> Option<usize> {
        let category = self.rights_open_category.take();
        if category.is_some() {
            self.close_property_rights();
        }
        category
    }
    pub fn apply_rights_inspection(
        &mut self,
        category: u32,
        result: Result<openmt2001::property_rights::Inspection, String>,
    ) {
        if self.category != Some(category as usize) || !self.property_rights_open {
            return;
        }
        match result {
            Ok(inspection) => {
                self.rights_open_category = inspection.open.then_some(category as usize);
                self.rights_inspection = Some(inspection);
                self.property_error = None;
            }
            Err(error) => self.property_error = Some(error),
        }
        self.dirty = true;
    }
    fn activate_property_rights(&mut self) {
        if self.property_rights_open
            && self.rights_inspection.is_some_and(|i| i.activate_visible)
            && let Some(category) = self.category
        {
            self.rights_request = Some(RightsRequest {
                category: category as u32,
                panel_mode: self.property_rights_mode,
                activate: true,
            });
        }
    }
    pub fn refresh_property_offer(&mut self) {
        if self.rights_request.is_none()
            && self.property_rights_open
            && let Some(category) = self.category
        {
            self.rights_request = Some(RightsRequest {
                category: category as u32,
                panel_mode: self.property_rights_mode,
                activate: false,
            });
        }
        if self.property_request.is_none()
            && self.property_offer_open
            && let Some(category) = self.category
        {
            self.property_request = Some(PropertyRequest {
                category: category as u32,
                panel: self.property_panel,
                submit: false,
                initialize_amount: self.property_initialize_amount,
            });
        }
    }
    pub fn property_offer_open(&self) -> bool {
        self.property_offer_open
    }
    pub fn property_panel_state(&self) -> Option<&crate::fresh_city::property::Inspection> {
        self.property_inspection.as_ref()
    }
    pub fn take_property_request(&mut self) -> Option<PropertyRequest> {
        self.property_request.take()
    }
    /// Responses carry the request identity, so switching blocks during a
    /// worker operation cannot apply its panel or error to the new selection.
    pub fn apply_property_inspection(
        &mut self,
        category: u32,
        result: Result<crate::fresh_city::property::Inspection, String>,
    ) {
        if self.category != Some(category as usize) || !self.property_offer_open {
            return;
        }
        match result {
            Ok(inspection) if inspection.category == category => {
                self.property_panel = inspection.panel;
                self.property_inspection = Some(inspection);
                self.property_error = None;
                self.property_initialize_amount = false;
            }
            Ok(_) => return,
            Err(error) => self.property_error = Some(error),
        }
        self.dirty = true;
    }
    pub fn property_error(&self) -> Option<&str> {
        self.property_error.as_deref()
    }
    pub fn property_control_pointer(&mut self, control: Option<u32>) {
        self.dirty |= self.property_control_hover != control;
        self.property_control_hover = control;
        if control.is_some() || self.property_offer_open {
            self.hover = None;
            self.row_hover = None;
        }
    }
    pub fn property_control_selected(&self, control: u32) -> bool {
        self.property_control_pressed == Some(control)
    }
    pub fn adjust_property_amount(&mut self, increase: bool) {
        if !self.property_offer_open
            || self.property_panel.visible == 0
            || self.property_panel.amount_visible == 0
        {
            return;
        }
        let panel = &mut self.property_panel;
        let delta = if increase { panel.step } else { -panel.step };
        panel.amount = (panel.amount + delta).max(panel.minimum).min(panel.maximum);
        if let Some(inspection) = &mut self.property_inspection {
            inspection.panel = *panel;
        }
        if let Some(category) = self.category {
            self.property_request = Some(PropertyRequest {
                category: category as u32,
                panel: *panel,
                submit: false,
                initialize_amount: false,
            });
        }
        self.dirty = true;
    }
    pub fn submit_property_offer(&mut self) {
        if let Some(inspection) = self.property_inspection
            && inspection.panel.visible != 0
            && inspection.panel.action_visible != 0
            && let Some(category) = self.category
        {
            self.property_request = Some(PropertyRequest {
                category: category as u32,
                panel: self.property_panel,
                submit: true,
                initialize_amount: false,
            });
            self.dirty = true;
        }
    }
    pub fn title(&self, utility: Option<usize>) -> usize {
        if self.details {
            utility.map_or(1812, |utility| 1806 + utility)
        } else {
            28
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original railroad report oracle"]
    fn matches_native_railroad_report() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_RAILROAD_REPORT").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTRAILR1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        for _ in 0..word(&mut at) {
            let station = word(&mut at) as usize;
            let total = f32::from_bits(word(&mut at));
            let freight = f32::from_bits(word(&mut at));
            let details = word(&mut at) != 0;
            let expected = (0..word(&mut at))
                .map(|_| word(&mut at))
                .collect::<Vec<_>>();
            let mut report = Railroads {
                totals: [0.; 4],
                freight,
            };
            report.totals[station] = total;
            assert_eq!(
                report
                    .amounts(station, details)
                    .unwrap()
                    .into_iter()
                    .map(f32::to_bits)
                    .collect::<Vec<_>>(),
                expected,
                "{station} {total} {freight} {details}"
            );
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "private original utility controls oracle"]
    fn matches_native_utility_controls() {
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_UTILITY_CONTROLS").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTUPCTRL");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        for _ in 0..word(&mut at) {
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let dimensions = std::array::from_fn(|_| word(&mut at));
            let layout = crate::ui::mpp_modes(bounds, dimensions).unwrap();
            assert_eq!(word(&mut at), 6);
            for rect in layout.buttons[..5]
                .iter()
                .chain(std::iter::once(&layout.header))
                .chain(layout.icons[..2].iter())
            {
                assert_eq!(
                    rect.map(f32::to_bits),
                    std::array::from_fn(|_| word(&mut at))
                );
            }
            assert_eq!(
                [291, 1157, 301, 1806],
                std::array::from_fn(|_| word(&mut at))
            );
        }
        for _ in 0..word(&mut at) {
            let utility = word(&mut at) as usize;
            let button = word(&mut at) as usize;
            let mode = word(&mut at);
            let details = word(&mut at);
            let title = word(&mut at);
            let mut view = View::default();
            view.select(Some(utility));
            let layout = crate::ui::mpp_modes([30., 29., 190., 300.], [25, 25]).unwrap();
            let [l, t, r, b] = layout.buttons[button];
            view.pointer(Some([(l + r) as i32, (t + b) as i32]), &layout.buttons);
            assert!(view.mouse(true));
            assert!(view.mouse(false));
            assert_eq!(view.take_tutorial_control(), Some(15 + button as u32));
            assert_eq!(view.details, details != 0);
            assert_eq!(
                if view.details {
                    if utility == 8 { 1 } else { 5 }
                } else {
                    0
                },
                mode
            );
            assert_eq!(
                if utility == 8 && view.details {
                    1402
                } else {
                    view.title((utility < 4).then_some(utility)) as u32
                },
                title
            );
            view.mouse(true);
            view.pointer(None, &layout.buttons);
            view.mouse(false);
            assert_eq!(view.take_tutorial_control(), None);
            assert_eq!(
                view.details,
                details != 0,
                "release outside cannot change report"
            );
            view.select(Some(utility + 1));
            assert!(!view.details);
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "private original utility widget setup/draw oracle"]
    fn matches_native_utility_style() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_UTILITY_STYLE").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTUSTYL1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        for _ in 0..word(&mut at) {
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let layout = Layout::new(bounds);
            assert_eq!(
                layout.bounds.map(f32::to_bits),
                std::array::from_fn(|_| word(&mut at))
            );
            // Font, horizontal inset, vertical inset, alignment for each column.
            assert_eq!(
                [3, 1, 3, 0, 3, 3, 3, 2],
                std::array::from_fn(|_| word(&mut at))
            );
            for _ in 0..3 {
                let next_row = word(&mut at) as usize;
                assert_eq!(
                    layout.heading(next_row - 1).map(f32::to_bits),
                    std::array::from_fn(|_| word(&mut at))
                );
                assert_eq!(word(&mut at), 0.5f32.to_bits());
                assert_eq!([0xff01762f; 4], std::array::from_fn(|_| word(&mut at)));
            }
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "private original utility report oracle"]
    fn matches_native_utility_report() {
        let path =
            std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_UTILITY_REPORT").unwrap());
        let b = std::fs::read(&path).unwrap();
        assert_eq!(&b[..8], b"MTUTILR1");
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let count = word(&mut at);
        for _ in 0..count {
            let n = word(&mut at) as usize;
            let name = std::str::from_utf8(&b[at..at + n]).unwrap();
            at += n + 32;
            let file = ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
                .into_iter()
                .map(|k| std::path::PathBuf::from(std::env::var(k).unwrap()))
                .find(|p| p.file_name().unwrap() == name)
                .unwrap_or_else(|| path.parent().unwrap().join(name));
            let save = std::fs::read(file).unwrap();
            let report = Report::from_save(&SaveFile::parse(&save).unwrap(), &image).unwrap();
            let utility = word(&mut at) as usize;
            let details = word(&mut at) != 0;
            let owner = word(&mut at) as usize;
            let n = word(&mut at);
            let expected = (0..n).map(|_| word(&mut at)).collect::<Vec<_>>();
            let actual = report
                .amounts(utility, details, owner)
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{name} {utility} {details} {owner}");
        }
        assert_eq!(at, b.len());
    }
    #[test]
    fn tutorial_rights_child_requires_its_enabled_control() {
        let mut view = View::default();
        view.open_property_rights(0, 0);
        assert!(!view.take_rights_request().unwrap().activate);
        view.apply_rights_inspection(
            0,
            Ok(openmt2001::property_rights::Inspection {
                message: Some(1202),
                activate_visible: true,
                open: false,
            }),
        );
        view.property_control_hover = Some(102);
        view.mouse_with_rights_admission(true, false);
        view.property_control_hover = Some(102);
        view.mouse_with_rights_admission(false, false);
        assert!(
            view.take_rights_request().is_none(),
            "disabled original tutorial control 91 accepted local child 102"
        );
        view.property_control_hover = Some(102);
        view.mouse_with_rights_admission(true, true);
        view.property_control_hover = Some(102);
        view.mouse_with_rights_admission(false, true);
        assert_eq!(
            view.take_rights_request(),
            Some(RightsRequest {
                category: 0,
                panel_mode: 0,
                activate: true,
            })
        );
        view.property_control_hover = Some(102);
        view.mouse_with_rights_admission(true, true);
        view.property_control_hover = Some(102);
        view.mouse_with_rights_admission(false, false);
        assert!(
            view.take_rights_request().is_none(),
            "a rights child disabled between press and release must not activate"
        );
    }
}
