//! `impl Intro`: draw_result .. draw, split out of `intro.rs` (#99).
use super::*;
#[path = "draw/adjustment.rs"]
mod adjustment;
#[path = "draw/auction.rs"]
mod auction;
#[path = "draw/camera_requests.rs"]
mod camera_requests;
#[path = "draw/camera_worker.rs"]
mod camera_worker;
#[path = "draw/inspection.rs"]
mod inspection;
#[path = "draw/loading.rs"]
mod loading;
#[path = "draw/planning.rs"]
mod planning;
#[path = "draw/selection.rs"]
mod selection;
#[path = "draw/session_replies.rs"]
mod session_replies;
#[path = "draw/start_routes.rs"]
mod start_routes;
#[path = "draw/world_sessions.rs"]
mod world_sessions;

impl Intro {
    pub(super) fn draw_result(&mut self, presenter: &Presenter) -> Result<bool, String> {
        let Some(view) = self.inspection.as_mut() else {
            return Ok(false);
        };
        if view.result.is_none()
            && let Some(review) = view.live_status.as_ref().and_then(|s| s.result_review)
        {
            match review {
                crate::live_host::ResultReview::Statistics => {
                    self.draw_frontend(presenter, Some(review))?;
                    return Ok(true);
                }
                crate::live_host::ResultReview::Menu(target) => {
                    self.retire_inspection();
                    if let Some(menu) = &mut self.menu {
                        menu.finish_result(target)?;
                    }
                    return Ok(false);
                }
            }
        }
        if view.result.is_none()
            && view.result_pending.is_none()
            && view
                .live_status
                .as_ref()
                .is_some_and(|status| status.result_request.is_some())
        {
            let sender = view
                ._fresh_session
                .as_ref()
                .ok_or("Result has no retained worker")?;
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::ResultState { reply })
                .map_err(|e| e.to_string())?;
            view.result_pending = Some(answer);
        }
        if let Some(receiver) = &view.result_pending {
            match focus::poll_result(receiver, view.music_admission.as_ref()) {
                Ok(result) => {
                    view.result_pending = None;
                    let snapshot = result?;
                    let state = snapshot.presentation;
                    view.environment_pending = None;
                    view.environment_tick = snapshot.tick;
                    if view.environment_time != Some((snapshot.hour, snapshot.counter)) {
                        let mut changed = view.scene.advance_daylight(
                            snapshot.hour,
                            snapshot.counter,
                            view.camera,
                        )?;
                        if let Some(trees) = &mut view.trees {
                            changed.extend(trees.reframe(&mut view.scene, view.camera)?);
                        }
                        for index in changed {
                            let draw = &view.scene.draws[index];
                            self.renderer.update_dynamic_surface(
                                &mut view.prepared,
                                index,
                                &draw.borrowed(),
                                draw.view_projection.unwrap_or(view.scene.camera),
                                view.scene.fog,
                            )?;
                        }
                        for (index, draw) in view.scene.draws.iter().enumerate() {
                            self.renderer.update_camera(
                                &view.prepared,
                                index,
                                draw.view_projection.unwrap_or(view.scene.camera),
                                view.scene.fog,
                            )?;
                        }
                        view.environment_time = Some((snapshot.hour, snapshot.counter));
                    }
                    let flyby = state
                        .flyby
                        .as_ref()
                        .ok_or("Result script did not set flyby")?;
                    let name = flyby
                        .rsplit('/')
                        .next()
                        .ok_or("Result flyby has no filename")?;
                    let path = self
                        .assets
                        .inputs
                        .installer
                        .join("Standard/Cut_scenes")
                        .join(name);
                    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                    let track = Flyby::parse(&bytes)?;
                    let samples = (1..track.frame_count())
                        .map(|frame| track.sample(frame))
                        .collect::<Result<Vec<_>, _>>()?;
                    // 4b3230 submits each owned block's four token quads during
                    // the same city draw as the result flyby. Keep their scene
                    // depth and camera, ahead of the actor/title passes.
                    let medallions = self
                        .assets
                        .result_medallions
                        .draws(&self.assets.selection.blocks, &state)?;
                    let fireworks = match (state.fireworks, state.fireworks_seed) {
                        (Some(setting), Some(seed)) => {
                            let image =
                                openmt2001::pe::PeImage::parse(&self.assets.inputs.executable)?;
                            Some(crate::fresh_city::result::fireworks::Fireworks::new(
                                &image, setting, seed,
                            )?)
                        }
                        (None, None) => None,
                        _ => return Err("result firework setting and seed differ".into()),
                    };
                    let fireworks_draw = fireworks
                        .as_ref()
                        .map(|_| view.scene.draws.len() + medallions.len());
                    if !medallions.is_empty() || fireworks.is_some() {
                        view.scene.draws.extend(medallions);
                        if fireworks.is_some() {
                            view.scene
                                .draws
                                .push(self.assets.result_firework_texture.empty_draw());
                        }
                        let draws = view
                            .scene
                            .draws
                            .iter()
                            .map(|draw| draw.borrowed())
                            .collect::<Vec<_>>();
                        view.prepared =
                            self.renderer
                                .prepare(&draws, view.scene.camera, view.scene.fog)?;
                    }
                    let (hour, counter) = view
                        .environment_time
                        .ok_or("result city lacks retained environment time")?;
                    let actor_light =
                        openmt2001::lighting::SunColors::parse(&self.assets.inputs.suncols)?
                            .sample(hour, counter)?;
                    let actor_poses = crate::auction_scene::result_layout(state.characters)?;
                    let actor_draws = self.assets.result_actors.draws_result(
                        &actor_poses,
                        &actor_light,
                        view.camera.eye,
                    )?;
                    let actors = self.renderer.prepare(
                        &actor_draws
                            .iter()
                            .map(|draw| draw.borrowed())
                            .collect::<Vec<_>>(),
                        view.scene.camera,
                        view.scene.fog,
                    )?;
                    let mut controller = crate::result_controller::Controller::new(
                        state.bypass_enabled,
                        state.credits,
                        state.credit_rows,
                    );
                    controller.set_identifier(state.cut_scene_identifier);
                    let music_address = view
                        .live_status
                        .as_ref()
                        .and_then(|status| status.result_request)
                        .ok_or("Result request disappeared before music selection")?
                        .music_address;
                    view.era_audio_player
                        .apply(crate::era_audio::Operation::Stop)?;
                    let music = match self.assets.result_audio.play_with_audio(
                        music_address,
                        &self
                            .process_frontend
                            .as_ref()
                            .ok_or("Result requires process audio owner")?
                            .borrow()
                            .audio,
                    ) {
                        Ok(playing) => Some(playing),
                        Err(error) => {
                            eprintln!("{error}");
                            None
                        }
                    };
                    view.result = Some(ResultView {
                        state,
                        fireworks,
                        fireworks_draw,
                        fireworks_ticks: 0,
                        _music: music,
                        samples,
                        started: Instant::now(),
                        phase: ResultPhase::Flyby,
                        controller,
                        serviced_ticks: 0,
                        bypass_input: false,
                        pending: None,
                        credit_pending: None,
                        cutscene_pending: None,
                        completed: None,
                        actor_light,
                        actors,
                        actor_draws: actor_draws.len(),
                        title: None,
                    });
                    view.keys = [false; 4];
                    view.taps = [false; 4];
                    view.drag = [0; 2];
                    view.mouse_buttons = [false; 2];
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(true),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result worker disconnected".into());
                }
            }
        }
        let Some(result) = view.result.as_mut() else {
            return Ok(false);
        };
        let service_tick = (result.started.elapsed().as_secs_f64() * 50.) as usize;
        if let Some(fireworks) = &mut result.fireworks {
            while result.fireworks_ticks < service_tick {
                result.fireworks_ticks += 1;
                fireworks.tick()?;
                for cue in fireworks.take_sounds() {
                    view.gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::ResultFirework(cue))?;
                }
            }
        }
        while result.serviced_ticks < service_tick
            && result.cutscene_pending.is_none()
            && result.credit_pending.is_none()
        {
            result.serviced_ticks += 1;
            let ended = result
                .controller
                .tick(std::mem::take(&mut result.bypass_input));
            if result.controller.take_credit_construction_edge() {
                let sender = view
                    ._fresh_session
                    .as_ref()
                    .ok_or("Result worker missing at credits edge")?;
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::ResultCredits { reply })
                    .map_err(|e| e.to_string())?;
                result.credit_pending = Some(answer);
            }
            if ended {
                let sender = view
                    ._fresh_session
                    .as_ref()
                    .ok_or("Result worker missing at cutscene end")?;
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::ResultEvent {
                        name: "CutSceneEnded",
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                result.cutscene_pending = Some(answer);
            }
        }
        if let Some(receiver) = &result.credit_pending {
            match receiver.try_recv() {
                Ok(state) => {
                    result.state = state?;
                    result.credit_pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result credits worker disconnected".into());
                }
            }
        }
        if matches!(result.phase, ResultPhase::Flyby) {
            let tick = (result.started.elapsed().as_secs_f64() * 50.) as usize;
            let sample = result.samples[tick.min(result.samples.len() - 1)];
            if view.camera != sample {
                view.camera = sample;
                if let Some(producer) = &mut view.camera_producer {
                    let _ = producer.commit(sample, if view.strategic { 3 } else { 0 });
                }
                let mut changed = view.scene.reframe(sample)?;
                if let Some(trees) = &mut view.trees {
                    changed.extend(trees.reframe(&mut view.scene, sample)?);
                }
                let draw_distance = u32::from(view.settings_menu.graphics.distance);
                changed.extend(view.detail_visibility.reframe(
                    &mut view.scene,
                    sample,
                    view.settings_menu.graphics.distance,
                )?);
                let residents = if let Some(people) = &view.people {
                    people.reframe(&mut view.scene, sample, draw_distance)?
                } else {
                    0..0
                };
                changed.extend(residents);
                if let Some(vehicles) = &view.vehicles {
                    changed.extend(vehicles.reframe(&mut view.scene, sample, draw_distance)?);
                }
                if let Some(transport) = &view.transport {
                    changed.extend(transport.reframe(&mut view.scene, sample, draw_distance)?);
                }
                for (i, draw) in view.scene.draws.iter().enumerate() {
                    if changed.contains(&i) {
                        self.renderer.update_dynamic_surface(
                            &mut view.prepared,
                            i,
                            &draw.borrowed(),
                            draw.view_projection.unwrap_or(view.scene.camera),
                            view.scene.fog,
                        )?;
                    } else {
                        self.renderer.update_camera(
                            &view.prepared,
                            i,
                            draw.view_projection.unwrap_or(view.scene.camera),
                            view.scene.fog,
                        )?;
                    }
                }
                let actor_poses = crate::auction_scene::result_layout(result.state.characters)?;
                let actor_draws = self.assets.result_actors.draws_result(
                    &actor_poses,
                    &result.actor_light,
                    sample.eye,
                )?;
                if actor_draws.len() != result.actor_draws {
                    return Err("result actor draw count changed during flyby".into());
                }
                for (i, draw) in actor_draws.iter().enumerate() {
                    self.renderer.update_surface(
                        &mut result.actors,
                        i,
                        &draw.borrowed(),
                        view.scene.camera,
                        view.scene.fog,
                    )?;
                }
            }
            if tick >= result.samples.len() && result.pending.is_none() {
                let sender = view
                    ._fresh_session
                    .as_ref()
                    .ok_or("Result worker missing at flyby end")?;
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::ResultEvent {
                        name: "FlybyEnded",
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                result.pending = Some(answer);
            }
            if let Some(receiver) = &result.pending {
                match receiver.try_recv() {
                    Ok(state) => {
                        result.state = state?;
                        result
                            .controller
                            .set_identifier(result.state.cut_scene_identifier);
                        result.controller.set_bypass(result.state.bypass_enabled);
                        result.pending = None;
                        result.phase = ResultPhase::CutScene;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("Result event worker disconnected".into());
                    }
                }
            }
        }
        if let Some(receiver) = &result.cutscene_pending {
            match receiver.try_recv() {
                Ok(state) => {
                    result.state = state?;
                    result.cutscene_pending = None;
                    if result.state.end_requested.is_none() {
                        return Err("Result CutSceneEnded did not request Game.End".into());
                    }
                    let sender = view
                        ._fresh_session
                        .as_ref()
                        .ok_or("Result worker missing at completion")?;
                    let (reply, answer) = mpsc::channel();
                    sender
                        .send(FreshRequest::ResultComplete { reply })
                        .map_err(|e| e.to_string())?;
                    result.completed = Some(answer);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result event worker disconnected".into());
                }
            }
        }
        if let Some(receiver) = &result.completed {
            match receiver.try_recv() {
                Ok(status) => {
                    let status = status?;
                    // Discard a status poll queued before completion. Its old
                    // result request must never restart the presentation.
                    view.status_pending = None;
                    view.stats_requested = true;
                    view.live_status = Some(status);
                    view.result = None;
                    view.keys = [false; 4];
                    view.taps = [false; 4];
                    view.mouse_buttons = [false; 2];
                    view.drag = [0; 2];
                    return self.draw_result(presenter);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result completion worker disconnected".into());
                }
            }
        }
        if let (Some(fireworks), Some(index)) = (&result.fireworks, result.fireworks_draw) {
            let draw = &mut view.scene.draws[index];
            crate::fresh_city::result::particles_gpu::fill(draw, fireworks.points(), view.camera)?;
            self.renderer.update_surface(
                &mut view.prepared,
                index,
                &draw.borrowed(),
                view.scene.camera,
                None,
            )?;
        }
        update_city_pulse(
            &self.assets.city_pulse,
            &self.renderer,
            view.pulse_draw,
            view.pulse_state.as_ref(),
            view.camera,
            &mut view.scene,
            &mut view.prepared,
        )?;
        self.renderer
            .draw_prepared(&self.color_view, &self.depth_view, &view.prepared);
        self.renderer
            .draw_world_overlay(&self.color_view, &self.depth_view, &result.actors);
        let title_id = result
            .controller
            .title_visible()
            .then(|| result.state.text_id())
            .flatten();
        let credit_row = result.controller.credit_row();
        let text_alpha = result.controller.alpha();
        if (title_id.is_none() && credit_row.is_none()) || text_alpha == 0 {
            result.title = None;
        } else if result.title.as_ref().is_none_or(|(id, row, alpha, _)| {
            *id != title_id || *row != credit_row || *alpha != text_alpha
        }) {
            let scene = if let Some(row) = credit_row {
                self.assets.result_text.prepare_credit(
                    &self.renderer,
                    &result.state,
                    row,
                    text_alpha,
                )?
            } else {
                self.assets
                    .result_text
                    .prepare(&self.renderer, &result.state, text_alpha)?
            };
            result.title = scene.map(|scene| (title_id, credit_row, text_alpha, scene));
        }
        if let Some((_, _, _, title)) = &result.title {
            self.renderer
                .draw_overlay(&self.color_view, &self.depth_view, title);
        }
        presenter
            .present_binding(&self.binding)
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub fn draw(&mut self, presenter: &Presenter) -> Result<(), String> {
        if let Some(view) = &mut self.inspection {
            sync_water_options(
                &mut view.scene,
                &self.renderer,
                &mut view.prepared,
                view.settings_menu.graphics,
            )?;
        }
        let graphics = self
            .menu
            .as_ref()
            .map_or(self.graphics_levels, |menu| menu.settings.graphics);
        sync_water_options(
            &mut self.assets.scene,
            &self.renderer,
            &mut self.prepared,
            graphics,
        )?;
        // App serviced focus and fed admitted physical samples at redraw entry.
        self.service_camera_turns();
        self.poll_camera_worker()?;
        let camera_start_ready = self.process_start_ready();
        if !camera_start_ready && let Some(menu) = &mut self.menu {
            if menu.load.request.is_some() {
                menu.load.message = Some(
                    self.camera_worker_failure
                        .clone()
                        .unwrap_or_else(|| "Finishing previous city before loading...".into()),
                );
            }
            if menu.single.request.is_some() {
                menu.single.message = Some(
                    self.camera_worker_failure
                        .clone()
                        .unwrap_or_else(|| "Finishing previous city before starting...".into()),
                );
            }
            // Skirmish request/busy remains retained; its ordinary start error
            // is surfaced by finish on a terminal failure, not fake completion.
            if menu.skirmish.request.is_some()
                && let Some(error) = &self.camera_worker_failure
            {
                menu.skirmish.finish(Some(error.clone()));
            }
        }
        self.commit_menu_settings()?;
        // A host worker reuses the same original byte inputs and shared assembler.
        // Native startup enables gameplay; player pause owns reason3 separately.
        // The worker retains its Lua VM and world until inspection is dismissed.
        if !self.refreshing_fresh
            && self.menu.as_ref().is_some_and(|m| {
                if self.loading_connected {
                    !m.multiplayer.busy
                } else if self.loading_skirmish {
                    !m.skirmish.busy
                } else if self.loading_from_setup {
                    !m.single.visible || !m.single.busy
                } else {
                    !m.load.visible
                }
            })
        {
            self.loading = None;
        }
        if self.loading.is_none()
            && self.inspection.is_some()
            && self.refresh_retry.due(Instant::now())
        {
            if let Err(error) = self.refresh_fresh_city() {
                self.refresh_retry.reject(error);
            }
        }
        // Normal level startup4b9fa0->4b9d40 writes this mode6 overview
        // at4b9e5f..91. Represent the same pose in retained city controls;
        // clicking a block then uses the original120/80 dimensions.
        let (mut inspection_camera, mut inspection_controls) = fresh_city_camera();
        if self.refreshing_fresh
            && let Some(view) = &self.inspection
        {
            inspection_controls = view.controls;
            inspection_camera = view.camera;
        }
        self.start_requested_city(inspection_camera)?;
        self.receive_loading(presenter, inspection_camera, inspection_controls)?;
        self.service_camera_requests()?;
        self.refresh_inspection_projection()?;
        if !self.inspection.as_ref().is_some_and(|v| {
            v.auction.scene.is_some()
                || v.prompts.request().is_some()
                || v.result.is_some()
                || v.result_pending.is_some()
                || v.live_status
                    .as_ref()
                    .is_some_and(|s| s.result_request.is_some() || s.result_review.is_some())
        }) {
            self.inspection_update()?;
        }
        self.poll_world_sessions()?;
        self.poll_adjustment()?;
        self.sync_management_selection()?;
        self.poll_planning()?;
        let refresh = self.poll_session_replies()?;
        if self.draw_result(presenter)? {
            return Ok(());
        }
        if refresh {
            self.refresh_fresh_city()?;
        }
        if self.draw_auction(presenter)? {
            return Ok(());
        }
        if self.draw_inspection(presenter)? {
            return Ok(());
        }

        self.draw_frontend(presenter, None)
    }
}
