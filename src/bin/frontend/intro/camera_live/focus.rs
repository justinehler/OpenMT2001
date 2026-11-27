//! Finite 590d98 owner: immutable Navigator category descriptors and the
//! already implemented identifier-only toolbar/locator children.
//! This is independent of Inspection.selected and its information/markers.
use super::{
    super::{Assets, Inspection},
    owner::Owner,
};
use openmt2001::render::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Primary {
    category: usize,
    identifier: u32,
}

/// Widget identity is separate from its mutable caption identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildKey {
    Toolbar(usize),
    Locator(u32),
    Compass,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Child {
    key: ChildKey,
    identifier: u32,
}

/// 43e040 initializes hidden, dwell0, no primary/child, empty text.
/// No related business is supported by this finite descriptor producer.
#[derive(Default)]
pub struct Focus {
    visible: bool,
    dwell: i32,
    primary: Option<Primary>,
    child: Option<Child>,
    text: Option<u32>,
}
impl Focus {
    /// 43e0a0's actually reached identifier-only branches. Reset text and
    /// ticks, prefer retained child, otherwise read the category descriptor.
    /// The font/layout consumer measures this identifier before publishing.
    fn refresh(&mut self) {
        self.dwell = 0;
        self.text = None;
        if let Some(child) = self.child {
            self.text = Some(child.identifier);
        } else if let Some(primary) = self.primary {
            self.text = Some(primary.identifier);
        } else {
            self.visible = false;
        }
    }
    /// 43e2c0. A repeat of the same descriptor does not clear a child.
    /// alternate1 is the actual Navigator producer's immediate-caption path.
    pub(crate) fn show_category(&mut self, category: usize, identifier: u32, alternate1: bool) {
        if self.primary.is_some_and(|p| p.category == category) {
            return;
        }
        self.visible = true;
        self.primary = Some(Primary {
            category,
            identifier,
        });
        self.child = None;
        self.refresh();
        if alternate1 {
            self.dwell = 30;
        }
    }
    /// 43e370: primary+8=0; child+12 is retained, even if already primary-null.
    /// The related slot has no reader on this branch and is not synthesized.
    pub fn clear_primary(&mut self) {
        self.primary = None;
        if self.child.is_none() {
            self.visible = false;
        }
        self.refresh();
    }
    /// 43e310, finite nonempty identifier child. Distinct widget identity
    /// resets dwell even when it happens to carry the same identifier.
    pub fn show_child(
        &mut self,
        key: ChildKey,
        identifier: u32,
        force1: bool,
    ) -> Result<(), String> {
        if identifier == 0 {
            return Err("Identifier0/inline focus child needs its real text producer".into());
        }
        if self.child.is_some_and(|c| c.key == key) && !force1 {
            // 425400 changes the caption of an existing compass widget.
            // This is the finite set-secondary-identifier/refresh path,
            // not another child show that silently ignores the new label.
            if self.child.is_some_and(|c| c.identifier != identifier) {
                self.child.as_mut().unwrap().identifier = identifier;
                self.refresh();
            }
            return Ok(());
        }
        self.visible = true;
        self.child = Some(Child { key, identifier });
        self.refresh();
        if force1 {
            self.dwell = 30;
        }
        Ok(())
    }
    ///43e310 with actual F node identifier0/empty text; unlike a nonempty
    ///secondary this disables the label even if the primary is retained.
    pub(crate) fn show_empty_panel_child(&mut self) {
        self.visible = false;
        self.child = None;
        self.refresh();
    }
    ///43e390's conditional refresh preserves a primary category caption.
    pub fn clear_child(&mut self) {
        if self.child.take().is_some() {
            if self.primary.is_none() {
                self.visible = false;
            }
            self.refresh();
        }
    }
    /// 43e3b0 is signed32 wrap, not saturation or elapsed wall time.
    pub fn advance(&mut self) {
        self.dwell = self.dwell.wrapping_add(1);
    }
    pub(crate) fn retained_text_identifier(&self) -> Option<u32> {
        self.text
    }
    pub(crate) fn retained_primary_identifier(&self) -> Option<u32> {
        self.primary.map(|value| value.identifier)
    }
    pub fn caption(&self) -> Option<u32> {
        (self.visible && self.dwell > 25)
            .then_some(self.text)
            .flatten()
    }
}

/// Concrete UiJoins.primary_clear operation. Its finite refresh is local
/// state/text; it has no callback into a worker, timer, or camera setter.
/// Owner is available for future *recovered* reentrant effects; none are
/// fabricated for these immutable descriptor/identifier branches.
pub fn primary_clear(
    view: &mut Inspection,
    assets: &Assets,
    _owner: &mut Owner,
) -> Result<(), String> {
    view.camera_focus.clear_primary();
    // Every clear refresh resets dwell, so any previously prepared draw is
    // temporarily hidden even if the child text survives in the owner.
    view.tooltip = None;
    view.tooltip_state = None;
    if let Some(identifier) = view.camera_focus.text {
        // 43e0a0 finishes the retained child text's real font measurement
        // before navigation reads raw kind. No render/worker query is used.
        assets.tooltip.measure_focus_identifier(identifier)?;
    }
    Ok(())
}

/// Actual finite producers + existing font/GPU consumer. Replace the old
/// navigator_tooltip/IconDwell block with this at that same interface pass.
/// Root must keep exactly one advance per chosen interface update boundary.
pub fn present(
    view: &mut Inspection,
    assets: &Assets,
    menu: Option<&crate::menu::Menu>,
    renderer: &Renderer,
) -> Result<(), String> {
    //F43d290 already produced/cleared the primary through its SAME owner.
    //Never pick immutable Board/Scene again in this presentation pass.
    let navigator = view
        .ui_graph
        .as_ref()
        .filter(|g| g.body(super::ui_graph::PanelId::F))
        .and_then(|g| g.upper.as_ref())
        .and_then(|s| s.focus_category().map(|i| (i, s.pointer.xy)));
    let locator = if navigator.is_none()
        && view.strategic
        && !view.game_options.open
        && view
            .ui_graph
            .as_ref()
            .is_none_or(|graph| graph.input(super::ui_graph::PanelId::C))
        && view.prompts.request().is_none()
        && view.adjustment.is_none()
        && view.planning.is_none()
    {
        view.pointer.and_then(|point| {
            view.locator.as_ref().and_then(|locator| {
                menu.and_then(|menu| locator.icon_tooltip_at(menu.locator_assets(), point))
            })
        })
    } else {
        None
    };
    let toolbar = menu.and_then(|menu| view.toolbar_target(menu, &assets.selection.blocks));
    let child = if let Some(action) = toolbar {
        Some((ChildKey::Toolbar(action.index()), action.tooltip()))
    } else if let Some(id) = locator {
        Some((ChildKey::Locator(id), id))
    } else if view.over_navigation() {
        // Only an explicitly installed UI mode state supplies this label.
        // Raw camera kind and a guessed/default mode are never consulted.
        view.camera_navigation
            .as_ref()
            .map(|state| (ChildKey::Compass, state.compass_identifier()))
    } else {
        None
    };
    match child {
        Some((key, id)) => view.camera_focus.show_child(key, id, false)?,
        None => view.camera_focus.clear_child(),
    }
    if let Some(identifier) = view.camera_focus.text {
        assets.tooltip.measure_focus_identifier(identifier)?;
    }
    //43e3b0 advances once at actual central tail, never per rendered frame.
    let point = navigator
        .map(|(_, point)| point)
        .or_else(|| view.pointer.map(|point| point.map(|v| v / 2)));
    let next = view.camera_focus.caption().zip(point);
    if next != view.tooltip_state {
        let prepared = next
            .map(|(identifier, point)| assets.tooltip.prepare(renderer, identifier, point))
            .transpose()?;
        view.tooltip = prepared;
        view.tooltip_state = next;
    }
    Ok(())
}
