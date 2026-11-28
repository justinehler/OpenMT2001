//! `Intro::draw_inspection`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn draw_inspection(&mut self, presenter: &Presenter) -> Result<bool, String> {
        if let Some(view) = &mut self.inspection {
            if view.tutorial_start.holds_loading(
                view.tutorial_camera_seen,
                !view.camera_turns.is_empty(),
                view.save_dialog.message.is_some()
                    || view
                        .live_status
                        .as_ref()
                        .is_some_and(|status| status.failure.is_some()),
            ) {
                // Retain the last loading frame while the worker runs BlockCam
                // and the existing selection transaction commits on the UI.
                return Ok(true);
            }
            if view.ui_graph.is_some() {
                if let Some(prepared) =
                    camera_live::upper_f::body(view, &self.assets, &self.renderer)?
                {
                    view.navigator = prepared;
                }
            } else {
                //43d0f0 reads the logic-step eye93af64/6c, not the interpolated render eye.
                let arrow_eye = if view.strategic {
                    view.camera.eye
                } else {
                    view.inertia.sample(0.)?.0
                };
                self.assets.navigator.update_ownership_arrow(
                    &self.renderer,
                    &mut view.navigator,
                    [arrow_eye[0], arrow_eye[2]],
                    view.controls.angle,
                    view.live_status.as_ref().map_or(0, |status| status.ticks),
                    &view.navigator_owners,
                )?;
            }
            if view.ui_graph.is_some() {
                camera_live::focus::present(
                    view,
                    &self.assets,
                    self.menu.as_ref(),
                    &self.renderer,
                )?;
            } else {
                let navigator_tooltip = view
                    .pointer
                    .filter(|_| view.over_navigator())
                    .and_then(|point| {
                        self.assets
                            .navigator
                            .pick_inspection(&self.assets.selection.blocks, point)
                    })
                    .map(|(category, point)| {
                        (self.assets.selection.blocks[category].text_id, point)
                    });
                let icon = if navigator_tooltip.is_none()
                    && view.strategic
                    && !view.options_visible()
                    && view.prompts.request().is_none()
                    && view.adjustment.is_none()
                    && view.planning.is_none()
                {
                    view.pointer.and_then(|point| {
                        view.locator.as_ref().and_then(|locator| {
                            self.menu.as_ref().and_then(|menu| {
                                locator.icon_tooltip_at(menu.locator_assets(), point)
                            })
                        })
                    })
                } else {
                    None
                };
                let side_icon = self
                    .menu
                    .as_ref()
                    .and_then(|menu| view.toolbar_target(menu, &self.assets.selection.blocks))
                    .map(|a| a.tooltip());
                let time_icon = view.time_tooltip(&self.assets.time_panel);
                let speed_icon = time_icon
                    .filter(|(button, _)| *button == crate::time_panel::Button::Speed)
                    .map(|(_, id)| id);
                let visible_icon = view
                    .icon_tooltip_dwell
                    .advance(speed_icon.or(side_icon).or(icon));
                let tooltip_state = time_icon
                    .filter(|(button, _)| {
                        *button == crate::time_panel::Button::ScenarioReminder
                            && view.time_panel.tooltip_ready()
                    })
                    .map(|(_, id)| id)
                    .zip(view.pointer.map(|p| p.map(|v| v / 2)))
                    .or(navigator_tooltip)
                    .or_else(|| {
                        visible_icon.zip(view.pointer.map(|point| {
                            //New CityMode captions use the same 640-space
                            //pointer as the live focus owner and speed help.
                            if speed_icon.is_some() || matches!(icon, Some(1150..=1153)) {
                                point.map(|v| v / 2)
                            } else {
                                point
                            }
                        }))
                    });
                if tooltip_state != view.tooltip_state {
                    view.tooltip = tooltip_state
                        .map(|(text_id, point)| {
                            self.assets.tooltip.prepare(&self.renderer, text_id, point)
                        })
                        .transpose()?;
                    view.tooltip_state = tooltip_state;
                }
            }
            view.locator_blocked = view.strategic
                && (view.ui_graph.is_none() || view.graph_input(camera_live::ui_graph::PanelId::C))
                && view.locator.as_ref().is_some_and(|locator| {
                    self.menu.as_ref().is_some_and(|menu| {
                        view.pointer
                            .is_some_and(|p| menu.locator_assets().contains(locator, p))
                    })
                });
            if let (Some(locator), Some(menu)) = (&mut view.locator, &self.menu) {
                locator.refresh_scene(&view.scene);
                if let Some(category) = locator.take_category_selection() {
                    view.selected = inspection_hit(&self.assets.selection.blocks, category, None);
                }
                locator.select_poll_category(view.selected.and_then(|hit| {
                    self.assets
                        .selection
                        .blocks
                        .get(hit.category)
                        .map(|block| block.id)
                }));
                locator.pointer(
                    menu.locator_assets(),
                    if view.strategic
                        && view.adjustment.is_none()
                        && view
                            .ui_graph
                            .as_ref()
                            .is_none_or(|graph| graph.input(camera_live::ui_graph::PanelId::C))
                    {
                        view.pointer
                    } else {
                        None
                    },
                );
            }
            if view.strategic
                && let (Some(locator), Some(menu)) = (&mut view.locator, &self.menu)
                && (view.ui_graph.is_some() || locator.dirty || locator.prepared.is_none())
            {
                locator.prepared = Some(if let Some(graph) = &mut view.ui_graph {
                    menu.locator_assets().prepare_graph(
                        &self.renderer,
                        locator,
                        &view.scene,
                        graph.body(camera_live::ui_graph::PanelId::C),
                        graph.body(camera_live::ui_graph::PanelId::D),
                        Some(&graph.world.colors),
                        graph.world.mode,
                        &mut graph.world.messages,
                    )?
                } else {
                    menu.locator_assets()
                        .prepare(&self.renderer, locator, &view.scene)?
                });
                locator.dirty = false;
            }
            if view.statistics_visible()
                && let (Some(comparison), Some(menu)) = (&mut view.comparison, &self.menu)
            {
                comparison.pointer(
                    menu.comparison_assets(),
                    if view.adjustment.is_none() {
                        view.pointer
                    } else {
                        None
                    },
                );
                comparison.prepare(
                    menu.comparison_assets(),
                    &self.renderer,
                    view.locator
                        .as_ref()
                        .map(|l| l.financial_records())
                        .unwrap_or(&view.scene.saved_players),
                )?;
            }
            if let Some(menu) = &self.menu {
                view.utility_report
                    .select(view.utility_category(&self.assets.selection.blocks));
                let side_policy = view.toolbar_policy(menu, &self.assets.selection.blocks, true);
                view.toolbar_input.update(
                    side_policy,
                    view.toolbar_bounds,
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                );
                let visible_policy =
                    view.toolbar_policy(menu, &self.assets.selection.blocks, false);
                view.toolbar_regions = std::array::from_fn(|i| {
                    visible_policy
                        .filter(|q| q.visible(crate::locator::toolbar_collapse::Action::ALL[i]))
                        .map(|_| view.toolbar_bounds[i])
                });
                let point = if !view.strategic
                    && view.main_panel_visible()
                    && view.details_visible
                    && view.adjustment.is_none()
                    && view
                        .utility_category(&self.assets.selection.blocks)
                        .is_some()
                {
                    view.pointer
                } else {
                    None
                };
                menu.locator_assets()
                    .utility_pointer(&mut view.utility_report, point);
                if let Some(pending) = &view.property_pending {
                    let response = match pending.reply.try_recv() {
                        Ok(result) => Some(result),
                        Err(mpsc::TryRecvError::Empty) => None,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            Some(Err("Session closed before inspecting the property".into()))
                        }
                    };
                    if let Some(result) = response {
                        view.utility_report
                            .apply_property_inspection(pending.category, result);
                        view.property_pending = None;
                    }
                }
                if view.property_pending.is_none()
                    && let Some(request) = view.utility_report.take_property_request()
                {
                    let category = request.category;
                    if let Some(sender) = &view._fresh_session {
                        let (reply, response) = mpsc::channel();
                        if sender
                            .send(FreshRequest::PropertyOffer { request, reply })
                            .is_ok()
                        {
                            view.property_pending = Some(PendingPropertyInspection {
                                category,
                                reply: response,
                            });
                        } else {
                            view.utility_report.apply_property_inspection(
                                category,
                                Err("Property offer failed: session closed".into()),
                            );
                        }
                    } else {
                        view.utility_report.apply_property_inspection(
                            category,
                            Err("Property offers require a running game session".into()),
                        );
                    }
                }
                if let Some(pending) = &view.rights_pending {
                    let response = match pending.reply.try_recv() {
                        Ok(result) => Some(result),
                        Err(mpsc::TryRecvError::Empty) => None,
                        Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                            "Session closed before inspecting building rights".into(),
                        )),
                    };
                    if let Some(result) = response {
                        view.utility_report
                            .apply_rights_inspection(pending.category, result);
                        view.rights_pending = None;
                    }
                }
                if view.rights_pending.is_none()
                    && let Some(request) = view.utility_report.take_rights_request()
                {
                    let category = request.category;
                    if let Some(sender) = &view._fresh_session {
                        let (reply, response) = mpsc::channel();
                        if sender
                            .send(FreshRequest::PropertyRights {
                                category,
                                panel_mode: request.panel_mode,
                                activate: request.activate,
                                reply,
                            })
                            .is_ok()
                        {
                            view.rights_pending = Some(PendingRightsInspection {
                                category,
                                reply: response,
                            });
                        } else {
                            view.utility_report.apply_rights_inspection(
                                category,
                                Err("Building rights request failed: session closed".into()),
                            );
                        }
                    } else {
                        view.utility_report.apply_rights_inspection(
                            category,
                            Err("Building rights require a running game session".into()),
                        );
                    }
                }
                if let Some(category) = view.utility_report.take_rights_open()
                    && view.planning.is_none()
                    && view.planning_pending.is_none()
                    && let Some(sender) = &view._fresh_session
                {
                    let (reply, response) = mpsc::channel();
                    if sender
                        .send(FreshRequest::Planning { category, reply })
                        .is_ok()
                    {
                        view.planning_pending = Some(response);
                    }
                }
                if view.utility_report.dirty {
                    view.title_state = None;
                    view.utility_report.dirty = false;
                }
            }
            let hovered = view.scene_pointer().and_then(|point| {
                self.assets
                    .selection
                    .pick(view.camera, &view.scene, point, view.selected)
            });
            // Host status carries world.clock.timing[0] (00822d40), the
            // simulation tick read by the original marker renderer at 4b4f40.
            if let Some(surface) = self.assets.selection.refresh_marker_surface(
                &mut view.marker_state,
                [view.selected, hovered],
                &view.scene,
                view.live_status.as_ref().map_or(0, |status| status.ticks),
                view.title_state != Some((view.selected, hovered)),
            )? {
                let mut draw = surface.draw();
                draw.mode = openmt2001::render::SurfaceMode::StaticLit;
                self.renderer.update_surface(
                    &mut view.markers,
                    0,
                    &draw,
                    view.scene.camera,
                    view.scene.fog,
                )?;
            }
            if view.title_state != Some((view.selected, hovered)) {
                if view.title_state.map(|s| s.0) != Some(view.selected) {
                    view.information = match (
                        information_target(view.selected, &view.scene.inspection_buildings),
                        &self.menu,
                    ) {
                        (InformationTarget::Welcome, Some(menu)) => {
                            Some(menu.locator_assets().welcome_panel(&self.renderer)?)
                        }
                        (InformationTarget::Block(hit), Some(menu)) => {
                            // Original 4c39ed..4c3a09 selects main-panel mode1
                            // for a picked building whose +1c business link is -1.
                            // The building stays picked and retains its camera.
                            let category = hit.category;
                            let block = &self.assets.selection.blocks[category];
                            if let Some(report) = &view.block_information
                                && report.available(block.id)
                            {
                                Some(
                                    if view
                                        .ordinary_toolbar_category(&self.assets.selection.blocks)
                                        .is_some()
                                    {
                                        menu.locator_assets().ordinary_property_body(
                                            &self.renderer,
                                            report,
                                            block.id,
                                            &view.utility_report,
                                        )?
                                    } else {
                                        menu.locator_assets().property_panel(
                                            &self.renderer,
                                            report,
                                            block.id,
                                            &view.utility_report,
                                        )?
                                    },
                                )
                            } else {
                                // There is no original Block Information
                                // report until the current player snapshot is
                                // available. Defer the panel for that interval.
                                None
                            }
                        }
                        _ => None,
                    };
                }
                view.title_state = Some((view.selected, hovered));
                view.hovered = hovered;
                let name = |index: Option<crate::selection::Hit>| {
                    index
                        .map(|i| {
                            let category = &self.assets.selection.blocks[i.category].name;
                            match i.building {
                                Some(b) => match view.scene.inspection_buildings[b].saved_slot {
                                    Some(slot) => format!("{category}, building #{slot}"),
                                    None => format!("{category}, scene building {b}"),
                                },
                                None => category.clone(),
                            }
                        })
                        .unwrap_or_else(|| "none".to_owned())
                };
                presenter.window.set_title(&format!(
                    "OpenMT2001 - {} {} / selected: {} / hover: {} / {}",
                    view.live_status
                        .as_ref()
                        .map_or("inspection", |s| if s.failure.is_some() {
                            "stopped"
                        } else if s.running && !s.script_paused {
                            "running"
                        } else {
                            "paused"
                        }),
                    if view.strategic { "strategic" } else { "city" },
                    name(view.selected),
                    name(view.hovered),
                    if view.strategic {
                        "double-click block to enter, compass restores camera, Esc returns"
                    } else {
                        "right-drag orbit, compass overview, Esc returns"
                    }
                ));
            }
            if let Some(detail) = view.normal_star_detail() {
                let changed = view.scene.apply_normal_star_detail(detail, view.camera)?;
                let updates = changed
                    .into_iter()
                    .map(|index| (index, view.scene.draws[index].borrowed()))
                    .collect::<Vec<_>>();
                self.renderer
                    .update_dynamic_vertices_batch(&mut view.prepared, &updates)?;
            }
            if view.strategic {
                let colors = view
                    .block_information
                    .as_ref()
                    .map(|report| &report.title_colors);
                let map = self.strategic_scene.prepare(
                    &self.assets.strategic_scene,
                    &self.renderer,
                    view.scene.camera,
                    colors,
                )?;
                self.renderer
                    .draw_prepared(&self.color_view, &self.depth_view, map);
            } else {
                update_city_pulse(
                    &self.assets.city_pulse,
                    &self.renderer,
                    view.pulse_draw,
                    view.pulse_state.as_ref(),
                    view.camera,
                    &mut view.scene,
                    &mut view.prepared,
                )?;
                update_city_smoke(
                    &self.renderer,
                    (
                        view.smoke_draw,
                        view.smoke_state.as_ref(),
                        view.settings_menu.graphics.detail,
                    ),
                    view.trees.as_ref(),
                    view.camera,
                    &mut view.scene,
                    &mut view.prepared,
                )?;
                self.renderer
                    .draw_prepared(&self.color_view, &self.depth_view, &view.prepared);
                if view.dust.active && view.dust.prepared.is_some() {
                    // Recompute the billboard basis after camera movement, even
                    //between authoritative point updates from the worker.
                    if let Some(status) = &view.live_status {
                        view.dust.update(
                            &self.assets.dust,
                            &self.renderer,
                            &status.demolition_points,
                            view.camera,
                            view.scene.camera,
                        )?;
                    }
                    self.renderer.draw_world_overlay(
                        &self.color_view,
                        &self.depth_view,
                        view.dust
                            .prepared
                            .as_ref()
                            .ok_or("dust draw lost prepared resources")?,
                    );
                }
                self.assets.sidewalk_labels.update(
                    &self.renderer,
                    &mut self.sidewalk_labels,
                    &mut self.sidewalk_shading,
                    &view.scene,
                    view.camera,
                    view.settings_menu.graphics.distance,
                )?;
                self.renderer.draw_world_overlay(
                    &self.color_view,
                    &self.depth_view,
                    &self.sidewalk_labels,
                );
                for index in 0..self.mr_monopoly_draws {
                    self.renderer.update_camera(
                        &self.mr_monopoly,
                        index,
                        view.scene.camera,
                        view.scene.fog,
                    )?;
                }
                self.renderer.draw_world_overlay(
                    &self.color_view,
                    &self.depth_view,
                    &self.mr_monopoly,
                );
                // The quoted model is world geometry. Composite it while the city
                // depth attachment is still retained, before any UI pass clears it.
                if let (Some(planning), Some(_menu)) = (&mut view.planning, &self.menu) {
                    let inputs = &self.assets.inputs;
                    let files = crate::build_view::PreviewFiles {
                        build_file: &inputs.build_file,
                        executable: &inputs.executable,
                        directory: &inputs.directory,
                        archive: &inputs.archive,
                        suncols: &inputs.suncols,
                        phase: view.live_status.as_ref().map_or(9, |status| status.hour),
                    };
                    if let Some(preview) = planning.preview(
                        &self.renderer,
                        view.scene.camera,
                        view.scene.fog,
                        &self.assets.selection,
                        files,
                    )? {
                        self.renderer.draw_world_overlay(
                            &self.color_view,
                            &self.depth_view,
                            preview,
                        );
                        if let Some(marker) = planning.preview_marker() {
                            self.renderer.draw_world_overlay(
                                &self.color_view,
                                &self.depth_view,
                                marker,
                            );
                        }
                    }
                }
                self.renderer
                    .update_camera(&view.markers, 0, view.scene.camera, view.scene.fog)?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &view.markers);
            }
            if view.city_layers().hud_visible()
                && let (Some(menu), Some(status)) = (&self.menu, &view.live_status)
            {
                view.event_list.update(
                    menu.event_list_assets(),
                    &self.renderer,
                    status.event_list_visible,
                    status.event_list_settled,
                )?;
                if let Some(panel) = &view.event_list.prepared {
                    //43ce12 inserts model14, then13..0 move to head;43cb40 draws tail first.
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
            }
            if view.city_layers().hud_visible() {
                // Native 404eb0 closes the current main/Block Information panel
                // before 43be20 opens the construction planner. Keep its retained
                // state for dismissal, but do not composite glyphs behind the
                // planner's translucent body.
                let base_panel_visible = view.main_panel_visible() && view.planning.is_none();
                if base_panel_visible
                    && view.details_visible
                    && !view.strategic
                    && view.operations.building.is_none()
                    && let Some((panel, _)) = &view.information
                {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
                if base_panel_visible
                    && view.details_visible
                    && !view.strategic
                    && view.adjustment.is_none()
                    && view.operations.building.is_some()
                    && let (Some(menu), Some(data)) = (&self.menu, &view.operations.inspection)
                {
                    let panel = menu.business_assets().prepare(
                        &self.renderer,
                        &view.operations,
                        data.demolition_label,
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &panel);
                }
                if base_panel_visible
                    && let Some(menu) = &self.menu
                    && let Some(policy) =
                        view.toolbar_policy(menu, &self.assets.selection.blocks, false)
                    && policy.shown
                {
                    let panel = menu.locator_assets().ordinary_toolbar_scene(
                        &self.renderer,
                        policy,
                        &view.toolbar_input,
                        &view.utility_report,
                        view.details_visible,
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &panel);
                }
                if base_panel_visible {
                    self.renderer.draw_overlay(
                        &self.color_view,
                        &self.depth_view,
                        &view.detail_icons[usize::from(view.details_visible)],
                    );
                }
                // Mode3 entry4b5370 hides the miniature navigator. Keeping
                // it over the full map obscures the southeastern properties.
                if !view.strategic
                    && (view.ui_graph.is_none()
                        || view
                            .ui_graph
                            .as_ref()
                            .is_some_and(|g| g.shell(camera_live::ui_graph::PanelId::F)))
                {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &view.navigator);
                }
                if view.strategic
                    && let Some(panel) = view.locator.as_ref().and_then(|l| l.prepared.as_ref())
                {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
                if view.statistics_visible()
                    && let Some(panel) = view.comparison.as_ref().and_then(|c| c.prepared.as_ref())
                {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
                // Connected chat: retained lines along the bottom edge of the 3D view,
                // and the Chat panel while it is open.
                if let (Some(chat), Some(menu)) = (
                    view.live_status.as_ref().and_then(|s| s.chat.as_ref()),
                    &self.menu,
                ) && (view.chat_panel.open || !chat.lines.is_empty())
                {
                    let panel = menu.comparison_assets().prepare_chat(
                        &self.renderer,
                        chat,
                        &view.chat_panel,
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &panel);
                }
                let icon = if view.navigation_pressed && view.over_navigation() {
                    2
                } else if view.over_navigation() {
                    1
                } else {
                    0
                };
                if view.ui_graph.is_none() || view.graph_body(camera_live::ui_graph::PanelId::E) {
                    self.renderer.draw_overlay(
                        &self.color_view,
                        &self.depth_view,
                        &view.navigation[icon],
                    );
                }
                // The bottom-right round controls share the navigation backing;
                // draw their original-texture faces after it, along with the clock.
                if let Some(status) = &view.live_status {
                    if view.prompts.request().is_some()
                        || view.stats_requested
                        || status.stats.is_some()
                    {
                        view.time_panel.cancel_input();
                        view.time_pressed = None;
                    }
                    view.time_panel
                        .update(&self.assets.time_panel, &self.renderer, status)?;
                }
                if let Some(graph) = &view.ui_graph {
                    view.time_panel.graph_gates(
                        graph.body(camera_live::ui_graph::PanelId::Clock),
                        graph.body(camera_live::ui_graph::PanelId::E),
                        graph.speed_disabled != 0,
                        graph.speed_blink,
                    );
                    view.time_panel
                        .prepare_graph(&self.assets.time_panel, &self.renderer)?;
                    if let Some(bottom) = &view.time_panel.bottom_prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, bottom);
                    }
                }
                if let Some(panel) = &view.time_panel.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
                if view.cash_arrow.active
                    && let Some(arrow) = &view.cash_arrow.prepared
                {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, arrow);
                }
                if let Some(tooltip) = &view.tooltip {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, tooltip);
                }
                if let (Some(adjustment), Some(menu)) = (&mut view.adjustment, &self.menu) {
                    adjustment.pointer(menu.adjustment_assets(), view.pointer);
                    adjustment.prepare(menu.adjustment_assets(), &self.renderer)?;
                    if let Some(panel) = &adjustment.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                }
                if let Some(menu) = &self.menu {
                    if let Some(planning) = &mut view.planning {
                        planning.sync_model_variants(&self.assets.inputs.build_file)?;
                        if let Some(grid) = planning.grid_overlay(&self.renderer)? {
                            self.renderer
                                .draw_overlay(&self.color_view, &self.depth_view, &grid);
                        }
                        menu.planning_assets().prepare(&self.renderer, planning)?;
                        if let Some(panel) = &planning.prepared {
                            self.renderer
                                .draw_overlay(&self.color_view, &self.depth_view, panel);
                        }
                    } else if !view.strategic
                        && view.main_panel_visible()
                        && view.details_visible
                        && view
                            .ordinary_toolbar_category(&self.assets.selection.blocks)
                            .is_none()
                        && view._fresh_session.is_some()
                        && view.selected.is_some_and(|s| s.building.is_none())
                        && !view.selected.is_some_and(|s| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status
                                    .auction_locked_categories
                                    .contains(&self.assets.selection.blocks[s.category].id)
                            })
                        })
                    {
                        let [l, t, r, b] = menu.planning_assets().entry_bounds;
                        let hovered = view.pointer.is_some_and(|p| {
                            let [x, y] = p.map(|v| v as f32 * 0.5);
                            x >= l && x <= r && y >= t && y <= b
                        });
                        let icon = menu.planning_assets().entry(&self.renderer, hovered)?;
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, &icon);
                    }
                }
                if let Some(menu) = &self.menu {
                    view.auction_notice.prepare(
                        menu.auction_notice_assets(),
                        &self.renderer,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )?;
                    if let Some(panel) = &view.auction_notice.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                    let notices = view
                        .live_status
                        .as_ref()
                        .map(|s| s.notices.as_slice())
                        .unwrap_or(&[]);
                    view.notifications.prepare(
                        menu.notification_assets(),
                        &self.renderer,
                        if view.strategic { &[] } else { notices },
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )?;
                    if let Some(panel) = &view.notifications.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                }
                if let (Some(menu), Some(status)) = (&self.menu, &view.live_status) {
                    let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                    let entry = menu.card_assets().entry_scene(
                        &self.renderer,
                        status.card_entry,
                        status.card_visual,
                        &view.card_input,
                        pointer,
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &entry);
                    if let Some(card) = status.card {
                        let panel = menu.card_assets().prepare(
                            &self.renderer,
                            card,
                            &view.card_input,
                            pointer,
                        )?;
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, &panel);
                    }
                }
                Self::draw_tutorial_visual(
                    &self.assets,
                    &self.menu,
                    &self.renderer,
                    &self.color_view,
                    &self.depth_view,
                    view,
                )?;
                if let Some(menu) = &self.menu {
                    view.market_share_panel.update(
                        menu.market_share_assets(),
                        &self.renderer,
                        view.live_status
                            .as_ref()
                            .and_then(|s| s.market_share.as_ref()),
                    )?;
                    if let Some(panel) = &view.market_share_panel.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                }
                if view.prompts.visible()
                    && let Some(menu) = &self.menu
                {
                    view.prompts.cursor(
                        view.pointer
                            .map(|p| p.map(|v| v as f32 * 0.5))
                            .unwrap_or([-1.; 2]),
                    );
                    let panel = menu.prompt_assets().prepare_positioned(
                        &self.renderer,
                        &view.prompts,
                        view.live_status.as_ref().map_or([0.; 2], |status| {
                            status.tutorial_interaction.prompt_offset()
                        }),
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &panel);
                }
            }
            // Statistics is a secondary-group screen, like Options. Do not
            // composite the retained primary HUD underneath its translucent mesh.
            // 49dcb0/4b5370 retain the city camera and add no fullscreen fade.
            if let Some(menu) = &self.menu {
                view.stats_panel.update(
                    menu.stats_assets(),
                    &self.renderer,
                    view.live_status
                        .as_ref()
                        .and_then(|status| status.stats.as_ref()),
                )?;
                if let Some(panel) = &view.stats_panel.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
            }
            if view.options_visible() {
                if view.save_visible() {
                    let panel = self.assets.save_dialog.prepare(
                        &self.renderer,
                        &view.save_dialog,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &panel);
                    if let Some(modal) = self.assets.save_dialog.prepare_name_modal(
                        &self.renderer,
                        &view.save_dialog,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )? {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, &modal);
                    }
                    if let Some((dialog, pressed_button)) = view.save_dialog.generic_prompt()
                        && let Some(menu) = &self.menu
                    {
                        let panel = menu.frontend_prompt_assets().prepare_screen_confirmation(
                            &self.renderer,
                            dialog,
                            view.pointer
                                .map(|p| p.map(|v| v as f32 * 0.5))
                                .unwrap_or([-1.; 2]),
                            pressed_button,
                        )?;
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, &panel);
                    }
                } else if view.settings_visible() {
                    view.settings_menu.update_levels(
                        &self.assets.settings_menu,
                        &self.renderer,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                        self.audio_levels,
                    )?;
                    if let Some(panel) = &view.settings_menu.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                } else {
                    view.game_options.update(
                        &self.assets.game_options,
                        &self.renderer,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )?;
                    if let Some(panel) = &view.game_options.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                }
            }
            if view.game_options.quit_prompt.request().is_some()
                && let Some(menu) = &self.menu
            {
                view.game_options.quit_prompt.cursor(
                    view.pointer
                        .map(|p| p.map(|v| v as f32 * 0.5))
                        .unwrap_or([-1.; 2]),
                );
                let panel = menu
                    .frontend_prompt_assets()
                    .prepare_frontend_prompt(&self.renderer, &view.game_options.quit_prompt)?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            // 4b4dae draws the pause banner after every interface group.
            if let Some(banner) = view.pause_banner.update(
                &self.assets.pause_banner,
                &self.renderer,
                view.live_status.as_ref().and_then(|s| s.pause_banner),
            )? {
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, banner);
            }
            // 0x458af0 captures the fully painted frame, interface included.
            if self.screenshot.take_pending() {
                screenshot::capture(presenter, &self.binding);
            }
            presenter
                .present_binding(&self.binding)
                .map_err(|e| e.to_string())?;
            return Ok(true);
        }
        Ok(false)
    }
}
