//! `Intro::poll_adjustment`, a phase of `Intro::draw` (#99).
use super::*;

impl Intro {
    pub(super) fn poll_adjustment(&mut self) -> Result<(), String> {
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
        Ok(())
    }
}
