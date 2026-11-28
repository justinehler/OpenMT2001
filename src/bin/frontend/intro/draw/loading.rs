//! `Intro::receive_loading`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn receive_loading(
        &mut self,
        presenter: &Presenter,
        inspection_camera: CameraSample,
        inspection_controls: openmt2001::camera::BlockCamera,
    ) -> Result<(), String> {
        if let Some(rx) = &self.loading {
            let result = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(e) => Some(Err(e.to_string().into())),
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
                    scene.apply_halo_options(graphics_levels.halos, graphics_levels.detail)?;
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
                        scene.draws.push(
                            self.assets
                                .city_pulse
                                .empty_draw(&self.assets.result_firework_texture),
                        );
                        index
                    });
                    let smoke_draw = retained_session.as_ref().map(|_| {
                        let index = scene.draws.len();
                        scene.draws.push(crate::chimney_smoke_scene::empty(&self.assets.dust));
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
                        // The loaded road tick's crossing masks (0x44dbe0 glows).
                        for i in scene.set_signal_lanes(vehicles.signal_lanes())? {
                            let draw = &scene.draws[i];
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &draw.borrowed(),
                                draw.view_projection.unwrap_or(scene.camera), scene.fog,
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
                        0,
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
                    let mut auction_audio_player = crate::auction_audio::Player::new(
                        self.assets.auction_audio.clone());
                    if let Some(process)=&self.process_frontend {auction_audio_player.bind_audio(process.borrow().audio.clone());}
                    else {auction_audio_player.set_level(self.audio_levels.effects);}
                    let mut transport_audio_player = crate::transport_audio::Player::new(
                        self.assets.transport_audio.clone());
                    if let Some(process)=&self.process_frontend {transport_audio_player.bind_audio(process.borrow().audio.clone());}
                    else {transport_audio_player.set_level(self.audio_levels.effects);}
                    transport_audio_player.set_listener(inspection_camera.eye, inspection_camera.target);
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
                        tutorial_start: camera_producer::TutorialStart::new(loaded.tutorial_session),
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
                        smoke_draw,
                        smoke_state: None,
                        status_pending: None,
                        environment_pending: None,
                        environment_tick: loaded.logic_tick.unwrap_or(0),
                        environment_time: loaded.environment_time,
                        last_status_query: Instant::now() - std::time::Duration::from_secs(1),
                        last_snapshot: Instant::now(),
                        construction_active: loaded.construction_active,
                        snapshot_tick: 0,
                        card_input: crate::card_panel::Input::default(),
                        card_selection: None,
                        prompts: crate::prompt::State::default(),
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
                        chat_panel: crate::chat_panel::Panel::default(),
                        comparison: loaded.comparison.map(crate::comparison::View::new),
                        trees,
                        people,
                        people_pending: None,
                        last_people_query: Instant::now(),
                        vehicles,
                        vehicles_pending: None,
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
                        pause_banner: Default::default(),
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
                        marker_state: None,
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
                                view.smoke_state = previous.smoke_state.take();
                                view.status_pending = previous.status_pending.take();
                                view.last_status_query = previous.last_status_query;
                                view.snapshot_tick =
                                    view.live_status.as_ref().map_or(0, |s| s.ticks);
                                view.operations = std::mem::take(&mut previous.operations);
                                view.operations.refresh = true;
                                view.card_input = std::mem::take(&mut previous.card_input);
                                view.card_selection = previous.card_selection;
                                view.prompts = std::mem::take(&mut previous.prompts);
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
                                self.refresh_retry.reject(e.to_string());
                                eprintln!("Scene refresh deferred: {e}");
                            }
                            if self.loading_connected {
                                menu.multiplayer.finish(Some(e.to_string()));
                            } else if self.loading_skirmish {
                                menu.skirmish.finish(Some(e.to_string()));
                            } else if self.loading_from_setup {
                                menu.single.message = Some(e.to_string())
                            } else {
                                menu.load.refuse(e)
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
        Ok(())
    }
}
