//! Split 4b5370 UI operations and graph-toggle completion. No camera record
//! or pose lives here. Concrete missing consumers remain explicit, mandatory
//! joins; this module does not activate a successful placeholder route.
use super::main_panel::ProcessTextAccess;
use super::{
    super::{Assets, Inspection},
    context::NavigationUi,
    owner::Owner,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Secondary,
    Primary,
    Tertiary,
}

/// Each function MUST operate on its actual UI consumer, synchronously and
/// without waiting/querying the blocked Host, writing a clock, emitting a
/// FreshRequest, or staging TaskEffects. Owner admits recovered setter effects.
/// No default implementation exists: publication/activation requires concrete
/// source implementations for EVERY reached consumer, not logs/flag mirrors.
#[derive(Clone, Copy)]
pub struct Consumers {
    pub visible: fn(&mut ProcessTextAccess<'_>, Object, &mut Owner) -> Result<u8, String>,
    pub visibility: fn(&mut ProcessTextAccess<'_>, Object, bool, &mut Owner) -> Result<(), String>,
    pub reset_group: fn(&mut ProcessTextAccess<'_>, Group, &mut Owner) -> Result<(), String>,
    pub enable_group: fn(&mut ProcessTextAccess<'_>, Group, bool, &mut Owner) -> Result<(), String>,
    pub toolbar_mode: fn(&mut ProcessTextAccess<'_>, u32, &mut Owner) -> Result<(), String>,
    /// 4c39c0 reads current published selection and its actual sequence-link
    /// state at the call, not an earlier Scene management-link approximation.
    pub restore_picked_panel: fn(&mut ProcessTextAccess<'_>, &mut Owner) -> Result<(), String>,
    ///425400 caption only;4253a0 network-disable/blink is a separate reset.
    /// The compass identifier has a real local focus renderer below.
    pub refresh_bottom_hud: fn(&mut ProcessTextAccess<'_>, &mut Owner) -> Result<(), String>,
    /// 48dc40 event73; a synchronous semantic effect, NOT visibility=true.
    pub event73: Option<fn(&mut ProcessTextAccess<'_>, &mut Owner) -> Result<(), String>>,
}

pub struct State {
    mode: i32,
    shadow: i32,
    previous: i32,
    saved: [u8; 3],
    compass: u32,
    consumers: Option<Consumers>,
}
impl State {
    /// Explicit process UI values from an actual install boundary. No Default,
    /// raw-camera-kind inference, scene-derived pose, or UI visibility reset.
    pub fn new(
        mode: i32,
        shadow: i32,
        previous: i32,
        saved: [u8; 3],
        consumers: Option<Consumers>,
    ) -> Self {
        Self {
            mode,
            shadow,
            previous,
            saved,
            compass: if mode == 3 { 1143 } else { 1142 },
            consumers,
        }
    }
    pub fn compass_identifier(&self) -> u32 {
        self.compass
    }
    pub fn mode(&self) -> i32 {
        self.mode
    }
    pub fn shadow(&self) -> i32 {
        self.shadow
    }
    pub fn previous(&self) -> i32 {
        self.previous
    }
    pub fn saved_visibility(&self) -> [u8; 3] {
        self.saved
    }
}

fn state<'v>(view: &'v mut ProcessTextAccess<'_>) -> Result<&'v mut State, String> {
    view.view
        .camera_navigation
        .as_mut()
        .ok_or_else(|| "Navigation UI mode state has no actual installation producer".into())
}
fn consumers(view: &mut ProcessTextAccess<'_>) -> Result<Consumers, String> {
    state(view)?.consumers.ok_or_else(||
        "Navigation blocked: actual A..H panel visibility, three group reset/input consumers, 41f290 toolbar, 4c39c0 picked-panel and 425400 compass caption are unjoined".into())
}

fn tutorial_session(view: &ProcessTextAccess<'_>) -> Result<bool, String> {
    Ok(view
        .view
        .camera_ui
        .as_ref()
        .ok_or("Event73 has no current admission owner")?
        .admission()
        .tutorial_session)
}

/// Root's direct PE finding:48dc40 returns at48dc66 if selectedlevel86d338
/// is null or selectedlevel+85c!=1. Admission is sampled at this boundary from
/// that actual tutorial byte. Ordinary false is a SOURCE-SUPPRESSED branch;
/// it is not inferred from missing scripts or represented by a fake callback.
pub fn event73(view: &mut ProcessTextAccess<'_>, owner: &mut Owner) -> Result<(), String> {
    if !tutorial_session(view)? {
        return Ok(());
    }
    let call = consumers(view)?.event73
        .ok_or("Tutorial event73 requires actual synchronous 4d5fb0/Lua consumer; queued FreshRequest is forbidden")?;
    call(view, owner)
}

/// Install THIS as UiJoins.primary_clear for navigation, so an unimplemented
/// route fails before the clear, raw kind read, or either raw-record copy.
/// The independently usable focus::primary_clear is the actual43e370 body.
pub fn primary_clear(
    view: &mut Inspection,
    assets: &mut Assets,
    owner: &mut Owner,
) -> Result<(), String> {
    {
        let access = ProcessTextAccess {
            view,
            resources: &mut assets.ui_text,
        };
        super::ui_graph::preflight(&access, owner)?;
        let bindings = access
            .view
            .camera_navigation
            .as_ref()
            .ok_or("Navigation state absent")?
            .consumers
            .ok_or("Navigation consumers absent")?;
        if tutorial_session(&access)? && bindings.event73.is_none() {
            return Err("Navigation requires synchronous tutorial event73 consumer".into());
        }
    }
    super::focus::primary_clear(view, assets, owner)
}
fn visible(
    view: &mut ProcessTextAccess<'_>,
    object: Object,
    owner: &mut Owner,
) -> Result<u8, String> {
    let call = consumers(view)?.visible;
    call(view, object, owner)
}
fn visibility(
    view: &mut ProcessTextAccess<'_>,
    object: Object,
    shown: bool,
    owner: &mut Owner,
) -> Result<(), String> {
    let call = consumers(view)?.visibility;
    call(view, object, shown, owner)
}
fn group(
    view: &mut ProcessTextAccess<'_>,
    group: Group,
    enabled: bool,
    owner: &mut Owner,
) -> Result<(), String> {
    let reset = consumers(view)?.reset_group;
    reset(view, group, owner)?;
    let enable = consumers(view)?.enable_group;
    enable(view, group, enabled, owner)
}
fn mode_header(view: &mut ProcessTextAccess<'_>, requested: i32) -> Result<(), String> {
    let state = state(view)?;
    let previous = state.mode;
    state.mode = requested;
    state.shadow = requested;
    state.previous = previous;
    Ok(())
}

pub fn apply(
    view: &mut Inspection,
    resources: &mut super::ui_text_resources::UiTextResources,
    phase: NavigationUi,
    owner: &mut Owner,
) -> Result<(), String> {
    apply_text(&mut ProcessTextAccess { view, resources }, phase, owner)
}

/// Context supplies the exact raw-copy/setter order.
/// This callback implements only its named UI phases; it never calls the raw
/// strategic helper a second time or adds a late numeric fallback.
fn apply_text(
    view: &mut ProcessTextAccess<'_>,
    phase: NavigationUi,
    owner: &mut Owner,
) -> Result<(), String> {
    consumers(view)?;
    match phase {
        NavigationUi::EnterMode3BeforeCamera => {
            mode_header(view, 3)?;
            for object in [Object::G, Object::H, Object::B] {
                if visible(view, object, owner)? == 1 {
                    visibility(view, object, false, owner)?;
                }
            }
            visibility(view, Object::A, true, owner)?;
            // Read previous AFTER enable(A), then reread visibility suppliers
            // after every callback. A/B are saved at this point, not on entry.
            if state(view)?.previous == 0 {
                state(view)?.saved = [0; 3];
                if visible(view, Object::A, owner)? != 0 {
                    state(view)?.saved[0] = 1;
                }
                if visible(view, Object::B, owner)? != 0 {
                    state(view)?.saved[2] = 1;
                }
            }
        }
        NavigationUi::EnterMode3AfterCamera => {
            group(view, Group::Secondary, false, owner)?;
            group(view, Group::Primary, true, owner)?;
            group(view, Group::Tertiary, false, owner)?;
            visibility(view, Object::A, false, owner)?;
            visibility(view, Object::F, false, owner)?;
            visibility(view, Object::D, true, owner)?;
            visibility(view, Object::C, true, owner)?;
        }
        NavigationUi::ReturnMode0ThenToolbar1 => {
            // The raw secondary->primary copy has already completed.
            mode_header(view, 0)?;
            if state(view)?.previous == 3 {
                if state(view)?.saved[0] != 0 {
                    visibility(view, Object::A, true, owner)?;
                }
                // The second saved byte is read AFTER A's callback.
                if state(view)?.saved[2] != 0 {
                    visibility(view, Object::B, true, owner)?;
                }
            }
            visibility(view, Object::C, false, owner)?;
            visibility(view, Object::D, false, owner)?;
            visibility(view, Object::E, true, owner)?;
            visibility(view, Object::F, true, owner)?;
            group(view, Group::Secondary, false, owner)?;
            group(view, Group::Primary, true, owner)?;
            group(view, Group::Tertiary, false, owner)?;
            let call = consumers(view)?.toolbar_mode;
            call(view, 1, owner)?;
        }
        NavigationUi::RestorePickedPanel => {
            let call = consumers(view)?.restore_picked_panel;
            call(view, owner)?;
        }
        NavigationUi::RefreshThenShowGroup73 => {
            let call = consumers(view)?.refresh_bottom_hud;
            call(view, owner)?;
            //425400 queries live UI mode and changes only E compass caption.
            // The focus producer reads this label for the actual compass child.
            let mode = state(view)?.mode;
            state(view)?.compass = if mode == 3 { 1143 } else { 1142 };
            event73(view, owner)?;
        }
    }
    Ok(())
}

///Actual UI-mode command at native startup or timing400 restore boundary.
///Root must execute this BEFORE later camera section restoration, in the
///original section order. This is not a cold-load camera-kind inference.
///It deliberately requires the reached existing consumers.
fn install_mode_text(
    view: &mut ProcessTextAccess<'_>,
    owner: &mut Owner,
    mode: i32,
) -> Result<(), String> {
    match mode {
        3 => {
            apply_text(view, NavigationUi::EnterMode3BeforeCamera, owner)?;
            owner.enter_strategic_fields()?;
            apply_text(view, NavigationUi::EnterMode3AfterCamera, owner)
        }
        0 => {
            //Original4b5370 mode0 does not contain graph-toggle's toolbar1.
            mode_header(view, 0)?;
            if state(view)?.previous == 3 {
                if state(view)?.saved[0] != 0 {
                    visibility(view, Object::A, true, owner)?;
                }
                if state(view)?.saved[2] != 0 {
                    visibility(view, Object::B, true, owner)?;
                }
            }
            for object in [Object::C, Object::D] {
                visibility(view, object, false, owner)?;
            }
            for object in [Object::E, Object::F] {
                visibility(view, object, true, owner)?;
            }
            group(view, Group::Secondary, false, owner)?;
            group(view, Group::Primary, true, owner)?;
            group(view, Group::Tertiary, false, owner)
        }
        1 => {
            mode_header(view, 1)?;
            group(view, Group::Secondary, false, owner)?;
            group(view, Group::Primary, false, owner)?;
            group(view, Group::Tertiary, true, owner)
        }
        4 => {
            mode_header(view, 4)?;
            let previous = state(view)?.previous;
            state(view)?.shadow = match previous {
                1 => 1,
                3 => 3,
                _ => 0,
            };
            group(view, Group::Secondary, true, owner)?;
            group(view, Group::Primary, false, owner)?;
            group(view, Group::Tertiary, false, owner)
        }
        _ => Err(format!(
            "Finite install has no recovered mode{mode} command"
        )),
    }
}

///Same caller-owned process manager at startup/timing400; no factory fallback.
pub fn install_mode_command(
    view: &mut Inspection,
    resources: &mut super::ui_text_resources::UiTextResources,
    owner: &mut Owner,
    mode: i32,
) -> Result<(), String> {
    install_mode_text(&mut ProcessTextAccess { view, resources }, owner, mode)
}
