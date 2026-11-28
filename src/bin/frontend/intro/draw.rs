//! `impl Intro`: draw_result .. draw, split out of `intro.rs` (#99).
use super::*;
#[path = "draw/auction.rs"]
mod auction;
#[path = "draw/inspection.rs"]
mod inspection;
#[path = "draw/planning.rs"]
mod planning;

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
        if let Some(worker) = &mut self.camera_worker {
            if let Some(result) = worker.poll() {
                let custody = worker.take_custody();
                self.camera_worker = None;
                let result = result.and_then(|()| {
                    if let Some(process) = &self.process_frontend {
                        process
                            .borrow_mut()
                            .return_custody(custody.ok_or("Joined worker returned no custody")?)?;
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    if let Some(process) = &self.process_frontend {
                        process.borrow_mut().fail(error.clone());
                    }
                    self.camera_worker_failure = Some(error.clone());
                    self.loading = None;
                    self.retire_inspection();
                    eprintln!("City worker failed; replacement startup blocked: {error}");
                    if let Some(menu) = &mut self.menu {
                        menu.load.message = Some(error.clone());
                        menu.single.message = Some(error.clone());
                        menu.skirmish.error = Some(error);
                        menu.load.busy = false;
                        menu.single.busy = false;
                        menu.invalidate();
                    }
                }
            }
        }
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
        if self.process_start_ready()
            && self.loading.is_none()
            && let Some(path) = self.menu.as_mut().and_then(|m| m.load.request.take())
        {
            self.loading_connected = false;
            self.loading_skirmish = false;
            self.loading_from_setup = false;
            let save_profile_root = self.menu.as_ref().and_then(|m| m.current_save_root());
            let pavement = self
                .assets
                .selection
                .pavement_resource()
                .binding()?
                .handle();
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody = self.start_custody(false)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            self.camera_worker = Some(camera_producer::spawn_session_joined(
                "saved-city-simulation",
                inputs,
                WorkerStart::Saved(path, save_profile_root),
                inspection_camera,
                tx,
                Some(pavement),
                focus_enabled,
                custody,
            )?);
        }
        // A cancelled preparation belongs to its captured attempt. Discard it
        // before a replacement route can receive an old worker's completion.
        if self.pending_connected.is_some()
            && self
                .menu
                .as_ref()
                .is_none_or(|m| !m.multiplayer.busy || m.multiplayer.request.is_some())
        {
            self.pending_connected = None;
        }
        if self.pending_connected.is_none()
            && let Some(request) = self
                .menu
                .as_mut()
                .and_then(|m| m.multiplayer.request.take())
        {
            match connected_start::Pending::start(request, self.assets.inputs.clone()) {
                Ok(pending) => self.pending_connected = Some(pending),
                Err(error) => {
                    if let Some(menu) = &mut self.menu {
                        menu.multiplayer.finish(Some(error));
                    }
                }
            }
        }
        if let Some(pending) = &mut self.pending_connected
            && let Err(error) = pending.poll()
        {
            self.pending_connected = None;
            if let Some(menu) = &mut self.menu {
                menu.multiplayer.finish(Some(error));
            }
        }
        if self.process_start_ready()
            && self.loading.is_none()
            && let Some(request) = self
                .pending_connected
                .as_mut()
                .and_then(connected_start::Pending::take_ready)
        {
            self.pending_connected = None;
            self.loading_connected = true;
            self.loading_skirmish = false;
            self.loading_from_setup = true;
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody = self.start_custody(true)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            match camera_producer::spawn_session_joined(
                "connected-city-simulation",
                inputs,
                WorkerStart::Connected(Box::new(request)),
                inspection_camera,
                tx,
                None,
                focus_enabled,
                custody,
            ) {
                Ok(worker) => self.camera_worker = Some(worker),
                Err(error) => {
                    self.camera_worker_failure = Some(error.clone());
                    if let Some(process) = &self.process_frontend {
                        process.borrow_mut().fail(error.clone());
                    }
                    self.loading = None;
                    self.loading_connected = false;
                    self.loading_from_setup = false;
                    if let Some(menu) = &mut self.menu {
                        menu.multiplayer.finish(Some(error));
                    }
                }
            }
        }
        if self.process_start_ready()
            && self.loading.is_none()
            && let Some(request) = self.menu.as_mut().and_then(|m| m.skirmish.request.take())
        {
            self.loading_skirmish = true;
            self.loading_from_setup = true;
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody = self.start_custody(true)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            match camera_producer::spawn_session_joined(
                "skirmish-simulation",
                inputs,
                WorkerStart::Skirmish(request),
                inspection_camera,
                tx,
                None,
                focus_enabled,
                custody,
            ) {
                Ok(worker) => self.camera_worker = Some(worker),
                Err(error) => {
                    self.camera_worker_failure = Some(error.clone());
                    if let Some(process) = &self.process_frontend {
                        process.borrow_mut().fail(error.clone());
                    }
                    self.loading = None;
                    self.loading_connected = false;
                    self.loading_skirmish = false;
                    self.loading_from_setup = false;
                    if let Some(menu) = self.menu.as_mut() {
                        menu.skirmish.finish(Some(error.to_string()));
                        menu.invalidate();
                    }
                }
            }
        }
        if self.process_start_ready()
            && self.loading.is_none()
            && let Some(preview) = self.menu.as_mut().and_then(|m| m.single.request.take())
        {
            self.loading_connected = false;
            self.loading_skirmish = false;
            self.loading_from_setup = true;
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody = self.start_custody(true)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            self.camera_worker = Some(camera_producer::spawn_session_joined(
                "city-simulation",
                inputs,
                WorkerStart::Fresh(preview),
                inspection_camera,
                tx,
                None,
                focus_enabled,
                custody,
            )?);
        }
        if let Some(rx) = &self.loading {
            let result = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(e) => Some(Err(e.to_string())),
            };
            if let Some(result) = result {
                self.loading = None;
                let refreshing = self.refreshing_fresh;
                self.refreshing_fresh = false;
                let result = result.and_then(|loaded| {
                    let camera_initial = if loaded.camera_publisher.is_some() {
                        Some(camera_producer::Initial::from_loaded(&loaded, camera_producer::Controls {
                                camera: inspection_camera, controls: inspection_controls,
                                inertia: openmt2001::camera::BlockCameraInertia::new(inspection_camera.eye, inspection_camera.target),
                                saved: false, building: false, building_angle: 0., selected: None,
                            })?)
                    } else { None };
                    let (inspection_camera, inspection_controls) = camera_initial.as_ref()
                        .map(|i|(i.active.camera,i.active.controls))
                        .or(loaded.saved_viewpoint).unwrap_or((inspection_camera, inspection_controls));
                    let retained_session = loaded.fresh_session.or_else(|| {
                        if refreshing {
                            self.inspection
                                .as_ref()
                                .and_then(|view| view._fresh_session.clone())
                        } else {
                            None
                        }
                    });
                    let graphics_levels = self.inspection.as_ref().map_or(self.graphics_levels,
                        |view| view.settings_menu.graphics);
                    let star_detail = self.inspection.as_ref().map_or(Some(graphics_levels.detail),
                        Inspection::normal_star_detail);
                    let mut scene = loaded.scene;
                    scene.apply_water_options(graphics_levels.water_effects, graphics_levels.detail)?;
                    let mut detail_visibility = loaded.detail_visibility;
                    if refreshing && let Some(previous) = &self.inspection {
                        scene.retain_environment(&previous.scene)?;
                    }
                    let mut environment_changed = if let Some(tick) = loaded.logic_tick {
                        scene.advance_environment(tick, inspection_camera.eye)?
                    } else {
                        Vec::new()
                    };
                    if let Some((hour, counter)) = loaded.environment_time {
                        environment_changed.extend(scene.advance_daylight(hour, counter, inspection_camera)?);
                    }
                    environment_changed.extend(loaded.updated_city_draws);
                    let environment_changed = normal_city_scene_updates(
                        &mut scene, inspection_camera, star_detail, environment_changed,
                    )?;
                    let mut trees = loaded.trees;
                    let people = loaded.people;
                    let vehicles = loaded.vehicles;
                    let transport = loaded.transport;
                    if let Some((source, year, hour)) = loaded.status {
                        eprintln!("[session] Loaded {source}: year {year}, hour {hour}");
                    }
                    if let Some(menu) = &self.menu {
                        menu.validate_inspection_names(&scene)?;
                    }
                    let pulse_draw = retained_session.as_ref().map(|_| {
                        let index = scene.draws.len();
                        scene.draws.push(self.assets.city_pulse.empty_draw());
                        index
                    });
                    let draws = scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>();
                    let profile_prepare = std::env::var_os("OPENMT2001_PROFILE_SNAPSHOTS")
                        .map(|_| Instant::now());
                    let mut prepared = self.renderer.prepare(&draws, scene.camera, scene.fog)?;
                    if let Some(started) = profile_prepare {
                        eprintln!(
                            "Snapshot frontend tick{:?} refreshing{refreshing} reuse_city{} full_prepare{} draws{} detached_prepare host elapsed {:?}; includes waits, excludes subsequent updates and GPU execution",
                            loaded.logic_tick, loaded.reuse_city,
                            true, draws.len(), started.elapsed(),
                        );
                    }
                    if refreshing && loaded.reuse_city {
                        for index in environment_changed {
                            let draw = &scene.draws[index];
                            self.renderer.update_dynamic_surface(
                                &mut prepared, index, &draw.borrowed(),
                                draw.view_projection.unwrap_or(scene.camera), scene.fog,
                            )?;
                        }
                    }
                    // A cached scene may have been created from another view.
                    // Reapply normal admission before this second upload seam.
                    if trees.is_some() || people.is_some() || vehicles.is_some() || transport.is_some() {
                        let changed = scene.reframe(inspection_camera)?;
                        let changed = normal_city_scene_updates(
                            &mut scene, inspection_camera, star_detail, changed,
                        )?;
                        for i in changed {
                            self.renderer.update_dynamic_surface(
                                &mut prepared,
                                i,
                                &scene.draws[i].borrowed(),
                                scene.camera,
                                scene.fog,
                            )?;
                        }
                        let cameras = scene.draws.iter().map(|draw|
                            draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
                        self.renderer.update_cameras(&prepared, &cameras, scene.fog)?;
                    }
                    let gameplay_levels = self.inspection.as_ref().map_or(self.gameplay_levels,
                        |view| view.settings_menu.gameplay);
                    let draw_distance = u32::from(graphics_levels.distance);
                    for i in detail_visibility.reframe(&mut scene, inspection_camera,
                        draw_distance as u8)? {
                        self.renderer.update_dynamic_vertices(&mut prepared, i,
                            &scene.draws[i].borrowed())?;
                    }
                    if let Some(trees) = &mut trees {
                        for i in trees.reframe(&mut scene, inspection_camera)? {
                            let draw = &scene.draws[i];
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &draw.borrowed(),
                                draw.view_projection.unwrap_or(scene.camera), scene.fog,
                            )?;
                        }
                    }
                    if let Some(people) = &people {
                        for i in people.reframe(&mut scene, inspection_camera, draw_distance)? {
                            self.renderer.update_dynamic_surface(
                                &mut prepared,
                                i,
                                &scene.draws[i].borrowed(),
                                scene.camera,
                                scene.fog,
                            )?;
                        }
                    }
                    if let Some(vehicles) = &vehicles {
                        for i in vehicles.reframe(&mut scene, inspection_camera, draw_distance)? {
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &scene.draws[i].borrowed(),
                                scene.camera, scene.fog,
                            )?;
                        }
                    }
                    if let Some(transport) = &transport {
                        for i in transport.reframe(&mut scene, inspection_camera, draw_distance)? {
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &scene.draws[i].borrowed(),
                                scene.camera, scene.fog,
                            )?;
                        }
                    }
                    let capacity = self.assets.selection.marker_surface(
                        Some(crate::selection::Hit {
                            category: 0,
                            building: None,
                        }),
                        Some(crate::selection::Hit {
                            category: 1,
                            building: None,
                        }),
                        &scene,
                    )?;
                    let mut draw = capacity.draw();
                    draw.mode = openmt2001::render::SurfaceMode::StaticLit;
                    let markers = self.renderer.prepare(&[draw], scene.camera, scene.fog)?;
                    let mut locator = self.menu.as_ref().map(|menu| {
                        if let Some(inputs) = loaded.runtime_locator.as_ref() {
                            menu.locator_assets().runtime_state(inputs).map(Some)
                        } else {
                            menu.locator_assets().state(&scene)
                        }
                    }).transpose()?.flatten();
                    if locator.is_none() && loaded.city_people.is_some() {
                        locator = self
                            .menu
                            .as_ref()
                            .map(|m| m.locator_assets().people_state())
                            .transpose()?;
                    }
                    if let (Some(locator), Some(report)) = (&mut locator, loaded.city_people) {
                        locator.attach_people(report);
                    }
                    if let Some(locator) = &mut locator {
                        if let Some(players) = loaded.player_info {
                            locator.attach_players(players);
                        }
                        locator.refresh_scene(&scene);
                        locator.attach_polls(loaded.consumer_polls);
                        locator.attach_ownership(loaded.city_ownership);
                        locator.attach_rights(loaded.city_rights);
                        locator.attach_prestige(loaded.city_prestige);
                    }
                    let mut era_audio_player = crate::era_audio::Player::new(
                        self.assets.era_audio.clone());
                    if let Some(process)=&self.process_frontend {era_audio_player.bind_audio(process.borrow().audio.clone())?;}
                    else {era_audio_player.set_levels(self.audio_levels.music,self.audio_levels.effects);}
                    let mut auction_audio_player = crate::live_host::auction_audio::Player::new(
                        self.assets.auction_audio.clone());
                    if let Some(process)=&self.process_frontend {auction_audio_player.bind_audio(process.borrow().audio.clone());}
                    else {auction_audio_player.set_level(self.audio_levels.effects);}
                    let mut transport_audio_player = crate::transport_audio::Player::new(
                        self.assets.transport_audio.clone());
                    if let Some(process)=&self.process_frontend {transport_audio_player.bind_audio(process.borrow().audio.clone());}
                    else {transport_audio_player.set_level(self.audio_levels.effects);}
                    let mut gameplay_sound_player = crate::gameplay_sound::Player::new(
                        self.assets.gameplay_sound.clone());
                    if let Some(process)=&self.process_frontend {gameplay_sound_player.bind_audio(process.borrow().audio.clone());}
                    else {gameplay_sound_player.set_level(self.audio_levels.effects);}
                    // Native fresh startup 4b9f3f calls 4806b0 with the
                    // current era, selecting its music through 4ad6f0.
                    // Later decade changes use the same 4806b0 callback.
                    let mut settings_menu = crate::settings_menu::View::default();
                    settings_menu.graphics = graphics_levels;
                    settings_menu.gameplay = gameplay_levels;
                    let camera_inbox = loaded.camera_inbox;
                    let imported_cameras = loaded.imported_cameras;
                    let pending_music_era=loaded.music_era;
                    let mut view = Inspection {
                        music_admission:self.focus.enabled().then(||self.focus.music_admission()),
                        camera_epoch: loaded.camera_epoch,
                        camera_turns: Default::default(),
                        camera_producer: None,
                        camera_ready: None,
                        camera_endpoint: None,
                        camera_capture_pending: None,
                        camera_ui: None,
                        ui_graph:None,
                        camera_navigation:None,
                        camera_focus:camera_live::focus::Focus::default(),
                        navigation_click_latched:false,
                        save_profile_root: loaded.save_profile_root,
                        saved_camera: loaded.saved_viewpoint.is_some(),
                        result_pending: None,
                        result: None,
                        auction_requests: loaded.auction_requests,
                        auction_audio_requests: loaded.auction_audio_requests,
                        auction_audio_player,
                        transport_audio_requests: loaded.transport_audio_requests,
                        transport_audio_player,
                        era_audio_requests: loaded.era_audio_requests,
                        era_audio_player,
                        gameplay_sound_requests: loaded.gameplay_sound_requests,
                        gameplay_sound_player,
                        auction: crate::auction_bridge::View::default(),
                        auction_panel: crate::auction_panel::View::default(),
                        planning: None,
                        planning_pending: None,
                        planning_pressed: false,
                        live_status: None,
                        time_panel: crate::time_panel::View::default(),
                        cash_arrow: crate::cash_arrow::View::default(),
                        dust: crate::dust_scene::View::default(),
                        time_pressed: None,
                        game_options: crate::game_options::View::default(),
                        save_dialog: crate::save_dialog::View::default(),
                        save_pending: None,
                        options_ack: options_ack::Gate::default(),
                        save_delete_pending: None,
                        settings_menu,
                        detail_visibility,
                        pulse_draw,
                        pulse_state: None,
                        status_pending: None,
                        environment_pending: None,
                        environment_tick: loaded.logic_tick.unwrap_or(0),
                        environment_time: loaded.environment_time,
                        last_environment_query: Instant::now()
                            - std::time::Duration::from_millis(40),
                        last_status_query: Instant::now() - std::time::Duration::from_secs(1),
                        last_snapshot: Instant::now(),
                        construction_active: loaded.construction_active,
                        snapshot_tick: 0,
                        card_input: crate::card_panel::Input::default(),
                        card_selection: None,
                        prompts: crate::prompt::State::default(),
                        tutorial_message: crate::prompt::State::default(),
                        event_list: crate::event_list::View::default(),
                        stats_panel: crate::stats_panel::View::default(),
                        market_share_panel: crate::market_share_panel::View::default(),
                        stats_requested: false,
                        tutorial_camera_seen: 0,
                        auction_notice: crate::auction_notice::State::default(),
                        notifications: crate::notice_panel::View::default(),
                        prompt_reply: None,
                        block_information: loaded.block_information,
                        utility_report: crate::utility_report::View::default(),
                        property_pending: None,
                        rights_pending: None,
                        comparison: loaded.comparison.map(crate::comparison::View::new),
                        trees,
                        people,
                        people_pending: None,
                        last_people_query: Instant::now(),
                        vehicles,
                        vehicles_pending: None,
                        last_vehicles_query: Instant::now(),
                        transport,
                        transport_pending: None,
                        last_transport_query: Instant::now(),
                        _fresh_session: retained_session,
                        adjustment: None,
                        operations: crate::business_view::View::default(),
                        locator,
                        locator_blocked: false,
                        information: None,
                        details_visible: true,
                        details_pressed: false,
                        toolbar_input: Default::default(),
                        toolbar_right_held: false,
                        toolbar_bounds: self.menu.as_ref().ok_or("ordinary toolbar assets missing")?
                            .locator_assets().ordinary_toolbar_bounds(),
                        toolbar_regions: [None;3],
                        detail_icons: if refreshing && self.inspection.is_some() {
                            Default::default()
                        } else { self.assets.navigation.prepare_details(&self.renderer)? },
                        detail_bounds: self.assets.navigation.detail_bounds,
                        navigator: self
                            .assets
                            .navigator
                            .prepare_with_ownership(&self.renderer, &loaded.navigator_owners)?,
                        navigator_owners: loaded.navigator_owners,
                        navigator_bounds: self.assets.navigator.bounds,
                        navigator_pressed: false,
                        tutorial_go_to_block_pressed: false,
                        tooltip: None,
                        tooltip_state: None,
                        icon_tooltip_dwell: crate::tooltip::IconDwell::default(),
                        navigation: if refreshing && self.inspection.is_some() {
                            Default::default()
                        } else { self.assets.navigation.prepare(&self.renderer)? },
                        navigation_bounds: self.assets.navigation.bounds,
                        navigation_pressed: false,
                        double_click: crate::navigation::DoubleClick::default(),
                        strategic: camera_initial.as_ref().map_or(loaded.saved_prior_mode == Some(3), |initial|initial.strategic()),
                        return_camera: None,
                        reframe_pending: false,
                        scene,
                        prepared,
                        markers,
                        camera: inspection_camera,
                        controls: inspection_controls,
                        building_angle: 0.,
                        building_camera: false,
                        inertia: openmt2001::camera::BlockCameraInertia::new(
                            inspection_camera.eye,
                            inspection_camera.target,
                        ),
                        keys: [false; 4],
                        taps: [false; 4],
                        cursor: None,
                        mouse_buttons: [false; 2],
                        drag: [0; 2],
                        pointer: None,
                        hovered: None,
                        selected: None,
                        title_state: None,
                        started: Instant::now(),
                        tick: 0,
                    };
                    let needs_ui = camera_inbox.is_some() || (refreshing && self.inspection.as_ref()
                        .is_some_and(|old| old.camera_endpoint.is_some() && old.camera_ui.is_none()));
                    if needs_ui {
                        view.camera_ui = Some(camera_live::State::new(
                            camera_live::Globals::after_normal_initialization(camera_live::OriginalGraphicsPolicy::NormalD3d,false),
                            camera_live::Cursor::new(0,0)?, None, None));
                    }
                    if camera_inbox.is_some() && imported_cameras.is_none() {
                        return Err("review channel requires imported pair".into());
                    }
                    if refreshing && camera_inbox.is_some() {
                        return Err("scene refresh must retain its actual endpoint".into());
                    }
                    let property_install = if camera_inbox.is_some() {
                        let image=openmt2001::pe::PeImage::parse(&self.assets.inputs.executable)?;
                        Some(camera_live::endpoint::PropertyInstall::capture(&self.assets.selection,&image)?)
                    } else {None};
                    let plan = if refreshing {
                        let previous = self.inspection.as_mut().ok_or("refresh has no retained inspection")?;
                        let menu = self.menu.as_ref().ok_or("refresh menu assets missing")?;
                        // Selection is a prospective presentation index, never
                        // the C stable key or raw current-selection authority.
                        view.selected = previous.selected.and_then(|hit| {
                            let building = match hit.building {
                                Some(index) => {
                                    let slot = previous.scene.inspection_buildings.get(index)?.runtime_slot?;
                                    Some(view.scene.inspection_buildings.iter()
                                        .position(|b| b.runtime_slot == Some(slot))?)
                                }
                                None => None,
                            };
                            Some(crate::selection::Hit { category: hit.category, building })
                        });
                        let inputs = refresh::Inputs {
                            previous_buildings: &previous.scene.inspection_buildings,
                            next_scene: (&view.scene).into(),
                            locator_extents: menu.locator_assets().refresh_extents(),
                            business_owner: previous.live_status.as_ref().and_then(|s| s.business_owner),
                        };
                        // Borrow distinct production fields so the exact
                        // GPU-independent transaction also serves the tests.
                        let mut old = refresh::Owners {
                            locator: &mut previous.locator, adjustment: &mut previous.adjustment,
                            graph: &mut previous.ui_graph, ui: &mut previous.camera_ui,
                            navigation: &mut previous.camera_navigation, focus: &mut previous.camera_focus,
                            endpoint: &mut previous.camera_endpoint,
                            navigation_click_latched: &mut previous.navigation_click_latched,
                            tooltip: &mut previous.tooltip,
                            tooltip_state: &mut previous.tooltip_state,
                            icon_tooltip_dwell: &mut previous.icon_tooltip_dwell,
                        };
                        let mut next = refresh::Owners {
                            locator: &mut view.locator, adjustment: &mut view.adjustment,
                            graph: &mut view.ui_graph, ui: &mut view.camera_ui,
                            navigation: &mut view.camera_navigation, focus: &mut view.camera_focus,
                            endpoint: &mut view.camera_endpoint,
                            navigation_click_latched: &mut view.navigation_click_latched,
                            tooltip: &mut view.tooltip,
                            tooltip_state: &mut view.tooltip_state,
                            icon_tooltip_dwell: &mut view.icon_tooltip_dwell,
                        };
                        Some(refresh::prepare(&mut old, &mut next, inputs,
                            menu.adjustment_assets(), &mut self.refresh_retry)?)
                    } else { None };
                    if let Some(initial) = camera_initial {
                        initial.active.put(&mut view);
                        view.return_camera = initial.normal_return;
                        view.camera_producer = Some(camera_producer::Producer::attach(&mut self.camera_parked,
                            loaded.camera_publisher.expect("prepared unique publisher"), &initial)?);
                    }
                    Ok((view, plan, camera_inbox, property_install,pending_music_era))
                });
                if let Some(menu) = &mut self.menu {
                    match result {
                        Ok((mut view, plan, camera_inbox, property_install, pending_music_era)) => {
                            if let Some(plan) = plan {
                                let previous = self
                                    .inspection
                                    .as_mut()
                                    .expect("prepared retained inspection");
                                refresh::commit(
                                    &mut refresh::Owners::of(previous),
                                    &mut refresh::Owners::of(&mut view),
                                    plan,
                                    &mut self.refresh_retry,
                                );
                                // From here through publication every operation is infallible.
                                // These five overlay states depend only on this Intro's
                                // immutable navigation assets and renderer, not city reports.
                                view.detail_icons = std::mem::take(&mut previous.detail_icons);
                                view.navigation = std::mem::take(&mut previous.navigation);
                                view.auction_requests = previous.auction_requests.take();
                                view.auction_audio_requests =
                                    previous.auction_audio_requests.take();
                                std::mem::swap(
                                    &mut view.auction_audio_player,
                                    &mut previous.auction_audio_player,
                                );
                                view.transport_audio_requests =
                                    previous.transport_audio_requests.take();
                                std::mem::swap(
                                    &mut view.transport_audio_player,
                                    &mut previous.transport_audio_player,
                                );
                                view.era_audio_requests = previous.era_audio_requests.take();
                                std::mem::swap(
                                    &mut view.era_audio_player,
                                    &mut previous.era_audio_player,
                                );
                                view.gameplay_sound_requests =
                                    previous.gameplay_sound_requests.take();
                                std::mem::swap(
                                    &mut view.gameplay_sound_player,
                                    &mut previous.gameplay_sound_player,
                                );
                                view.result = previous.result.take();
                                view.auction = std::mem::take(&mut previous.auction);
                                view.auction_panel = std::mem::take(&mut previous.auction_panel);
                                view.notifications = std::mem::take(&mut previous.notifications);
                                view.planning = previous.planning.take();
                                view.planning_pending = previous.planning_pending.take();
                                view.live_status = previous.live_status.take();
                                view.time_panel = std::mem::take(&mut previous.time_panel);
                                view.cash_arrow = std::mem::take(&mut previous.cash_arrow);
                                view.dust = std::mem::take(&mut previous.dust);
                                view.time_pressed = previous.time_pressed.take();
                                view.game_options = std::mem::take(&mut previous.game_options);
                                // A scene snapshot carries render state, not the selected
                                // profile identity. Keep the worker's writable save scope.
                                view.save_profile_root = previous.save_profile_root.take();
                                view.save_dialog = std::mem::take(&mut previous.save_dialog);
                                view.save_pending = previous.save_pending.take();
                                view.options_ack = std::mem::take(&mut previous.options_ack);
                                view.save_delete_pending = previous.save_delete_pending.take();
                                view.settings_menu = std::mem::take(&mut previous.settings_menu);
                                view.pulse_state = previous.pulse_state.take();
                                view.status_pending = previous.status_pending.take();
                                view.last_status_query = previous.last_status_query;
                                view.snapshot_tick =
                                    view.live_status.as_ref().map_or(0, |s| s.ticks);
                                view.operations = std::mem::take(&mut previous.operations);
                                view.operations.refresh = true;
                                view.card_input = std::mem::take(&mut previous.card_input);
                                view.card_selection = previous.card_selection;
                                view.prompts = std::mem::take(&mut previous.prompts);
                                view.tutorial_message =
                                    std::mem::take(&mut previous.tutorial_message);
                                view.event_list = std::mem::take(&mut previous.event_list);
                                view.stats_panel = std::mem::take(&mut previous.stats_panel);
                                view.market_share_panel =
                                    std::mem::take(&mut previous.market_share_panel);
                                view.stats_requested = previous.stats_requested;
                                view.auction_notice = std::mem::take(&mut previous.auction_notice);
                                view.prompt_reply = previous.prompt_reply.take();
                                view.cursor = previous.cursor;
                                view.pointer = previous.pointer;
                                view.mouse_buttons = previous.mouse_buttons;
                                view.camera_capture_pending =
                                    previous.camera_capture_pending.take();
                                view.keys = previous.keys;
                                view.taps = previous.taps;
                                view.drag = previous.drag;
                                view.started = previous.started;
                                view.tick = previous.tick;
                                view.navigation_pressed = previous.navigation_pressed;
                                view.navigator_pressed = previous.navigator_pressed;
                                view.details_pressed = previous.details_pressed;
                                view.strategic = previous.strategic;
                                view.return_camera = previous.return_camera.map(|mut state| {
                                    state.selected = state.selected.and_then(|hit| {
                                        let building = match hit.building {
                                            Some(index) => {
                                                let slot = previous
                                                    .scene
                                                    .inspection_buildings
                                                    .get(index)?
                                                    .runtime_slot?;
                                                Some(
                                                    view.scene
                                                        .inspection_buildings
                                                        .iter()
                                                        .position(|b| {
                                                            b.runtime_slot == Some(slot)
                                                        })?,
                                                )
                                            }
                                            None => None,
                                        };
                                        Some(crate::selection::Hit {
                                            category: hit.category,
                                            building,
                                        })
                                    });
                                    state
                                });
                                camera_producer::commit_refresh(previous, &mut view);
                                view.details_visible = previous.details_visible;
                                view.toolbar_input = std::mem::take(&mut previous.toolbar_input);
                                // Read the current event-thread sample; never
                                // derive it from a snapshot's stale camera flags.
                                view.toolbar_right_held = previous.toolbar_right_held;

                                if let Some(locator) = &mut view.locator
                                    && !locator.has_strategic()
                                {
                                    locator.select_poll_category(view.selected.and_then(|hit| {
                                        self.assets
                                            .selection
                                            .blocks
                                            .get(hit.category)
                                            .map(|block| block.id)
                                    }));
                                    locator.pointer(
                                        menu.locator_assets(),
                                        if view.strategic && view.adjustment.is_none() {
                                            view.pointer
                                        } else {
                                            None
                                        },
                                    );
                                }
                                if let (Some(comparison), Some(old)) =
                                    (&mut view.comparison, &previous.comparison)
                                {
                                    comparison.retain_ui_from(old);
                                }
                                view.utility_report = std::mem::take(&mut previous.utility_report);
                                view.utility_report
                                    .select(view.utility_category(&self.assets.selection.blocks));
                                view.utility_report.refresh_property_offer();
                                view.property_pending = previous.property_pending.take();
                                view.rights_pending = previous.rights_pending.take();
                                view.reframe_pending = true;
                            }
                            if !refreshing {
                                eprintln!(
                                    "{}: {} buildings; simulation paused",
                                    if self.loading_from_setup {
                                        "Scenario preview"
                                    } else {
                                        "Saved city inspection"
                                    },
                                    view.scene.building_count
                                );
                            }
                            if let Some(inbox) = camera_inbox {
                                let owner = self
                                    .camera_parked
                                    .take()
                                    .expect("one process camera Owner parked before native attach");
                                view.camera_endpoint =
                                    Some(camera_live::endpoint::Endpoint::with_property_install(
                                        owner,
                                        inbox,
                                        property_install
                                            .expect("prepared actual property install resources"),
                                    ));
                            }
                            self.refresh_retry.complete();
                            if !refreshing && view.camera_producer.is_some() {
                                if let Some(sender) = &view._fresh_session {
                                    let (reply, result) = mpsc::channel();
                                    if let Err(error) =
                                        sender.send(FreshRequest::UiCameraReady { reply })
                                    {
                                        view.camera_producer
                                            .as_mut()
                                            .expect("attached producer")
                                            .invalidate(error.to_string());
                                    }
                                    view.camera_ready = Some(result);
                                }
                            }
                            if !refreshing {
                                if let Some(process) = &self.process_frontend {
                                    process.borrow_mut().stop_menu()?;
                                }
                                if view._fresh_session.is_some()
                                    && let Some(era) = pending_music_era
                                {
                                    view.era_audio_player.apply(
                                        crate::era_audio::Operation::Select { era, repeat: true },
                                    )?;
                                }
                            }
                            let focus_sender = (!refreshing && self.focus.enabled())
                                .then(|| view._fresh_session.clone())
                                .flatten();
                            self.inspection = Some(view);
                            if let Some(sender) = focus_sender {
                                self.focus.attach(sender)?;
                                if let Some(process) = &self.process_frontend {
                                    process.borrow_mut().note_sent(self.focus.sent());
                                }
                            }
                            if self.loading_connected {
                                menu.multiplayer.finish(None);
                            } else if self.loading_skirmish {
                                menu.skirmish.finish(None);
                            } else if self.loading_from_setup {
                                menu.single.message = None;
                            } else {
                                menu.load.message = None;
                            }
                            presenter.window.set_title("OpenMT2001 - city inspection (simulation paused) / arrows pan, right-drag orbit/zoom, Esc returns");
                        }
                        Err(e) => {
                            if refreshing {
                                // Keep the SAME inspection/endpoint running. A
                                // bounded retry sends the real Inspect request;
                                // no Intro::draw error reaches AppExit here.
                                self.refresh_retry.reject(e.clone());
                                eprintln!("Scene refresh deferred: {e}");
                            }
                            if self.loading_connected {
                                menu.multiplayer.finish(Some(e));
                            } else if self.loading_skirmish {
                                menu.skirmish.finish(Some(e));
                            } else if self.loading_from_setup {
                                menu.single.message = Some(e)
                            } else {
                                menu.load.message = Some(e)
                            }
                        }
                    }
                    if self.loading_from_setup {
                        menu.single.busy = false;
                    } else {
                        menu.load.busy = false;
                    }
                    menu.invalidate();
                }
            }
        }
        if let Some(view) = &mut self.inspection {
            if let Some(receiver) = &view.camera_ready {
                match receiver.try_recv() {
                    Ok(Ok(())) => {
                        view.camera_ready = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    other => {
                        let error = match other {
                            Ok(Err(e)) => e,
                            _ => "Camera attachment boundary closed".into(),
                        };
                        if let Some(producer) = &mut view.camera_producer {
                            producer.invalidate(error.clone());
                        }
                        view.save_dialog.message = Some(error);
                        view.camera_ready = None;
                    }
                }
            }
            match view.options_ack.poll() {
                Ok(Some(_)) => view.game_options.set_save_disabled(
                    !view.options_ack.ready()
                        || view
                            .camera_producer
                            .as_ref()
                            .is_some_and(|p| p.error().is_some())
                        || view
                            .live_status
                            .as_ref()
                            .is_none_or(|s| s.tutorial_session || s.failure.is_some()),
                ),
                Ok(None) => {}
                Err(error) => {
                    view.game_options.set_save_disabled(true);
                    view.game_options.set_message(error);
                }
            }
            camera_live::ui_cursor_pass(view, &self.assets, self.menu.as_ref())?;
            camera_live::audio::service_with_music_poll(
                view,
                self.focus.sent() == self.focus.completed(),
            )?;
            if let Some(mut endpoint) = view.camera_endpoint.take() {
                let result = (|| {
                    let joins = camera_live::UiJoins {
                        primary_clear: None,
                        navigation: None,
                        tutorial: None,
                    };
                    let mut context = camera_live::UiContext::new(
                        view,
                        &mut self.camera_physical,
                        &mut self.assets,
                        &joins,
                        camera_live::SoundBoundary::Existing2d,
                    )?;
                    // A direct ack wakes the worker, which produces the next
                    // ordered request. Do not force a full software-rendered
                    // frame between every such request. Bound transport work
                    // to8ms, waiting at most1ms between completed operations.
                    // These times never enter numeric sampling or Host timers.
                    let started = Instant::now();
                    let budget = std::time::Duration::from_millis(8);
                    let mut completed = 0usize;
                    loop {
                        let remaining = budget.saturating_sub(started.elapsed());
                        if completed > 0 && remaining.is_zero() {
                            break;
                        }
                        let service = if completed == 0 {
                            endpoint.service(&mut context)?
                        } else {
                            endpoint.wait_service(
                                &mut context,
                                remaining.min(std::time::Duration::from_millis(1)),
                            )?
                        };
                        if service == crate::live_host::camera_bridge::Service::Idle {
                            break;
                        }
                        completed += 1;
                    }
                    if completed > 1
                        && std::env::var_os("OPENMT2001_CAMERA_OWNER_NATIVE_REVIEW").is_some()
                    {
                        static REPORTED: std::sync::atomic::AtomicBool =
                            std::sync::atomic::AtomicBool::new(false);
                        if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            eprintln!(
                                "[camera] serviced{completed} ordered requests within one bounded UI pass"
                            );
                        }
                    }
                    Ok::<_, String>(())
                })();
                view.camera_endpoint = Some(endpoint);
                result?;
            }
            Self::poll_camera_save(view);
            while let Some(request) = view
                .auction_requests
                .as_ref()
                .and_then(|r| r.try_recv().ok())
            {
                let result = (|| {
                    view.auction.service_actors(&self.renderer)?;
                    for cue in view.auction.take_cues() {
                        view.gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::AuctionActor(cue))?;
                    }
                    let inputs = &self.assets.inputs;
                    view.auction.present(
                        request.operation,
                        &self.renderer,
                        (&inputs.executable, &inputs.directory, &inputs.archive),
                        if view.strategic { 3 } else { 0 },
                    )
                })();
                view.keys = [false; 4];
                view.taps = [false; 4];
                view.drag = [0; 2];
                let _ = request.reply.send(result);
            }
            view.auction.service_actors(&self.renderer)?;
            for cue in view.auction.take_cues() {
                view.gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::AuctionActor(cue))?;
            }
        }
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
        if let Some(view) = &mut self.inspection {
            let draw_distance = u32::from(view.settings_menu.graphics.distance);
            if let Some(receiver) = &view.people_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.people_pending = None;
                        let mut people = result?;
                        if let Some(previous) = &view.people {
                            people.replace_attachment(previous)?;
                            for i in people.reframe(&mut view.scene, view.camera, draw_distance)? {
                                self.renderer.update_dynamic_surface(
                                    &mut view.prepared,
                                    i,
                                    &view.scene.draws[i].borrowed(),
                                    view.scene.camera,
                                    view.scene.fog,
                                )?;
                            }
                            view.people = Some(people);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.people_pending = None;
                        return Err("Resident simulation session disconnected".into());
                    }
                }
            }
            if self.loading.is_none()
                && view.people_pending.is_none()
                && view.last_people_query.elapsed().as_millis() >= 100
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.running && !s.script_paused && s.failure.is_none())
                && let Some(sender) = &view._fresh_session
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::Residents { reply })
                    .map_err(|e| e.to_string())?;
                view.people_pending = Some(answer);
                view.last_people_query = Instant::now();
            }
            if let Some(receiver) = &view.vehicles_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.vehicles_pending = None;
                        let mut vehicles = result?;
                        if let Some(previous) = &view.vehicles {
                            vehicles.replace_attachment(previous)?;
                            for i in
                                vehicles.reframe(&mut view.scene, view.camera, draw_distance)?
                            {
                                self.renderer.update_dynamic_surface(
                                    &mut view.prepared,
                                    i,
                                    &view.scene.draws[i].borrowed(),
                                    view.scene.camera,
                                    view.scene.fog,
                                )?;
                            }
                            view.vehicles = Some(vehicles);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.vehicles_pending = None;
                        return Err("Road simulation session disconnected".into());
                    }
                }
            }
            if self.loading.is_none()
                && view.vehicles_pending.is_none()
                && view.last_vehicles_query.elapsed().as_millis() >= 40
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.running && !s.script_paused && s.failure.is_none())
                && let Some(sender) = &view._fresh_session
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::Vehicles { reply })
                    .map_err(|e| e.to_string())?;
                view.vehicles_pending = Some(answer);
                view.last_vehicles_query = Instant::now();
            }
            if let Some(receiver) = &view.transport_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.transport_pending = None;
                        let mut transport = result?;
                        if let Some(previous) = &view.transport {
                            transport.replace_attachment(previous)?;
                            for i in
                                transport.reframe(&mut view.scene, view.camera, draw_distance)?
                            {
                                self.renderer.update_dynamic_surface(
                                    &mut view.prepared,
                                    i,
                                    &view.scene.draws[i].borrowed(),
                                    view.scene.camera,
                                    view.scene.fog,
                                )?;
                            }
                            view.transport = Some(transport);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.transport_pending = None;
                        return Err("Transport simulation session disconnected".into());
                    }
                }
            }
            if self.loading.is_none()
                && view.transport_pending.is_none()
                && view.last_transport_query.elapsed().as_millis() >= 40
                && view.live_status.as_ref().is_some_and(|status| {
                    status.running && !status.script_paused && status.failure.is_none()
                })
                && let Some(sender) = &view._fresh_session
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::Transport { reply })
                    .map_err(|e| e.to_string())?;
                view.transport_pending = Some(answer);
                view.last_transport_query = Instant::now();
            }
        }
        if let Some(view) = &mut self.inspection
            && let Some(state) = &mut view.adjustment
        {
            if let Some(receiver) = &state.edit_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        state.edit_pending = None;
                        state.receive_edit(result);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        state.edit_pending = None;
                        state.receive_edit(Err("Business session disconnected".into()));
                    }
                }
            }
            if state.edit_pending.is_none()
                && let Some(action) = state.edit_request.take()
                && let (Some(target), Some(sender)) = (state.edit_target, &view._fresh_session)
            {
                let (reply, answer) = mpsc::channel();
                match sender.send(FreshRequest::EditBusiness {
                    target,
                    action,
                    reply,
                }) {
                    Ok(()) => {
                        state.edit_pending = Some(answer);
                        state.edit_inflight = Some(action);
                    }
                    Err(error) => state.receive_edit(Err(error.to_string())),
                }
            }
            let producer = view
                .adjustment
                .as_mut()
                .and_then(|s| s.take_tutorial_producer());
            if let Some((control, event)) = producer {
                view.send_tutorial_producer(control, event);
            }
            let control = view
                .adjustment
                .as_mut()
                .and_then(|s| s.take_tutorial_control());
            if let Some(id) = control
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.tutorial_enabled_controls.contains(&id))
            {
                view.send_tutorial_control(id);
            }
        }
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
        self.poll_planning()?;
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
                        for id in view.prompts.advance_to(status.prompt_updates) {
                            view.dismiss_prompt(id)?;
                        }
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
                        for request in status.prompts.drain(..) {
                            view.prompts.submit(request);
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            view.mouse_buttons = [false; 2];
                        }
                        if status.tutorial_message != view.tutorial_message.request() {
                            view.tutorial_message = crate::prompt::State::default();
                            if let Some(message) = status.tutorial_message.clone() {
                                view.tutorial_message.submit(message);
                            }
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
                && view.last_environment_query.elapsed().as_millis() >= 40
                && let Some(sender) = &view._fresh_session
            {
                let (reply, result) = mpsc::channel();
                sender
                    .send(FreshRequest::LogicTick(reply))
                    .map_err(|e| e.to_string())?;
                view.environment_pending = Some(result);
                view.last_environment_query = Instant::now();
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
