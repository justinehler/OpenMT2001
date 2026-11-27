//! UI-owned whole-prefix broker. Workers retain only a revocable per-World
//! mailbox, never a process Effects provider. The UI never waits for a worker.
use super::FreshRequest;
use std::{
    cell::Cell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
};

/// UI observation fence only; never an audio activation/volume guard. Route
///alone closes/opens it. Views may only read it and keep progress nonblocking.
#[derive(Clone)]
pub(crate) struct MusicAdmission(Rc<Cell<bool>>);
impl Default for MusicAdmission {
    fn default() -> Self {
        Self(Rc::new(Cell::new(true)))
    }
}
impl MusicAdmission {
    pub(crate) fn permits_attachment(&self) -> bool {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prefix {
    ReceiverAbsent,
    Complete(Option<openmt2001::native_input::focus::AudioOutcome>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AttachmentId(u64);
impl AttachmentId {
    pub(crate) fn value(self) -> u64 {
        self.0
    }
}

/// Actual synchronous UI effects. All input/frontend/audio mutations occur
/// within before_pause, not when the worker enqueues a request. No method may
/// wait on that Session worker or reenter Route. UI-only audio owners are valid.
pub(crate) trait Effects {
    fn activate(&mut self, attachment: AttachmentId) -> Result<(), String>;
    fn retire(&mut self, attachment: AttachmentId) -> Result<(), String>;
    fn before_pause(
        &mut self,
        attachment: AttachmentId,
        focused: bool,
        reason3: i8,
    ) -> Result<Prefix, String>;
}

struct PrefixRequest {
    focused: bool,
    reason3: i8,
    reply: mpsc::Sender<Result<Prefix, String>>,
}
struct Queue {
    live: bool,
    requests: VecDeque<PrefixRequest>,
}
struct Mailbox {
    id: AttachmentId,
    queue: Mutex<Queue>,
    #[cfg(test)]
    ready: std::sync::Condvar,
}
impl Mailbox {
    fn new(id: AttachmentId) -> Self {
        Self {
            id,
            queue: Mutex::new(Queue {
                live: true,
                requests: VecDeque::new(),
            }),
            #[cfg(test)]
            ready: std::sync::Condvar::new(),
        }
    }
    fn request(&self, focused: bool, reason3: i8) -> Result<Prefix, String> {
        let (reply, answer) = mpsc::channel();
        {
            let mut queue = self.queue.lock().map_err(|_| "Focus mailbox poisoned")?;
            if !queue.live {
                return Err("Focus attachment retired".into());
            }
            queue.requests.push_back(PrefixRequest {
                focused,
                reason3,
                reply,
            });
            #[cfg(test)]
            self.ready.notify_all();
        }
        // No mailbox lock crosses this wait. Detach can always invalidate and
        // wake it, even if the UI has not serviced the admitted request yet.
        answer
            .recv()
            .map_err(|_| "Focus prefix service retired".to_owned())?
    }
    fn take(&self) -> Result<Option<PrefixRequest>, String> {
        let mut queue = self.queue.lock().map_err(|_| "Focus mailbox poisoned")?;
        if !queue.live {
            return Ok(None);
        }
        Ok(queue.requests.pop_front())
    }
    fn retire(&self) {
        // No user callback executes under this lock. Even poisoned transport
        // storage must be revoked and its waiting replies closed, never reused.
        let pending = {
            let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
            queue.live = false;
            std::mem::take(&mut queue.requests)
        };
        for request in pending {
            let _ = request.reply.send(Err("Focus attachment retired".into()));
        }
    }
    #[cfg(test)]
    fn wait_pending(&self) {
        let queue = self.queue.lock().unwrap();
        let (queue, timeout) = self
            .ready
            .wait_timeout_while(queue, std::time::Duration::from_secs(5), |q| {
                q.live && q.requests.is_empty()
            })
            .unwrap();
        assert!(
            !timeout.timed_out() && !queue.requests.is_empty(),
            "worker did not reach prefix boundary"
        );
    }
}

pub(crate) struct Event {
    focused: bool,
    attachment: bool,
    mailbox: Arc<Mailbox>,
    reply: mpsc::Sender<Result<(), String>>,
}
impl Event {
    pub(super) fn is_attachment(&self) -> bool {
        self.attachment
    }
    pub(super) fn apply(
        self,
        host: &mut crate::live_host::Host,
        pause: &mut openmt2001::pause::State,
        timers: &mut [openmt2001::timers::Task],
    ) -> Result<(), String> {
        let result = host.focus(pause, timers, self.focused, |reason3| {
            self.mailbox.request(self.focused, reason3)
        });
        let _ = self.reply.send(result.clone());
        result
    }
}

pub(super) enum BeforeFrame {
    FocusApplied,
    Request(Option<FreshRequest>),
}
pub(super) fn before_frame(
    request: Option<FreshRequest>,
    host: &mut crate::live_host::Host,
    pause: &mut openmt2001::pause::State,
    timers: &mut [openmt2001::timers::Task],
) -> Result<BeforeFrame, String> {
    match request {
        Some(FreshRequest::Focus(event)) => {
            event.apply(host, pause, timers)?;
            Ok(BeforeFrame::FocusApplied)
        }
        other => Ok(BeforeFrame::Request(other)),
    }
}

struct Binding {
    target: mpsc::Sender<FreshRequest>,
    mailbox: Arc<Mailbox>,
}
#[derive(Default)]
pub(super) struct Route {
    current: Option<bool>,
    effects: Option<Box<dyn Effects>>,
    binding: Option<Binding>,
    next_id: u64,
    pending: VecDeque<mpsc::Receiver<Result<(), String>>>,
    failure: Option<String>,
    sent: u64,
    completed: u64,
    music: MusicAdmission,
}
impl Route {
    pub(super) fn music_admission(&self) -> MusicAdmission {
        self.music.clone()
    }

    pub(super) fn sent(&self) -> u64 {
        self.sent
    }
    pub(super) fn completed(&self) -> u64 {
        self.completed
    }

    pub(super) fn install(&mut self, effects: Box<dyn Effects>) -> Result<(), String> {
        if self.effects.is_some() || self.binding.is_some() {
            return Err("Process focus effects already installed".into());
        }
        self.effects = Some(effects);
        Ok(())
    }
    pub(super) fn enabled(&self) -> bool {
        self.effects.is_some()
    }
    #[cfg(test)]
    pub(super) fn wait_prefix(&self) {
        self.binding.as_ref().unwrap().mailbox.wait_pending();
    }
    #[cfg(test)]
    pub(super) fn attachment_id(&self) -> Option<u64> {
        self.binding.as_ref().map(|b| b.mailbox.id.value())
    }
    fn send(&mut self, focused: bool, attachment: bool) -> Result<(), String> {
        let binding = self.binding.as_ref().ok_or("Focus Session not attached")?;
        let (reply, result) = mpsc::channel();
        binding
            .target
            .send(FreshRequest::Focus(Event {
                focused,
                attachment,
                mailbox: Arc::clone(&binding.mailbox),
                reply,
            }))
            .map_err(|_| "Focus Session retired".to_owned())?;
        self.sent = self
            .sent
            .checked_add(1)
            .ok_or("Focus operation sequence exhausted")?;
        self.pending.push_back(result);
        self.music.0.set(false);
        Ok(())
    }
    pub(super) fn observe(&mut self, focused: bool) -> Result<(), String> {
        self.current = Some(focused);
        if self.binding.is_some() {
            self.send(focused, false)?;
        }
        Ok(())
    }
    /// Snapshot/send is one UI-thread operation. Loading history is represented
    /// once; later source events use the same worker FIFO. No warm reattachment.
    pub(super) fn attach(&mut self, target: mpsc::Sender<FreshRequest>) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.binding.is_some() {
            return Err("Focus Session already attached".into());
        }
        let current = self
            .current
            .ok_or("No OS focus observation for attachment")?;
        let id = self
            .next_id
            .checked_add(1)
            .ok_or("Focus attachment identity exhausted")?;
        self.next_id = id;
        let mailbox = Arc::new(Mailbox::new(AttachmentId(id)));
        let effects = self.effects.as_mut().ok_or("Focus effects unavailable")?;
        if let Err(error) = effects.activate(mailbox.id) {
            mailbox.retire();
            let _ = effects.retire(mailbox.id);
            self.failure = Some(error.clone());
            return Err(error);
        }
        self.binding = Some(Binding { target, mailbox });
        if let Err(error) = self.send(current, true) {
            self.detach();
            return Err(error);
        }
        Ok(())
    }
    /// Same exclusive UI owner as service/attach. No admitted actual prefix can
    /// overlap this call. A pending enqueue is revoked/drained under the queue
    /// lock, and a delayed old Event can never reach process Effects again.
    pub(super) fn detach(&mut self) {
        if let Some(binding) = self.binding.take() {
            self.music.0.set(false);
            binding.mailbox.retire();
            if let Some(effects) = &mut self.effects {
                if let Err(error) = effects.retire(binding.mailbox.id) {
                    self.failure = Some(error);
                }
            }
        }
        self.pending.clear();
        self.completed = self.sent;
    }
    /// Execute only actual UI prefix work, never send a reciprocal request to
    /// the waiting worker. Mutex protects transport only; it is released before
    /// callbacks. &mut Route serializes effect completion, retire and next attach.
    pub(super) fn service_one(&mut self) -> Result<bool, String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let Some(binding) = &self.binding else {
            return Ok(false);
        };
        let Some(request) = binding.mailbox.take()? else {
            return Ok(false);
        };
        let result = self
            .effects
            .as_mut()
            .ok_or("Focus effects unavailable")?
            .before_pause(binding.mailbox.id, request.focused, request.reason3);
        let failed = result.as_ref().err().cloned();
        let _ = request.reply.send(result);
        if let Some(error) = failed {
            self.music.0.set(false);
            self.failure = Some(error.clone());
            return Err(error);
        }
        Ok(true)
    }
    pub(super) fn poll(&mut self) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        while let Some(reply) = self.pending.front() {
            match reply.try_recv() {
                Err(mpsc::TryRecvError::Empty) => break,
                Ok(result) => {
                    self.pending.pop_front();
                    result?;
                    self.completed = self
                        .completed
                        .checked_add(1)
                        .ok_or("Focus completion sequence exhausted")?;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending.pop_front();
                    return Err("Focus operation retired before acknowledgement".into());
                }
            }
        }
        self.music
            .0
            .set(self.failure.is_none() && self.binding.is_some() && self.pending.is_empty());
        Ok(())
    }
}
impl Drop for Route {
    fn drop(&mut self) {
        self.detach();
    }
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;

/// Typed result completion gate used by draw_result; a held completion stays
///in its existing receiver while camera and worker servicing can continue.
pub(super) fn poll_result(
    receiver: &mpsc::Receiver<Result<super::ResultSnapshot, String>>,
    admission: Option<&MusicAdmission>,
) -> Result<Result<super::ResultSnapshot, String>, mpsc::TryRecvError> {
    if admission.is_some_and(|gate| !gate.permits_attachment()) {
        return Err(mpsc::TryRecvError::Empty);
    }
    receiver.try_recv()
}
