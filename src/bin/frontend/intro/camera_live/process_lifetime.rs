//! Once-only process graph ownership, separate from same-session refresh.
//! Intro keeps this alongside parked rawOwner. It is moved, never regenerated.
use super::{super::Inspection, focus::Focus, navigation, ui_graph::UiGraphState};
pub struct Parked {
    graph: UiGraphState,
    navigation: navigation::State,
    focus: Focus,
    click_latched: bool,
    locator: Option<crate::locator::Locator>,
}
impl Parked {
    pub fn take_from(view: &mut Inspection) -> Result<Self, String> {
        if view.ui_graph.is_none() || view.camera_navigation.is_none() {
            return Err("cannot park incomplete actual process UI".into());
        }
        Ok(Self {
            graph: view.ui_graph.take().unwrap(),
            navigation: view.camera_navigation.take().unwrap(),
            //Default below is only a moved-from temporary Inspection cell.
            //It never initializes the retained process Focus or its targets.
            focus: std::mem::take(&mut view.camera_focus),
            click_latched: view.navigation_click_latched,
            locator: view.locator.take(),
        })
    }
    pub fn put_into(slot: &mut Option<Self>, view: &mut Inspection) -> Result<(), String> {
        if view.ui_graph.is_some() || view.camera_navigation.is_some() {
            return Err("cannot replace actual process UI on warm attach".into());
        }
        //Do not consume/drop the parked owner on a rejected destination.
        let parked = slot.take().ok_or("actual process UI was not parked")?;
        view.ui_graph = Some(parked.graph);
        view.camera_navigation = Some(parked.navigation);
        view.camera_focus = parked.focus;
        view.navigation_click_latched = parked.click_latched;
        //Move the SAME C tree, rows, selected indices, cache and typed controls.
        //The destination's legacy Scene projection is a disposable view shell.
        view.locator = parked.locator;
        Ok(())
    }
}
///Projection of the two represented4b9d40 cells at their actual boundary.
///Root must ALSO clear the distinct823168 and6175f8 owners there; the raw-cell
///producer in native_initializers writes all four in source order. Neither
///590f58 publication, sequence link nor list indices is written by the prefix.
pub fn reset_world_selection_projection(owner: &mut super::owner::Owner) -> Result<(), String> {
    let state = owner.ui_selection_mut()?;
    state.current = None;
    state.selected_building = None;
    Ok(())
}
///Native43c7c0 is a different operation from focus/dwell reset43c840.
///It walks HEAD->NEXT (reverse current draw order), zeros velocity and
///descriptor X/Y, invokes each real layout-reset+30, then selected/hovered=0.
///It does not clear dwell/key, mode/saved bytes, frame, shown or enabled.
pub fn reset_group_layout(
    view: &mut super::main_panel::ProcessTextAccess<'_>,
    which: super::navigation::Group,
    owner: &mut super::owner::Owner,
    check: fn(
        &super::main_panel::ProcessTextAccess<'_>,
        super::ui_graph::PanelId,
        &super::owner::Owner,
    ) -> Result<(), String>,
    reset: fn(
        &mut super::main_panel::ProcessTextAccess<'_>,
        super::ui_graph::PanelId,
        &mut super::owner::Owner,
    ) -> Result<(), String>,
) -> Result<(), String> {
    let mut next = view
        .view
        .ui_graph
        .as_ref()
        .ok_or("process UI graph absent")?
        .group(which)
        .links
        .head;
    while let Some(id) = next {
        //Read-only admission at this reached boundary only; no whole-pass
        //preflight, no callback duplication and no promise of rollback.
        check(view, id, owner)?;
        {
            let panel = view
                .view
                .ui_graph
                .as_mut()
                .ok_or("process UI graph absent")?
                .panel_mut(id)?;
            panel.velocity = [0.; 2];
            panel.position = [0.; 2];
            if let Some(pointer) = panel.f_pointer.as_mut() {
                pointer.velocity_z = 0.;
            }
        }
        reset(view, id, owner)?;
        next = view
            .view
            .ui_graph
            .as_ref()
            .ok_or("process UI graph absent")?
            .group(which)
            .links
            .next(id)?;
    }
    let group = view
        .view
        .ui_graph
        .as_mut()
        .ok_or("process UI graph absent")?
        .group_mut(which);
    group.selected = None;
    group.hovered = None;
    Ok(())
}
///Native order inside4b9d40: Primary, Frontend, Auction layout reset.
pub const WORLD_LAYOUT_RESET_ORDER: [super::navigation::Group; 3] = [
    super::navigation::Group::Primary,
    super::navigation::Group::Secondary,
    super::navigation::Group::Tertiary,
];

// Moved into the library with the save decoder (#99).
pub use openmt2001::save::restore_timing400;
