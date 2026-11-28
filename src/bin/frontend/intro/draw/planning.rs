//! `Intro::poll_planning`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn poll_planning(&mut self) -> Result<(), String> {
        let mut build_request = None;
        if let Some(view) = &mut self.inspection {
            if let Some(receiver) = &view.planning_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.planning_pending = None;
                        match result {
                            Ok(context) => {
                                if context.entry() == crate::build_panel::Entry::Exit {
                                    view.planning = None;
                                } else if matches!(
                                    context.entry(),
                                    crate::build_panel::Entry::Message {
                                        show_rights_action: true,
                                        ..
                                    }
                                ) {
                                    view.utility_report
                                        .open_property_rights(context.category, 0);
                                    view.details_visible = true;
                                    view.title_state = None;
                                } else if let Some(menu) = &self.menu {
                                    let mut planning = menu.planning_assets().open(context)?;
                                    planning.set_tutorial_build_type(
                                        view.live_status
                                            .as_ref()
                                            .map_or(-1, |status| status.tutorial_build_type),
                                    );
                                    planning.set_tutorial_build_use(
                                        view.live_status
                                            .as_ref()
                                            .map_or(999, |status| status.tutorial_build_use),
                                    );
                                    view.planning = Some(planning);
                                }
                            }
                            Err(error) => eprintln!("Planning unavailable: {error}"),
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.planning_pending = None;
                    }
                }
            }
            if let Some(planning) = &mut view.planning {
                let occupied = view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.event_list_visible)
                    && self.menu.as_ref().is_some_and(|menu| {
                        view.pointer.is_some_and(|point| {
                            menu.event_list_assets()
                                .contains(point.map(|v| v as f32 * 0.5))
                        })
                    });
                planning.pointer(view.pointer.filter(|_| !occupied));
                let block = self
                    .assets
                    .selection
                    .blocks
                    .iter()
                    .find(|b| b.id == planning.context.category)
                    .ok_or("Planning block missing")?;
                planning.project_grid(view.camera, block.center)?;
                if let Some(receiver) = &planning.placement_pending {
                    match receiver.try_recv() {
                        Ok(Ok(Some(choices))) => planning.apply_placement_choices(choices),
                        Ok(Ok(None)) => planning.close = true,
                        Ok(Err(error)) => {
                            planning.placement_pending = None;
                            planning.diagnostic = Some(error);
                            planning.dirty = true;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            planning.placement_pending = None;
                            planning.diagnostic =
                                Some("Build placement session disconnected".into());
                            planning.dirty = true;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }
                if let Some(receiver) = &planning.pending {
                    match receiver.try_recv() {
                        Ok(result) => {
                            planning.pending = None;
                            match result {
                                Ok(None) => planning.close = true,
                                Ok(Some(quote)) if quote.submitted => {
                                    // The native pending-world entry begins at scale zero.
                                    // Show its growth before the idle one-second refresh.
                                    view.construction_active = true;
                                    planning.close = true;
                                }
                                Ok(Some(quote)) => {
                                    if planning.request().ok() == planning.quote_request {
                                        planning.diagnostic = if quote.category_locked {
                                            Some("This block is locked".into())
                                        } else if !quote.placement_allowed {
                                            Some("This lot is occupied".into())
                                        } else if !quote.affordable {
                                            Some("Insufficient funds".into())
                                        } else if quote.preview.is_none() {
                                            Some("No building model matches".into())
                                        } else {
                                            None
                                        };
                                        planning.reset_preview();
                                        planning.quote = Some(quote);
                                    } else {
                                        planning.needs_quote = true;
                                    }
                                }
                                Err(error) => planning.diagnostic = Some(error),
                            }
                            planning.dirty = true;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            planning.pending = None;
                            planning.diagnostic = Some("Session disconnected".into());
                            planning.dirty = true;
                        }
                    }
                }
                if !planning.close
                    && planning.pending.is_none()
                    && (planning.needs_quote || planning.submit)
                {
                    match planning.request() {
                        Ok(form) => {
                            planning.quote_request = Some(form.clone());
                            build_request = Some((form, planning.submit));
                        }
                        Err(error) => {
                            planning.diagnostic = Some(error);
                            planning.dirty = true;
                        }
                    }
                    planning.needs_quote = false;
                    planning.submit = false;
                }
                if planning.close {
                    view.planning = None;
                }
            }
        }
        if let Some((form, submit)) = build_request {
            let result = self.request_build_form(form, submit);
            if let Some(planning) = self
                .inspection
                .as_mut()
                .and_then(|view| view.planning.as_mut())
            {
                match result {
                    Ok(receiver) => planning.pending = Some(receiver),
                    Err(error) => planning.diagnostic = Some(error),
                }
                planning.dirty = true;
            }
        }
        Ok(())
    }
}
