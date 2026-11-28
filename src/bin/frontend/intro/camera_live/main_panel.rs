//! Concrete41f290 parent switch. Child algorithms are explicit mandatory
//! services; there is no empty implementation for any reached child stage.
use super::{super::Inspection, owner::Owner, ui_selection::Selected};
//One actual mode-button owner: lifecycle/model and future render/input use it.
pub type Control = super::property_child::Widget;
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub check: fn(&ProcessTextAccess<'_>, &Owner, Option<Selected>) -> Result<(), String>,
    pub apply: fn(&mut ProcessTextAccess<'_>, &mut Owner) -> Result<(), String>,
}
#[derive(Clone, Copy, Debug)]
pub struct Child {
    pub exit: Step,
    pub enter: Step,
    pub activate: Step,
}
#[derive(Clone, Debug)]
pub struct State {
    pub mode: u32,
    ///Same native+508 byte: expanded=true selects activate, zero refreshes
    ///the closed-state model and exits the newly selected child once more.
    pub expanded: u8,
    pub transition_pending: u8,
    pub controls: [Control; 6],
    ///Actual notice control text, not an invented completion marker.
    pub notice: Vec<u8>,
    pub notice_caption: Option<u32>,
    /// Actual parent+3a0 text storage; scalar fields above are projections.
    pub notice_text: super::case_zero_text::Text,
    pub case_zero: super::case_zero::State,
    /// SAME parent+4b8 text consumed by property and business control flows.
    pub info_text: super::case_zero_text::Text,
    pub business_reset: Option<super::other_child_resets::Business>,
    pub five_reset: Option<super::other_child_resets::Five>,
    pub property: super::property_child::State,
    pub model: super::main_model::State,
    pub children: [Option<Child>; 6],
    ///Concrete41fae0 implementation must prepare actual model target1/10,
    ///overlay region/frame and real exit/activation. No fallback exists.
    pub refresh_closed: Option<Step>,
}
impl State {
    pub fn installed(
        mode: u32,
        expanded: u8,
        transition_pending: u8,
        controls: [Control; 6],
        notice: Vec<u8>,
        notice_caption: Option<u32>,
        property: super::property_child::State,
        model: super::main_model::State,
        notice_text: super::case_zero_text::Text,
        case_zero: super::case_zero::State,
        info_text: super::case_zero_text::Text,
        business_reset: Option<super::other_child_resets::Business>,
        five_reset: Option<super::other_child_resets::Five>,
        mut children: [Option<Child>; 6],
    ) -> Self {
        children[0] = Some(super::case_zero::child());
        children[1] = Some(super::property_child::child());
        let refresh_closed = Some(super::main_model::step());
        Self {
            mode,
            expanded,
            transition_pending,
            controls,
            notice,
            notice_caption,
            notice_text,
            case_zero,
            info_text,
            business_reset,
            five_reset,
            property,
            model,
            children,
            refresh_closed,
        }
    }
    pub fn control_visible(&self, index: usize) -> bool {
        self.controls.get(index).is_some_and(|c| c.shown != 0)
    }
    pub fn control_enabled(&self, index: usize) -> bool {
        self.controls
            .get(index)
            .is_some_and(|c| c.shown != 0 && c.disabled == 0)
    }
}
/// Access only the finite owned Main state and its model panel. No Host,
/// renderer, event dispatch or camera state is copied or called by a plan.
pub(crate) trait Access {
    fn main(&self) -> Result<&State, String>;
    fn main_mut(&mut self) -> Result<&mut State, String>;
    ///Inspection alone has no process lifetime resource authority. The real
    ///caller supplies ProcessTextAccess; no per-city Fonts may be installed.
    fn fonts(&self) -> Result<&super::case_zero_text::Fonts, String> {
        Err("shared process text resource borrow absent".into())
    }
    fn text_parts_mut(
        &mut self,
    ) -> Result<(&mut State, &mut super::case_zero_text::Fonts), String> {
        Err("shared process text resource borrow absent".into())
    }
    fn panel(&self) -> Result<&super::ui_graph::Panel, String>;
    fn panel_mut(&mut self) -> Result<&mut super::ui_graph::Panel, String>;
}
impl Access for Inspection {
    fn main(&self) -> Result<&State, String> {
        Ok(&self
            .ui_graph
            .as_ref()
            .ok_or("Main panel graph absent")?
            .main)
    }
    fn main_mut(&mut self) -> Result<&mut State, String> {
        Ok(&mut self
            .ui_graph
            .as_mut()
            .ok_or("Main panel graph absent")?
            .main)
    }
    fn panel(&self) -> Result<&super::ui_graph::Panel, String> {
        self.ui_graph
            .as_ref()
            .ok_or("Main panel graph absent")?
            .panel(super::ui_graph::PanelId::A)
    }
    fn panel_mut(&mut self) -> Result<&mut super::ui_graph::Panel, String> {
        self.ui_graph
            .as_mut()
            .ok_or("Main panel graph absent")?
            .panel_mut(super::ui_graph::PanelId::A)
    }
}
///Finite actual caller adapter. Borrows Intro.assets.ui_text and the real
///Inspection together; drop it at the end of the synchronous operation.
///Every installed finite Step receives this borrow. Read-only checks borrow
///it immutably; apply uses the existing prospective commit/discard boundary.
pub(crate) struct ProcessTextAccess<'a> {
    pub view: &'a mut Inspection,
    pub resources: &'a mut super::ui_text_resources::UiTextResources,
}
impl Access for ProcessTextAccess<'_> {
    fn main(&self) -> Result<&State, String> {
        self.view.main()
    }
    fn main_mut(&mut self) -> Result<&mut State, String> {
        self.view.main_mut()
    }
    fn fonts(&self) -> Result<&super::case_zero_text::Fonts, String> {
        Ok(self.resources.fonts())
    }
    fn text_parts_mut(
        &mut self,
    ) -> Result<(&mut State, &mut super::case_zero_text::Fonts), String> {
        Ok((self.view.main_mut()?, self.resources.fonts_mut()))
    }
    fn panel(&self) -> Result<&super::ui_graph::Panel, String> {
        self.view.panel()
    }
    fn panel_mut(&mut self) -> Result<&mut super::ui_graph::Panel, String> {
        self.view.panel_mut()
    }
}
#[derive(Clone, Debug)]
pub(super) struct Prospective {
    pub fonts: super::case_zero_text::Fonts,
    pub main: State,
    pub panel: Option<super::ui_graph::Panel>,
    pub current: Option<Selected>,
}
impl Access for Prospective {
    fn fonts(&self) -> Result<&super::case_zero_text::Fonts, String> {
        Ok(&self.fonts)
    }
    fn text_parts_mut(
        &mut self,
    ) -> Result<(&mut State, &mut super::case_zero_text::Fonts), String> {
        Ok((&mut self.main, &mut self.fonts))
    }
    fn main(&self) -> Result<&State, String> {
        Ok(&self.main)
    }
    fn main_mut(&mut self) -> Result<&mut State, String> {
        Ok(&mut self.main)
    }
    fn panel(&self) -> Result<&super::ui_graph::Panel, String> {
        self.panel
            .as_ref()
            .ok_or_else(|| "Main model panel absent".into())
    }
    fn panel_mut(&mut self) -> Result<&mut super::ui_graph::Panel, String> {
        self.panel
            .as_mut()
            .ok_or_else(|| "Main model panel absent".into())
    }
}
impl Prospective {
    /// Validate the exact prospective source, which can differ from live
    /// current after C hide. No cached category or display key is substituted.
    pub fn resolved_current<'a>(
        &self,
        owner: &'a Owner,
    ) -> Result<Option<super::owner::ResolvedSelected<'a>>, String> {
        owner.resolve_ui_selected(self.current)
    }
    pub fn capture(view: &impl Access, current: Option<Selected>) -> Result<Self, String> {
        Ok(Self {
            fonts: view.fonts()?.clone(),
            main: view.main()?.clone(),
            panel: view.panel().ok().cloned(),
            current,
        })
    }
    fn commit(self, view: &mut impl Access) {
        // No callback runs between capture and commit. All fallible work has
        // completed; these are the same owners captured above.
        let (main, fonts) = view
            .text_parts_mut()
            .expect("captured Main/process resource borrow");
        *main = self.main;
        *fonts = self.fonts;
        if let Some(panel) = self.panel {
            *view.panel_mut().expect("captured Main panel") = panel;
        }
    }
}
pub(super) fn transact(
    view: &mut impl Access,
    current: Option<Selected>,
    operation: impl FnOnce(&mut Prospective) -> Result<(), String>,
) -> Result<(), String> {
    let mut plan = Prospective::capture(view, current)?;
    operation(&mut plan)?;
    plan.commit(view);
    Ok(())
}
fn state(view: &impl Access) -> Result<&State, String> {
    view.main()
}
fn state_mut(view: &mut impl Access) -> Result<&mut State, String> {
    view.main_mut()
}
fn recognized(mode: u32) -> bool {
    matches!(mode, 0 | 1 | 2 | 5)
}
fn child(view: &impl Access, mode: u32) -> Result<Child, String> {
    state(view)?
        .children
        .get(mode as usize)
        .and_then(|v| *v)
        .ok_or_else(|| format!("Reached real A child{mode} lifecycle is unimplemented"))
}
#[derive(Clone, Copy)]
enum Stage {
    Exit,
    Enter,
    Activate,
}
fn same_step(a: Step, b: Step) -> bool {
    std::ptr::fn_addr_eq(a.check, b.check) && std::ptr::fn_addr_eq(a.apply, b.apply)
}
fn child_owned(
    view: &mut Prospective,
    owner: &Owner,
    mode: u32,
    stage: Stage,
) -> Result<(), String> {
    if !recognized(mode) {
        return Ok(());
    }
    let installed = child(view, mode)?;
    let expected = match mode {
        0 => super::case_zero::child(),
        1 => super::property_child::child(),
        _ => {
            return Err(format!(
                "Reached real A child{mode} lifecycle has no finite owned transaction"
            ));
        }
    };
    let (actual, wanted) = match stage {
        Stage::Exit => (installed.exit, expected.exit),
        Stage::Enter => (installed.enter, expected.enter),
        Stage::Activate => (installed.activate, expected.activate),
    };
    if !same_step(actual, wanted) {
        return Err(
            "Main transaction requires its actual nonreentrant owned child algorithm".into(),
        );
    }
    match (mode, stage) {
        (0, Stage::Exit) => super::case_zero::exit_owned(view),
        (0, Stage::Enter) => super::case_zero::enter_owned(view),
        (0, Stage::Activate) => super::case_zero::activate_owned(view),
        (1, Stage::Exit) => super::property_child::exit_owned(view, owner),
        (1, Stage::Enter) => super::property_child::enter_owned(view, owner),
        (1, Stage::Activate) => super::property_child::activate_owned(view, owner),
        _ => unreachable!("finite owned child stage"),
    }
}
pub(super) fn exit_owned(view: &mut Prospective, owner: &Owner) -> Result<(), String> {
    view.resolved_current(owner)?;
    let mode = view.main.mode;
    child_owned(view, owner, mode, Stage::Exit)
}
fn switch_owned(view: &mut Prospective, requested: u32, owner: &Owner) -> Result<(), String> {
    view.resolved_current(owner)?;
    clear_notice(view)?;
    exit_owned(view, owner)?;
    view.main.mode = requested;
    for control in &mut view.main.controls {
        control.shown = 0;
        control.set_selected(0);
        control.disabled = 0;
    }
    // These finite owned operations are nonreentrant. Reread the same source
    // fields at each stage; external callback/Host dispatch is not admitted.
    let mode = view.main.mode;
    child_owned(view, owner, mode, Stage::Enter)?;
    if view.main.expanded == 0 {
        let step = view
            .main
            .refresh_closed
            .ok_or("A41fae0 closed model/reset consumer absent")?;
        if !same_step(step, super::main_model::step()) {
            return Err("Main closed refresh requires actual owned model algorithm".into());
        }
        return super::main_model::refresh_owned(view, owner);
    }
    let mode = view.main.mode;
    child_owned(view, owner, mode, Stage::Activate)
}
pub fn preflight(
    view: &impl Access,
    requested: u32,
    owner: &Owner,
    current: Option<Selected>,
) -> Result<(), String> {
    let current = owner.validate_ui_selected(current)?;
    let mut plan = Prospective::capture(view, current)?;
    switch_owned(&mut plan, requested, owner)
}
pub fn switch(view: &mut impl Access, requested: u32, owner: &mut Owner) -> Result<(), String> {
    let current = owner.current_ui_selected()?;
    transact(view, current, |p| switch_owned(p, requested, owner))
}

///Concrete41fc80 phase1/11 prelude and original source-suppressed branches.
///Input messages12/28 stay with their real child owners and are not accepted.
fn message_owned(
    view: &mut Prospective,
    event: u32,
    owner: &Owner,
    frame: u16,
) -> Result<(), String> {
    view.resolved_current(owner)?;
    if !matches!(event, 1 | 11) {
        return Err("A message requires real input/selection child consumer".into());
    }
    if frame == 9 && view.main.transition_pending == 1 {
        view.main.transition_pending = 0;
        for control in &mut view.main.controls {
            control.shown = 0;
            control.set_selected(0);
            control.disabled = 0;
        }
        let mode = view.main.mode;
        child_owned(view, owner, mode, Stage::Enter)?;
        if view.main.expanded != 0 {
            let mode = view.main.mode;
            child_owned(view, owner, mode, Stage::Activate)?;
        }
    }
    if frame == 0 && view.main.transition_pending == 1 {
        view.main.transition_pending = 0;
    }
    if event == 11 && view.main.expanded != 0 {
        let mode = view.main.mode;
        child_owned(view, owner, mode, Stage::Activate)?;
    }
    Ok(())
}
pub fn check_message(
    view: &impl Access,
    event: u32,
    owner: &Owner,
    frame: u16,
) -> Result<(), String> {
    let mut plan = Prospective::capture(view, owner.current_ui_selected()?)?;
    message_owned(&mut plan, event, owner, frame)
}
pub fn message(view: &mut impl Access, event: u32, owner: &mut Owner) -> Result<(), String> {
    let frame = view.panel()?.frame;
    let current = owner.current_ui_selected()?;
    transact(view, current, |p| message_owned(p, event, owner, frame))
}

///441940 owns both text and identifier. Clearing just Vec bytes leaves a stale
///property caption after a null selection/closed refresh.
pub fn clear_notice(view: &mut impl Access) -> Result<(), String> {
    let (main, font) = view.text_parts_mut()?;
    main.notice_text.reset(font)?;
    main.notice.clear();
    main.notice_caption = None;
    Ok(())
}

///441220 source identifier append to the SAME actual parent text owner.
///Existing scalar caption fields remain renderer compatibility projections.
pub fn set_notice_identifier(view: &mut impl Access, id: u32) -> Result<(), String> {
    let (main, font) = view.text_parts_mut()?;
    main.notice_text.identifier(font, id)?;
    main.notice_caption = Some(id);
    Ok(())
}
///Render the actual parent notice owner after its lifecycle/model/body gates.
pub fn paint_notice(
    view: &mut impl Access,
    input: &super::case_zero::Input,
    palette: &[u32],
) -> Result<Vec<super::case_zero_text::Packet>, String> {
    let (main, font) = view.text_parts_mut()?;
    main.notice_text
        .paint(font, input.placement, input.tick, palette)
}

///Read-only actual441940 preflight for enclosing parent/child operations.
pub fn check_notice_clear(view: &impl Access) -> Result<(), String> {
    let main = state(view)?;
    let mut text = main.notice_text.clone();
    let mut font = view.fonts()?.clone();
    text.reset(&mut font)
}
pub fn check_notice_identifier(view: &impl Access, id: u32) -> Result<(), String> {
    let main = state(view)?;
    let mut text = main.notice_text.clone();
    let mut font = view.fonts()?.clone();
    text.reset(&mut font)?;
    text.identifier(&mut font, id)
}

///441940/441220/440ad0 on the shared parent+4b8 text owner. The property
///Widget remains its actual visibility/control owner with Caption projection.
pub fn reset_info_text(view: &mut impl Access) -> Result<(), String> {
    let (m, font) = view.text_parts_mut()?;
    m.info_text.reset(font)
}
pub fn append_info_identifier(view: &mut impl Access, id: u32) -> Result<(), String> {
    let (m, font) = view.text_parts_mut()?;
    m.info_text.identifier(font, id)
}
pub fn color_info_text(view: &mut impl Access, color: u8) -> Result<(), String> {
    let m = state_mut(view)?;
    m.info_text.resize()?;
    for c in &mut m.info_text.colors[..m.info_text.group.count as usize] {
        *c = color;
    }
    Ok(())
}
pub fn paint_info_text(
    view: &mut impl Access,
    input: &super::case_zero::Input,
    palette: &[u32],
) -> Result<Vec<super::case_zero_text::Packet>, String> {
    let (m, font) = view.text_parts_mut()?;
    m.info_text
        .paint(font, input.placement, input.tick, palette)
}
///41eb40 finite822d50-null parent reset.590f60 remains independently read by
///the child2 reset preflight; absent native child2/5 pointers skip only their
///corresponding optional suffix calls. Their reached lifecycles stay errors.
pub(super) fn reset_no_selection_owned(
    view: &mut Prospective,
    owner: &Owner,
) -> Result<(), String> {
    view.resolved_current(owner)?;
    if view.current.is_some() {
        return Err("A41eb40 selected822d50 reset requires selected property family".into());
    }
    view.main.expanded = 1;
    view.main.transition_pending = 0;
    view.main.property.selected_category = u32::MAX;
    super::main_model::refresh_owned(view, owner)?;
    switch_owned(view, 0, owner)?;
    super::property_child::reset_no_selection_owned(view, owner)?;
    super::other_child_resets::business_owned(view, owner)?;
    if let Some(five) = &mut view.main.five_reset {
        five.state24 = 0;
    }
    super::case_zero::reset_owned(view)
}
pub fn reset_no_selection(view: &mut impl Access, owner: &mut Owner) -> Result<(), String> {
    let current = owner.current_ui_selected()?;
    transact(view, current, |p| reset_no_selection_owned(p, owner))
}

///Preflight the shared report caption before enclosing reset mutates controls.
pub fn check_info_identifier(view: &impl Access, id: u32) -> Result<(), String> {
    let m = state(view)?;
    let mut text = m.info_text.clone();
    let mut font = view.fonts()?.clone();
    text.reset(&mut font)?;
    text.identifier(&mut font, id)?;
    text.resize()?;
    Ok(())
}

#[cfg(test)]
#[path = "case_zero_repair_tests.rs"]
pub(crate) mod regression_tests;
