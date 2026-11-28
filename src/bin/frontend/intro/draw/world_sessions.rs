//! `Intro::poll_world_sessions`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn poll_world_sessions(&mut self) -> Result<(), String> {
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
        Ok(())
    }
}
