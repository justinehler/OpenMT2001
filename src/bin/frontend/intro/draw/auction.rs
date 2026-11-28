//! `Intro::draw_auction`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn draw_auction(&mut self, presenter: &Presenter) -> Result<bool, String> {
        if let Some(view) = &mut self.inspection
            && view
                .ui_graph
                .as_ref()
                .is_none_or(|g| g.group_enabled(camera_live::navigation::Group::Tertiary))
            && let Some((scene, true)) = &view.auction.scene
        {
            self.renderer
                .draw_prepared(&self.color_view, &self.depth_view, scene);
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
                if let Some(panel) = &view.time_panel.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
            }
            if let Some(menu) = &self.menu {
                view.auction_panel.prepare(
                    menu.auction_panel_assets(),
                    &self.renderer,
                    view.live_status.as_ref().and_then(|s| s.auction.as_ref()),
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                )?;
                if let Some(panel) = &view.auction_panel.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
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
            if view.prompts.request().is_some()
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
            if view.prompts.request().is_none()
                && view.tutorial_message.request().is_some()
                && let Some(menu) = &self.menu
            {
                let panel = menu.prompt_assets().prepare_message_positioned(
                    &self.renderer,
                    &view.tutorial_message,
                    view.live_status.as_ref().map_or([0.; 2], |status| {
                        status.tutorial_interaction.prompt_offset()
                    }),
                )?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            if let Some(status) = &view.live_status {
                presenter.window.set_title(&format!(
                    "OpenMT2001 - auction{}",
                    status
                        .failure
                        .as_ref()
                        .map_or(String::new(), |e| format!(" - stopped: {e}"))
                ));
            }
            presenter
                .present_binding(&self.binding)
                .map_err(|e| e.to_string())?;
            return Ok(true);
        }
        Ok(false)
    }
}
