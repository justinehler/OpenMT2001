//! `impl Inspection`, split out of `intro.rs` (#99). Behaviour lives here unchanged.
use super::*;

impl Inspection {
    pub(super) fn city_layers(&self) -> city_ui::Layers {
        city_ui::Layers::new(
            self.options_visible(),
            self.stats_requested,
            self.live_status.as_ref().and_then(|s| s.stats.as_ref()),
        )
    }
    pub(super) fn statistics_mouse(
        &mut self,
        assets: &crate::stats_panel::Assets,
        button: winit::event::MouseButton,
        pressed: bool,
    ) -> bool {
        let city_ui::MouseRoute::Statistics(action) = self.city_layers().mouse(
            &mut self.stats_panel,
            assets,
            self.live_status.as_ref().and_then(|s| s.stats.as_ref()),
            self.pointer.map(|point| point.map(|v| v as f32 * 0.5)),
            button == winit::event::MouseButton::Left,
            pressed,
        ) else {
            return false;
        };
        self.time_panel.cancel_input();
        self.time_pressed = None;
        self.auction_panel.cancel_press();
        if button == winit::event::MouseButton::Left {
            if let Some(action) = action
                && let Some(sender) = &self._fresh_session
            {
                let _ = sender.send(FreshRequest::Stats(action));
                self.stats_requested = true;
                self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
            }
            self.mouse_buttons[0] = pressed;
        }
        self.drag = [0; 2];
        true
    }
    pub(super) fn apply_status_camera_events(
        &mut self,
        status: &crate::live_host::Status,
        selection: &crate::selection::Assets,
        navigation: &crate::navigation::Navigation,
    ) -> Result<(), String> {
        if status.tutorial_camera_events.len() < self.tutorial_camera_seen {
            return Err("tutorial camera event log regressed".into());
        }
        if self.camera_endpoint.is_some()
            && status.tutorial_camera_events.len() != self.tutorial_camera_seen
        {
            return Err("Bridged camera received unordered Status camera effects".into());
        }
        for &action in &status.tutorial_camera_events[self.tutorial_camera_seen..] {
            self.apply_tutorial_camera(action, selection, navigation)?;
        }
        self.tutorial_camera_seen = status.tutorial_camera_events.len();
        Ok(())
    }
    pub(super) fn service_camera_turns(
        &mut self,
        selection: &crate::selection::Assets,
        adjustment_assets: Option<&crate::business_adjustment::Assets>,
    ) {
        let view = self;
        loop {
            let Some(front) = view.camera_turns.front() else {
                break;
            };
            let result = match front.reply.try_recv() {
                Ok(value) => value,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("Camera selection Session closed".into())
                }
            };
            let mut pending = view.camera_turns.pop_front().expect("front pending turn");
            let payload = match result {
                Ok(payload) => payload,
                Err(error) => {
                    view.save_dialog.message = Some(error);
                    continue;
                }
            };
            let prepared = (|| {
                if view.camera_epoch != Some(payload.turn.epoch) {
                    return Err("Retired camera selection origin".to_owned());
                }
                if let camera_producer::Event::Actor {
                    selected,
                    auto_rotate,
                    origin,
                } = pending.event
                {
                    let identity = pending.turn.building.ok_or("Actor turn omitted identity")?;
                    let index = view
                        .scene
                        .inspection_buildings
                        .iter()
                        .position(|b| {
                            b.runtime_slot.or(b.saved_slot) == Some(identity.building_slot as usize)
                                && b.category as u32 == identity.category_id
                        })
                        .ok_or("Live selected building is not in current presentation")?;
                    let category = selection
                        .blocks
                        .iter()
                        .position(|b| b.id as u32 == identity.category_id)
                        .ok_or("Selected category no longer presented")?;
                    let _ = selected; // Scene vector position is presentation only.
                    pending.event = camera_producer::Event::Actor {
                        selected: crate::selection::Hit {
                            category,
                            building: Some(index),
                        },
                        auto_rotate,
                        origin,
                    };
                    if pending.open_adjustment {
                        if let Some(assets) = adjustment_assets {
                            pending.adjustment = assets.open_for_owner(
                                &view.scene,
                                index,
                                view.live_status
                                    .as_ref()
                                    .and_then(|status| status.business_owner),
                            )?;
                        }
                    }
                }
                Ok::<(), String>(())
            })();
            if let Err(error) = prepared {
                payload.receipt.complete(Err(error.clone()));
                view.save_dialog.message = Some(error);
                continue;
            }
            let Some(mut producer) = view.camera_producer.take() else {
                payload
                    .receipt
                    .complete(Err("Camera producer retired".into()));
                continue;
            };
            let mut state = camera_producer::View::read(view);
            let result = camera_producer::complete_with_posts(
                &mut producer,
                &mut state,
                pending.turn,
                pending.event,
                payload,
                selection,
                std::mem::take(&mut pending.posts),
                |state| {
                    state.put(view);
                    if let Some(adjustment) = pending.adjustment.take() {
                        view.adjustment = Some(adjustment);
                    }
                    view.hovered = None;
                    view.title_state = None;
                    view.reframe_pending = true;
                    let (reset_keys, reset_mouse) = match pending.event {
                        camera_producer::Event::Actor {
                            origin: camera_producer::ActorOrigin::Tutorial,
                            ..
                        } => (true, true),
                        camera_producer::Event::Actor {
                            origin: camera_producer::ActorOrigin::Scene,
                            ..
                        } => (pending.open_adjustment, pending.open_adjustment),
                        camera_producer::Event::Actor {
                            origin: camera_producer::ActorOrigin::Locator,
                            ..
                        } => (true, false),
                        camera_producer::Event::Actor { .. } | camera_producer::Event::Deselect => {
                            (false, false)
                        }
                        camera_producer::Event::ChooseView { .. } => (true, true),
                        _ => (true, false),
                    };
                    if reset_keys {
                        view.keys = [false; 4];
                        view.taps = [false; 4];
                        view.drag = [0; 2];
                    }
                    if reset_mouse {
                        view.mouse_buttons = [false; 2];
                    }
                    if let Some(locator) = &mut view.locator {
                        if matches!(pending.event, camera_producer::Event::Compass { .. }) {
                            locator.cancel_press();
                        }
                        locator.dirty = true;
                    }
                    if matches!(
                        pending.event,
                        camera_producer::Event::Actor {
                            origin: camera_producer::ActorOrigin::Locator,
                            ..
                        }
                    ) {
                        if let Some(comparison) = &mut view.comparison {
                            comparison.cancel_press();
                        }
                    }
                },
            );
            view.camera_producer = Some(producer);
            if let Err(error) = result {
                view.save_dialog.message = Some(error);
            }
        }
    }
    pub(super) fn time_target(
        &self,
        assets: &crate::time_panel::Assets,
        point: [f32; 2],
        auction: bool,
    ) -> Option<crate::time_panel::Button> {
        let target = assets.button_in_mode(point, auction)?;
        if target == crate::time_panel::Button::Speed
            && self.live_status.as_ref().is_some_and(|s| s.network_enabled)
        {
            return None;
        }
        let Some(g) = &self.ui_graph else {
            return Some(target);
        };
        use camera_live::ui_graph::PanelId;
        match target {
            crate::time_panel::Button::Options | crate::time_panel::Button::ScenarioReminder => {
                g.input(PanelId::Clock).then_some(target)
            }
            crate::time_panel::Button::Manager => g.input(PanelId::E).then_some(target),
            crate::time_panel::Button::Speed => {
                (g.input(PanelId::E) && g.speed_disabled == 0).then_some(target)
            }
        }
    }
    /// A press of the clock's Options button is in progress.
    pub(super) fn options_button_pressed(&self) -> bool {
        self.time_pressed == Some(crate::time_panel::Button::Options)
    }
    /// Whether this left-button edge belongs to the clock's Options button:
    /// a press over it, or the release of a press that began on it.
    pub(super) fn options_button_input(&self, assets: &crate::time_panel::Assets) -> bool {
        if self._fresh_session.is_none()
            || !self
                .live_status
                .as_ref()
                .is_some_and(|status| status.time_display.visible)
        {
            return false;
        }
        if self.options_button_pressed() {
            return true;
        }
        let auction = self
            .live_status
            .as_ref()
            .is_some_and(|status| status.auction.is_some());
        self.pointer
            .map(|p| p.map(|v| v as f32 * 0.5))
            .and_then(|point| self.time_target(assets, point, auction))
            == Some(crate::time_panel::Button::Options)
    }
    pub(super) fn time_tooltip(
        &self,
        assets: &crate::time_panel::Assets,
    ) -> Option<(crate::time_panel::Button, u32)> {
        if self.options_visible()
            || self.settings_visible()
            || self.save_visible()
            || self.stats_requested
            || self.prompts.request().is_some()
            || self.live_status.as_ref().is_none_or(|s| {
                !s.time_display.visible
                    || s.stats.is_some()
                    || s.market_share.is_some()
                    || s.tutorial_controls.all_disabled
            })
        {
            return None;
        }
        let point = self.pointer?.map(|v| v as f32 * 0.5);
        let auction = self
            .live_status
            .as_ref()
            .is_some_and(|s| s.auction.is_some());
        let target = self.time_target(assets, point, auction)?;
        let status = self.live_status.as_ref()?;
        let speed = self.time_panel.displayed_speed().unwrap_or(status.speed);
        target
            .tooltip(speed, status.network_enabled)
            .map(|id| (target, id))
    }
    pub(super) fn time_occupied(
        &self,
        assets: &crate::time_panel::Assets,
        point: [f32; 2],
        auction: bool,
    ) -> bool {
        let Some(g) = &self.ui_graph else {
            return assets.contains_in_mode(point, auction);
        };
        if !assets.contains_in_mode(point, auction) {
            return false;
        }
        match assets.button_in_mode(point, auction) {
            Some(crate::time_panel::Button::Speed | crate::time_panel::Button::Manager) => {
                g.input(camera_live::ui_graph::PanelId::E)
            }
            _ => g.input(camera_live::ui_graph::PanelId::Clock),
        }
    }
    pub(super) fn secondary_enabled(&self) -> bool {
        self.ui_graph
            .as_ref()
            .is_none_or(|g| g.group_enabled(camera_live::navigation::Group::Secondary))
    }
    pub(super) fn options_visible(&self) -> bool {
        self.secondary_enabled() && self.game_options.open
    }
    pub(super) fn save_visible(&self) -> bool {
        self.secondary_enabled() && self.save_dialog.visible
    }
    /// Whether an 88c0d4 confirmation is raised into the secondary group:
    /// Save Game's Overwrite or Delete, or Options Quit.
    pub(super) fn secondary_confirmation_open(&self) -> bool {
        self.save_visible() && self.save_dialog.generic_prompt().is_some()
            || self.options_visible() && self.game_options.quit_prompt.request().is_some()
    }
    /// Advance the secondary group's fade to the worker's interface samples.
    pub(super) fn observe_secondary_fade(&mut self, interface_ticks: u64) {
        let modal = self.secondary_confirmation_open();
        self.secondary_fade.observe(interface_ticks, modal);
    }
    pub(super) fn settings_visible(&self) -> bool {
        self.secondary_enabled() && self.settings_menu.page.is_some()
    }
    pub(super) fn auction_visible(&self) -> bool {
        self.auction.scene.is_some()
            && self
                .ui_graph
                .as_ref()
                .is_none_or(|g| g.group_enabled(camera_live::navigation::Group::Tertiary))
    }
    pub(super) fn graph_input(&self, id: camera_live::ui_graph::PanelId) -> bool {
        self.ui_graph.as_ref().is_some_and(|graph| graph.input(id))
    }
    pub(super) fn graph_body(&self, id: camera_live::ui_graph::PanelId) -> bool {
        self.ui_graph.as_ref().is_some_and(|graph| graph.body(id))
    }
    pub(super) fn graph_group(&self, group: camera_live::navigation::Group) -> bool {
        self.ui_graph
            .as_ref()
            .is_some_and(|graph| graph.group_enabled(group))
    }
    pub(super) fn normal_star_detail(&self) -> Option<u8> {
        (self.result.is_none()
            && self.result_pending.is_none()
            && !self
                .live_status
                .as_ref()
                .is_some_and(|status| status.result_request.is_some()))
        .then_some(self.settings_menu.graphics.detail)
    }
    pub(super) fn open_options(&mut self) -> Result<(), String> {
        self.game_options
            .open(self.live_status.as_ref().is_some_and(|s| s.running));
        // Before the first worker status, keep Save inert; authoritative
        // tutorial state refreshes the row as soon as Status arrives.
        self.game_options.set_save_disabled(true);
        self.options_ack.cancel();
        if let Some(sender) = &self._fresh_session {
            let (reply, result) = mpsc::channel();
            sender
                .send(FreshRequest::Options { open: true, reply })
                .map_err(|e| e.to_string())?;
            self.options_ack.begin(true, result);
        }
        // Status carries drained, exactly-once prompt events. Retain its
        // receiver across menu transitions, then request current state again.
        self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
        self.mouse_buttons = [false; 2];
        Ok(())
    }
    pub(super) fn close_options(&mut self) -> Result<(), String> {
        self.game_options.close();
        self.options_ack.cancel();
        if let Some(sender) = &self._fresh_session {
            let (reply, result) = mpsc::channel();
            sender
                .send(FreshRequest::Options { open: false, reply })
                .map_err(|e| e.to_string())?;
            self.options_ack.begin(false, result);
        }
        self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        Ok(())
    }
    pub(super) fn queue_camera_event(
        &mut self,
        event: camera_producer::Event,
        building: Option<camera_producer::ExpectedBuilding>,
        adjustment: Option<crate::business_adjustment::State>,
        open_adjustment: bool,
    ) -> Result<&mut camera_producer::Pending, String> {
        let turn = camera_producer::Turn {
            epoch: self
                .camera_epoch
                .ok_or("Camera UI has no actual Session origin")?,
            building,
        };
        let sender = self
            ._fresh_session
            .as_ref()
            .ok_or("Camera UI Session closed")?;
        camera_producer::enqueue(
            &mut self.camera_turns,
            sender,
            turn,
            event,
            adjustment,
            open_adjustment,
        )?;
        Ok(self.camera_turns.back_mut().expect("enqueued camera turn"))
    }
    pub(super) fn queue_actor_camera(
        &mut self,
        selected: crate::selection::Hit,
        assets: &crate::selection::Assets,
        origin: camera_producer::ActorOrigin,
        adjustment: Option<crate::business_adjustment::State>,
        open_adjustment: bool,
    ) -> Result<&mut camera_producer::Pending, String> {
        let expected = camera_producer::identity_for_hit(&self.scene, assets, selected)?;
        self.queue_camera_event(
            camera_producer::Event::Actor {
                selected,
                auto_rotate: self.settings_menu.gameplay.auto_rotate,
                origin,
            },
            Some(expected),
            adjustment,
            open_adjustment,
        )
    }
    pub(super) fn commit_camera(&mut self) {
        if let Some(producer) = &mut self.camera_producer {
            if let Err(error) = producer.commit(self.camera, if self.strategic { 3 } else { 0 }) {
                self.game_options.set_save_disabled(true);
                self.save_dialog.message = Some(format!("Save failed: {error}"));
            }
        }
    }
    pub(super) fn toggle_navigation(&mut self, navigation: &crate::navigation::Navigation) {
        if self.camera_producer.is_some() {
            let post = self.camera_control_post(73);
            match self.queue_camera_event(
                camera_producer::Event::Compass {
                    strategic: navigation.camera,
                },
                None,
                None,
                false,
            ) {
                Ok(pending) => pending.posts.extend(post),
                Err(error) => self.save_dialog.message = Some(error),
            }
            return;
        }
        if let Some(locator) = &mut self.locator {
            locator.cancel_press();
        }
        let current = camera_producer::Controls::read(self);
        if let Some(normal) = self.return_camera.take() {
            normal.put(self);
            self.strategic = false;
        } else {
            self.return_camera = Some(current);
            self.camera = navigation.camera;
            self.selected = None;
            self.strategic = true;
        }
        self.commit_camera();
        self.send_tutorial_control(73);
        self.hovered = None;
        self.title_state = None;
        self.reframe_pending = true;
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
    }
    pub(super) fn apply_tutorial_camera(
        &mut self,
        action: crate::live_host::TutorialCameraAction,
        selection: &crate::selection::Assets,
        navigation: &crate::navigation::Navigation,
    ) -> Result<(), String> {
        use crate::live_host::TutorialCameraAction;
        if self.camera_producer.is_some() {
            match action {
                TutorialCameraAction::Mode { mode: 12, target } => {
                    self.queue_camera_event(
                        camera_producer::Event::script_block(target, selection)?,
                        None,
                        None,
                        false,
                    )?;
                }
                TutorialCameraAction::Mode { mode: 13, target } => {
                    let index = self
                        .scene
                        .inspection_buildings
                        .iter()
                        .position(|b| {
                            b.business().is_some_and(|b| b.slot == target as usize)
                                || b.initialized_apartment
                                    .as_ref()
                                    .is_some_and(|b| b.slot == target as usize)
                        })
                        .ok_or("Tutorial live building identity not presented")?;
                    let b = &self.scene.inspection_buildings[index];
                    let hit = inspection_hit(&selection.blocks, b.category, Some(index))
                        .ok_or("Tutorial category identity missing")?;
                    self.queue_actor_camera(
                        hit,
                        selection,
                        camera_producer::ActorOrigin::Tutorial,
                        None,
                        false,
                    )?;
                }
                TutorialCameraAction::PlanView => {
                    self.queue_camera_event(
                        camera_producer::Event::ChooseView {
                            mode: 3,
                            strategic: navigation.camera,
                        },
                        None,
                        None,
                        false,
                    )?;
                }
                TutorialCameraAction::CityView => {
                    self.queue_camera_event(
                        camera_producer::Event::ChooseView {
                            mode: 0,
                            strategic: navigation.camera,
                        },
                        None,
                        None,
                        false,
                    )?;
                }
                TutorialCameraAction::Mode { mode, target } => {
                    return Err(format!(
                        "Unsupported tutorial camera mode{mode} target{target}"
                    ));
                }
            }
            return Ok(());
        }
        match action {
            TutorialCameraAction::Mode { mode: 12, target } => {
                let category = selection
                    .blocks
                    .iter()
                    .position(|block| block.id == target as usize)
                    .ok_or_else(|| format!("tutorial block {target} leaves original board"))?;
                self.focus_category(category, selection);
            }
            TutorialCameraAction::PlanView => {
                // SetPlanView calls choose_view(3), not the compass's4baf10
                // copy transaction. Its unjoined raw boundary cannot invent S.
                self.return_camera = Some(camera_producer::Controls::read(self));
                self.camera = navigation.camera;
                self.selected = None;
                self.strategic = true;
                self.reframe_pending = true;
            }
            TutorialCameraAction::CityView => {
                self.inertia = openmt2001::camera::BlockCameraInertia::new(
                    self.camera.eye,
                    self.camera.target,
                );
                self.strategic = false;
                self.return_camera = None;
                self.reframe_pending = true;
            }
            TutorialCameraAction::Mode { mode: 13, target } => {
                // Native 4d9d80 interprets target as a business slot, then
                // 462f30 resolves its parent building before 4c3a20 frames it.
                let building_index = self
                    .scene
                    .inspection_buildings
                    .iter()
                    .position(|building| {
                        building
                            .initialized_business
                            .as_ref()
                            .is_some_and(|business| business.slot == target as usize)
                            || building
                                .saved_business
                                .as_ref()
                                .is_some_and(|business| business.slot == target as usize)
                    })
                    .ok_or_else(|| {
                        format!("tutorial business {target} has no rendered building")
                    })?;
                let building = &self.scene.inspection_buildings[building_index];
                let hit =
                    inspection_hit(&selection.blocks, building.category, Some(building_index))
                        .ok_or("tutorial building category leaves original board")?;
                let category = hit.category;
                let (controls, angle) = crate::navigation::building_entry(
                    selection.blocks[category].center,
                    [building.translation[0], building.translation[2]],
                    building.camera_entrance,
                    building.kind == 2,
                    self.controls.angle,
                    self.building_angle,
                )
                .ok_or("tutorial building camera lacks original entrance")?;
                self.selected = Some(hit);
                self.inertia = openmt2001::camera::BlockCameraInertia::new(
                    self.camera.eye,
                    self.camera.target,
                );
                self.controls = controls;
                self.building_angle = angle;
                self.building_camera = true;
                self.strategic = false;
                self.return_camera = None;
                self.reframe_pending = true;
            }
            TutorialCameraAction::Mode { mode, target } => {
                return Err(format!(
                    "tutorial camera mode {mode} target {target} is unsupported"
                ));
            }
        }
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
        self.mouse_buttons = [false; 2];
        self.commit_camera();
        Ok(())
    }
    pub(super) fn camera_control_post(&self, id: u32) -> Option<camera_producer::Post> {
        (self.live_status.as_ref().is_some_and(|status| {
            status.failure.is_none() && status.tutorial_enabled_controls.contains(&id)
        }) && self._fresh_session.is_some())
        .then_some(camera_producer::Post::TutorialControl(id))
    }
    pub(super) fn send_tutorial_control(&mut self, id: u32) {
        if self.live_status.as_ref().is_some_and(|status| {
            status.failure.is_none() && status.tutorial_enabled_controls.contains(&id)
        }) && let Some(sender) = &self._fresh_session
        {
            let _ = sender.send(FreshRequest::TutorialControl(id));
            self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
    }
    pub(super) fn send_tutorial_producer(&mut self, control: u32, event: u32) {
        if self
            .live_status
            .as_ref()
            .is_some_and(|s| s.tutorial_enabled_controls.contains(&control))
            && let Some(sender) = &self._fresh_session
        {
            let _ = sender.send(FreshRequest::TutorialProducer { control, event });
            self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
    }
    pub(super) fn send_tutorial_pick(&mut self, pick: crate::tutorial_two_interaction::Pick) {
        if let Some(sender) = &self._fresh_session {
            let _ = sender.send(FreshRequest::TutorialPick(pick));
            self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
    }
    /// Shared by the business panel's parent-block control and Home (manual p49).
    pub(super) fn focus_parent_block(&mut self, selection: &crate::selection::Assets) {
        let Some(hit) = self.selected.filter(|hit| hit.building.is_some()) else {
            return;
        };
        if self.camera_producer.is_some() {
            let post = self.camera_control_post(14);
            match self.queue_camera_event(
                camera_producer::Event::select_block(hit.category, selection),
                None,
                None,
                false,
            ) {
                Ok(pending) => pending.posts.extend(post),
                Err(error) => self.save_dialog.message = Some(error),
            }
        } else {
            self.focus_category(hit.category, selection);
            self.send_tutorial_control(14);
        }
    }
    pub(super) fn focus_category(&mut self, category: usize, selection: &crate::selection::Assets) {
        if self.camera_producer.is_some() {
            if let Err(error) = self.queue_camera_event(
                camera_producer::Event::select_block(category, selection),
                None,
                None,
                false,
            ) {
                self.save_dialog.message = Some(error);
            }
            return;
        }
        self.selected = Some(crate::selection::Hit {
            category,
            building: None,
        });
        self.inertia =
            openmt2001::camera::BlockCameraInertia::new(self.camera.eye, self.camera.target);
        self.controls = crate::navigation::block_entry(selection.center(category));
        self.building_camera = false;
        self.strategic = false;
        self.return_camera = None;
        self.reframe_pending = true;
        self.title_state = None;
        self.hovered = None;
        self.adjustment = None;
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
        self.commit_camera();
    }
    pub(super) fn reply_human_dialog(
        &mut self,
        response: crate::human_dialog::Reply,
    ) -> Result<(), String> {
        let sender = self
            ._fresh_session
            .as_ref()
            .ok_or("Human dialog has no live session")?;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::HumanDialog(response, reply))
            .map_err(|e| e.to_string())?;
        self.prompt_reply = Some(answer);
        Ok(())
    }
    pub(super) fn dismiss_prompt(&mut self, id: u32) -> Result<(), String> {
        let sender = self
            ._fresh_session
            .as_ref()
            .ok_or("prompt has no live session")?;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::DismissPrompt(id, Some(reply)))
            .map_err(|e| e.to_string())?;
        self.prompt_reply = Some(answer);
        Ok(())
    }
    pub(super) fn ordinary_toolbar_category(
        &self,
        blocks: &[openmt2001::board::Block],
    ) -> Option<usize> {
        if self.strategic
            || self.operations.building.is_some()
            || !self.selected.is_some_and(|hit| hit.building.is_none())
        {
            return None;
        }
        self.utility_category(blocks).filter(|&category| {
            self.block_information.as_ref().is_some_and(|report| {
                report.available(category) && report.categories[category].is_some()
            })
        })
    }
    pub(super) fn toolbar_policy(
        &self,
        menu: &crate::menu::Menu,
        blocks: &[openmt2001::board::Block],
        input: bool,
    ) -> Option<crate::locator::toolbar_collapse::Policy> {
        use crate::locator::toolbar_collapse::Action;
        let category = self.ordinary_toolbar_category(blocks)?;
        let mut shown = self.main_panel_visible()
            && !self.game_options.open
            && self.planning.is_none()
            && self.adjustment.is_none();
        let mut allowed = Action::ALL.map(|a| {
            self.live_status.as_ref().is_none_or(|s| {
                !s.tutorial_session || s.tutorial_enabled_controls.contains(&a.tutorial())
            })
        });
        if input {
            shown &= !self.toolbar_right_held
                && self.settings_menu.page.is_none()
                && !self.save_dialog.visible
                && !self.stats_requested
                && !self.live_status.as_ref().is_some_and(|s| s.stats.is_some())
                && self.planning_pending.is_none()
                && self.auction.scene.is_none();
            let pointer = self.pointer.map(|p| p.map(|v| v as f32 * 0.5));
            let [l, t, r, b] = menu.card_assets().entry;
            let entry_hit = pointer.is_some_and(|[x, y]| x >= l && x <= r && y >= t && y <= b);
            if entry_hit
                || self.card_input.captured()
                || self.live_status.as_ref().is_some_and(|s| s.card.is_some())
                    && menu.card_assets().contains(pointer)
            {
                shown = false;
            }
            if self.prompts.request().is_some() {
                let occupied = self.prompts.input_captured()
                    || self.pointer.is_some_and(|p| {
                        menu.prompt_assets().input_occupied(
                            p.map(|v| v as f32 * 0.5),
                            self.live_status
                                .as_ref()
                                .map_or([0.; 2], |s| s.tutorial_interaction.prompt_offset()),
                        )
                    });
                shown &= !occupied;
                for a in Action::ALL {
                    allowed[a.index()] &= self
                        .live_status
                        .as_ref()
                        .is_some_and(|s| s.tutorial_enabled_controls.contains(&a.tutorial()));
                }
            }
            if self
                .live_status
                .as_ref()
                .is_some_and(|s| s.event_list_visible && self.auction.scene.is_none())
                && self
                    .pointer
                    .is_some_and(|p| menu.event_list_assets().contains(p.map(|v| v as f32 * 0.5)))
            {
                shown = false;
            }
        }
        let build = self._fresh_session.is_some()
            && !self
                .live_status
                .as_ref()
                .is_some_and(|s| s.auction_locked_categories.contains(&category));
        Some(menu.locator_assets().ordinary_toolbar_policy(
            self.block_information.as_ref()?,
            category,
            shown,
            build,
            allowed,
        ))
    }
    pub(super) fn toolbar_target(
        &self,
        menu: &crate::menu::Menu,
        blocks: &[openmt2001::board::Block],
    ) -> Option<crate::locator::toolbar_collapse::Action> {
        let policy = self.toolbar_policy(menu, blocks, true)?;
        policy.target(self.toolbar_bounds, self.pointer?.map(|v| v as f32 * 0.5))
    }
    pub(super) fn begin_toolbar_planning(&mut self, category: usize) {
        if let Some(sender) = &self._fresh_session {
            let (reply, result) = mpsc::channel();
            if sender
                .send(FreshRequest::Planning { category, reply })
                .is_ok()
            {
                self.planning_pending = Some(result);
                self.send_tutorial_control(11);
            }
        }
    }
    pub(super) fn main_panel_visible(&self) -> bool {
        if !self.city_layers().hud_visible() {
            return false;
        }
        if self.ui_graph.is_some() {
            return self.graph_body(camera_live::ui_graph::PanelId::A);
        }
        self.live_status.as_ref().is_none_or(|s| s.player_panels[0])
    }
    pub(super) fn statistics_visible(&self) -> bool {
        if !self.city_layers().hud_visible() {
            return false;
        }
        if self.ui_graph.is_some() {
            return self.graph_body(camera_live::ui_graph::PanelId::Comparison);
        }
        self.live_status.as_ref().is_none_or(|s| s.player_panels[1])
    }
    pub(super) fn over_navigation(&self) -> bool {
        self.city_layers().hud_visible()
            && (self.ui_graph.is_none() || self.graph_input(camera_live::ui_graph::PanelId::E))
            && self.pointer.is_some_and(|[x, y]| {
                let [l, t, r, b] = self.navigation_bounds;
                let x = x as f32 * 0.5;
                let y = y as f32 * 0.5;
                x >= l && x <= r && y >= t && y <= b
            })
    }
    pub(super) fn over_details(&self) -> bool {
        self.main_panel_visible()
            && self.pointer.is_some_and(|[x, y]| {
                let [l, t, r, b] = self.detail_bounds;
                let x = x as f32 * 0.5;
                let y = y as f32 * 0.5;
                x >= l && x <= r && y >= t && y <= b
            })
    }
    pub(super) fn over_navigator(&self) -> bool {
        self.city_layers().hud_visible()
            && !self.strategic
            && (self.ui_graph.is_none() || self.graph_input(camera_live::ui_graph::PanelId::F))
            && self.pointer.is_some_and(|[x, y]| {
                let [l, t, r, b] = self.navigator_bounds;
                let x = x as f32 * 0.5;
                let y = y as f32 * 0.5;
                x >= l && x <= r && y >= t && y <= b
            })
    }
    pub(super) fn utility_category(&self, blocks: &[openmt2001::board::Block]) -> Option<usize> {
        let selected = self.selected.map(|hit| {
            if hit.building.is_some_and(|i| {
                self.scene
                    .inspection_buildings
                    .get(i)
                    .is_some_and(|building| !building_has_management_link(building))
            }) {
                crate::selection::Hit {
                    category: hit.category,
                    building: None,
                }
            } else {
                hit
            }
        });
        utility_report_category(selected, blocks, self.block_information.as_ref())
    }
    pub(super) fn scene_pointer(&self) -> Option<[i32; 2]> {
        self.pointer.filter(|[x, y]| {
            if self.adjustment.is_some()
                || self.planning.is_some()
                || self.planning_pending.is_some()
                || (self.statistics_visible()
                    && self
                        .comparison
                        .as_ref()
                        .is_some_and(|c| c.contains([*x, *y])))
                || (self.operations.building.is_some()
                    && self.main_panel_visible()
                    && self.details_visible
                    && !self.strategic
                    && *x >= 0
                    && *x <= 392
                    && *y >= 0
                    && *y <= 640)
                || self.locator_blocked
                || self.over_navigation()
                || self.over_details()
                || (self.main_panel_visible()
                    && !self.strategic
                    && self.operations.building.is_none()
                    && self.toolbar_regions.iter().flatten().any(|b| {
                        crate::locator::toolbar_collapse::hit(
                            *b,
                            [*x as f32 * 0.5, *y as f32 * 0.5],
                        )
                    }))
                || self.over_navigator()
            {
                return false;
            }
            self.information
                .iter()
                .filter(|_| self.main_panel_visible() && self.details_visible && !self.strategic)
                .all(|(_, [l, t, r, b])| {
                    let x = *x as f32 * 0.5;
                    let y = *y as f32 * 0.5;
                    x < *l || x > *r || y < *t || y > *b
                })
        })
    }
}
