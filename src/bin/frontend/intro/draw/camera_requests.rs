//! `Intro::service_camera_requests`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn service_camera_requests(&mut self) -> Result<(), String> {
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
            camera_live::audio::service_frame(view, self.focus.sent() == self.focus.completed())?;
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
                        camera_live::SoundBoundary::RequireOriginalListener,
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
        Ok(())
    }
}
