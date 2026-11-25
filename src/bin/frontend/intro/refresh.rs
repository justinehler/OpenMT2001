//! Scene-refresh owner transaction. Preparation may change only the detached
//! destination presentation; rejection keeps every retained owner serviceable.
//! This is not an atomic camera/world cut and does not activate the UI graph.
use super::{camera_live, Inspection};
use crate::{business_adjustment::{self, RefreshScene}, locator::Locator};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Retry {
    pub(super) error: Option<String>,
    next_attempt: Option<Instant>,
}
impl Retry {
    pub(super) fn reject(&mut self, error: String) {
        self.error = Some(error);
        self.next_attempt = Some(Instant::now() + Duration::from_secs(1));
    }
    pub(super) fn due(&self, now: Instant) -> bool {
        self.next_attempt.is_some_and(|at| now >= at)
    }
    pub(super) fn requested(&mut self) { self.next_attempt = None; }
    pub(super) fn complete(&mut self) { *self = Self::default(); }
    /// The existing Inspect protocol, also used for a locally rejected refresh.
    /// A send rejection is retained as a retry error, never converted to success.
    pub(super) fn request(&mut self, sender: &std::sync::mpsc::Sender<super::FreshRequest>,
        camera: openmt2001::flyby::CameraSample,
    ) -> Result<std::sync::mpsc::Receiver<Result<super::LoadedInspection,String>>,String> {
        let (reply,result)=std::sync::mpsc::channel();
        match sender.send(super::FreshRequest::Inspect { camera,reply }) {
            Ok(()) => { self.requested(); Ok(result) }
            Err(error) => { let error=error.to_string(); self.reject(error.clone()); Err(error) }
        }
    }
}

/// Every city-exit route uses this same cancellation/retirement boundary.
/// Initial loads are independent of a refresh and keep their reply receiver.
pub(super) fn retire(retry:&mut Retry,refreshing:&mut bool,
    loading:&mut Option<std::sync::mpsc::Receiver<Result<super::LoadedInspection,String>>>,
    endpoint:Option<&mut Option<camera_live::endpoint::Endpoint>>,
    parked:&mut Option<camera_live::owner::Owner>)->Result<(),String> {
    retry.complete();
    if *refreshing { *loading=None; *refreshing=false; }
    if let Some(endpoint)=endpoint.and_then(|slot|slot.take()) {
        let owner=endpoint.into_owner(); // pool retirement precedes inbox wake
        let error=owner.property_lifecycle_error().map(str::to_owned);
        *parked=Some(owner);
        if let Some(error)=error{return Err(error);}
    }
    Ok(())
}

/// Borrow the production owners without constructing an Inspection's GPU
/// resources. Tests use these SAME concrete owners, preparation and commit.
pub(super) struct Owners<'a> {
    pub(super) locator: &'a mut Option<Locator>,
    pub(super) adjustment: &'a mut Option<business_adjustment::State>,
    pub(super) graph: &'a mut Option<camera_live::ui_graph::UiGraphState>,
    pub(super) ui: &'a mut Option<camera_live::State>,
    pub(super) navigation: &'a mut Option<camera_live::navigation::State>,
    pub(super) focus: &'a mut camera_live::focus::Focus,
    pub(super) endpoint: &'a mut Option<camera_live::endpoint::Endpoint>,
    pub(super) navigation_click_latched: &'a mut bool,
}
impl<'a> Owners<'a> {
    pub(super) fn of(view: &'a mut Inspection) -> Self {
        Self { locator: &mut view.locator, adjustment: &mut view.adjustment,
            graph: &mut view.ui_graph, ui: &mut view.camera_ui,
            navigation: &mut view.camera_navigation, focus: &mut view.camera_focus,
            endpoint: &mut view.camera_endpoint,
            navigation_click_latched: &mut view.navigation_click_latched }
    }
}

pub(super) struct Inputs<'a> {
    pub(super) previous_buildings: &'a [openmt2001::render_scene::InspectionBuilding],
    pub(super) next_scene: RefreshScene<'a>,
    pub(super) locator_extents: [usize; 2],
    pub(super) business_owner: Option<u8>,
}
pub(super) struct Prepared {
    adjustment: Option<business_adjustment::PreparedRefresh>,
}

pub(super) fn prepare(previous: &Owners<'_>, next: &mut Owners<'_>,
    inputs: Inputs<'_>, adjustment_assets: &business_adjustment::Assets,
    retry: &mut Retry,
) -> Result<Prepared, String> {
    let result: Result<Prepared, String> = (|| {
        if next.graph.is_some() { return Err("Refresh must not construct a replacement graph".into()); }
        if next.endpoint.is_some() { return Err("Refresh must not replace the retained endpoint".into()); }
        if next.adjustment.is_some() || next.navigation.is_some()
            || next.ui.is_some() && previous.ui.is_some() {
            return Err("Refresh constructed a replacement retained UI owner".into());
        }
        if let Some(g) = previous.graph.as_ref() {
            camera_live::strategic_panel::check_supplier_owner(previous.locator.as_ref(),
                &g.world.colors, g.panels.get(&camera_live::ui_graph::PanelId::C)
                    .map(|p| p.children.as_slice()), None, None)?;
            if next.locator.is_none() { return Err("Refresh C destination projection shell absent".into()); }
        }
        if let Some(old) = previous.locator.as_ref() {
            if old.has_strategic() && next.locator.is_none() {
                return Err("Refresh actual C destination projection shell absent".into());
            }
            if let Some(locator) = next.locator.as_mut() {
                locator.prepare_strategic_refresh(old, inputs.next_scene.buildings)?;
                if !old.has_strategic() {
                    // Legacy component-classification route also completes its
                    // financial report and scroll remapping BEFORE any take.
                    locator.retain_refresh_ui_from(old, inputs.locator_extents,
                        inputs.previous_buildings, inputs.next_scene.buildings)?;
                }
            }
        }
        let adjustment = previous.adjustment.as_ref().map(|state|
            adjustment_assets.prepare_refresh(inputs.next_scene,
                inputs.previous_buildings, state, inputs.business_owner))
            .transpose()?.flatten();
        Ok(Prepared { adjustment })
    })();
    if let Err(error) = &result { retry.reject(error.clone()); }
    result
}

/// Called immediately after successful preparation, with no intervening UI
/// callbacks or supplier publication. There is no Result or decoder here.
pub(super) fn commit(previous: &mut Owners<'_>, next: &mut Owners<'_>,
    prepared: Prepared, retry: &mut Retry,
) {
    if let (Some(locator), Some(old)) = (next.locator.as_mut(), previous.locator.as_mut()) {
        locator.commit_strategic_refresh(old);
    }
    *next.graph = previous.graph.take();
    *next.navigation_click_latched = *previous.navigation_click_latched;
    if previous.ui.is_some() { *next.ui = previous.ui.take(); }
    *next.navigation = previous.navigation.take();
    *next.focus = std::mem::take(previous.focus);
    *next.endpoint = previous.endpoint.take();
    if let Some(prepared) = prepared.adjustment {
        let mut adjustment = previous.adjustment.take().expect("prepared actual adjustment owner");
        prepared.commit(&mut adjustment);
        *next.adjustment = Some(adjustment);
    } else {
        // The stable building/business disappeared: close only at commit.
        *previous.adjustment = None;
        *next.adjustment = None;
    }
    retry.complete();
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
