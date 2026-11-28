//! `impl Intro`: inspection_key .. inspection_unfocus, split out of `intro.rs` (#99).
use super::*;

impl Intro {
    /// Record a window key event in the city's physical key history and say
    /// whether a city shortcut may act on it; see `city_keys`. Called for
    /// every key event, so a key released outside a city does not stay held.
    pub fn admit_city_key(
        &mut self,
        key: winit::keyboard::NamedKey,
        pressed: bool,
        repeat: bool,
        synthetic: bool,
    ) -> bool {
        self.city_keys.admit(key, pressed, repeat, synthetic)
    }
    /// A window key event while a city is shown. `admitted` comes from
    /// `admit_city_key`: Escape acts once per physical press.
    pub fn inspection_keyboard(
        &mut self,
        key: winit::keyboard::NamedKey,
        pressed: bool,
        admitted: bool,
    ) -> Result<(), String> {
        if !admitted {
            // The result sequence's skip reads held levels, not new presses.
            if let Some(result) = self.inspection.as_mut().and_then(|v| v.result.as_mut()) {
                result.bypass.key(key, pressed);
            }
            return Ok(());
        }
        self.inspection_key(key, pressed)
    }
    pub fn inspection_key(
        &mut self,
        key: winit::keyboard::NamedKey,
        pressed: bool,
    ) -> Result<(), String> {
        self.service_camera_turns();
        let result = self.inspection_key_inner(key, pressed);
        if let Some(view) = &mut self.inspection {
            view.commit_camera();
        }
        result
    }
    pub(super) fn inspection_key_inner(
        &mut self,
        key: winit::keyboard::NamedKey,
        pressed: bool,
    ) -> Result<(), String> {
        use winit::keyboard::NamedKey;
        // Releases must retire held arrows even when a modal consumes the event.
        // Keep admitted taps until the camera callback consumes them.
        if !pressed && let Some(view) = &mut self.inspection {
            let index = match key {
                NamedKey::ArrowRight => Some(0),
                NamedKey::ArrowLeft => Some(1),
                NamedKey::ArrowDown => Some(2),
                NamedKey::ArrowUp => Some(3),
                _ => None,
            };
            if let Some(i) = index {
                view.keys[i] = false;
            }
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.result.is_some()
                || view.result_pending.is_some()
                || view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.result_request.is_some())
        }) {
            if let Some(result) = self
                .inspection
                .as_mut()
                .and_then(|view| view.result.as_mut())
            {
                result.bypass.key(key, pressed);
            }
            return Ok(());
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.stats_requested
                || view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.stats.is_some())
        }) {
            return Ok(());
        }
        if pressed && let Some(view) = &mut self.inspection {
            if view.game_options.quit_prompt.request().is_some() {
                // Physical Return/Backspace edges are sampled by dialog_keys.
                return Ok(());
            }
            if view.save_visible() {
                match key {
                    NamedKey::Escape => {
                        view.save_dialog.escape();
                    }
                    NamedKey::Backspace => view.save_dialog.erase(),
                    NamedKey::Enter => {
                        if let Some(action) = view.save_dialog.enter() {
                            Self::submit_save_dialog_action(view, action);
                        }
                    }
                    _ => (),
                }
                return Ok(());
            }
            if view.prompts.blocks_controls() {
                // A notice's Return is the physical edge sampled by dialog_keys.
                return Ok(());
            }
            if view.options_visible() {
                match city_keys::options_key(key, view.settings_visible()) {
                    Some(city_keys::OptionsKey::Back) => view.settings_menu.back(),
                    Some(city_keys::OptionsKey::Close) => view.close_options()?,
                    None => (),
                }
                return Ok(());
            }
            if view.chat_panel.open {
                match key {
                    NamedKey::Escape => view.chat_panel.close(),
                    NamedKey::Backspace => {
                        view.chat_panel.input.pop();
                    }
                    NamedKey::Enter => {
                        if let Some(chat) = view.live_status.as_ref().and_then(|s| s.chat.as_ref())
                            && let Some((text, recipients)) = view.chat_panel.take_submission(chat)
                            && let Some(sender) = &view._fresh_session
                        {
                            // A closed channel means the session already ended.
                            let _ = sender.send(FreshRequest::Chat(text, recipients));
                        }
                    }
                    _ => (),
                }
                return Ok(());
            }
            if key == NamedKey::Escape
                && view._fresh_session.is_some()
                && view.planning.is_none()
                && view.planning_pending.is_none()
                && view.adjustment.is_none()
            {
                view.open_options()?;
                return Ok(());
            }
            if view
                .live_status
                .as_ref()
                .is_some_and(|status| status.tutorial_controls.all_disabled)
                && key != NamedKey::Escape
            {
                return Ok(());
            }
        }
        if key == NamedKey::Escape && pressed {
            if let Some(view) = &mut self.inspection
                && (view.planning.take().is_some() || view.planning_pending.take().is_some())
            {
                return Ok(());
            }
            if let Some(view) = &mut self.inspection
                && let Some(adjustment) = &mut view.adjustment
            {
                if adjustment.request_close() {
                    view.adjustment = None;
                }
                return Ok(());
            }
            self.retire_inspection();
            self.last_sample = None;
            return Ok(());
        }
        if let Some(view) = &mut self.inspection {
            if view.auction_visible()
                || view.strategic
                || view.adjustment.is_some()
                || view.planning.is_some()
                || view.live_status.as_ref().is_some_and(|status| {
                    status.tutorial_controls.camera_disabled
                        || status.tutorial_controls.all_disabled
                })
            {
                return Ok(());
            }
            let index = match key {
                NamedKey::ArrowRight => Some(0),
                NamedKey::ArrowLeft => Some(1),
                NamedKey::ArrowDown => Some(2),
                NamedKey::ArrowUp => Some(3),
                _ => None,
            };
            if let Some(i) = index
                && self.process_frontend.is_none()
            {
                view.keys[i] = pressed;
                view.taps[i] |= pressed;
            }
            // Height keys remain a diagnostic host control, separate from native mouse zoom.
            if pressed && matches!(key, NamedKey::PageUp | NamedKey::PageDown) {
                view.controls.height = page_height(view.controls.height, key == NamedKey::PageUp);
            }
        }
        Ok(())
    }
    pub fn inspection_text(&mut self, text: &str) {
        if let Some(view) = &mut self.inspection
            && view.chat_panel.open
        {
            view.chat_panel.text(text);
            return;
        }
        if let Some(view) = &mut self.inspection
            && view.options_visible()
            && view.save_visible()
        {
            view.save_dialog.input(&self.assets.save_dialog, text);
        }
    }
    pub(super) fn submit_save_dialog_action(
        view: &mut Inspection,
        action: crate::save_dialog::Action,
    ) {
        use crate::save_dialog::Action;
        if !matches!(&action, Action::Back) && !view.options_ack.ready() {
            view.save_dialog.message = Some("Preparing Save Game...".into());
            return;
        }
        match action {
            Action::Back => (),
            Action::NewName(name) => {
                Self::submit_save_target(view, crate::save_menu::Target::NewName(name))
            }
            Action::ExistingSlot(id) => {
                Self::submit_save_target(view, crate::save_menu::Target::ExistingSlot(id))
            }
            Action::DeleteSlot(id) => {
                if view.save_delete_pending.is_some() {
                    return;
                }
                let Some(sender) = &view._fresh_session else {
                    view.save_dialog.message = Some("Delete unavailable in this view".into());
                    return;
                };
                let (reply, result) = mpsc::channel();
                match sender.send(FreshRequest::DeleteSave { id, reply }) {
                    Ok(()) => {
                        view.save_delete_pending = Some(result);
                        view.save_dialog.busy = true;
                        view.save_dialog.message = Some("Deleting...".into());
                    }
                    Err(_) => {
                        view.save_dialog.message = Some("Delete failed: session closed".into())
                    }
                }
            }
        }
    }
    pub(super) fn submit_save_target(view: &mut Inspection, target: crate::save_menu::Target) {
        if let Some(error) = view.camera_producer.as_ref().and_then(|p| p.error()) {
            view.save_dialog.message = Some(format!("Save failed: {error}"));
            return;
        }
        if view.save_pending.is_some() || view.camera_capture_pending.is_some() {
            return;
        }
        if !view.options_ack.ready() {
            view.save_dialog.message = Some("Preparing Save Game...".into());
            return;
        }
        if let Some(error) = view
            .live_status
            .as_ref()
            .and_then(|status| status.failure.as_ref())
        {
            view.save_dialog.busy = false;
            view.save_dialog.message = Some(format!("Save failed: {error}"));
            return;
        }
        if let Some(state) = &mut view.camera_ui {
            view.camera_capture_pending = Some((target, state.queue_save_capture()));
            view.save_dialog.busy = true;
            view.save_dialog.message = Some("Saving...".into());
            return;
        }
        Self::send_save_target(view, target, None);
    }
    pub(super) fn poll_camera_save(view: &mut Inspection) {
        let Some((_, receiver)) = &view.camera_capture_pending else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(value) => value,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("Camera capture boundary closed".into()),
        };
        let (target, _) = view.camera_capture_pending.take().unwrap();
        match result {
            Ok(capture) => Self::send_save_target(view, target, Some(capture)),
            Err(error) => {
                view.save_dialog.busy = false;
                view.save_dialog.message = Some(format!("Save failed: {error}"));
            }
        }
    }
    pub(super) fn send_save_target(
        view: &mut Inspection,
        target: crate::save_menu::Target,
        capture: Option<openmt2001::camera::bank::Capture>,
    ) {
        if !view.options_ack.ready() {
            view.save_dialog.busy = false;
            view.save_dialog.message = Some("Save cancelled: Options closed".into());
            return;
        }
        let Some(sender) = &view._fresh_session else {
            view.save_dialog.busy = false;
            view.save_dialog.message = Some("Save unavailable in this view".into());
            return;
        };
        let (reply, result) = mpsc::channel();
        let request = FreshRequest::SaveSelected {
            target,
            camera: view.camera,
            capture,
            game_mode: if view.auction.scene.is_some() { 2 } else { 1 },
            interface_mode: if view.strategic { 3 } else { 0 },
            reply,
        };
        match sender.send(request) {
            Ok(()) => {
                view.save_pending = Some(result);
                view.save_dialog.busy = true;
                view.save_dialog.message = Some("Saving...".into());
            }
            Err(_) => {
                view.save_dialog.busy = false;
                view.save_dialog.message = Some("Save failed: session closed".into());
            }
        }
    }
    pub fn inspection_cursor(&mut self, position: [f64; 2], size: [u32; 2]) {
        if let Some(view) = &mut self.inspection {
            if view.adjustment.is_none()
                && !view.options_visible()
                && view.auction.scene.is_none()
                && !view.prompts.blocks_controls()
                && !view.stats_requested
                && !view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.stats.is_some())
                && !view.live_status.as_ref().is_some_and(|status| {
                    status.tutorial_controls.camera_disabled
                        || status.tutorial_controls.all_disabled
                })
                && !view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.market_share.is_some())
                && view.mouse_buttons[1]
                && self.process_frontend.is_none()
                && let Some(previous) = view.cursor
            {
                for i in 0..2 {
                    view.drag[i] = view.drag[i].saturating_add(
                        (position[i] - previous[i]).round().clamp(-4096., 4096.) as i32,
                    );
                }
            }
            view.cursor = Some(position);
            view.pointer = internal_pointer(position, size);
            let time_input = !view
                .live_status
                .as_ref()
                .is_some_and(|s| s.market_share.is_some())
                && !view.stats_requested
                && !view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.stats.is_some())
                && !view.options_visible()
                && view.settings_menu.page.is_none()
                && !view.prompts.blocks_controls()
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.time_display.visible);
            let time_pointer = time_input
                .then_some(view.pointer)
                .flatten()
                .map(|point| point.map(|v| v as f32 * 0.5));
            let auction_time = view
                .live_status
                .as_ref()
                .is_some_and(|status| status.auction.is_some());
            let time_hit = time_pointer
                .and_then(|point| view.time_target(&self.assets.time_panel, point, auction_time));
            if view
                .time_panel
                .graph_cursor(time_hit, time_input && view.mouse_buttons[0])
                .is_some()
                && let Err(error) = view
                    .gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Time held re-entry sound: {error}");
            }
            if let (Some(menu), Some(snapshot)) = (
                &self.menu,
                view.live_status
                    .as_ref()
                    .and_then(|status| status.stats.as_ref()),
            ) {
                let _ = view.stats_panel.pointer(
                    menu.stats_assets(),
                    view.pointer.map(|point| point.map(|v| v as f32 * 0.5)),
                    view.mouse_buttons[0],
                    snapshot,
                );
            }
            if view.secondary_enabled()
                && matches!(
                    view.settings_menu.page,
                    Some(
                        crate::settings_menu::Page::Sound
                            | crate::settings_menu::Page::Graphics
                            | crate::settings_menu::Page::Gameplay
                    )
                )
            {
                let distance = view.settings_menu.graphics.distance;
                if view.settings_menu.drag(
                    &self.assets.settings_menu,
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    &mut self.audio_levels,
                ) {
                    Self::apply_audio_levels(&self.process_frontend, self.audio_levels);
                }
                if view.settings_menu.graphics.distance != distance {
                    view.reframe_pending = true;
                }
            }
            if view.settings_visible()
                && view.settings_menu.pointer_click_cue(
                    &self.assets.settings_menu,
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                )
                && let Err(error) = view
                    .gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Settings held re-entry sound: {error}");
            }
            if view.options_visible()
                && !view.save_visible()
                && view.settings_menu.page.is_none()
                && view.game_options.pointer_click_cue(
                    &self.assets.game_options,
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                )
                && let Err(error) = view
                    .gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Options held re-entry sound: {error}");
            }
            if view.save_visible() {
                let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                let cue = if view.save_dialog.generic_prompt().is_some() {
                    self.menu.as_ref().is_some_and(|menu| {
                        let bounds = menu.prompt_assets().dialog_buttons(2);
                        bounds.len() == 2
                            && view
                                .save_dialog
                                .prompt_click_cue(pointer, [bounds[0], bounds[1]])
                    })
                } else {
                    view.save_dialog
                        .pointer_click_cue(&self.assets.save_dialog, pointer)
                };
                if cue
                    && let Err(error) = view
                        .gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::UiLightClick)
                {
                    eprintln!("Save Game held re-entry sound: {error}");
                }
            }
            if view.auction_visible()
                && let (Some(menu), Some(auction)) = (
                    &self.menu,
                    view.live_status
                        .as_ref()
                        .and_then(|status| status.auction.as_ref()),
                )
                && view.auction_panel.pointer_click_cue(
                    menu.auction_panel_assets(),
                    auction,
                    view.pointer
                        .filter(|_| !view.prompts.blocks_controls() && !view.stats_requested)
                        .map(|point| point.map(|v| v as f32 * 0.5)),
                )
                && let Err(error) = view
                    .gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Auction held re-entry sound: {error}");
            }
            if let Some(planning) = &mut view.planning {
                let occupied = view.live_status.as_ref().is_some_and(|status| {
                    status.event_list_visible && view.auction.scene.is_none()
                }) && self.menu.as_ref().is_some_and(|menu| {
                    view.pointer.is_some_and(|point| {
                        menu.event_list_assets()
                            .contains(point.map(|v| v as f32 * 0.5))
                    })
                });
                planning.pointer(view.pointer.filter(|_| !occupied));
                if planning.pointer_click_cue()
                    && let Err(error) = view
                        .gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::UiLightClick)
                {
                    eprintln!("Planning held re-entry sound: {error}");
                }
            }
        }
    }
    pub fn inspection_scroll(&mut self, delta: i32) {
        if let Some(view) = &mut self.inspection
            && view.save_visible()
        {
            view.save_dialog.scroll(&self.assets.save_dialog, delta);
            return;
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.stats_requested
                || view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.stats.is_some())
        }) {
            return;
        }
        if self
            .inspection
            .as_ref()
            .is_some_and(|v| v.auction.scene.is_some() || v.prompts.blocks_controls())
        {
            return;
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.live_status
                .as_ref()
                .is_some_and(|status| status.tutorial_controls.all_disabled)
        }) {
            return;
        }
        if let Some(view) = &mut self.inspection
            && view.main_panel_visible()
            && view.details_visible
            && !view.strategic
            && view.operations.building.is_some()
            && view.operations.mode == 3
            && let Some(menu) = &self.menu
            && view
                .pointer
                .is_some_and(|p| menu.business_assets().contains(p.map(|v| v as f32 * 0.5)))
        {
            let rows = view
                .operations
                .inspection
                .as_ref()
                .map_or(0, |d| d.conversions.len());
            view.operations.scroll = (view.operations.scroll as i64 - i64::from(delta))
                .clamp(0, rows.saturating_sub(1) as i64)
                as usize;
            return;
        }
        if let Some(planning) = self
            .inspection
            .as_mut()
            .and_then(|view| view.planning.as_mut())
        {
            planning.scroll(delta);
            return;
        }
        if let Some(view) = &mut self.inspection
            && !view.strategic
            && view.main_panel_visible()
            && view.details_visible
            && view.over_details()
            && view.utility_report.details
            && let Some(category) = view.utility_category(&self.assets.selection.blocks)
            && let Some(report) = &view.block_information
            && report.categories[category].is_some()
            && let Some(menu) = &self.menu
        {
            view.utility_report.scroll = (view.utility_report.scroll as i64 - i64::from(delta))
                .clamp(
                    0,
                    report.land_rent[category]
                        .len()
                        .saturating_sub(menu.locator_assets().land_rent_visible_rows())
                        as i64,
                ) as usize;
            view.utility_report.dirty = true;
            return;
        }
        if let (Some(view), Some(menu)) = (&mut self.inspection, &self.menu)
            && view.strategic
            && view.locator_blocked
            && let Some(locator) = &mut view.locator
        {
            locator.scroll(menu.locator_assets(), delta);
        }
    }
    pub fn inspection_mouse(&mut self, button: winit::event::MouseButton, pressed: bool) {
        self.service_camera_turns();
        self.inspection_mouse_inner(button, pressed);
        if let Some(view) = &mut self.inspection {
            view.commit_camera();
        }
    }
    pub(super) fn inspection_mouse_inner(
        &mut self,
        button: winit::event::MouseButton,
        pressed: bool,
    ) {
        if let Some(view) = &mut self.inspection {
            if view
                .live_status
                .as_ref()
                .is_some_and(|s| s.result_review.is_some())
            {
                if let Some(menu) = &self.menu {
                    view.statistics_mouse(menu.stats_assets(), button, pressed);
                }
                return;
            }

            // Capture both edges BEFORE stats/save/settings/prompt/card/auction
            // returns. Do not move the camera dispatcher's late Right assignment.
            if button == winit::event::MouseButton::Right && view.toolbar_right_held != pressed {
                view.toolbar_right_held = pressed;
                view.toolbar_input.cancel();
            }
            // The result sequence replaces city input; 4b1980 tests both
            // buttons as cut-scene bypass input.
            if view.result.is_some()
                || view.result_pending.is_some()
                || view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.result_request.is_some())
            {
                if let Some(result) = &mut view.result {
                    result.bypass.button(button, pressed);
                }
                return;
            }
            // Mode4 captures input before any primary-group hit test, including
            // the installed navigator path and its staged GUI input owner.
            if let Some(menu) = &self.menu
                && view.statistics_mouse(menu.stats_assets(), button, pressed)
            {
                return;
            }
            if view.city_layers().statistics {
                return;
            }

            if view
                .live_status
                .as_ref()
                .is_some_and(|s| s.market_share.is_some())
                && !view.prompts.blocks_controls()
                && let Some(menu) = &self.menu
            {
                if button == winit::event::MouseButton::Left {
                    if view.market_share_panel.pointer(
                        menu.market_share_assets(),
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                        pressed,
                    ) && let Some(sender) = &view._fresh_session
                    {
                        let _ = sender.send(FreshRequest::DismissMarketShare);
                        view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
                    }
                    view.mouse_buttons[0] = pressed;
                }
                if button == winit::event::MouseButton::Right {
                    view.mouse_buttons[1] = pressed;
                    view.toolbar_right_held = pressed;
                    view.toolbar_input.cancel();
                }
                view.time_panel.cancel_input();
                view.time_pressed = None;
                view.auction_panel.cancel_press();
                view.drag = [0; 2];
                return;
            }
            if view.ui_graph.is_some() && button == winit::event::MouseButton::Left {
                //Read the SAME staged GUI owner; physical/display state
                //cannot overwrite old current/prior GUI cells before poll.
                let hit = camera_live::upper_f::panel_hit(view);
                let rejected=match hit{
                    Ok(false)=>None,
                    Ok(true)=>Some("F original parent/group button transaction is unjoined; legacy Board/release-camera path rejected".to_owned()),
                    Err(error)=>Some(error),
                };
                if let Some(error) = rejected {
                    if let Some(state) = &mut view.camera_ui {
                        state.reject_upper_input(error);
                    }
                    return;
                }
            }
            let side_policy = self
                .menu
                .as_ref()
                .and_then(|menu| view.toolbar_policy(menu, &self.assets.selection.blocks, true));
            view.toolbar_input.update(
                side_policy,
                view.toolbar_bounds,
                view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
            );
            if view.settings_visible() || view.options_visible() || view.prompts.blocks_controls() {
                view.time_panel.cancel_input();
                view.time_pressed = None;
            }
            if view.game_options.quit_prompt.request().is_some() {
                if button == winit::event::MouseButton::Left
                    && let Some(menu) = &self.menu
                {
                    view.game_options.quit_prompt.cursor(
                        view.pointer
                            .map(|p| p.map(|v| v as f32 * 0.5))
                            .unwrap_or([-1.; 2]),
                    );
                    view.game_options.quit_prompt.dialog_mouse(
                        pressed,
                        &menu.frontend_prompt_assets().frontend_dialog_buttons(2),
                    );
                }
                return;
            }
            if view.save_visible() {
                if button == winit::event::MouseButton::Left {
                    let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                    let action = if view.save_dialog.generic_prompt().is_some() {
                        self.menu.as_ref().and_then(|menu| {
                            let bounds = menu.prompt_assets().dialog_buttons(2);
                            (bounds.len() == 2)
                                .then(|| {
                                    view.save_dialog.mouse_generic(
                                        pointer,
                                        pressed,
                                        [bounds[0], bounds[1]],
                                    )
                                })
                                .flatten()
                        })
                    } else {
                        view.save_dialog
                            .mouse(&self.assets.save_dialog, pointer, pressed)
                    };
                    if let Some(action) = action {
                        Self::submit_save_dialog_action(view, action);
                    }
                    let cue = if view.save_dialog.generic_prompt().is_some() {
                        self.menu.as_ref().is_some_and(|menu| {
                            let bounds = menu.prompt_assets().dialog_buttons(2);
                            bounds.len() == 2
                                && view
                                    .save_dialog
                                    .prompt_click_cue(pointer, [bounds[0], bounds[1]])
                        })
                    } else {
                        view.save_dialog
                            .pointer_click_cue(&self.assets.save_dialog, pointer)
                    };
                    if cue
                        && let Err(error) = view
                            .gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Save Game button press sound: {error}");
                    }
                }
                return;
            }
            if view.settings_visible() {
                if button == winit::event::MouseButton::Left {
                    let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                    let distance = view.settings_menu.graphics.distance;
                    if view.settings_menu.mouse(
                        &self.assets.settings_menu,
                        pointer,
                        pressed,
                        &mut self.audio_levels,
                    ) {
                        Self::apply_audio_levels(&self.process_frontend, self.audio_levels);
                    }
                    if view.settings_menu.graphics.distance != distance {
                        view.reframe_pending = true;
                    }
                    if view
                        .settings_menu
                        .pointer_click_cue(&self.assets.settings_menu, pointer)
                        && let Err(error) = view
                            .gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Settings button press sound: {error}");
                    }
                }
                return;
            }
            if view.options_visible() {
                if button == winit::event::MouseButton::Left {
                    let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                    let activation =
                        view.game_options
                            .mouse(&self.assets.game_options, pointer, pressed);
                    if view
                        .game_options
                        .pointer_click_cue(&self.assets.game_options, pointer)
                        && let Err(error) = view
                            .gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Options button press sound: {error}");
                    }
                    if !pressed {
                        match activation {
                            Some(crate::game_options::Button::Continue) => {
                                if let Err(error) = view.close_options() {
                                    eprintln!("[Options] {error}");
                                }
                            }
                            // The menu action retained the city and opened
                            // its abandonment prompt. Its reply owns teardown.
                            Some(crate::game_options::Button::Quit) => {}
                            Some(crate::game_options::Button::Settings) => {
                                view.settings_menu.open_in_game();
                            }
                            Some(crate::game_options::Button::Save)
                                if view.save_pending.is_none() && view.options_ack.ready() =>
                            {
                                if let Some(root) = view.save_profile_root.as_deref() {
                                    view.save_dialog.open(root);
                                } else {
                                    view.game_options
                                        .set_message("Save unavailable in this view".into());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                return;
            }
            if view.chat_panel.open {
                if button == winit::event::MouseButton::Left
                    && let (Some(menu), Some(chat)) = (
                        &self.menu,
                        view.live_status.as_ref().and_then(|s| s.chat.as_ref()),
                    )
                {
                    view.chat_panel.mouse(
                        view.pointer,
                        pressed,
                        menu.comparison_assets().chat_layout(),
                        chat,
                    );
                }
                view.mouse_buttons = [false; 2];
                view.double_click.clear();
                return;
            }
            let event_list_occupied =
                view.live_status.as_ref().is_some_and(|status| {
                    status.event_list_visible && view.auction.scene.is_none()
                }) && self.menu.as_ref().is_some_and(|menu| {
                    view.pointer.is_some_and(|point| {
                        menu.event_list_assets()
                            .contains(point.map(|v| v as f32 * 0.5))
                    })
                });
            // Original tutorials enable selected controls while a prompt stays
            // open. Preserve a captured planning press through its release.
            if let Some(planning) = &mut view.planning {
                planning.pointer(view.pointer.filter(|_| !event_list_occupied));
            }
            let prompt_input_occupied = view.prompts.request().is_some()
                && (view.prompts.input_captured()
                    || self.menu.as_ref().is_some_and(|menu| {
                        view.pointer.is_some_and(|pointer| {
                            menu.prompt_assets().input_occupied(
                                pointer.map(|value| value as f32 * 0.5),
                                view.live_status.as_ref().map_or([0.; 2], |status| {
                                    status.tutorial_interaction.prompt_offset()
                                }),
                            )
                        })
                    }));
            let tutorial_planning_hit = button == winit::event::MouseButton::Left
                && !prompt_input_occupied
                && view.planning.as_ref().is_some_and(|planning| {
                    planning.tutorial_control_pressed()
                        || planning.tutorial_drag_at()
                            && view.selected.is_some_and(|hit| hit.building.is_none())
                        || planning.tutorial_control_at().is_some_and(|id| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        })
                });
            let tutorial_auction_hit = button == winit::event::MouseButton::Left
                && view.auction.scene.is_some()
                && (view.auction_panel.tutorial_control_pressed()
                    || !prompt_input_occupied
                        && self.menu.as_ref().is_some_and(|menu| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status.auction.as_ref().is_some_and(|auction| {
                                    view.auction_panel
                                        .tutorial_control_at(
                                            menu.auction_panel_assets(),
                                            view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                                            auction,
                                        )
                                        .is_some_and(|id| {
                                            status.tutorial_enabled_controls.contains(&id)
                                        })
                                })
                            })
                        }));
            let tutorial_locator_hit =
                button == winit::event::MouseButton::Left
                    && view.strategic
                    && self.menu.as_ref().zip(view.locator.as_ref()).is_some_and(
                        |(menu, locator)| {
                            locator
                                .tutorial_control_at(menu.locator_assets(), view.pointer)
                                .is_some_and(|id| {
                                    view.live_status.as_ref().is_some_and(|status| {
                                        status.tutorial_enabled_controls.contains(&id)
                                    })
                                })
                                || locator.tutorial_control_pressed()
                        },
                    );
            let tutorial_side_hit = button == winit::event::MouseButton::Left
                && self.menu.as_ref().is_some_and(|menu| {
                    view.toolbar_target(menu, &self.assets.selection.blocks)
                        .is_some()
                        || view.toolbar_input.captured()
                });
            let tutorial_property_hit = tutorial_side_hit
                || button == winit::event::MouseButton::Left
                    && !view.strategic
                    && view.main_panel_visible()
                    && view.details_visible
                    && self
                        .menu
                        .as_ref()
                        .zip(view.block_information.as_ref())
                        .zip(view.utility_category(&self.assets.selection.blocks))
                        .is_some_and(|((menu, report), category)| {
                            (if view
                                .ordinary_toolbar_category(&self.assets.selection.blocks)
                                .is_some()
                            {
                                menu.locator_assets().ordinary_body_tutorial_at(
                                    &view.utility_report,
                                    view.pointer,
                                    report,
                                    category,
                                )
                            } else {
                                menu.locator_assets().property_tutorial_control_at(
                                    &view.utility_report,
                                    view.pointer,
                                    report,
                                    category,
                                )
                            })
                            .is_some_and(|id| {
                                view.live_status.as_ref().is_some_and(|status| {
                                    status.tutorial_enabled_controls.contains(&id)
                                })
                            }) || view.utility_report.tutorial_control_pressed()
                        });
            let tutorial_business_hit = button == winit::event::MouseButton::Left
                && self.menu.as_ref().is_some_and(|menu| {
                    let id = if let Some(adjustment) = &mut view.adjustment {
                        adjustment.tutorial_control_at(menu.adjustment_assets(), view.pointer)
                    } else if !view.strategic
                        && view.main_panel_visible()
                        && view.details_visible
                        && view.operations.building.is_some()
                    {
                        view.operations.tutorial_control_at(
                            menu.business_assets(),
                            view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                        )
                    } else {
                        None
                    };
                    !prompt_input_occupied
                        && id.is_some_and(|id| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        })
                        || view
                            .adjustment
                            .as_ref()
                            .is_some_and(|a| a.tutorial_control_pressed())
                        || view.operations.tutorial_control_pressed()
                });
            if (view.prompts.blocks_controls() || prompt_input_occupied)
                && !tutorial_locator_hit
                && !tutorial_property_hit
                && !tutorial_planning_hit
                && !tutorial_business_hit
                && !tutorial_auction_hit
            {
                if let Some(menu) = &self.menu {
                    view.prompts.cursor(
                        view.pointer
                            .map(|p| p.map(|v| v as f32 * 0.5))
                            .unwrap_or([-1.; 2]),
                    );
                    if button == winit::event::MouseButton::Left {
                        if let Some(dialog) = view.prompts.request().and_then(|r| r.dialog) {
                            let offset = view.live_status.as_ref().map_or([0.; 2], |status| {
                                status.tutorial_interaction.prompt_offset()
                            });
                            let buttons = menu
                                .prompt_assets()
                                .dialog_buttons(dialog.button_count)
                                .into_iter()
                                .map(|b| {
                                    [
                                        b[0] + offset[0],
                                        b[1] + offset[1],
                                        b[2] + offset[0],
                                        b[3] + offset[1],
                                    ]
                                })
                                .collect::<Vec<_>>();
                            view.prompts.dialog_mouse(pressed, &buttons);
                        } else if let Some(id) = view.prompts.request_left_button(
                            pressed,
                            menu.prompt_assets().button_at(
                                view.live_status.as_ref().map_or([0.; 2], |status| {
                                    status.tutorial_interaction.prompt_offset()
                                }),
                            ),
                        ) {
                            let _ = view.dismiss_prompt(id);
                        }
                    }
                }
                return;
            }
            let tutorial_go_to_block_hit = button == winit::event::MouseButton::Left
                && view.main_panel_visible()
                && view.details_visible
                && !view.strategic
                && view.selected.is_some_and(|hit| hit.building.is_some())
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.tutorial_enabled_controls.contains(&14))
                && self.menu.as_ref().is_some_and(|menu| {
                    view.pointer.is_some_and(|point| {
                        menu.business_assets()
                            .go_to_block_contains(point.map(|value| value as f32 * 0.5))
                    })
                });
            let tutorial_control_hit = view.live_status.as_ref().is_some_and(|status| {
                status.tutorial_enabled_controls.contains(&103) && view.over_navigator()
                    || status.tutorial_enabled_controls.contains(&73) && view.over_navigation()
                    || status.tutorial_interaction.has_pick_gate() && view.scene_pointer().is_some()
                    || status
                        .tutorial_enabled_controls
                        .iter()
                        .any(|id| matches!(id, 76..=78))
                        && view.statistics_visible()
                        && view.comparison.as_ref().is_some_and(|comparison| {
                            view.pointer.is_some_and(|point| comparison.contains(point))
                        })
            }) || tutorial_go_to_block_hit
                || view.tutorial_go_to_block_pressed
                || tutorial_locator_hit
                || tutorial_property_hit
                || tutorial_planning_hit
                || tutorial_business_hit
                || tutorial_auction_hit;
            if view
                .live_status
                .as_ref()
                .is_some_and(|status| status.tutorial_controls.all_disabled)
                && !tutorial_control_hit
            {
                view.mouse_buttons = [false; 2];
                view.drag = [0; 2];
                return;
            }
            if button == winit::event::MouseButton::Left
                && (tutorial_go_to_block_hit || view.tutorial_go_to_block_pressed)
            {
                if pressed {
                    view.tutorial_go_to_block_pressed = tutorial_go_to_block_hit;
                } else {
                    let activate = view.tutorial_go_to_block_pressed && tutorial_go_to_block_hit;
                    view.tutorial_go_to_block_pressed = false;
                    if activate {
                        view.focus_parent_block(&self.assets.selection);
                    }
                }
                view.mouse_buttons[0] = false;
                view.double_click.clear();
                return;
            }
            if let (Some(menu), Some(status)) = (&self.menu, &view.live_status) {
                let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                let [l, t, r, b] = menu.card_assets().entry;
                let entry_hit = pointer.is_some_and(|[x, y]| x >= l && x <= r && y >= t && y <= b);
                if status.card.is_some() && menu.card_assets().contains(pointer)
                    || entry_hit
                    || view.card_input.captured()
                {
                    if button == winit::event::MouseButton::Left
                        && let Some(action) = view.card_input.mouse(
                            menu.card_assets(),
                            status.card,
                            status.card_entry,
                            pointer,
                            pressed,
                        )
                        && let Some(sender) = &view._fresh_session
                    {
                        let _ = sender.send(FreshRequest::Card(action));
                        view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
                    }
                    view.mouse_buttons = [false; 2];
                    view.drag = [0; 2];
                    return;
                }
            }
            if button == winit::event::MouseButton::Left
                && view._fresh_session.is_some()
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.time_display.visible)
                && let Some(pointer) = view.pointer.map(|p| p.map(|v| v as f32 * 0.5))
            {
                let auction = view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.auction.is_some());
                let hit = view.time_target(&self.assets.time_panel, pointer, auction);
                if view.time_occupied(&self.assets.time_panel, pointer, auction)
                    || view.time_pressed.is_some()
                {
                    view.time_panel.graph_cursor(hit, false);
                    if view.time_panel.mouse(pressed).is_some()
                        && let Err(error) = view
                            .gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Time button press sound: {error}");
                    }
                    if pressed {
                        view.time_pressed = hit;
                    } else if hit.is_some() && hit == view.time_pressed {
                        match hit.unwrap() {
                            crate::time_panel::Button::Speed => {
                                if let Some(sender) = &view._fresh_session {
                                    let _ = sender.send(FreshRequest::ToggleSpeed);
                                    view.last_status_query =
                                        Instant::now() - std::time::Duration::from_secs(1);
                                }
                            }
                            crate::time_panel::Button::Options => {
                                if let Err(error) = view.open_options() {
                                    eprintln!("[Options] {error}");
                                }
                            }
                            crate::time_panel::Button::ScenarioReminder => {
                                if let Some(sender) = &view._fresh_session {
                                    let _ = sender.send(FreshRequest::ScenarioReminder);
                                    view.keys = [false; 4];
                                    view.taps = [false; 4];
                                    view.drag = [0; 2];
                                    view.last_status_query =
                                        Instant::now() - std::time::Duration::from_secs(1);
                                }
                            }
                            crate::time_panel::Button::Manager => {
                                if let Some(sender) = &view._fresh_session {
                                    let _ = sender.send(FreshRequest::Stats(
                                        openmt2001::stats_panel::Action::Open,
                                    ));
                                    view.stats_requested = true;
                                    view.keys = [false; 4];
                                    view.taps = [false; 4];
                                    view.drag = [0; 2];
                                    view.last_status_query =
                                        Instant::now() - std::time::Duration::from_secs(1);
                                }
                            }
                        }
                    }
                    if !pressed {
                        view.time_pressed = None;
                    }
                    view.mouse_buttons = [false; 2];
                    view.drag = [0; 2];
                    return;
                }
            }
            if view.auction_visible() {
                if button == winit::event::MouseButton::Left
                    && let (Some(menu), Some(auction)) = (
                        &self.menu,
                        view.live_status.as_ref().and_then(|s| s.auction.as_ref()),
                    )
                {
                    let pointer = view
                        .pointer
                        .filter(|_| !prompt_input_occupied)
                        .map(|point| point.map(|v| v as f32 * 0.5));
                    let activation = view.auction_panel.mouse(
                        menu.auction_panel_assets(),
                        auction,
                        pointer,
                        pressed,
                    );
                    if view.auction_panel.pointer_click_cue(
                        menu.auction_panel_assets(),
                        auction,
                        pointer,
                    ) && let Err(error) = view
                        .gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Auction button press sound: {error}");
                    }
                    if let (Some(button), Some(sender)) = (activation, &view._fresh_session) {
                        let _ = sender.send(FreshRequest::AuctionButton(button));
                        view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
                    }
                }
                return;
            }
            if !view.strategic
                && button == winit::event::MouseButton::Left
                && let Some(category) = view.live_status.as_ref().and_then(|status| {
                    crate::notice_panel::View::category(
                        &status.notices,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )
                })
            {
                if pressed
                    && let Some(index) = self
                        .assets
                        .selection
                        .blocks
                        .iter()
                        .position(|b| b.id == category as usize)
                {
                    view.planning = None;
                    view.planning_pending = None;
                    view.focus_category(index, &self.assets.selection);
                }
                view.mouse_buttons = [false; 2];
                view.double_click.clear();
                return;
            }
            if let Some(planning) = &mut view.planning {
                planning.pointer(view.pointer.filter(|_| !event_list_occupied));
                if button == winit::event::MouseButton::Left {
                    planning.mouse(pressed);
                    let control = planning.take_tutorial_control();
                    if planning.take_placement_request() {
                        let (reply, receiver) = mpsc::channel();
                        if view._fresh_session.as_ref().is_some_and(|sender| {
                            sender
                                .send(FreshRequest::BuildDrag {
                                    category: planning.context.category,
                                    reply,
                                })
                                .is_ok()
                        }) {
                            planning.placement_pending = Some(receiver);
                        } else {
                            planning.diagnostic =
                                Some("Build placement session disconnected".into());
                            planning.dirty = true;
                        }
                    }
                    if planning.pointer_click_cue()
                        && let Err(error) = view
                            .gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Planning button press sound: {error}");
                    }
                    if let Some(id) = control {
                        view.send_tutorial_control(id);
                    }
                }
                return;
            }
            if view.planning_pending.is_some() {
                return;
            }
            if button == winit::event::MouseButton::Left
                && let Some(menu) = &self.menu
            {
                let (consumed, category) = view.auction_notice.mouse(
                    menu.auction_notice_assets(),
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    pressed,
                );
                if consumed {
                    if let Some(category) = category
                        && let Some(category) = self
                            .assets
                            .selection
                            .blocks
                            .iter()
                            .position(|block| block.id == category as usize)
                    {
                        view.focus_category(category, &self.assets.selection);
                    }
                    view.mouse_buttons = [false; 2];
                    view.double_click.clear();
                    return;
                }
            }
            if button == winit::event::MouseButton::Left
                && let Some(policy) = self
                    .menu
                    .as_ref()
                    .and_then(|menu| view.toolbar_policy(menu, &self.assets.selection.blocks, true))
            {
                let (consumed, action) = view.toolbar_input.mouse(
                    policy,
                    view.toolbar_bounds,
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    pressed,
                );
                if let Some(action) = action {
                    use crate::locator::toolbar_collapse::Action;
                    // Original Info/lease reveal BEFORE dispatching their modes.
                    action.reveal(&mut view.details_visible);
                    view.utility_report.select(Some(policy.category));
                    match action {
                        Action::Info => {
                            view.utility_report.close_property_offer();
                            view.utility_report.close_property_rights();
                            view.send_tutorial_control(9);
                        }
                        Action::Lease => {
                            if !view.utility_report.property_offer_open() {
                                view.utility_report.open_property_offer(policy.category);
                            }
                            view.send_tutorial_control(10);
                        }
                        Action::Build => view.begin_toolbar_planning(policy.category),
                    }
                    view.title_state = None;
                    view.utility_report.dirty = true;
                }
                if consumed {
                    view.mouse_buttons[0] = false;
                    view.double_click.clear();
                    return;
                }
            }
            if !view.strategic
                && view.main_panel_visible()
                && view.details_visible
                && view._fresh_session.is_some()
                && view
                    .ordinary_toolbar_category(&self.assets.selection.blocks)
                    .is_none()
                && view.selected.is_some_and(|s| s.building.is_none())
                && !view.selected.is_some_and(|s| {
                    view.live_status.as_ref().is_some_and(|status| {
                        status
                            .auction_locked_categories
                            .contains(&self.assets.selection.blocks[s.category].id)
                    })
                })
                && let Some(menu) = &self.menu
            {
                let [l, t, r, b] = menu.planning_assets().entry_bounds;
                let over = view.pointer.is_some_and(|p| {
                    let [x, y] = p.map(|v| v as f32 * 0.5);
                    x >= l && x <= r && y >= t && y <= b
                });
                if button == winit::event::MouseButton::Left && (over || view.planning_pressed) {
                    if pressed {
                        view.planning_pressed = over;
                    } else {
                        let activate = view.planning_pressed && over;
                        view.planning_pressed = false;
                        if activate {
                            let category =
                                self.assets.selection.blocks[view.selected.unwrap().category].id;
                            let (reply, result) = mpsc::channel();
                            if view
                                ._fresh_session
                                .as_ref()
                                .unwrap()
                                .send(FreshRequest::Planning { category, reply })
                                .is_ok()
                            {
                                view.planning_pending = Some(result);
                                view.send_tutorial_control(11);
                            }
                        }
                    }
                    view.mouse_buttons = [false; 2];
                    view.double_click.clear();
                    return;
                }
            }
            if view.main_panel_visible()
                && view.details_visible
                && !view.strategic
                && view.adjustment.is_none()
                && view.operations.building.is_some()
                && let Some(menu) = &self.menu
                && (view.operations.captured()
                    || view.pointer.is_some_and(|p| {
                        menu.business_assets().contains(p.map(|v| v as f32 * 0.5))
                    }))
            {
                if button == winit::event::MouseButton::Left {
                    view.operations.mouse(
                        menu.business_assets(),
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                        pressed,
                    );
                    if let Some((control, event)) = view.operations.take_tutorial_producer() {
                        view.send_tutorial_producer(control, event);
                    }
                    if let Some(id) = view.operations.take_tutorial_control()
                        && view
                            .live_status
                            .as_ref()
                            .is_some_and(|s| s.tutorial_enabled_controls.contains(&id))
                    {
                        view.send_tutorial_control(id);
                    }
                }
                view.mouse_buttons = [false; 2];
                return;
            }
            if let Some(adjustment) = &mut view.adjustment {
                if let Some(menu) = &self.menu {
                    adjustment.pointer(menu.adjustment_assets(), view.pointer);
                }
                let close = button == winit::event::MouseButton::Left && adjustment.mouse(pressed);
                let requested = adjustment.requested_building.take();
                let tutorial_control = adjustment.take_tutorial_control();
                if close {
                    view.adjustment = None;
                } else if let Some(building) = requested
                    && let Some(menu) = &self.menu
                {
                    match menu
                        .adjustment_assets()
                        .navigate(&view.scene, adjustment, building)
                    {
                        Ok(panel) => {
                            let b = &view.scene.inspection_buildings[building];
                            let Some(hit) = inspection_hit(
                                &self.assets.selection.blocks,
                                b.category,
                                Some(building),
                            ) else {
                                return;
                            };
                            if view.camera_producer.is_some() {
                                if let Err(error) = view.queue_actor_camera(
                                    hit,
                                    &self.assets.selection,
                                    camera_producer::ActorOrigin::Adjustment,
                                    Some(panel),
                                    false,
                                ) {
                                    view.save_dialog.message = Some(error);
                                }
                                if let Some(id) = tutorial_control {
                                    view.send_tutorial_control(id);
                                }
                                return;
                            }
                            view.adjustment = Some(panel);
                            view.selected = Some(hit);
                            if let Some((controls, angle)) = crate::navigation::building_entry(
                                self.assets.selection.blocks[hit.category].center,
                                [b.translation[0], b.translation[2]],
                                b.camera_entrance,
                                b.kind == 2,
                                view.controls.angle,
                                view.building_angle,
                            ) {
                                view.controls = controls;
                                view.building_angle = angle;
                                view.building_camera = true;
                            } else {
                                view.building_camera = false;
                                view.controls.center = self.assets.selection.center(hit.category);
                                view.controls.height = 120.;
                                view.controls.radius = 80.;
                            }
                        }
                        Err(error) => {
                            eprintln!("Business Adjustment navigation unavailable: {error}")
                        }
                    }
                }
                if let Some(id) = tutorial_control
                    && view
                        .live_status
                        .as_ref()
                        .is_some_and(|s| s.tutorial_enabled_controls.contains(&id))
                {
                    view.send_tutorial_control(id);
                }
                return;
            }
            if view.statistics_visible()
                && let (Some(comparison), Some(menu)) = (&mut view.comparison, &self.menu)
            {
                comparison.pointer(menu.comparison_assets(), view.pointer);
                if view.pointer.is_some_and(|p| comparison.contains(p)) || comparison.pressed() {
                    if button == winit::event::MouseButton::Left {
                        let activated = comparison.mouse(pressed);
                        if let Some(player) =
                            comparison.chat_mouse(menu.comparison_assets(), view.pointer, pressed)
                            && let Some(chat) =
                                view.live_status.as_ref().and_then(|s| s.chat.as_ref())
                        {
                            view.chat_panel.select_player(player, chat);
                        }
                        if matches!(activated, Some(1 | 2))
                            && let Some(sender) = &view._fresh_session
                        {
                            let _ =
                                sender.send(FreshRequest::ComparisonMode(comparison.state.mode));
                        }
                        if let Some(id) =
                            activated.and_then(|button| [76, 77, 78].get(button).copied())
                            && view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        {
                            view.send_tutorial_control(id);
                        }
                        view.mouse_buttons[0] = false;
                        view.double_click.clear();
                    } else if button == winit::event::MouseButton::Right {
                        comparison.cancel_press();
                        view.mouse_buttons[1] = false;
                        view.drag = [0; 2];
                    }
                    return;
                }
            }
            if event_list_occupied
                && !(view.strategic
                    && view.locator.as_ref().is_some_and(|locator| {
                        locator.pressed()
                            || self.menu.as_ref().is_some_and(|menu| {
                                view.pointer.is_some_and(|point| {
                                    menu.locator_assets().contains(locator, point)
                                })
                            })
                    }))
            {
                view.mouse_buttons = [false; 2];
                view.drag = [0; 2];
                view.double_click.clear();
                return;
            }
            view.locator_blocked = view.strategic
                && (view.ui_graph.is_none() || view.graph_input(camera_live::ui_graph::PanelId::C))
                && view.locator.as_ref().is_some_and(|locator| {
                    self.menu.as_ref().is_some_and(|menu| {
                        view.pointer
                            .is_some_and(|p| menu.locator_assets().contains(locator, p))
                    })
                });
            match button {
                winit::event::MouseButton::Left => {
                    if view.mouse_buttons[0] != pressed && view.mouse_buttons[1] {
                        // Do not reinterpret accumulated orbit deltas as pan, or vice versa.
                        view.drag = [0; 2];
                    }
                    view.mouse_buttons[0] = pressed;
                    if view.strategic
                        && !view.mouse_buttons[1]
                        && (view.locator_blocked
                            || view.locator.as_ref().is_some_and(|l| l.pressed()))
                        && let (Some(locator), Some(menu)) = (&mut view.locator, &self.menu)
                    {
                        view.double_click.clear();
                        let selected = locator.mouse(
                            menu.locator_assets(),
                            &view.scene,
                            view.pointer,
                            pressed,
                        );
                        let tutorial_control = locator.take_tutorial_control();
                        if let Some(id) = tutorial_control
                            && view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        {
                            view.send_tutorial_control(id);
                        }
                        if let Some(i) = selected {
                            let building = &view.scene.inspection_buildings[i];
                            let Some(category) = self
                                .assets
                                .selection
                                .blocks
                                .iter()
                                .position(|b| b.id == building.category)
                            else {
                                return;
                            };
                            let hit = crate::selection::Hit {
                                category,
                                building: Some(i),
                            };
                            if view.camera_producer.is_some() {
                                if let Err(error) = view.queue_actor_camera(
                                    hit,
                                    &self.assets.selection,
                                    camera_producer::ActorOrigin::Locator,
                                    None,
                                    false,
                                ) {
                                    view.save_dialog.message = Some(error);
                                }
                                return;
                            }
                            view.selected = Some(hit);
                            let close_up = crate::navigation::building_entry(
                                self.assets.selection.blocks[category].center,
                                [building.translation[0], building.translation[2]],
                                building.camera_entrance,
                                building.kind == 2,
                                view.controls.angle,
                                view.building_angle,
                            );
                            if let Some((controls, angle)) = close_up {
                                view.controls = controls;
                                view.building_angle = angle;
                                view.building_camera = true;
                            } else {
                                view.controls = crate::navigation::block_entry(
                                    self.assets.selection.center(category),
                                );
                                view.building_camera = false;
                            }
                            view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                view.camera.eye,
                                view.camera.target,
                            );
                            view.strategic = false;
                            view.return_camera = None;
                            view.reframe_pending = true;
                            view.title_state = None;
                            if let Some(comparison) = &mut view.comparison {
                                comparison.cancel_press();
                            }
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            view.hovered = None;
                            eprintln!("Locator saved business -> building {i}, paused inspection");
                        }
                        return;
                    }
                    if view.ui_graph.is_none()
                        && pressed
                        && view.over_navigator()
                        && !view.mouse_buttons[1]
                    {
                        view.navigator_pressed = true;
                        view.double_click.clear();
                        return;
                    }
                    if view.ui_graph.is_none() && !pressed && view.navigator_pressed {
                        view.navigator_pressed = false;
                        if view.over_navigator()
                            && let Some(point) = view.pointer
                            && let Some((category, _)) = self
                                .assets
                                .navigator
                                .pick_inspection(&self.assets.selection.blocks, point)
                        {
                            if view.camera_producer.is_some() {
                                let event = camera_producer::Event::Block {
                                    category: self.assets.selection.blocks[category].id as u32,
                                    selected: crate::selection::Hit {
                                        category,
                                        building: None,
                                    },
                                    origin: camera_producer::BlockOrigin::Navigator,
                                };
                                let post = view.camera_control_post(103);
                                match view.queue_camera_event(event, None, None, false) {
                                    Ok(pending) => pending.posts.extend(post),
                                    Err(error) => view.save_dialog.message = Some(error),
                                }
                                return;
                            }
                            view.selected = Some(crate::selection::Hit {
                                category,
                                building: None,
                            });
                            if view.strategic {
                                view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                    view.camera.eye,
                                    view.camera.target,
                                );
                            }
                            view.hovered = None;
                            view.controls.center = self.assets.selection.center(category);
                            view.controls.height = 120.;
                            view.controls.radius = 80.;
                            view.building_camera = false;
                            view.strategic = false;
                            view.return_camera = None;
                            view.reframe_pending = true;
                            view.title_state = None;
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            eprintln!(
                                "Navigator category {} -> mode12",
                                self.assets.selection.blocks[category].id
                            );
                            if view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&103)
                            }) {
                                view.send_tutorial_control(103);
                            }
                        }
                        return;
                    }
                    if !view.strategic
                        && view.main_panel_visible()
                        && view.details_visible
                        && view
                            .utility_category(&self.assets.selection.blocks)
                            .is_some()
                        && let Some(menu) = &self.menu
                    {
                        view.utility_report
                            .select(view.utility_category(&self.assets.selection.blocks));
                        menu.locator_assets()
                            .utility_pointer(&mut view.utility_report, view.pointer);
                        let category = view
                            .utility_category(&self.assets.selection.blocks)
                            .unwrap();
                        if view
                            .ordinary_toolbar_category(&self.assets.selection.blocks)
                            .is_some()
                        {
                            menu.locator_assets().ordinary_body_pointer(
                                &mut view.utility_report,
                                view.pointer,
                                view.block_information.as_ref().unwrap(),
                                category,
                            );
                        } else {
                            menu.locator_assets().land_rent_pointer(
                                &mut view.utility_report,
                                view.pointer,
                                view.block_information.as_ref().unwrap(),
                                category,
                            );
                        }
                        let rights_enabled = view.live_status.as_ref().is_none_or(|status| {
                            !status.tutorial_session
                                || status.tutorial_enabled_controls.contains(&91)
                        });
                        if view
                            .utility_report
                            .mouse_with_rights_admission(pressed, rights_enabled)
                        {
                            let tutorial_after = view.utility_report.take_tutorial_control();
                            let camera_post = (tutorial_after == Some(6))
                                .then(|| view.camera_control_post(6))
                                .flatten();
                            let mut hyperlink_committed = false;
                            if let Some(slot) = view.utility_report.requested_slot.take()
                                && let Some(i) =
                                    view.scene.inspection_buildings.iter().position(|b| {
                                        b.category == category
                                            && (b.saved_slot == Some(slot)
                                                || b.runtime_slot == Some(slot))
                                    })
                                && let Some(hit) =
                                    inspection_hit(&self.assets.selection.blocks, category, Some(i))
                            {
                                if view.camera_producer.is_some() {
                                    match view.queue_actor_camera(
                                        hit,
                                        &self.assets.selection,
                                        camera_producer::ActorOrigin::Hyperlink,
                                        None,
                                        false,
                                    ) {
                                        Ok(pending) => {
                                            pending.posts.extend(camera_post);
                                            hyperlink_committed = true;
                                        }
                                        Err(error) => view.save_dialog.message = Some(error),
                                    }
                                } else {
                                    let building = &view.scene.inspection_buildings[i];
                                    view.selected = Some(hit);
                                    if let Some((controls, angle)) =
                                        crate::navigation::building_entry(
                                            self.assets.selection.blocks[hit.category].center,
                                            [building.translation[0], building.translation[2]],
                                            building.camera_entrance,
                                            building.kind == 2,
                                            view.controls.angle,
                                            view.building_angle,
                                        )
                                    {
                                        view.controls = controls;
                                        view.building_angle = angle;
                                        view.building_camera = true;
                                    } else {
                                        view.controls = crate::navigation::block_entry(
                                            self.assets.selection.center(hit.category),
                                        );
                                        view.building_camera = false;
                                    }
                                    view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                        view.camera.eye,
                                        view.camera.target,
                                    );
                                    view.reframe_pending = true;
                                    view.title_state = None;
                                    hyperlink_committed = true;
                                }
                            }
                            if let Some(id) = tutorial_after
                                && (id != 6 || hyperlink_committed)
                            {
                                // Only6 is conditional on the camera/UI hyperlink
                                // commit. Other handler actions remain unconditional.
                                if id != 6 || view.camera_producer.is_none() {
                                    view.send_tutorial_control(id);
                                }
                            }
                            view.mouse_buttons[0] = false;
                            view.double_click.clear();
                            return;
                        }
                    }
                    if pressed && view.over_details() {
                        view.details_pressed = true;
                        view.double_click.clear();
                        return;
                    }
                    if !pressed && view.details_pressed {
                        view.details_pressed = false;
                        if view.over_details() {
                            view.details_visible = !view.details_visible;
                            view.toolbar_input.cancel();
                        }
                        return;
                    }
                    if pressed && view.over_navigation() {
                        view.navigation_pressed = true;
                        view.double_click.clear();
                        return;
                    }
                    if !pressed && view.navigation_pressed {
                        view.navigation_pressed = false;
                        if view.over_navigation() {
                            view.toggle_navigation(&self.assets.navigation);
                        }
                        return;
                    }
                    if pressed
                        && !view.mouse_buttons[1]
                        && !view
                            .live_status
                            .as_ref()
                            .is_some_and(|status| status.tutorial_controls.picking_disabled)
                        && view.scene_pointer().is_some()
                    {
                        let picked = view.scene_pointer().and_then(|point| {
                            self.assets.selection.pick(
                                view.camera,
                                &view.scene,
                                point,
                                view.selected,
                            )
                        });
                        let tutorial_pick = picked.and_then(|hit| {
                            let pick = match (view.strategic, hit.building) {
                                (false, Some(index)) => {
                                    let building = &view.scene.inspection_buildings[index];
                                    crate::tutorial_two_interaction::Pick::Building {
                                        slot: building
                                            .runtime_slot
                                            .or(building.saved_slot)
                                            .unwrap_or(index),
                                    }
                                }
                                _ => crate::tutorial_two_interaction::Pick::Block {
                                    category_id: self.assets.selection.blocks[hit.category].id
                                        as u32,
                                },
                            };
                            view.live_status.as_ref().and_then(|status| {
                                status.tutorial_interaction.callback(pick).map(|_| pick)
                            })
                        });
                        if view
                            .live_status
                            .as_ref()
                            .is_some_and(|status| status.tutorial_interaction.has_pick_gate())
                            && tutorial_pick.is_none()
                        {
                            view.double_click.clear();
                            return;
                        }
                        let double = view.cursor.is_some_and(|point| {
                            view.double_click.press(view.started.elapsed(), point)
                        });
                        let selected = if view.strategic {
                            picked.map(|hit| crate::selection::Hit {
                                category: hit.category,
                                building: None,
                            })
                        } else {
                            picked
                        };
                        let changed = view.selected != selected;
                        if !view.strategic && view.camera_producer.is_some() {
                            if let Some(hit) = selected {
                                let post = if changed {
                                    tutorial_pick.map(camera_producer::Post::TutorialPick)
                                } else {
                                    None
                                };
                                let open_adjustment = double
                                    && !view
                                        .live_status
                                        .as_ref()
                                        .is_some_and(|s| s.tutorial_interaction.has_pick_gate())
                                    && !view.live_status.as_ref().is_some_and(|s| {
                                        s.auction_locked_categories.contains(
                                            &self.assets.selection.blocks[hit.category].id,
                                        )
                                    });
                                let result = if hit.building.is_some() {
                                    view.queue_actor_camera(
                                        hit,
                                        &self.assets.selection,
                                        camera_producer::ActorOrigin::Scene,
                                        None,
                                        open_adjustment,
                                    )
                                } else {
                                    view.queue_camera_event(
                                        camera_producer::Event::Block {
                                            category: self.assets.selection.blocks[hit.category].id
                                                as u32,
                                            selected: hit,
                                            origin: camera_producer::BlockOrigin::Selection,
                                        },
                                        None,
                                        None,
                                        false,
                                    )
                                };
                                match result {
                                    Ok(pending) => pending.posts.extend(post),
                                    Err(error) => view.save_dialog.message = Some(error),
                                }
                                return;
                            } else {
                                if let Err(error) = view.queue_camera_event(
                                    camera_producer::Event::Deselect,
                                    None,
                                    None,
                                    false,
                                ) {
                                    view.save_dialog.message = Some(error);
                                }
                                return;
                            }
                        }
                        view.selected = selected;
                        if view.strategic {
                            if changed && let Some(pick) = tutorial_pick {
                                view.send_tutorial_pick(pick);
                            }
                            if double && let Some(hit) = view.selected {
                                if view.camera_producer.is_some() {
                                    let event = camera_producer::Event::Block {
                                        category: self.assets.selection.blocks[hit.category].id
                                            as u32,
                                        selected: hit,
                                        origin: camera_producer::BlockOrigin::StrategicDoubleClick,
                                    };
                                    let post = match tutorial_pick {
                                        Some(crate::tutorial_two_interaction::Pick::Block {
                                            category_id,
                                        }) => Some(camera_producer::Post::TutorialPick(
                                            crate::tutorial_two_interaction::Pick::BlockEntry {
                                                category_id,
                                            },
                                        )),
                                        _ => None,
                                    };
                                    match view.queue_camera_event(event, None, None, false) {
                                        Ok(pending) => pending.posts.extend(post),
                                        Err(error) => view.save_dialog.message = Some(error),
                                    }
                                    return;
                                }
                                view.building_camera = false;
                                view.controls = crate::navigation::block_entry(
                                    self.assets.selection.center(hit.category),
                                );
                                view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                    view.camera.eye,
                                    view.camera.target,
                                );
                                view.strategic = false;
                                view.return_camera = None;
                                view.title_state = None;
                                view.reframe_pending = true;
                                eprintln!(
                                    "Strategic double-click: category {} -> mode12, height120/radius80",
                                    self.assets.selection.blocks[hit.category].id
                                );
                                if let Some(crate::tutorial_two_interaction::Pick::Block {
                                    category_id,
                                }) = tutorial_pick
                                {
                                    view.send_tutorial_pick(
                                        crate::tutorial_two_interaction::Pick::BlockEntry {
                                            category_id,
                                        },
                                    );
                                }
                            }
                        } else if let Some(index) = view.selected {
                            if double
                                && !view.live_status.as_ref().is_some_and(|status| {
                                    status.tutorial_interaction.has_pick_gate()
                                })
                                && !view.live_status.as_ref().is_some_and(|status| {
                                    status
                                        .auction_locked_categories
                                        .contains(&self.assets.selection.blocks[index.category].id)
                                })
                                && let (Some(building), Some(menu)) = (index.building, &self.menu)
                            {
                                match menu.adjustment_assets().open_for_owner(
                                    &view.scene,
                                    building,
                                    view.live_status.as_ref().and_then(|s| s.business_owner),
                                ) {
                                    Ok(panel) => view.adjustment = panel,
                                    Err(error) => {
                                        eprintln!("Business Adjustment unavailable: {error}")
                                    }
                                }
                                view.keys = [false; 4];
                                view.taps = [false; 4];
                                view.mouse_buttons = [false; 2];
                                view.drag = [0; 2];
                            }
                            let close_up = index.building.and_then(|i| {
                                let building = &view.scene.inspection_buildings[i];
                                crate::navigation::building_entry_with_options(
                                    self.assets.selection.blocks[index.category].center,
                                    [building.translation[0], building.translation[2]],
                                    building.camera_entrance,
                                    building.kind == 2,
                                    view.controls.angle,
                                    view.building_angle,
                                    self.assets.selection.center(index.category),
                                    view.settings_menu.gameplay.auto_rotate,
                                )
                            });
                            if let Some((controls, angle)) = close_up {
                                view.controls = controls;
                                view.building_angle = angle;
                                view.building_camera = true;
                                eprintln!(
                                    "Building selection -> mode13 camera; Auto Rotate {}",
                                    view.settings_menu.gameplay.auto_rotate
                                );
                            } else {
                                if index.building.is_some() {
                                    eprintln!(
                                        "Building close-up unresolved: no original entrance; retaining parent block camera"
                                    );
                                }
                                view.building_camera = false;
                                view.controls.center = self.assets.selection.center(index.category);
                                view.controls.height = 120.;
                                view.controls.radius = 80.;
                            }
                            if changed && let Some(pick) = tutorial_pick {
                                view.send_tutorial_pick(pick);
                            }
                        }
                    }
                }
                winit::event::MouseButton::Right => {
                    view.mouse_buttons[1] = pressed;
                    if pressed {
                        if let Some(locator) = &mut view.locator {
                            locator.cancel_press();
                        }
                        view.navigator_pressed = false;
                        view.double_click.clear();
                    }
                }
                _ => (),
            }
        }
    }
    pub fn inspection_unfocus(&mut self) {
        if let Some(view) = &mut self.inspection {
            view.toolbar_input.cancel();
            view.toolbar_right_held = false;
            view.settings_menu.cancel_input();
            view.game_options.quit_prompt.cancel_press();
            if let Some(comparison) = &mut view.comparison {
                comparison.cancel_press();
            }
            if let Some(adjustment) = &mut view.adjustment {
                adjustment.cancel_press();
            }
            if let Some(locator) = &mut view.locator {
                locator.cancel_press();
            }
            if let Some(result) = &mut view.result {
                result.bypass.clear();
            }
            view.keys = [false; 4];
            view.navigation_pressed = false;
            view.details_pressed = false;
            view.navigator_pressed = false;
            view.double_click.clear();
            view.stats_panel.cancel_input();
            view.time_panel.cancel_input();
            view.time_pressed = None;
            view.taps = [false; 4];
            view.mouse_buttons = [false; 2];
            view.drag = [0; 2];
            view.cursor = None;
        }
    }
}
