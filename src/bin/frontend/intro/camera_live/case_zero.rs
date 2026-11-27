//! Concrete A child0; all field mutations reach actual retained owners.
//! Exact lifecycle:40a910 exit,40a850 enter,40a7c0 activate,40a9b0 reset,
//! 40a9d0 refresh-when-clear,40aa90 show. No operations.select(None).
use super::case_zero_text::{Fonts, Literals, Packet, Text, trunc};
use super::property_child::{ListRange, Selector};
use super::{owner::Owner, ui_selection::Selected};

#[derive(Clone, Debug)]
pub enum FlagByte {
    Unwritten,
    Byte(u8),
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub bounds: [f32; 5],
    pub shown: u8,
    pub content: u32,
    pub detail: u32,
    pub choice: i32,
}
impl Entry {
    fn pair(&mut self, first: u32, second: u32) {
        self.detail = second;
        self.content = first;
        self.shown = if first != 0 {
            1
        } else {
            u8::from(self.choice != 0 && self.choice != -1)
        };
    }
    fn choice(&mut self, choice: i32) {
        self.choice = choice;
        self.shown = if choice == -1 {
            u8::from(self.content != 0)
        } else {
            1
        };
    }
}
#[derive(Clone, Debug)]
pub struct List<K = u32> {
    pub key: K,
    pub bounds: [f32; 4],
    pub shown: u8,
    pub disabled: u8,
    pub flag67c: u8,
    /// Native+13c and+2f0. The native count/stride/window fields alias these
    /// text owners (+2cc,+480,+27c,+284,+2e0); no duplicate list counters.
    pub primary: Text,
    pub secondary: Text,
    pub scratch: Text,
    pub flags: Vec<FlagByte>,
    pub slots: Vec<i32>,
    pub markers: Vec<u32>,
    pub states: Vec<u32>,
    pub entries: Vec<Entry>,
    pub chosen: i32,
    pub value: u32,
}
impl<K: Clone> List<K> {
    pub fn range(&self) -> ListRange {
        ListRange {
            limit480: self.secondary.group.count as i32,
            limit2cc: self.primary.group.count as i32,
            first284: self.primary.visible as i32,
        }
    }
    fn check(&self) -> Result<(), String> {
        let n = self.entries.len();
        if self.flags.len() != n
            || self.slots.len() != n
            || self.markers.len() != n
            || self.states.len() != n
        {
            return Err("case0 actual list capacity arrays disagree".into());
        }
        if n > i32::MAX as usize {
            return Err("case0 list capacity outside signed native loop domain".into());
        }
        Ok(())
    }
    ///42fe90 exact requested=count+1, no geometric growth. The native does
    ///not initialize new flag bytes; Unwritten records that real boundary.
    ///No admitted case0 consumer reads those bytes while flag67c is zero.
    pub(super) fn ensure(&mut self) -> Result<(), String> {
        self.check()?;
        let requested = self
            .primary
            .group
            .count
            .checked_add(1)
            .ok_or("case0 list requested wraps")? as usize;
        if self.entries.len() >= requested {
            return Ok(());
        }
        let old = self.entries.len();
        self.slots.resize(requested, -1);
        self.flags.resize(requested, FlagByte::Unwritten);
        self.markers.resize(requested, 0);
        self.states.resize(requested, 0);
        for row in old..requested {
            let pos = self.primary.pitch.wrapping_mul(row as i32);
            let top = (f64::from(pos) + f64::from(self.primary.scroll) + f64::from(self.bounds[1]))
                as f32;
            let width = self.bounds[2] - self.bounds[0];
            let right = self.bounds[0] + width;
            self.entries.push(Entry {
                bounds: [self.bounds[0], top, right, top + 10., 0.5],
                shown: 0,
                content: 0,
                detail: 0,
                choice: 0,
            });
        }
        Ok(())
    }
    fn sync_selector(&self, selector: &mut Selector) -> Result<(), String> {
        //41f750 always attaches this exact list at selector+15c. Other
        //sources are real nulls written431dd0 and left null by this factory.
        if selector.simple.is_some()
            || selector.text_list.is_some()
            || selector.last_list.is_some()
            || selector.first_list.is_none()
        {
            return Err("case0 selector attachments differ from actual41f750 list binding".into());
        }
        selector.first_list = Some(self.range());
        Ok(())
    }
    ///42f510. The selector's range gets a direct max-count store; no
    ///recalculate call occurs here and the visible/disabled bits are retained.
    pub(super) fn reset(
        &mut self,
        font: &mut Fonts,
        selector: Option<&mut Selector>,
    ) -> Result<(), String> {
        reset_fields(
            &mut self.primary,
            &mut self.secondary,
            &mut self.flags,
            &mut self.slots,
            &mut self.markers,
            &mut self.states,
            &mut self.entries,
            ResetTargets::Installed {
                chosen: &mut self.chosen,
                value: &mut self.value,
            },
            font,
            selector,
        )
    }
    ///42f3d0 ->42fdf0, reached successful normal fields. Constructor-only
    ///chosen/value remain Unknown until reset_fields supplies their FIRST
    ///writers. No fabricated raw key is required by this typed product API.
    pub(super) fn construct_layout(
        key: K,
        bounds: [f32; 4],
        font: &mut Fonts,
    ) -> Result<Self, String> {
        let mut field18 = 1;
        ListBirth::new(key).layout(bounds, font, &mut field18)
    }

    ///4305b0: markers and FIRST text color-array writes, then conditional
    ///metric recomputation, then primary/secondary clear-view in that order.
    fn adjust(
        &mut self,
        font: &mut Fonts,
        old: u32,
        selector: Option<&mut Selector>,
    ) -> Result<(), String> {
        self.ensure()?;
        for row in old..self.primary.group.count {
            self.markers[row as usize] = self.value;
            let color = match self.value {
                0 | 3 => Some(10),
                1 => Some(16),
                2 => Some(0),
                _ => None,
            };
            if let Some(color) = color {
                *self
                    .primary
                    .colors
                    .get_mut(row as usize)
                    .ok_or("case0 marker color capacity")? = color;
            }
        }
        if let Some(selector) = selector {
            let max = (self.primary.group.count as i32).max(self.secondary.group.count as i32);
            if max.wrapping_sub(self.primary.visible as i32) >= 0 {
                self.sync_selector(selector)?;
                selector.recalculate();
            }
        }
        self.primary.clear_view(font)?;
        self.secondary.clear_view(font)
    }
    ///4303d0 (packed) and430340 (literal). Both are actual two-column
    ///producers. Alternative resets only the scratch+4a4 text owner first.
    fn two(
        &mut self,
        font: &mut Fonts,
        literals: &Literals,
        id: Option<u32>,
        selector: Option<&mut Selector>,
    ) -> Result<(), String> {
        if id.is_some() {
            self.scratch.reset(font)?;
        }
        self.ensure()?;
        let old = self.primary.group.count;
        self.value = 2;
        if let Some(id) = id {
            self.primary.append_packed(font, id)?;
        } else {
            self.primary.literal(font, &literals.version, &[1, 4])?;
        }
        self.adjust(font, old, selector)?;
        let remaining = self
            .primary
            .group
            .count
            .wrapping_sub(self.secondary.group.count)
            .wrapping_sub(1);
        if remaining as i32 >= 0 {
            for _ in 0..remaining {
                self.secondary.literal(font, &literals.space, &[])?;
            }
        }
        Ok(())
    }
    ///430210 appends one real space row to each existing column. It clears
    ///neither list nor group; that ordering separates version and body rows.
    fn clear_columns(&mut self, font: &mut Fonts, literals: &Literals) -> Result<(), String> {
        self.primary.literal(font, &literals.space, &[])?;
        self.secondary.literal(font, &literals.space, &[])
    }
    ///42f640 exact pointer row lookup. It uses authoritative pointerY and
    ///+2e0 (primary.first); the incoming native word is absent/unused.
    pub fn pointer_index(&self, pointer_y: i32) -> Result<i32, String> {
        let index = self.primary.row_at(pointer_y)?;
        Ok(if index >= self.primary.group.count {
            -1
        } else {
            index as i32
        })
    }
    ///Finite42f740 informational rows. These are the actual produced rows:
    ///flag67c0, marker2, state0, hidden entry children. Preflight rejects any
    ///other native branch BEFORE cache/global/draw mutation.
    pub fn paint_information(
        &mut self,
        font: &mut Fonts,
        input: &Input,
        palette: &[u32],
    ) -> Result<Paint, String> {
        self.check()?;
        if self.flag67c != 0
            || self.entries.iter().any(|e| e.shown != 0)
            || self.states.iter().any(|&s| s == 3)
            || self.markers.iter().any(|&m| m == 1)
        {
            return Err(
                "case0 list paint reaches unsupported selectable/linked/gradient/line branch"
                    .into(),
            );
        }
        let mut candidate = self.clone();
        let mut fonts = font.clone();
        let result = candidate.paint_information_apply(&mut fonts, input, palette)?;
        *self = candidate;
        *font = fonts;
        Ok(result)
    }
    fn paint_information_apply(
        &mut self,
        font: &mut Fonts,
        input: &Input,
        palette: &[u32],
    ) -> Result<Paint, String> {
        self.value = 0;
        if self.primary.pitch == 0 {
            return Err("actual case0 list row pitch zero".into());
        }
        let pointer_row = trunc(
            (f64::from(input.pointer[1].wrapping_sub(3)) - f64::from(self.bounds[1]))
                / f64::from(self.primary.pitch),
        )?;
        let mut scene_id = None;
        if pointer_row >= 0
            && pointer_row < (self.primary.visible as i32)
            && f64::from(input.pointer[0]) >= f64::from(self.bounds[0])
            && f64::from(input.pointer[0]) <= f64::from(self.bounds[2])
            && f64::from(input.pointer[1]) >= f64::from(self.bounds[1])
            && f64::from(input.pointer[1]) <= f64::from(self.bounds[3])
        {
            let index = (self.primary.first as i32).wrapping_add(pointer_row);
            if index < (self.entries.len() as i32) {
                scene_id = Some(
                    self.entries
                        .get(usize::try_from(index).map_err(|_| "case0 pointer index negative")?)
                        .ok_or("actual case0 entry index")?
                        .detail,
                );
            }
        }
        let end = self
            .primary
            .first
            .wrapping_add(self.primary.visible)
            .min(self.primary.group.count);
        for row in 0..end {
            self.value = *self
                .markers
                .get(row as usize)
                .ok_or("case0 actual marker capacity")?;
        }
        //Native updates embedded secondary FIRST, then primary. Retained text
        //owners determine glyphs, layout, palette/blink and output geometry.
        let mut packets = self
            .secondary
            .paint(font, input.placement, input.tick, palette)?;
        packets.extend(
            self.primary
                .paint(font, input.placement, input.tick, palette)?,
        );
        Ok(Paint { packets, scene_id })
    }
}
///42f3d0 constructor product before containing42fdf0. It owns exactly
///three Texts; chosen/value have no invented constructor value. The wrapper
///binds its outer node before consuming this through the real layout/reset.
pub(super) struct ListBirth<K> {
    key: K,
    primary: Text,
    secondary: Text,
    scratch: Text,
}
impl<K: Clone> ListBirth<K> {
    pub(super) fn new(key: K) -> Self {
        Self {
            key,
            primary: Text::normal_fields(),
            secondary: Text::normal_fields(),
            scratch: Text::normal_fields(),
        }
    }
    pub(super) fn layout(
        mut self,
        bounds: [f32; 4],
        font: &mut Fonts,
        field18: &mut u32,
    ) -> Result<List<K>, String> {
        self.primary.extent440900(bounds)?;
        self.secondary.extent440900(bounds)?;
        self.scratch.extent440900(bounds)?;
        *field18 = 0; //42fe76, before the actual42f510 reset
        let mut flags = Vec::new();
        let mut slots = Vec::new();
        let mut markers = Vec::new();
        let mut states = Vec::new();
        let mut entries = Vec::new();
        let mut chosen = None;
        let mut value = None;
        reset_fields(
            &mut self.primary,
            &mut self.secondary,
            &mut flags,
            &mut slots,
            &mut markers,
            &mut states,
            &mut entries,
            ResetTargets::Birth {
                chosen: &mut chosen,
                value: &mut value,
            },
            font,
            None,
        )?;
        Ok(List {
            key: self.key,
            bounds,
            shown: 1,
            disabled: 0,
            flag67c: 0,
            primary: self.primary,
            secondary: self.secondary,
            scratch: self.scratch,
            flags,
            slots,
            markers,
            states,
            entries,
            chosen: chosen.ok_or("42f510 chosen first writer missing")?,
            value: value.ok_or("42f510 value first writer missing")?,
        })
    }
}
///Private typed first-writer targets, not a caller-supplied callback.
///Birth None is Unknown until the reached scalar stores in42f510.
enum ResetTargets<'a> {
    Installed {
        chosen: &'a mut i32,
        value: &'a mut u32,
    },
    Birth {
        chosen: &'a mut Option<i32>,
        value: &'a mut Option<u32>,
    },
}
///Single42f510 finite algorithm shared by retained raw and typed birth
///products. Scalar first writers precede row/storage and selector stores.
fn reset_fields(
    primary: &mut Text,
    secondary: &mut Text,
    flags: &mut [FlagByte],
    slots: &mut [i32],
    markers: &mut [u32],
    states: &mut [u32],
    entries: &mut [Entry],
    targets: ResetTargets<'_>,
    font: &mut Fonts,
    selector: Option<&mut Selector>,
) -> Result<(), String> {
    primary.reset(font)?;
    secondary.reset(font)?;
    match targets {
        ResetTargets::Installed { chosen, value } => {
            *chosen = -1;
            *value = 0;
        }
        ResetTargets::Birth { chosen, value } => {
            *chosen = Some(-1);
            *value = Some(0);
        }
    }
    markers.fill(0);
    for f in flags {
        *f = FlagByte::Byte(0)
    }
    states.fill(0);
    for row in 0..entries.len() {
        entries[row].pair(0, 0);
        entries[row].choice(-1);
        slots[row] = -1;
    }
    if let Some(selector) = selector {
        selector.extent = (primary.group.count as i32).max(secondary.group.count as i32) as f32;
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct State {
    pub state: u32,
    pub list: Option<List>,
    pub indicator: Option<Selector>,
    pub literals: Literals,
}
impl State {
    fn set_indicator(&mut self, value: u8) -> Result<(), String> {
        if let Some(indicator) = &mut self.indicator {
            self.list
                .as_ref()
                .ok_or("actual case0 selector requires attached list")?
                .sync_selector(indicator)?;
            indicator.show(value);
        }
        Ok(())
    }
    fn reset_list(&mut self, font: &mut Fonts) -> Result<(), String> {
        self.list
            .as_mut()
            .ok_or("actual case0 mandatory list missing")?
            .reset(font, self.indicator.as_mut())
    }
    fn two(&mut self, font: &mut Fonts, id: Option<u32>) -> Result<(), String> {
        self.list
            .as_mut()
            .ok_or("actual case0 mandatory list missing")?
            .two(font, &self.literals, id, self.indicator.as_mut())
    }
    fn clear_columns(&mut self, font: &mut Fonts) -> Result<(), String> {
        self.list
            .as_mut()
            .ok_or("actual case0 mandatory list missing")?
            .clear_columns(font, &self.literals)
    }
    fn reset_local(&mut self) -> Result<(), String> {
        self.state = 0;
        if let Some(list) = &mut self.list {
            list.shown = 0;
        }
        self.set_indicator(0)
    }
    ///Preflight runs actual finite operations on disposable typed state, not
    ///on live UI or Host. It validates all glyphs, ranges, mandatory owners,
    ///native conversion domains and prospective text counts before mutation.
    fn show_check(&self, header: &Text, font: &Fonts) -> Result<(), String> {
        let mut s = self.clone();
        let mut header = header.clone();
        let mut font = font.clone();
        s.show_local(&mut header, &mut font)
    }
    fn show_local(&mut self, header: &mut Text, font: &mut Fonts) -> Result<(), String> {
        header.reset(font)?;
        self.reset_list(font)?;
        header.identifier(font, 0x589)?;
        self.set_indicator(1)?;
        self.two(font, Some(0x58a))?;
        self.two(font, None)?;
        self.clear_columns(font)?;
        self.two(font, Some(0x58c))
    }
}
fn state(view: &impl super::main_panel::Access) -> Result<&State, String> {
    Ok(&view.main()?.case_zero)
}
pub fn exit_check(
    view: &impl super::main_panel::Access,
    _: &Owner,
    _: Option<Selected>,
) -> Result<(), String> {
    let mut copy = state(view)?.clone();
    let mut font = view.fonts()?.clone();
    let mut header = view.main()?.notice_text.clone();
    header.reset(&mut font)?;
    copy.set_indicator(0)
}
pub fn exit(view: &mut impl super::main_panel::Access, owner: &mut Owner) -> Result<(), String> {
    // This transaction copies/tests nullness only; no Selected field is read.
    let current = owner.ui_selection()?.current;
    super::main_panel::transact(view, current, exit_owned)
}
pub fn enter_check(
    view: &impl super::main_panel::Access,
    _: &Owner,
    _: Option<Selected>,
) -> Result<(), String> {
    let mut copy = state(view)?.clone();
    let mut font = view.fonts()?.clone();
    let mut header = view.main()?.notice_text.clone();
    header.reset(&mut font)?;
    copy.set_indicator(0)?;
    copy.show_local(&mut header, &mut font)
}
pub fn enter(view: &mut impl super::main_panel::Access, owner: &mut Owner) -> Result<(), String> {
    // This transaction copies/tests nullness only; no Selected field is read.
    let current = owner.ui_selection()?.current;
    super::main_panel::transact(view, current, enter_owned)
}
pub fn activate_check(
    view: &impl super::main_panel::Access,
    _: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    if current.is_none() || state(view)?.state != 0 {
        return Ok(());
    }
    state(view)?.show_check(&view.main()?.notice_text, view.fonts()?)
}
pub fn activate(
    view: &mut impl super::main_panel::Access,
    owner: &mut Owner,
) -> Result<(), String> {
    // This transaction copies/tests nullness only; no Selected field is read.
    let current = owner.ui_selection()?.current;
    super::main_panel::transact(view, current, activate_owned)
}
pub fn reset_check(
    view: &impl super::main_panel::Access,
    _: &Owner,
    _: Option<Selected>,
) -> Result<(), String> {
    let mut copy = state(view)?.clone();
    copy.reset_local()
}
pub fn reset(view: &mut impl super::main_panel::Access, owner: &mut Owner) -> Result<(), String> {
    // This transaction copies/tests nullness only; no Selected field is read.
    let current = owner.ui_selection()?.current;
    super::main_panel::transact(view, current, reset_owned)
}
pub fn refresh_check(
    view: &impl super::main_panel::Access,
    _: &Owner,
    _: Option<Selected>,
) -> Result<(), String> {
    if state(view)?.state != 0 {
        return Ok(());
    }
    let mut copy = state(view)?.clone();
    let mut font = view.fonts()?.clone();
    let mut header = view.main()?.notice_text.clone();
    header.reset(&mut font)?;
    copy.reset_local()?;
    copy.show_local(&mut header, &mut font)
}
pub fn refresh_when_clear(
    view: &mut impl super::main_panel::Access,
    owner: &mut Owner,
) -> Result<(), String> {
    // This transaction copies/tests nullness only; no Selected field is read.
    let current = owner.ui_selection()?.current;
    super::main_panel::transact(view, current, refresh_when_clear_owned)
}
pub fn show_check(
    view: &impl super::main_panel::Access,
    _: &Owner,
    _: Option<Selected>,
) -> Result<(), String> {
    state(view)?.show_check(&view.main()?.notice_text, view.fonts()?)
}
pub fn show(view: &mut impl super::main_panel::Access, owner: &mut Owner) -> Result<(), String> {
    // This transaction copies/tests nullness only; no Selected field is read.
    let current = owner.ui_selection()?.current;
    super::main_panel::transact(view, current, show_owned)
}
/// The finite algorithms shared by public transactions and parent plans.
/// All Text writes share the same prospective manager in native order.
pub(super) fn exit_owned(view: &mut super::main_panel::Prospective) -> Result<(), String> {
    exit_finite(&mut OwnedExit { view })
}
///40a910's finite sequence; each view borrows actual retained resources.
pub(super) trait ExitView {
    fn clear_notice(&mut self) -> Result<(), String>;
    fn restore_colors(&mut self) -> Result<(), String>;
    fn hide_list(&mut self) -> Result<(), String>;
    fn hide_selector(&mut self) -> Result<(), String>;
}
pub(super) fn exit_finite(view: &mut impl ExitView) -> Result<(), String> {
    view.clear_notice()?;
    view.restore_colors()?;
    view.hide_list()?;
    view.hide_selector()
}
struct OwnedExit<'a> {
    view: &'a mut super::main_panel::Prospective,
}
impl ExitView for OwnedExit<'_> {
    fn clear_notice(&mut self) -> Result<(), String> {
        super::main_panel::clear_notice(self.view)
    }
    fn restore_colors(&mut self) -> Result<(), String> {
        let m = &mut self.view.main;
        m.property.colors = m.case_zero.literals.colors;
        Ok(())
    }
    fn hide_list(&mut self) -> Result<(), String> {
        if let Some(l) = &mut self.view.main.case_zero.list {
            l.shown = 0;
        }
        Ok(())
    }
    fn hide_selector(&mut self) -> Result<(), String> {
        self.view.main.case_zero.set_indicator(0)
    }
}
pub(super) fn show_owned(view: &mut super::main_panel::Prospective) -> Result<(), String> {
    super::main_panel::clear_notice(view)?;
    let m = &mut view.main;
    let font = &mut view.fonts;
    m.case_zero.reset_list(font)?;
    m.notice_text.identifier(font, 0x589)?;
    m.notice_caption = Some(0x589);
    m.case_zero.set_indicator(1)?;
    m.case_zero.two(font, Some(0x58a))?;
    m.case_zero.two(font, None)?;
    m.case_zero.clear_columns(font)?;
    m.case_zero.two(font, Some(0x58c))
}
pub(super) fn enter_owned(view: &mut super::main_panel::Prospective) -> Result<(), String> {
    for info in &mut view.main.property.infos {
        info.shown = 0;
        info.set_selected(0);
        info.disabled = 0;
    }
    exit_owned(view)?;
    show_owned(view)
}
pub(super) fn activate_owned(view: &mut super::main_panel::Prospective) -> Result<(), String> {
    if view.current.is_none() || view.main.case_zero.state != 0 {
        return Ok(());
    }
    show_owned(view)
}
pub(super) fn reset_owned(view: &mut super::main_panel::Prospective) -> Result<(), String> {
    view.main.case_zero.reset_local()
}
pub(super) fn refresh_when_clear_owned(
    view: &mut super::main_panel::Prospective,
) -> Result<(), String> {
    if view.main.case_zero.state != 0 {
        return Ok(());
    }
    exit_owned(view)?;
    view.main.case_zero.state = 0;
    show_owned(view)
}
pub fn child() -> super::main_panel::Child {
    use super::main_panel::{Child, Step};
    Child {
        exit: Step {
            check: process_exit_check,
            apply: process_exit,
        },
        enter: Step {
            check: process_enter_check,
            apply: process_enter,
        },
        activate: Step {
            check: process_activate_check,
            apply: process_activate,
        },
    }
}
pub fn reset_step() -> super::main_panel::Step {
    super::main_panel::Step {
        check: process_reset_check,
        apply: process_reset,
    }
}
pub fn refresh_step() -> super::main_panel::Step {
    super::main_panel::Step {
        check: process_refresh_check,
        apply: process_refresh_when_clear,
    }
}
///Input is captured at the actual acknowledged UI transaction; no camera,
///display Hit, saved-index topology or default mouse point is a producer.
pub struct Input {
    pub pointer: [i32; 2],
    pub placement: [f32; 2],
    pub tick: u32,
}
pub struct Paint {
    pub packets: Vec<Packet>,
    pub scene_id: Option<u32>,
}
///42f740 list body inside40a790. Original does not test list.shown here.
///This helper intentionally names only the LIST operation;40a790 also calls
///indicator43e790, including real arrow visuals and432990/432a80 surfaces.
///Root must compose that real selector draw before enabling whole40a790.
pub fn draw_list_information(
    view: &mut impl super::main_panel::Access,
    input: &Input,
    palette: &[u32],
) -> Result<Paint, String> {
    if state(view)?.state != 0 {
        return Ok(Paint {
            packets: Vec::new(),
            scene_id: None,
        });
    }
    let (m, font) = view.text_parts_mut()?;
    let s = &mut m.case_zero;
    s.list
        .as_mut()
        .ok_or("case0 update mandatory list missing")?
        .paint_information(font, input, palette)
}

///Finite430690 list input: every returned effect is a synchronous mandatory
///root dispatch, not a deferred success log. Parent consumer must preflight
///the returned event before applying this helper's result to its live owner.
pub enum InputEffect {
    None,
    Parent { event: u32, key: u32, detail: u32 },
}
pub fn list_phase_information(
    view: &impl super::main_panel::Access,
    phase: u32,
    b: u32,
    c: u32,
    input: &Input,
) -> Result<InputEffect, String> {
    let s = state(view)?;
    let list = s.list.as_ref().ok_or("case0 input actual list missing")?;
    if list.disabled == 1 {
        return Ok(InputEffect::None);
    }
    match phase {
        4 => {
            if list.flag67c == 1 {
                return Err("case0 selectable list flags mutation unsupported".into());
            }
            if list.primary.pitch == 0 {
                return Err("case0 pointer phase pitch zero".into());
            }
            let row = trunc(
                (f64::from(input.pointer[1].wrapping_sub(3)) - f64::from(list.bounds[1]))
                    / f64::from(list.primary.pitch),
            )?;
            Ok(InputEffect::Parent {
                event: 28,
                key: list.key,
                detail: (row as u32).wrapping_add(list.primary.first),
            })
        }
        8 | 9 => {
            if s.indicator.is_some() {
                return Err("case0 input reaches actual selector432360 phase consumer".into());
            }
            let _ = (b, c);
            Ok(InputEffect::None)
        }
        13 => Ok(InputEffect::Parent {
            event: 13,
            key: list.key,
            detail: b,
        }),
        _ => Ok(InputEffect::None), //native430780 default branch, outside4..13 too
    }
}

//Concrete stored callback ABI; forwards once to the same finite owned transaction.
fn process_exit_check(
    view: &super::main_panel::ProcessTextAccess<'_>,
    owner: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    exit_check(view, owner, current)
}
fn process_exit(
    view: &mut super::main_panel::ProcessTextAccess<'_>,
    owner: &mut Owner,
) -> Result<(), String> {
    exit(view, owner)
}
fn process_enter_check(
    view: &super::main_panel::ProcessTextAccess<'_>,
    owner: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    enter_check(view, owner, current)
}
fn process_enter(
    view: &mut super::main_panel::ProcessTextAccess<'_>,
    owner: &mut Owner,
) -> Result<(), String> {
    enter(view, owner)
}
fn process_activate_check(
    view: &super::main_panel::ProcessTextAccess<'_>,
    owner: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    activate_check(view, owner, current)
}
fn process_activate(
    view: &mut super::main_panel::ProcessTextAccess<'_>,
    owner: &mut Owner,
) -> Result<(), String> {
    activate(view, owner)
}
fn process_reset_check(
    view: &super::main_panel::ProcessTextAccess<'_>,
    owner: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    reset_check(view, owner, current)
}
fn process_reset(
    view: &mut super::main_panel::ProcessTextAccess<'_>,
    owner: &mut Owner,
) -> Result<(), String> {
    reset(view, owner)
}
fn process_refresh_check(
    view: &super::main_panel::ProcessTextAccess<'_>,
    owner: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    refresh_check(view, owner, current)
}
fn process_refresh_when_clear(
    view: &mut super::main_panel::ProcessTextAccess<'_>,
    owner: &mut Owner,
) -> Result<(), String> {
    refresh_when_clear(view, owner)
}
