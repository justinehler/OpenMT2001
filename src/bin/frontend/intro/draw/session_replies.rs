//! `Intro::poll_session_replies`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    /// Returns whether the fresh city should be refreshed after the result
    /// screen has had its turn.
    pub(super) fn poll_session_replies(&mut self) -> Result<bool, String> {
        let mut refresh = false;
        let mut tutorial_completed = false;
        if let Some(view) = &mut self.inspection {
            if let Some(receiver) = &view.save_pending {
                let outcome = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Session closed before saving".into()))
                    }
                };
                if let Some(result) = outcome {
                    let message = match &result {
                        Ok(path) => {
                            eprintln!("Saved V2 game: {}", path.display());
                            format!(
                                "Saved: {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            )
                        }
                        Err(error) => {
                            eprintln!("Save failed: {error}");
                            format!("Save failed: {error}")
                        }
                    };
                    view.game_options.set_message(message);
                    if let Some(root) = view.save_profile_root.as_deref() {
                        view.save_dialog.saved(result, root);
                    }
                    view.save_pending = None;
                }
            }
            if let Some(receiver) = &view.save_delete_pending {
                let outcome = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Session closed before deletion".into()))
                    }
                };
                if let Some(result) = outcome {
                    if let Some(root) = view.save_profile_root.as_deref() {
                        view.save_dialog.deleted(result, root);
                    }
                    view.save_delete_pending = None;
                }
            }
            if let Some(hit) = view.selected {
                let selected = (
                    self.assets.selection.blocks[hit.category].id as i32,
                    hit.building
                        .and_then(|i| view.scene.inspection_buildings[i].runtime_slot)
                        .map_or(-1, |i| i as i32),
                );
                if view.card_selection != Some(selected)
                    && let Some(sender) = &view._fresh_session
                {
                    sender
                        .send(FreshRequest::Card(
                            crate::fresh_city::cards::Action::Select {
                                category: selected.0,
                                building: selected.1,
                            },
                        ))
                        .map_err(|e| e.to_string())?;
                    view.card_selection = Some(selected);
                }
            }
            if let Some(reply) = &view.game_options.quit_reply {
                match reply.try_recv() {
                    Ok(requests) => {
                        view.game_options.quit_reply = None;
                        view.game_options.quit_prompt.finish_dismiss(vec![]);
                        for request in requests {
                            view.prompts.submit(request);
                        }
                        view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("Abandonment session disconnected".into());
                    }
                }
            }
            for response in view.game_options.quit_prompt.take_dialog_replies() {
                let sender = view
                    ._fresh_session
                    .as_ref()
                    .ok_or("Abandonment has no live session")?;
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::HumanDialog(response, reply))
                    .map_err(|e| e.to_string())?;
                view.game_options.quit_reply = Some(answer);
            }
            if let Some(reply) = &view.prompt_reply {
                match reply.try_recv() {
                    Ok(requests) => {
                        view.prompt_reply = None;
                        view.prompts.finish_dismiss(requests);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("Prompt session disconnected".into());
                    }
                }
            }
            for response in view.prompts.take_dialog_replies() {
                view.reply_human_dialog(response)?;
            }
            if let Some(receiver) = &view.status_pending {
                match receiver.try_recv() {
                    Ok(mut status) => {
                        // Advance existing slots before admitting requests from
                        // this snapshot: they did not exist in earlier updates.
                        apply_prompt_updates(
                            &mut view.prompts,
                            view._fresh_session.as_ref(),
                            &mut view.prompt_reply,
                            status.prompt_updates,
                        )?;
                        if let Some(image) = &status.window_texture {
                            self.renderer.publish_window_texture(image.pixels.clone())?;
                            for index in view.scene.set_window_pixels(image.pixels.clone())? {
                                self.renderer.update_dynamic_vertices(
                                    &mut view.prepared,
                                    index,
                                    &view.scene.draws[index].borrowed(),
                                )?;
                            }
                            if let Some(sender) = &view._fresh_session {
                                sender
                                    .send(FreshRequest::WindowTexturePublished(image.revision))
                                    .map_err(|error| error.to_string())?;
                            }
                        }
                        crate::live_host::report_status(&status, view.live_status.as_ref());
                        view.stats_requested = status.stats.is_some();
                        tutorial_completed = status.tutorial_completed;
                        view.apply_status_camera_events(
                            &status,
                            &self.assets.selection,
                            &self.assets.navigation,
                        )?;
                        let created = !status.prompts.is_empty();
                        view.prompts.apply_requests(
                            std::mem::take(&mut status.prompts),
                            &status.prompt_closes,
                        );
                        if created {
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            view.mouse_buttons = [false; 2];
                        }
                        view.auction_notice.update(status.auction_notice.clone());
                        if let Some(planning) = &mut view.planning {
                            planning.set_tutorial_build_type(status.tutorial_build_type);
                            planning.set_tutorial_build_use(status.tutorial_build_use);
                        }
                        if !status.player_panels[0] {
                            view.planning = None;
                            view.planning_pending = None;
                            view.adjustment = None;
                            view.details_visible = false;
                        }
                        if !status.current_player_active {
                            view.planning = None;
                            view.planning_pending = None;
                        }
                        if view.selected.is_some_and(|selected| {
                            status
                                .auction_locked_categories
                                .contains(&self.assets.selection.blocks[selected.category].id)
                        }) {
                            //Original41ed10->4053a0/407cd0 cancels editing a
                            //category once its auction announcement is admitted.
                            view.planning = None;
                            view.planning_pending = None;
                            view.adjustment = None;
                        }
                        update_locator_financial(&mut view.locator, &status)?;
                        if let Some(locator) = &mut view.locator {
                            locator.refresh_scene(&view.scene);
                        }
                        if let (Some(locator), Some(rows)) =
                            (&mut view.locator, status.city_people_rows.take())
                        {
                            locator.apply_people_rows(rows);
                        }
                        if let (Some(comparison), Some(snapshot)) =
                            (&mut view.comparison, &status.comparison)
                        {
                            comparison.apply_live(snapshot);
                        }
                        view.pulse_state = Some(status.pulse_lights.clone());
                        view.smoke_state = Some(status.chimney_smoke.clone());
                        view.time_panel
                            .update(&self.assets.time_panel, &self.renderer, &status)?;
                        view.cash_arrow.update(
                            &self.assets.cash_arrow,
                            &self.renderer,
                            status.cash_arrow,
                        )?;
                        view.dust.update(
                            &self.assets.dust,
                            &self.renderer,
                            &status.demolition_points,
                            view.camera,
                            view.scene.camera,
                        )?;
                        if let Some(error) = &status.failure {
                            if let Some(state) = &mut view.camera_ui {
                                state.reject_save_captures(error);
                            }
                            if view.camera_capture_pending.take().is_some() {
                                view.save_dialog.busy = false;
                                view.save_dialog.message = Some(format!("Save failed: {error}"));
                            }
                        }
                        view.game_options.set_save_disabled(
                            !view.options_ack.ready()
                                || status.tutorial_session
                                || status.failure.is_some()
                                || view
                                    .camera_producer
                                    .as_ref()
                                    .is_some_and(|p| p.error().is_some()),
                        );
                        view.live_status = Some(status);
                        view.status_pending = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.status_pending = None;
                    }
                }
            }
            if view.status_pending.is_none()
                && view.last_status_query.elapsed().as_millis()
                    >= if view.auction.scene.is_some() || view.cash_arrow.active || view.dust.active
                    {
                        50
                    } else {
                        250
                    }
                && let Some(sender) = &view._fresh_session
            {
                let (reply, result) = mpsc::channel();
                sender
                    .send(FreshRequest::Status {
                        camera: view.camera.eye,
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                view.status_pending = Some(result);
                view.last_status_query = Instant::now();
            }
            if let Some(reply) = &view.environment_pending {
                match reply.try_recv() {
                    Ok(clock) => {
                        view.environment_pending = None;
                        apply_prompt_updates(
                            &mut view.prompts,
                            view._fresh_session.as_ref(),
                            &mut view.prompt_reply,
                            clock.prompt_updates,
                        )?;
                        view.time_panel.update_clock(
                            &self.assets.time_panel,
                            &self.renderer,
                            clock,
                        )?;
                        let (tick, hour, counter) = (clock.ticks, clock.hour, clock.fraction);
                        if tick != view.environment_tick
                            || view.environment_time != Some((hour, counter))
                        {
                            let mut changed =
                                view.scene.advance_environment(tick, view.camera.eye)?;
                            changed.extend(view.scene.advance_daylight(
                                hour,
                                counter,
                                view.camera,
                            )?);
                            if let Some(trees) = &mut view.trees {
                                changed.extend(trees.reframe(&mut view.scene, view.camera)?);
                            }
                            if let Some(people) = &view.people {
                                changed.extend(people.reframe(
                                    &mut view.scene,
                                    view.camera,
                                    u32::from(view.settings_menu.graphics.distance),
                                )?);
                            }
                            let distance = u32::from(view.settings_menu.graphics.distance);
                            if let Some(vehicles) = &view.vehicles {
                                changed.extend(vehicles.reframe(
                                    &mut view.scene,
                                    view.camera,
                                    distance,
                                )?);
                            }
                            if let Some(transport) = &view.transport {
                                changed.extend(transport.reframe(
                                    &mut view.scene,
                                    view.camera,
                                    distance,
                                )?);
                            }
                            let star_detail = view.normal_star_detail();
                            let changed = normal_city_scene_updates(
                                &mut view.scene,
                                view.camera,
                                star_detail,
                                changed,
                            )?;
                            for index in changed {
                                let draw = &view.scene.draws[index];
                                self.renderer.update_dynamic_vertices(
                                    &mut view.prepared,
                                    index,
                                    &draw.borrowed(),
                                )?;
                            }
                            let cameras = view
                                .scene
                                .draws
                                .iter()
                                .map(|draw| draw.view_projection.unwrap_or(view.scene.camera))
                                .collect::<Vec<_>>();
                            self.renderer.update_cameras(
                                &view.prepared,
                                &cameras,
                                view.scene.fog,
                            )?;
                            view.environment_tick = tick;
                            view.environment_time = Some((hour, counter));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => view.environment_pending = None,
                }
            }
            if view.environment_pending.is_none()
                && view.auction.scene.is_none()
                && view.result.is_none()
                && let Some(sender) = &view._fresh_session
            {
                let (reply, result) = mpsc::channel();
                sender
                    .send(FreshRequest::LogicTick(reply))
                    .map_err(|e| e.to_string())?;
                view.environment_pending = Some(result);
            }
            refresh = self.loading.is_none()
                && self.refresh_retry.error.is_none()
                && view.auction.scene.is_none()
                && view.adjustment.is_none()
                && view.planning.is_none()
                && view.planning_pending.is_none()
                && view.last_snapshot.elapsed() >= city_snapshot_interval(view.construction_active)
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.ticks != view.snapshot_tick);
        }
        if tutorial_completed {
            refresh = false;
            self.retire_inspection();
            if let Some(menu) = &mut self.menu {
                menu.refresh_campaign()?;
            }
            self.last_sample = None;
        }
        Ok(refresh)
    }
}

/// Dispatch expiry in the same draw pass that consumes the delivered updates.
/// Keep the queue slot until the worker replies, preserving callback admission.
fn apply_prompt_updates(
    prompts: &mut crate::prompt::State,
    sender: Option<&mpsc::Sender<FreshRequest>>,
    pending: &mut Option<mpsc::Receiver<Vec<crate::prompt::Request>>>,
    delivered: u64,
) -> Result<(), String> {
    for id in prompts.advance_to(delivered) {
        let sender = sender.ok_or("Prompt has no live session")?;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::DismissPrompt(id, Some(reply)))
            .map_err(|error| error.to_string())?;
        *pending = Some(answer);
    }
    for response in prompts.take_dialog_replies() {
        let sender = sender.ok_or("Human dialog has no live session")?;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::HumanDialog(response, reply))
            .map_err(|error| error.to_string())?;
        *pending = Some(answer);
    }
    Ok(())
}

#[cfg(test)]
mod modal_frame_tests {
    use super::*;
    use crate::{human_dialog::Dialog, prompt, time_panel::ClockSample};

    #[test]
    fn modal_expires_on_frame_sample_without_status_poll() {
        // Ordinary script messages and prompts use the same queue as native
        // dialogs. Tutorial records retain their disabled timeout policy.
        for (script, tutorial, buttons) in [
            (false, false, 0),
            (true, false, 0),
            (true, false, 1),
            (true, true, 0),
            (true, true, 1),
        ] {
            let (sender, requests) = mpsc::channel();
            let mut pending = None;
            let mut prompts = prompt::State::default();
            let mut sample = ClockSample {
                interface_ticks: 70,
                prompt_updates: 70,
                ticks: 0,
                year: 1930,
                hour: 9,
                fraction: 0,
                speed: 1,
            };
            apply_prompt_updates(
                &mut prompts,
                Some(&sender),
                &mut pending,
                sample.prompt_updates,
            )
            .unwrap();
            let mut request = if script {
                crate::live_host::script_request(
                    &[
                        openmt2001::lua4::Value::Number(1.),
                        openmt2001::lua4::Value::Number(2.),
                        openmt2001::lua4::Value::Number(17.),
                    ],
                    tutorial,
                    buttons != 0,
                )
                .unwrap()
            } else {
                prompt::Request {
                    script: None,
                    dialog: Some(Dialog {
                        context: 17,
                        context_written: true,
                        ..Default::default()
                    }),
                    title: 0,
                    body: 0,
                    dismissal_id: 0,
                    card: None,
                }
            };
            request.dialog.as_mut().unwrap().timeout_ticks = Some(2);
            prompts.submit(request);
            // Interface samples continue while normal modal updates are
            // paused/suppressed; only the delivered count ages the modal.
            for delivered in [70, 70, 71, 72, 72, 72] {
                sample.interface_ticks += 1;
                sample.prompt_updates = delivered;
                apply_prompt_updates(
                    &mut prompts,
                    Some(&sender),
                    &mut pending,
                    sample.prompt_updates,
                )
                .unwrap();
                assert!(prompts.visible());
                assert!(requests.try_recv().is_err());
                assert!(pending.is_none());
            }
            sample.interface_ticks += 1;
            sample.prompt_updates = 73; // N+1, with no Status ever requested.
            apply_prompt_updates(
                &mut prompts,
                Some(&sender),
                &mut pending,
                sample.prompt_updates,
            )
            .unwrap();
            if tutorial {
                assert!(prompts.visible());
                assert!(requests.try_recv().is_err());
                continue;
            }
            assert!(!prompts.visible(), "expiry frame no longer draws the modal");
            assert!(prompts.request().is_some(), "keep callback admission order");
            let FreshRequest::HumanDialog(response, reply) = requests.try_recv().unwrap() else {
                panic!("expiry did not dispatch a human dialog reply");
            };
            assert_eq!((response.context, response.button), (17, 0));
            assert!(requests.try_recv().is_err());
            // The worker's continuation can create a successor before the
            // old head is unlinked, without making the old modal visible.
            let mut next = prompts.request().unwrap();
            next.dialog.as_mut().unwrap().context = 29;
            reply.send(vec![next]).unwrap();
            prompts.finish_dismiss(pending.take().unwrap().try_recv().unwrap());
            assert!(prompts.visible());
            assert_eq!(prompts.request().unwrap().dialog.unwrap().context, 29);
        }
    }
}
