//! `Intro::sync_management_selection`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn sync_management_selection(&mut self) -> Result<(), String> {
        if let Some(view) = &mut self.inspection {
            let selected = view
                .selected
                .and_then(|h| h.building)
                .and_then(|i| view.scene.inspection_buildings.get(i))
                .filter(|b| building_has_management_link(b))
                .and_then(|b| b.runtime_slot);
            view.operations.select(selected);
            //Original report blink clock822d40 advances with25Hz logic ticks.
            view.operations.logic_tick = view.live_status.as_ref().map_or(0, |s| s.ticks);
            view.operations.poll();
            if let Some(menu) = &self.menu {
                view.operations.pointer(
                    menu.business_assets(),
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                );
            }
            if view.operations.refresh
                && view.operations.pending.is_none()
                && let (Some(building), Some(sender)) = (selected, &view._fresh_session)
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::InspectOperations {
                        building,
                        bounds: self
                            .menu
                            .as_ref()
                            .ok_or("building menu assets missing")?
                            .business_assets()
                            .bounds(),
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                view.operations.pending = Some(answer);
                view.operations.refresh = false;
            }
            if let Some(action) = view.operations.action.take()
                && let (Some(building), Some(sender)) = (selected, &view._fresh_session)
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::BusinessOperation {
                        building,
                        action,
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                view.operations.action_pending = Some(answer);
            }
            if let Some(mode) = view.operations.navigation.take() {
                if mode == 1
                    && let (Some(i), Some(menu)) =
                        (view.selected.and_then(|h| h.building), &self.menu)
                {
                    view.adjustment = menu.adjustment_assets().open_for_owner(
                        &view.scene,
                        i,
                        view.live_status.as_ref().and_then(|s| s.business_owner),
                    )?;
                }
                if mode == 5
                    && let Some(hit) = view.selected
                {
                    //40889b restores the original block camera on return.
                    view.focus_category(hit.category, &self.assets.selection);
                }
            }
        }
        Ok(())
    }
}
