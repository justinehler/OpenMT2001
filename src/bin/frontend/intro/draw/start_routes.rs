//! `Intro::start_requested_city`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn start_requested_city(
        &mut self,
        inspection_camera: CameraSample,
    ) -> Result<(), String> {
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
        Ok(())
    }
}
