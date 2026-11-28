//! 40f550 / 411140 / 411860(1,11) on the real Locator owner.
//! Nested under locator.rs, so projections update its actual private controls.
//! Inputs are one current worker boundary, never Scene/save-derived defaults.
use super::{Locator, OwnedRow, Row};
use std::sync::Arc;
#[path = "strategic_tree.rs"]
pub mod tree;
const NONE: u32 = u32::MAX;
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

pub use openmt2001::strategic_inputs::{Inputs, PollScratch, Writeback};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListRow {
    pub style: u32,
    pub label: u32,
    pub value: Option<u32>,
    pub payload: u32,
}
fn label(style: u32, id: u32, payload: u32) -> ListRow {
    ListRow {
        style,
        label: id,
        value: None,
        payload,
    }
}
fn count(value: u32) -> ListRow {
    ListRow {
        style: 0,
        label: 0,
        value: Some(value),
        payload: NONE,
    }
}
///Successful source allocation paths, not a claim of a Rust allocator header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalAllocationPath {
    Bump,
    ReusedSplit,
    ReusedWhole,
}
///Semantic witness carried by the actual684 list owner. No arbitrary byte,
///Default, Scene selection or absence-of-selection can mint this witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalLiveEnabledBuffer {
    path: NormalAllocationPath,
}
impl NormalLiveEnabledBuffer {
    ///Factory attests this CURRENT enabled+65c allocation is live, successful,
    ///from arena940a38 and published unadjusted by431605. The caller must
    ///invalidate/rebind provenance on release, foreign replacement or any
    ///unrepresented allocator/layout change. This does not inspect memory.
    ///4e63ef (bump) and4e6468 (reuse, before split/whole) zero header+0c;
    ///with alignment4/overhead16, enabled[-1] is that word's high byte.
    pub fn from_live_unadjusted_684_allocation(
        arena: u32,
        alignment: u32,
        overhead: u32,
        path: NormalAllocationPath,
    ) -> Result<Self, String> {
        if arena != 0x940a38 || alignment != 4 || overhead != 16 {
            return Err(
                "C enabled-buffer provenance requires live arena940a38/alignment4/overhead16"
                    .into(),
            );
        }
        Ok(Self { path })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnabledBufferProvenance {
    Unrepresented,
    NormalLive(NormalLiveEnabledBuffer),
}
///Only the final430a70 operation; earlier row rebuilding and later show/event11
///work are not elided. The existing one-selected-row representation is retained.
///Disabled/known-negative skip and all errors leave selection/effects untouched.
fn update_commodity_selection(
    rows: &[ListRow],
    requested: u32,
    selected: &mut Option<u32>,
    mode: crate::compatibility::Mode,
    provenance: EnabledBufferProvenance,
    tab: u32,
    page: u32,
    e: &mut Effects,
) -> Result<(), String> {
    if requested == NONE {
        if mode == crate::compatibility::Mode::Modern {
            return Ok(());
        }
        return match provenance{
            EnabledBufferProvenance::NormalLive(_)=>Ok(()),
            EnabledBufferProvenance::Unrepresented=>Err(
                "C UnsupportedOriginalMemoryRead: selector -1 lacks live normal enabled-buffer provenance".into()),
        };
    }
    //Exactly-1 is special; -2, other negatives, count and higher remain errors.
    let row = rows
        .get(requested as usize)
        .ok_or("C retained commodity row outside rebuilt list")?;
    if row.style != 0 || row.payload == NONE {
        return Ok(());
    }
    //430a70 clears selected flags then writes the requested row, BEFORE event28.
    //This packet's retained Option projection represents that single selection;
    //it does not port arbitrary raw multi-flag storage or the real input owner.
    *selected = Some(requested);
    //4119f2..411a2f source id/tab2/page2 gate; no payload or row fallback.
    if tab == 2 && page == 2 {
        e.commodity = Some(row.payload as i32);
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct State {
    pub input: Arc<Inputs>,
    pub tab: u32,
    pub city_page: u32,
    pub business_page: u32,
    pub player_page: u32,
    pub people_mode: u32,
    pub finance_page: u32,
    pub selected_player: u32,
    pub report_player: u32,
    pub roster_pages: [u32; 6],
    pub field460: u32,
    pub poll_guard: u32,
    pub poll_phase: u32,
    pub poll_capture: u32,
    pub poll_latch: u8,
    pub inhibit: u32,
    pub business_index: u32,
    pub commodity_index: u32,
    pub selected_business_row: Option<u32>,
    pub selected_commodity_row: Option<u32>,
    pub world_business: i32,
    pub world_commodity: i32,
    pub compatibility: crate::compatibility::Mode,
    pub commodity_enabled_provenance: EnabledBufferProvenance,
    pub cache: u32,
    pub owned_index: u32,
    pub marker_enabled: u8,
    pub marker_mode: u32,
    pub report_mode: u32,
    pub colors: [u32; 4],
    pub people_rows: Vec<crate::city_people::Row>,
    pub finance_rows: Vec<crate::player_info::Row>,
    pub poll: PollScratch,
    pub poll_left: Vec<ListRow>,
    pub poll_right: Vec<ListRow>,
    pub poll_label: Option<u32>,
    pub businesses: Vec<ListRow>,
    pub commodities: Vec<ListRow>,
    pub prestige: Vec<ListRow>,
    pub ownership: Vec<ListRow>,
    pub owned: Vec<ListRow>,
    pub tabs_disabled: [u8; 4],
    pub tabs_blink: [u8; 4],
    pub caption: u32,
    pub header: u32,
    pub dirty_writeback: bool,
    pub tree: tree::Tree,
    pub geometry: tree::Geometry,
    pub prestige_selection: i32,
    pub ownership_selection: i32,
    pub owned_selection: i32,
}
///Cells40ce40 does NOT write must come from the actual prior lifecycle owner.
///They are explicit even though a later40f550 overwrites poll_phase before use.
pub struct Prior {
    pub selected_business_row: Option<u32>,
    pub selected_commodity_row: Option<u32>,
    pub world_business: i32,
    pub world_commodity: i32,
    pub roster_pages: [u32; 6],
    pub compatibility: crate::compatibility::Mode,
    pub commodity_enabled_provenance: EnabledBufferProvenance,
    pub tree: tree::Tree,
    pub geometry: tree::Geometry,
    pub poll_phase: u32,
    pub poll_capture: u32,
    pub poll_latch: u8,
    pub selected_player: u32,
    pub report_player: u32,
    pub cache: u32,
    pub owned_index: u32,
    pub prestige_selection: i32,
    pub ownership_selection: i32,
    pub owned_selection: i32,
    pub colors: [u32; 4],
    pub tabs_disabled: [u8; 4],
    pub tabs_blink: [u8; 4],
}
#[derive(Clone, Debug, Default)]
pub struct Effects {
    pub close: bool,
    pub clear_current: bool,
    pub business: Option<i32>,
    pub commodity: Option<i32>,
    pub report_mode: Option<u32>,
}
impl State {
    ///40ce40 reset +410ee0(0)+410bc0(0). Fresh default comes from Source.
    pub fn after_reset(input: Arc<Inputs>, prior: Prior) -> Result<Self, String> {
        let s = Self {
            poll: input.poll.clone(),
            input,
            tab: 0,
            city_page: 0,
            business_page: 0,
            player_page: 0,
            people_mode: 0,
            finance_page: 1,
            selected_player: prior.selected_player,
            report_player: prior.report_player,
            roster_pages: prior.roster_pages,
            field460: NONE,
            poll_guard: NONE,
            poll_phase: prior.poll_phase,
            poll_capture: prior.poll_capture,
            poll_latch: prior.poll_latch,
            inhibit: 0,
            business_index: NONE,
            commodity_index: NONE,
            selected_business_row: prior.selected_business_row,
            selected_commodity_row: prior.selected_commodity_row,
            world_business: prior.world_business,
            world_commodity: prior.world_commodity,
            compatibility: prior.compatibility,
            commodity_enabled_provenance: prior.commodity_enabled_provenance,
            cache: prior.cache,
            owned_index: prior.owned_index,
            marker_enabled: 1,
            marker_mode: 4,
            report_mode: 0,
            colors: prior.colors,
            people_rows: Vec::new(),
            finance_rows: Vec::new(),
            poll_left: Vec::new(),
            poll_right: Vec::new(),
            poll_label: None,
            businesses: Vec::new(),
            commodities: Vec::new(),
            prestige: Vec::new(),
            ownership: Vec::new(),
            owned: Vec::new(),
            tabs_disabled: prior.tabs_disabled,
            tabs_blink: prior.tabs_blink,
            caption: 0x5aa,
            header: 0x5a2,
            dirty_writeback: false,
            tree: prior.tree,
            geometry: prior.geometry,
            prestige_selection: prior.prestige_selection,
            ownership_selection: prior.ownership_selection,
            owned_selection: prior.owned_selection,
        };
        //40ce40 does not rebuild5607d4;40f550 does so later.
        Ok(s)
    }
    pub fn check_suppliers(&self, input: &Inputs, colors: &[u32; 600]) -> Result<(), String> {
        if self.dirty_writeback || *colors != self.input.before_colors {
            return Err("C supplier replacement before actual writeback acknowledgement".into());
        }
        let n = input.roster_slots.len();
        if n > 6 || input.roster_references.len() != n || input.financial.records.len() != n {
            return Err("C supplied roster extents disagree".into());
        }
        let current = input
            .financial
            .current
            .ok_or("C supplied human index absent")?;
        if input.current_reference == 0
            || input.roster_references.get(current) != Some(&input.current_reference)
        {
            return Err("C supplied human reference/index disagree".into());
        }
        self.tree.check()
    }
    pub fn replace_suppliers(&mut self, input: Arc<Inputs>) -> Result<(), String> {
        // Caller checks working colors alongside the actual retained owner.
        if self.dirty_writeback {
            return Err("C supplier replacement before actual writeback acknowledgement".into());
        }
        self.poll = input.poll.clone();
        self.input = input;
        Ok(())
    }
    fn rebuild_roster(&mut self) -> Result<(), String> {
        let i = &self.input;
        if i.roster_slots.len() != i.financial.records.len() {
            return Err("C roster slot payload mismatch".into());
        }
        self.roster_pages[..i.roster_slots.len()].fill(NONE);
        let mut out = 0;
        if let Some(current) = i.financial.current {
            if i.financial
                .records
                .get(current)
                .ok_or("C current slot outside roster")?[0xc]
                == 1
            {
                self.roster_pages[0] = u32::from(i.roster_slots[current]);
                out = 1;
            }
        }
        for (n, p) in i.financial.records.iter().enumerate() {
            if Some(n) != i.financial.current && p[0xc] == 1 {
                self.roster_pages[out] = u32::from(i.roster_slots[n]);
                out += 1;
            }
        }
        Ok(())
    }
    fn rebuild_selectors(&mut self, e: &mut Effects) -> Result<(), String> {
        //40f260 always retains the current human; it does NOT select a business row.
        self.report_player = self.input.current_reference;
        self.businesses.clear();
        self.commodities.clear();
        //4308d0 resets enabled+65c/states+664/markers+668 and chosen+678,
        //NOT selected+658.431ba0/431860 append paths do not clear it either.
        //431330 growth zeros new flags then copies all old-capacity flags.
        //Keep existing selected-row projections independently of requested b38.
        let input = &self.input;
        if input.tables.business_count > 45 || input.tables.commodity_count > 32 {
            return Err("C finite supplied order extent exceeded".into());
        }
        for group in 0..3 {
            if group != 0 {
                self.businesses.push(label(2, 0, NONE));
            }
            self.businesses
                .push(label(1, [0x474, 0x5f6, 0x5f7][group], NONE));
            if input.tables.business_count == 0 {
                continue;
            }
            if group == 0 {
                for kind in [42, 43] {
                    let r = &input.catalog.rows()[kind];
                    self.businesses.push(label(0, word(r, 4), word(r, 0)));
                }
            } else {
                for &kind in input
                    .tables
                    .business_order
                    .iter()
                    .take(input.tables.business_count as usize)
                {
                    let r = input
                        .catalog
                        .rows()
                        .get(kind as usize)
                        .ok_or("C business order outside actual table")?;
                    if r[0x30] == 1 && r[0x5c] == if group == 1 { 9 } else { 18 } {
                        self.businesses.push(label(0, word(r, 4), word(r, 0)));
                    }
                }
            }
        }
        for group in 0..2 {
            if group != 0 {
                self.commodities.push(label(2, 0, NONE));
            }
            self.commodities.push(label(1, [0x5f6, 0x5f7][group], NONE));
            for &kind in input
                .tables
                .commodity_order
                .iter()
                .take(input.tables.commodity_count as usize)
            {
                //Runtime State rows start53be98; native declared view starts53be9c.
                let r = input
                    .commodities
                    .get(kind as usize)
                    .ok_or("C commodity order outside actual table")?;
                if r[0x10] == 1 && input.memberships[group + 1] & (1u32 << (kind & 31)) != 0 {
                    self.commodities.push(label(0, word(r, 4), kind));
                }
            }
        }
        update_commodity_selection(
            &self.commodities,
            self.commodity_index,
            &mut self.selected_commodity_row,
            self.compatibility,
            self.commodity_enabled_provenance,
            self.tab,
            self.business_page,
            e,
        )?;
        Ok(())
    }
    fn recolor(&mut self, colors: &mut [u32; 600]) {
        for n in 0..600 {
            if self.input.building_active[n] != 0 {
                colors[n] = self.input.owner_colors[n];
            }
        }
        self.dirty_writeback = true;
    }
    fn reset_poll_lists(&mut self) {
        self.poll_left.clear();
        self.poll_right.clear();
    }
    fn refresh_label(&mut self) -> Result<(), String> {
        self.poll_label = None;
        if self.field460 == NONE {
            self.poll_label = Some(0x5b9);
            return Ok(());
        }
        let r = self
            .input
            .categories
            .get(self.field460 as usize)
            .ok_or("C poll label category outside actual arena")?;
        if r[4] != 1 || r[0x90] & 1 != 0 {
            return Ok(());
        }
        let title = word(r, 0x8c) as i32;
        self.poll_label = Some(if title > 45 {
            0x5b9
        } else {
            *self
                .input
                .tables
                .property_labels
                .get(usize::try_from(title).map_err(|_| "C negative property label")?)
                .ok_or("C property label outside table")?
        });
        Ok(())
    }
    fn poll_rows(&mut self) {
        if self.poll_guard == NONE {
            return;
        }
        self.poll_phase = 0;
        if self.poll.active != 0 {
            self.poll.stop();
        }
        self.poll.clear();
        self.poll.start(self.poll_guard, &self.input);
        self.dirty_writeback = true;
        if self.poll.active == 0 {
            return;
        }
        self.poll_right.push(label(0, 0x19, NONE));
        self.poll_right
            .push(count(u32::from(self.poll.counters[33])));
        if self.poll.counters[33] == 0 {
            self.poll_left.push(label(0, 0x472, NONE));
            return;
        }
        let mut ids: [usize; 32] = std::array::from_fn(|i| i);
        let mut counts = self.poll.counters[..32].to_vec();
        //4122f0's descending contender traversal is observably different on ties.
        for a in 0..31 {
            for b in (a + 1..32).rev() {
                if counts[a] < counts[b] {
                    counts.swap(a, b);
                    ids.swap(a, b);
                }
            }
        }
        for n in 0..17 {
            if counts[n] == 0 {
                break;
            }
            if n == 0 {
                self.poll_left.push(label(1, 0x1a, NONE));
            }
            self.poll_left
                .push(label(0, word(&self.input.commodities[ids[n]], 8), NONE));
            self.poll_left.push(count(u32::from(counts[n])));
        }
    }
    fn poll_lists(&mut self, colors: &mut [u32; 600]) {
        self.reset_poll_lists();
        if self.poll_guard == NONE {
            return;
        }
        if self.poll_latch == 0 {
            self.poll_capture = NONE;
            if self.poll.active != 0 {
                self.poll_capture = self.poll.category;
            }
            self.poll_latch = 1;
            self.recolor(colors);
        }
        self.poll_rows();
    }
    fn refresh_poll_panel(
        &mut self,
        colors: &mut [u32; 600],
        e: &mut Effects,
    ) -> Result<(), String> {
        if self.poll_guard != NONE {
            match self.poll_phase as i32 {
                0..=3 => {
                    if self.poll.active != 0 {
                        self.poll.stop();
                    }
                    self.poll.clear();
                    self.poll.start(self.poll_guard, &self.input);
                    self.poll_phase = 4;
                    self.dirty_writeback = true;
                }
                4 => {
                    self.poll_guard = NONE;
                    self.field460 = NONE;
                    if self.poll.active != 0 {
                        self.poll.stop();
                    } else {
                        self.colors = self.input.tables.default_colors;
                        self.poll.clear();
                        self.poll_phase = 0;
                        self.poll_capture = NONE;
                    }
                    e.clear_current = true;
                    self.dirty_writeback = true;
                }
                _ => {}
            }
        }
        self.refresh_label()?;
        self.poll_lists(colors);
        Ok(())
    }
    fn rebuild_prestige(&mut self) -> Result<(), String> {
        self.prestige.clear();
        self.prestige_selection = -1;
        let values: Vec<_> = self
            .input
            .categories
            .iter()
            .map(|r| f32::from_bits(word(r, 0x84)))
            .collect();
        if values.iter().any(|v| v.is_nan()) {
            return Err("C prestige NaN outside admitted original sort domain".into());
        }
        let mut order: Vec<_> = (0..52).collect();
        loop {
            let mut changed = false;
            for n in 0..51 {
                if values[order[n]] < values[order[n + 1]] {
                    order.swap(n, n + 1);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut grade = 5;
        let mut heading = false;
        for n in order {
            let r = &self.input.categories[n];
            if r[4] != 1 || r[0x90] & 1 != 0 || word(r, 0x8c) as i32 > 45 {
                continue;
            }
            if values[n] < (grade as f32) {
                self.prestige.push(label(2, 0, NONE));
                grade -= 1;
                heading = false;
            }
            if grade == 0 {
                break;
            }
            if !heading {
                self.prestige.push(ListRow {
                    style: 1,
                    label: 0x2b,
                    value: Some(grade),
                    payload: NONE,
                });
                heading = true;
            }
            //Original uses CATEGORY index, not category.name8c, for this caption.
            self.prestige.push(label(
                0,
                self.input.tables.property_labels[n].wrapping_add(1),
                n as u32,
            ));
        }
        Ok(())
    }
    fn rebuild_ownership(&mut self) {
        self.ownership.clear();
        self.ownership_selection = -1;
        for p in &self.input.financial.records {
            //Retained name belongs to player prefix; renderer resolves this row.
            self.ownership.push(label(1, 0x53327c, NONE));
            let mut found = false;
            for n in 0..52 {
                let r = &self.input.categories[n];
                if r[0x74] != 0 && p[0x174 + n] != 0 {
                    self.ownership.push(label(
                        0,
                        self.input.tables.property_labels[n].wrapping_add(2),
                        word(r, 0),
                    ));
                    found = true;
                }
            }
            if !found {
                self.ownership.push(label(0, 19, NONE));
            }
        }
    }
    fn rebuild_owned(&mut self) {
        self.owned.clear();
        self.owned_selection = -1;
        let mut category = None;
        for r in &self.input.owned {
            if category != Some(r.category) {
                if category.is_some() {
                    self.owned.push(label(2, 0, NONE));
                }
                category = Some(r.category);
                let title = word(&self.input.categories[r.category], 0x8c) as usize;
                self.owned
                    .push(label(1, self.input.tables.property_labels[title], NONE));
            }
            self.owned.push(label(0, r.label, r.building_reference));
        }
        if category.is_some() {
            self.owned.push(label(2, 0, NONE));
        } else {
            self.owned.push(label(0, 0x470, NONE));
        }
    }
    ///Preflight copy of40f550. Actual cross-owner dispatch uses the remainder
    ///below so the global-cache and retained-C stores retain their exact order.
    pub fn show(&mut self, colors: &mut [u32; 600]) -> Result<Effects, String> {
        self.cache = NONE;
        self.show_after_retention_clear(colors)
    }
    ///Caller performs the preceding global-cache then retained-C stores in
    ///their actual cross-owner order before entering this remainder.
    pub fn show_after_retention_clear(
        &mut self,
        colors: &mut [u32; 600],
    ) -> Result<Effects, String> {
        let mut e = Effects::default();
        self.rebuild_roster()?;
        if self.input.human_enabled == 0 {
            e.close = true;
            return Ok(e);
        }
        self.rebuild_selectors(&mut e)?;
        self.poll_phase = 0;
        self.refresh_poll_panel(colors, &mut e)?;
        self.poll_lists(colors);
        self.recolor(colors);
        self.rebuild_prestige()?;
        self.rebuild_ownership();
        if self.tab == 3 {
            e.business = Some(42);
        }
        Ok(e)
    }
    fn active_page(&mut self, colors: &mut [u32; 600]) -> Result<(), String> {
        self.recolor(colors);
        if self.inhibit == 0 {
            self.poll_lists(colors);
        }
        match (self.tab, self.city_page, self.business_page) {
            (0, 0, _) => {
                self.people_rows = self.input.people_rows.clone();
            }
            (0, 1, _) => self.rebuild_prestige()?,
            (0, 3, _) => self.rebuild_ownership(),
            (1, _, _) => {
                self.rebuild_finance()?;
            }
            (2, _, 0) => self.rebuild_owned(),
            _ => {}
        }
        Ok(())
    }
    fn rebuild_finance(&mut self) -> Result<(), String> {
        let reference = *self
            .input
            .roster_table
            .get(self.selected_player as usize)
            .ok_or("C selected_b60 leaves actual six-slot roster table")?;
        //412810's actual NULL table entry falls back to current86cfc0.
        self.report_player = if reference == 0 {
            self.input.current_reference
        } else {
            reference
        };
        let p = self
            .input
            .roster_references
            .iter()
            .position(|&r| r == self.report_player)
            .ok_or("C finance receiver does not resolve current roster object")?;
        self.finance_rows =
            crate::player_info::report(&self.input.financial.records[p], self.finance_page == 1)?;
        Ok(())
    }
    ///411860 finite1/11 body. Geometry values are actual current4342c0 calls,
    ///not visibility/scene-hit/no-selection guesses. No sound/input12 is accepted.
    pub fn message(
        &mut self,
        event: u32,
        inside_panel: bool,
        inside_owned: bool,
        disabled1d: u8,
        colors: &mut [u32; 600],
    ) -> Result<Effects, String> {
        if !matches!(event, 1 | 11) {
            return Err("C finite callback accepts only actual1/11".into());
        }
        if event == 11 {
            self.active_page(colors)?;
        }
        let e = Effects::default();
        let tab = self.tab;
        if tab == 1 && self.input.scan_enable == 0 {
            return Err("C411860 tab1/scan0 reaches410ee0(0): actual410260/4103c0 child-control reset and page layout are unjoined".into());
        }
        let marker = *self
            .roster_pages
            .get(self.player_page as usize)
            .ok_or("C page390 outside actual player-page storage")?;
        if marker != self.selected_player && tab == 1 && self.input.scan_enable > 0 {
            return Err("C411860 changed player page reaches40f1d0/410ee0(1)/40ff70: actual portrait/control reset family is unjoined".into());
        }
        if inside_panel {
            self.cache = NONE;
            if self.tab == 0 && disabled1d == 0 {
                //List selected indices are updated from actual list input owner,
                //never inferred from mouse-nearest Scene geometry.
                let list = match self.city_page {
                    1 => Some(&self.prestige),
                    3 => Some(&self.ownership),
                    _ => None,
                };
                if let Some(list) = list {
                    let key = list
                        .get(self.selected_city_row() as usize)
                        .map_or(NONE, |r| r.payload);
                    if key != NONE {
                        self.cache = key;
                    }
                }
            }
        }
        self.owned_index = NONE;
        if self.tab == 2 && self.business_page == 0 && inside_owned {
            let index = self.selected_owned_row();
            if index >= 0 {
                self.owned_index = self.owned.get(index as usize).map_or(NONE, |r| r.payload);
            }
        }
        Ok(e)
    }
    //The actual selected indices are maintained with each list, independently
    //of reset/rebuild row counts. Accessors below use State's explicit cells.
    fn selected_city_row(&self) -> i32 {
        if self.city_page == 1 {
            self.prestige_selection
        } else {
            self.ownership_selection
        }
    }
    fn selected_owned_row(&self) -> i32 {
        self.owned_selection
    }
    pub fn take_writeback(&mut self, colors: [u32; 600]) -> Option<Writeback> {
        if !self.dirty_writeback && colors == self.input.before_colors {
            return None;
        }
        self.dirty_writeback = false;
        let write = Writeback {
            current_reference: self.input.current_reference,
            before_poll: self.input.poll.clone(),
            poll: self.poll.clone(),
            before_colors: self.input.before_colors,
            colors,
            active: self.input.building_active,
        };
        // This boundary's suspended Host is now the sole commit consumer. A
        // commit/finalization failure latches the endpoint; no retry can use
        // this before image as though a failed world commit had succeeded.
        let input = Arc::make_mut(&mut self.input);
        input.poll = self.poll.clone();
        input.before_colors = colors;
        Some(write)
    }
}
impl Locator {
    ///40fa00 draws the reached C body without the legacy modern expand bit.
    ///The caller still gates the whole body with actual43c3a0 requested/motion.
    pub(crate) fn body_open(&self) -> bool {
        self.strategic_state.is_some() || self.open
    }
    pub(crate) fn world_selected(&self) -> Result<Option<u32>, String> {
        let Some(s) = &self.strategic_state else {
            return Ok(self.selected);
        };
        let selected = if self.commodity {
            s.world_commodity
        } else {
            s.world_business
        };
        if selected == -1 {
            return Ok(None);
        }
        Ok(Some(u32::try_from(selected).map_err(
            |_| "C negative selected world key outside source domain",
        )?))
    }
    pub fn install_strategic(&mut self, state: State) -> Result<(), String> {
        if self.strategic_state.is_some() {
            return Err("C lifecycle replacement forbidden".into());
        }
        self.strategic_state = Some(state);
        self.publish_strategic()
    }
    pub fn strategic(&self) -> Result<&State, String> {
        self.strategic_state
            .as_ref()
            .ok_or_else(|| "C actual reset/load producer absent".into())
    }
    pub fn strategic_mut(&mut self) -> Result<&mut State, String> {
        self.strategic_state
            .as_mut()
            .ok_or_else(|| "C actual reset/load producer absent".into())
    }
    pub fn replace_strategic_suppliers(&mut self, input: Arc<Inputs>) -> Result<(), String> {
        self.strategic_mut()?.replace_suppliers(input)
    }
    pub fn take_strategic_writeback(
        &mut self,
        colors: [u32; 600],
    ) -> Result<Option<Writeback>, String> {
        Ok(self.strategic_mut()?.take_writeback(colors))
    }
    pub fn has_strategic(&self) -> bool {
        self.strategic_state.is_some()
    }
    /// Validate the destination shell's actual report attachments and every
    /// fallible projection input BEFORE consuming the retained owner.
    fn check_strategic_projection(&self, s: &State) -> Result<(), String> {
        if s.input.roster_slots.len() > 6
            || s.input.financial.records.len() != s.input.roster_slots.len()
        {
            return Err("C projection roster extent mismatch".into());
        }
        for slot in s
            .roster_pages
            .iter()
            .take(s.input.roster_slots.len())
            .filter(|&&slot| slot != NONE)
        {
            if !s.input.roster_slots.iter().any(|&v| u32::from(v) == *slot) {
                return Err("C roster-page slot unresolved".into());
            }
        }
        if s.tab == 0 && s.city_page == 0 && self.city_people.is_none() {
            return Err("C People report requires actual live attached renderer owner".into());
        }
        for r in &s.input.categories {
            let owner = r[5] as i8;
            if owner != 0 {
                let at = usize::try_from(i32::from(owner) - 1)
                    .map_err(|_| "C negative category owner")?;
                let p = s
                    .input
                    .financial
                    .records
                    .get(at)
                    .ok_or("C category owner leaves current roster")?;
                if word(p, 0x14c) >= 10 {
                    return Err("C ownership character leaves10-row table".into());
                }
            }
        }
        if let Some(current) = s.input.financial.current {
            let p = s
                .input
                .financial
                .records
                .get(current)
                .ok_or("C projection human index outside roster")?;
            let slot = *s
                .input
                .roster_slots
                .get(current)
                .ok_or("C projection human slot absent")?;
            if slot == u8::MAX {
                return Err("C projection human slot cannot form one-based owner".into());
            }
            let name = &p[0x154..];
            let end = name
                .iter()
                .position(|&b| b == 0)
                .ok_or("C player name not terminated")?;
            if name[..end].iter().any(|b| !(32..=126).contains(b)) {
                return Err("C player name needs actual glyph mapping".into());
            }
        }
        Ok(())
    }
    pub(crate) fn move_strategic_from(&mut self, old: &mut Self) -> Result<(), String> {
        if self.strategic_state.is_some() {
            return Err("Scene refresh constructed replacement C lifecycle".into());
        }
        let Some(s) = old.strategic_state.as_ref() else {
            return Ok(());
        };
        self.check_strategic_projection(s)?;
        self.strategic_state = old.strategic_state.take();
        if let Err(error) = self.publish_strategic() {
            // Even an unexpected projection error leaves the original owner
            // available for retry. Never first-reset or clone a replacement.
            old.strategic_state = self.strategic_state.take();
            return Err(error);
        }
        Ok(())
    }
    pub(crate) fn refresh_strategic_from(
        &mut self,
        old: &mut Self,
        scene: &openmt2001::render_scene::Scene,
    ) -> Result<(), String> {
        self.refresh_strategic_buildings_from(old, &scene.inspection_buildings)
    }
    fn refresh_strategic_buildings_from(
        &mut self,
        old: &mut Self,
        buildings: &[openmt2001::render_scene::InspectionBuilding],
    ) -> Result<(), String> {
        if self.strategic_state.is_some() {
            return Err("Scene refresh constructed replacement C lifecycle".into());
        }
        let Some(s) = old.strategic_state.as_ref() else {
            return Ok(());
        };
        self.check_strategic_projection(s)?;
        let rows = Self::strategic_owned_rows(s, buildings)?;
        self.move_strategic_from(old)?;
        self.owned_rows = rows;
        self.owned_scroll = 0;
        Ok(())
    }
    /// Prepare every report and stable-key projection using a borrowed actual
    /// State. The destination owns no C lifecycle until the caller commits.
    pub fn prepare_strategic_refresh(
        &mut self,
        old: &Self,
        buildings: &[openmt2001::render_scene::InspectionBuilding],
    ) -> Result<(), String> {
        if self.strategic_state.is_some() {
            return Err("Scene refresh constructed replacement C lifecycle".into());
        }
        if let Some(s) = old.strategic_state.as_ref() {
            self.check_strategic_projection(s)?;
            let rows = Self::strategic_owned_rows(s, buildings)?;
            self.project_strategic_state(s)?;
            self.owned_rows = rows;
            self.owned_scroll = 0;
        }
        Ok(())
    }
    /// Only after the caller's full preparation succeeds. No projection or
    /// fallible decoding remains; move the SAME tree/Arc/provenance owner.
    pub fn commit_strategic_refresh(&mut self, old: &mut Self) {
        self.strategic_state = old.strategic_state.take();
    }
    ///Read projection only. Actual report, mode, row and poll authority remains
    ///State. Modern GPU batches are invalidated, never reconstructed as state.
    pub fn publish_strategic(&mut self) -> Result<(), String> {
        let s = self
            .strategic_state
            .take()
            .ok_or("C projection without actual lifecycle")?;
        let result = self.project_strategic_state(&s);
        self.strategic_state = Some(s);
        result
    }
    fn project_strategic_state(&mut self, s: &State) -> Result<(), String> {
        self.check_strategic_projection(s)?;
        let finance_changed = !self.player_info
            || self.player.selected != s.player_page as usize
            || self.player.today != (s.finance_page == 1);
        self.city_info = s.tab == 0;
        self.player_info = s.tab == 1;
        self.poll_info = s.tab == 3;
        self.people_mode = s.people_mode as usize;
        self.prestige_mode = s.city_page == 1;
        self.rights_mode = s.city_page == 2;
        self.ownership_mode = s.city_page == 3;
        self.my_business = s.business_page == 0;
        self.commodity = s.business_page == 2;
        let to_rows = |rows: &[ListRow]| {
            rows.iter()
                .map(|r| Row {
                    label: if r.style == 2 { NONE } else { r.label },
                    kind: r.payload,
                })
                .collect::<Vec<_>>()
        };
        let (business, commodity) = (to_rows(&s.businesses), to_rows(&s.commodities));
        (self.rows, self.other_rows) = if self.commodity {
            (commodity, business)
        } else {
            (business, commodity)
        };
        let selected = |rows: &[ListRow], index: u32| {
            rows.get(index as usize)
                .filter(|r| r.style == 0)
                .map(|r| r.payload)
                .filter(|&id| id != NONE)
        };
        let (business_selected, commodity_selected) = (
            s.selected_business_row
                .and_then(|i| selected(&s.businesses, i)),
            s.selected_commodity_row
                .and_then(|i| selected(&s.commodities, i)),
        );
        (self.selected, self.other_selected) = if self.commodity {
            (commodity_selected, business_selected)
        } else {
            (business_selected, commodity_selected)
        };
        self.catalog = Some(s.input.catalog.clone());
        self.saved_panels = true;
        self.owned_eligible = std::array::from_fn(|i| {
            let r = &s.input.categories[i];
            r[4] == 1 && r[0x90] & 1 == 0 && (word(r, 0x8c) as i32) <= 45
        });
        self.player.records = s.input.financial.records.clone();
        self.player.current = s.input.financial.current;
        self.player.current_roster_slot = s.input.financial.current_roster_slot;
        self.player.players = s
            .roster_pages
            .iter()
            .take(s.input.roster_slots.len())
            .copied()
            .filter(|&slot| slot != NONE)
            .map(|slot| {
                s.input
                    .roster_slots
                    .iter()
                    .position(|&v| u32::from(v) == slot)
                    .ok_or("C roster-page slot unresolved")
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.player.selected = s.player_page as usize;
        self.player.today = s.finance_page == 1;
        if s.tab == 1 {
            self.player.rows = s.finance_rows.clone();
            self.player.scroll = if finance_changed {
                0
            } else {
                self.player.scroll.min(
                    self.player
                        .line_count()
                        .saturating_sub(self.player.visible_rows),
                )
            };
        }
        if s.tab == 0 && s.city_page == 0 {
            self.city_people
                .as_mut()
                .ok_or("C People report requires actual live attached renderer owner")?
                .rows = s.people_rows.clone();
        }
        self.poll_category = (s.field460 != NONE).then_some(s.field460 as usize);
        let mut polls = std::array::from_fn(|i| openmt2001::consumer_poll::Poll {
            counts: s.input.poll_queries[i][..32].try_into().unwrap(),
            residents: s.input.poll_queries[i][33],
        });
        if s.poll.active != 0 {
            if let Some(poll) = polls.get_mut(s.poll.category as usize) {
                poll.counts.copy_from_slice(&s.poll.counters[..32]);
                poll.residents = s.poll.counters[33];
            }
        }
        self.polls = Some(polls);
        self.prestige = Some(crate::city_prestige::Prestige {
            rows: s
                .prestige
                .iter()
                .map(|r| {
                    if r.style == 2 {
                        crate::city_prestige::Row::Space
                    } else if let Some(grade) = r.value {
                        crate::city_prestige::Row::Grade(grade as i32)
                    } else {
                        crate::city_prestige::Row::Block(r.payload as usize)
                    }
                })
                .collect(),
            values: std::array::from_fn(|i| f32::from_bits(word(&s.input.categories[i], 0x84))),
            overlay_categories: (0..52)
                .filter(|&i| {
                    let r = &s.input.categories[i];
                    ((r[4] == 1 && r[0x90] & 1 == 0) || i == 41) && (word(r, 0x8c) as i32) <= 45
                })
                .collect(),
        });
        let mut badges = [crate::ownership_badges::Badge {
            character: None,
            remaining: 0,
        }; 52];
        for (i, badge) in badges.iter_mut().enumerate() {
            let r = &s.input.categories[i];
            badge.remaining = word(r, 0x7c) as i32;
            let owner = r[5] as i8;
            if owner != 0 {
                let p = s
                    .input
                    .financial
                    .records
                    .get(
                        usize::try_from(i32::from(owner) - 1)
                            .map_err(|_| "C negative category owner")?,
                    )
                    .ok_or("C category owner leaves current roster")?;
                let character = word(p, 0x14c) as usize;
                if character >= 10 {
                    return Err("C ownership character leaves10-row table".into());
                }
                badge.character = Some(character);
            }
        }
        self.ownership = Some(crate::city_ownership::Ownership {
            badges,
            groups: s
                .input
                .financial
                .records
                .iter()
                .map(|p| {
                    (0..52)
                        .filter(|&i| s.input.categories[i][0x74] != 0 && p[0x174 + i] != 0)
                        .collect()
                })
                .collect(),
        });
        if let Some(current) = s.input.financial.current {
            self.rights = Some(crate::city_rights::Rights {
                categories: std::array::from_fn(|i| {
                    let r = &s.input.categories[i];
                    crate::city_rights::Category {
                        mode: word(r, 0x78) as i32,
                        owner: r[5],
                        remaining: word(r, 0x80) as i32,
                    }
                }),
                owner_number: s.input.roster_slots[current] + 1,
            });
        }
        self.dirty = true;
        self.prepared = None;
        Ok(())
    }
    ///Stable building keys are resolved only for rendering/current scene input.
    ///4138b0 supplier itself never takes a display-vector index as authority.
    pub fn project_strategic_owned(
        &mut self,
        scene: &openmt2001::render_scene::Scene,
    ) -> Result<(), String> {
        let rows = Self::strategic_owned_rows(self.strategic()?, &scene.inspection_buildings)?;
        self.owned_rows = rows;
        self.owned_scroll = 0;
        Ok(())
    }
    fn strategic_owned_rows(
        s: &State,
        buildings: &[openmt2001::render_scene::InspectionBuilding],
    ) -> Result<Vec<OwnedRow>, String> {
        let mut rows = Vec::new();
        let mut category = None;
        for r in &s.input.owned {
            if category != Some(r.category) {
                if category.is_some() {
                    rows.push(OwnedRow {
                        label: NONE,
                        building: None,
                    });
                }
                category = Some(r.category);
                rows.push(OwnedRow {
                    label: 0x8000_0000 | r.category as u32,
                    building: None,
                });
            }
            let index = buildings
                .iter()
                .position(|b| b.runtime_slot.or(b.saved_slot) == Some(r.building_slot))
                .ok_or("C stable owned building absent from refreshed display projection")?;
            rows.push(OwnedRow {
                label: r.label,
                building: Some(index),
            });
        }
        if category.is_some() {
            rows.push(OwnedRow {
                label: NONE,
                building: None,
            });
        } else {
            rows.push(OwnedRow {
                label: 0x470,
                building: None,
            });
        }
        Ok(rows)
    }
}
impl Prior {
    ///Once at the first process UI birth/reset, never on scene refresh/load
    ///resume.4398c0 constructs the static58e8d0 C object in PE.data zero-fill;
    ///439980 leaves these cells untouched and40ce40 only writes the named
    ///reset fields. Global535638/545584/5607d4 initial words are pinned below.
    ///The actual child tree/layout is still supplied by its real factory.
    pub fn first_process_reset(
        tree: tree::Tree,
        geometry: tree::Geometry,
        compatibility: crate::compatibility::Mode,
        commodity_enabled_provenance: EnabledBufferProvenance,
    ) -> Self {
        Self {
            tree,
            geometry,
            compatibility,
            commodity_enabled_provenance,
            selected_business_row: None,
            selected_commodity_row: None,
            world_business: -1,
            world_commodity: -1,
            roster_pages: [0; 6],
            poll_phase: 0,
            poll_capture: 0,
            poll_latch: 0,
            selected_player: 0,
            report_player: 0,
            cache: NONE,
            owned_index: NONE,
            prestige_selection: -1,
            ownership_selection: -1,
            owned_selection: -1,
            colors: [0; 4],
            tabs_disabled: [0; 4],
            tabs_blink: [0; 4],
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod selector_boundary_tests {
    use super::*;
    use crate::compatibility::Mode;
    fn normal(path: NormalAllocationPath) -> EnabledBufferProvenance {
        EnabledBufferProvenance::NormalLive(
            NormalLiveEnabledBuffer::from_live_unadjusted_684_allocation(0x940a38, 4, 16, path)
                .unwrap(),
        )
    }
    fn existing_effects() -> Effects {
        Effects {
            close: true,
            clear_current: false,
            business: Some(42),
            commodity: Some(19),
            report_mode: Some(4),
        }
    }
    fn unchanged(e: &Effects) {
        assert!(e.close);
        assert!(!e.clear_current);
        assert_eq!(e.business, Some(42));
        assert_eq!(e.commodity, Some(19));
        assert_eq!(e.report_mode, Some(4));
    }
    #[test]
    fn source_allocator_header_fixture_and_witness_paths() {
        //Inert byte layout only. Free-node word starts nonzero in reused
        //fixtures; the recovered successful allocation store zeros all4 bytes.
        //No native allocator or PE code is executed by this fixture.
        let alignment = 4u32;
        let overhead = 16 + ((alignment - (16 & (alignment - 1))) & (alignment - 1));
        assert_eq!(overhead, 16);
        for path in [
            NormalAllocationPath::Bump,
            NormalAllocationPath::ReusedSplit,
            NormalAllocationPath::ReusedWhole,
        ] {
            let mut header = [0xa5u8; 16];
            header[12..16].copy_from_slice(&0u32.to_le_bytes()); //4e63ef/4e6468
            let preceding = overhead as usize - 1;
            assert_eq!(preceding, 15);
            assert_eq!(header[preceding], 0);
            let provenance = normal(path);
            for mode in [Mode::Modern, Mode::StrictV14] {
                let mut selected = Some(7);
                let mut e = existing_effects();
                update_commodity_selection(
                    &[],
                    NONE,
                    &mut selected,
                    mode,
                    provenance,
                    2,
                    2,
                    &mut e,
                )
                .unwrap();
                assert_eq!(selected, Some(7));
                unchanged(&e);
            }
        }
        for (arena, alignment, overhead) in [(0, 4, 16), (0x940a38, 8, 16), (0x940a38, 4, 32)] {
            assert!(
                NormalLiveEnabledBuffer::from_live_unadjusted_684_allocation(
                    arena,
                    alignment,
                    overhead,
                    NormalAllocationPath::Bump
                )
                .is_err()
            );
        }
    }
    #[test]
    fn exact_negative_skip_or_checked_unsupported_changes_nothing() {
        for mode in [Mode::Modern, Mode::StrictV14] {
            let mut selected = Some(3);
            let mut e = existing_effects();
            let result = update_commodity_selection(
                &[],
                NONE,
                &mut selected,
                mode,
                EnabledBufferProvenance::Unrepresented,
                2,
                2,
                &mut e,
            );
            if mode == Mode::Modern {
                result.unwrap();
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .contains("UnsupportedOriginalMemoryRead")
                );
            }
            assert_eq!(selected, Some(3));
            unchanged(&e);
        }
    }
    #[test]
    fn valid_enabled_disabled_and_invalid_rows_preserve_policy_and_gating() {
        let rows = [label(0, 10, 7), label(1, 11, NONE), label(0, 12, 31)];
        for mode in [Mode::Modern, Mode::StrictV14] {
            //Valid indices must not require irrelevant-1 provenance.
            for (row, payload) in [(0, 7), (2, 31)] {
                for (tab, page) in [(2, 2), (2, 0), (0, 2), (3, 2)] {
                    let mut selected = Some(1);
                    let mut e = existing_effects();
                    update_commodity_selection(
                        &rows,
                        row,
                        &mut selected,
                        mode,
                        EnabledBufferProvenance::Unrepresented,
                        tab,
                        page,
                        &mut e,
                    )
                    .unwrap();
                    assert_eq!(selected, Some(row));
                    assert_eq!(
                        e.commodity,
                        Some(if (tab, page) == (2, 2) { payload } else { 19 })
                    );
                    assert_eq!(e.business, Some(42));
                    assert_eq!(e.report_mode, Some(4));
                    assert!(e.close);
                    assert!(!e.clear_current);
                }
            }
            let mut selected = Some(2);
            let mut e = existing_effects();
            update_commodity_selection(
                &rows,
                1,
                &mut selected,
                mode,
                EnabledBufferProvenance::Unrepresented,
                2,
                2,
                &mut e,
            )
            .unwrap();
            assert_eq!(selected, Some(2));
            unchanged(&e);
            for request in [3, 4, 0xffff_fffe, 0x8000_0000] {
                assert!(
                    update_commodity_selection(
                        &rows,
                        request,
                        &mut selected,
                        mode,
                        normal(NormalAllocationPath::ReusedWhole),
                        2,
                        2,
                        &mut e
                    )
                    .is_err()
                );
                assert_eq!(selected, Some(2));
                unchanged(&e);
            }
            assert!(
                update_commodity_selection(
                    &[],
                    0,
                    &mut selected,
                    mode,
                    normal(NormalAllocationPath::Bump),
                    2,
                    2,
                    &mut e
                )
                .is_err()
            );
            assert_eq!(selected, Some(2));
            unchanged(&e);
        }
    }
    pub fn state_fixture(mode: Mode, provenance: EnabledBufferProvenance) -> State {
        let mut p = [0u8; 440];
        p[0xc] = 1;
        let tables = Arc::new(openmt2001::strategic_inputs::Tables {
            business_order: [0; 45],
            commodity_order: [0; 32],
            business_count: 0,
            commodity_count: 1,
            property_labels: [100; 52],
            default_colors: [0; 4],
            owner_palette: [0; 10],
        });
        let mut commodities = [[0u8; 28]; 32];
        commodities[0][0x10] = 1;
        commodities[0][4..8].copy_from_slice(&55u32.to_le_bytes());
        let mut active = [0; 600];
        active[0] = 1;
        let mut owners = [0; 600];
        owners[0] = 0xff12_3456;
        let input = Arc::new(Inputs {
            tables,
            financial: crate::player_info::Snapshot {
                records: vec![p],
                current: Some(0),
                current_roster_slot: Some(0),
            },
            current_reference: 0x1234,
            human_enabled: 1,
            roster_slots: vec![0],
            roster_references: vec![0x1234],
            roster_table: [0; 6],
            scan_enable: 1,
            poll: PollScratch {
                active: 0,
                collecting: 0,
                category: 0,
                counters: [0; 34],
            },
            poll_queries: [[0; 34]; 52],
            catalog: openmt2001::business_pool::BusinessCatalog::from_rows([[0; 0x74]; 45]),
            commodities,
            memberships: [0, 1, 0, 0, 0],
            categories: [[0; 380]; 52],
            building_active: active,
            before_colors: [0; 600],
            owner_colors: owners,
            people_rows: Vec::new(),
            owned: Vec::new(),
        });
        let tree = tree::Tree::installed(
            Some(5),
            std::collections::BTreeMap::from([(
                5,
                tree::Node {
                    shown: 1,
                    first: None,
                    next: None,
                    handler: tree::Handler::Button {
                        pulse: 3,
                        previous: 9,
                    },
                },
            )]),
        )
        .unwrap();
        let geometry = tree::Geometry {
            open: [-1., -1., 1., 1.],
            closed: [-1., -1., 1., 1.],
            owned: [-1., -1., 1., 1.],
            pointer: [0, 0],
            pointer_blocked: 0,
        };
        let mut prior = Prior::first_process_reset(tree, geometry, mode, provenance);
        prior.selected_business_row = Some(2);
        prior.selected_commodity_row = Some(1);
        prior.world_business = 42;
        prior.world_commodity = 19;
        State::after_reset(input, prior).unwrap()
    }
    #[test]
    fn reset_minus_one_continues_show_and_retains_selection_across_rebuild() {
        for (mode, provenance) in [
            (Mode::Modern, EnabledBufferProvenance::Unrepresented),
            (Mode::StrictV14, normal(NormalAllocationPath::Bump)),
            (Mode::StrictV14, normal(NormalAllocationPath::ReusedSplit)),
            (Mode::StrictV14, normal(NormalAllocationPath::ReusedWhole)),
        ] {
            let mut s = state_fixture(mode, provenance);
            let mut colors = [0; 600];
            for _ in 0..2 {
                s.poll_phase = 9;
                s.poll_label = None;
                let e = s.show(&mut colors).unwrap();
                assert_eq!(s.commodity_index, NONE);
                assert_eq!(s.business_index, NONE);
                assert_eq!(s.selected_business_row, Some(2));
                assert_eq!(s.selected_commodity_row, Some(1));
                assert_eq!(s.world_business, 42);
                assert_eq!(s.world_commodity, 19);
                assert_eq!(s.report_player, 0x1234);
                assert_eq!(s.commodities.len(), 4);
                assert_eq!(s.poll_phase, 0);
                assert_eq!(s.poll_label, Some(0x5b9));
                assert_eq!(colors[0], 0xff12_3456);
                assert!(s.dirty_writeback);
                assert!(e.commodity.is_none());
                assert!(e.business.is_none());
                assert!(!e.close);
                assert!(!e.clear_current);
                assert_eq!(s.ownership.len(), 2); //post-selector report rebuild ran
            }
        }
    }
    #[test]
    fn human_disabled_show_returns_before_the_selector_contract() {
        let mut s = state_fixture(Mode::StrictV14, EnabledBufferProvenance::Unrepresented);
        Arc::make_mut(&mut s.input).human_enabled = 0;
        s.poll_phase = 9;
        let mut colors = [0; 600];
        let e = s.show(&mut colors).unwrap();
        assert!(e.close);
        assert!(e.commodity.is_none());
        assert!(s.commodities.is_empty());
        assert_eq!(s.poll_phase, 9);
        assert_eq!(s.selected_commodity_row, Some(1));
        assert_eq!(colors, [0; 600]);
    }
    #[test]
    fn strict_unknown_stops_final_operation_and_valid_selection_still_notifies() {
        let mut s = state_fixture(Mode::StrictV14, EnabledBufferProvenance::Unrepresented);
        let mut colors = [0; 600];
        assert!(
            s.show(&mut colors)
                .unwrap_err()
                .contains("UnsupportedOriginalMemoryRead")
        );
        //Earlier row rebuilding really ran; no operation-wide rollback claim.
        assert_eq!(s.commodities.len(), 4);
        assert_eq!(s.selected_commodity_row, Some(1));
        assert_eq!(s.world_commodity, 19);
        assert_eq!(colors, [0; 600]);
        s.tab = 2;
        s.business_page = 2;
        s.commodity_index = 1;
        let e = s.show(&mut colors).unwrap();
        assert_eq!(e.commodity, Some(0)); //actual row1 payload0, not a default
        assert_eq!(s.selected_commodity_row, Some(1));
        //A disabled heading skips only final select; an existing choice remains.
        s.commodity_index = 0;
        assert!(s.show(&mut colors).unwrap().commodity.is_none());
        assert_eq!(s.selected_commodity_row, Some(1));
    }
}

#[cfg(test)]
mod refresh_owner_tests {
    use super::*;
    #[test]
    fn actual_refresh_rejects_destination_before_consumption_and_retry_moves_same_tree_and_provenance()
     {
        let witness = NormalLiveEnabledBuffer::from_live_unadjusted_684_allocation(
            0x940a38,
            4,
            16,
            NormalAllocationPath::ReusedWhole,
        )
        .unwrap();
        let mut state = selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::StrictV14,
            EnabledBufferProvenance::NormalLive(witness),
        );
        state.tab = 2;
        state.cache = 0x1234;
        state.poll.counters[3] = 17;
        state.dirty_writeback = true;
        let mut old = crate::locator::refresh_ui_tests::fresh();
        old.install_strategic(state).unwrap();
        let input = old.strategic().unwrap().input.clone();
        let tree_node = old.strategic().unwrap().tree.nodes.get(&5).unwrap() as *const tree::Node;
        let mut next = crate::locator::refresh_ui_tests::fresh();
        // Destination already owns a lifecycle: fail without consuming either.
        let mut replacement = selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::Modern,
            EnabledBufferProvenance::Unrepresented,
        );
        replacement.tab = 2;
        next.install_strategic(replacement).unwrap();
        assert!(
            next.refresh_strategic_buildings_from(&mut old, &[])
                .is_err()
        );
        assert!(old.has_strategic() && next.has_strategic());
        next.strategic_state = None;
        // Missing current owned-building projection must also preserve old.
        Arc::make_mut(&mut old.strategic_mut().unwrap().input)
            .owned
            .push(openmt2001::strategic_inputs::Owned {
                category: 3,
                label: 4,
                building_slot: 8,
                building_reference: 0x591318 + 8 * 916,
            });
        assert!(
            next.refresh_strategic_buildings_from(&mut old, &[])
                .is_err()
        );
        assert!(old.has_strategic() && !next.has_strategic());
        Arc::make_mut(&mut old.strategic_mut().unwrap().input)
            .owned
            .clear();
        let current_input = old.strategic().unwrap().input.clone();
        next.refresh_strategic_buildings_from(&mut old, &[])
            .unwrap();
        assert!(!old.has_strategic());
        let s = next.strategic().unwrap();
        assert_eq!(
            s.tree.nodes.get(&5).unwrap() as *const tree::Node,
            tree_node
        );
        assert!(Arc::ptr_eq(&current_input, &s.input));
        assert_eq!(s.cache, 0x1234);
        assert_eq!(
            (s.selected_business_row, s.selected_commodity_row),
            (Some(2), Some(1))
        );
        assert_eq!(s.poll.counters[3], 17);
        assert!(s.dirty_writeback);
        assert_eq!(
            s.commodity_enabled_provenance,
            EnabledBufferProvenance::NormalLive(witness)
        );
        assert_eq!(s.world_business, 42);
        assert_eq!(s.world_commodity, 19);
        // The next protected cut still has the same pending poll effects.
        let write = next
            .take_strategic_writeback(current_input.before_colors)
            .unwrap()
            .unwrap();
        assert_eq!(write.poll.counters[3], 17);
        assert_eq!(write.before_poll, input.poll);
        let refreshed = next.strategic().unwrap().input.clone();
        next.strategic()
            .unwrap()
            .check_suppliers(&refreshed, &refreshed.before_colors)
            .unwrap();
    }
    #[test]
    fn missing_destination_report_rejects_without_losing_old_state() {
        let mut s = selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::Modern,
            EnabledBufferProvenance::Unrepresented,
        );
        s.tab = 2;
        let mut old = crate::locator::refresh_ui_tests::fresh();
        old.install_strategic(s).unwrap();
        old.strategic_mut().unwrap().tab = 0;
        old.strategic_mut().unwrap().city_page = 0;
        let mut next = crate::locator::refresh_ui_tests::fresh();
        assert!(next.move_strategic_from(&mut old).is_err());
        assert!(old.has_strategic() && !next.has_strategic());
        next.attach_people(crate::city_people::Report {
            initialized: true,
            markers: vec![],
            rows: vec![],
        });
        next.move_strategic_from(&mut old).unwrap();
        assert!(!old.has_strategic() && next.has_strategic());
    }
}
