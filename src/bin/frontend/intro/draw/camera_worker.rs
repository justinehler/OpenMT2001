//! `Intro::poll_camera_worker`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn poll_camera_worker(&mut self) -> Result<(), String> {
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
        Ok(())
    }
}
