//! Ordered camera ownership boundary. Worker waits only on the direct reply;
//! the event loop must drain Inbox during modal UI and pending worker requests.
//! This transport never owns a second bank, advances a clock, or invents an ack.
use openmt2001::camera::bank::Capture;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub type Generation = u64;
pub type Sequence = u64;

/// Retirement is admitted only by the event thread explicitly relinquishing
/// this generation. It is not an acknowledgement or a successful operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestError {
    Retired,
    Failed(String),
}
impl From<String> for RequestError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}
impl From<&str> for RequestError {
    fn from(error: &str) -> Self {
        Self::Failed(error.into())
    }
}
impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retired => f.write_str("Camera generation explicitly retired"),
            Self::Failed(error) => f.write_str(error),
        }
    }
}
impl std::error::Error for RequestError {}

/// Raw MOV words: observations do not decode or validate opaque camera storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    pub eye: [u32; 3],
    pub target: [u32; 3],
    pub up: [u32; 3],
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Admission {
    pub tutorial_session: bool,
    pub all_disabled: bool,
    pub camera_disabled: bool,
    pub enabled_controls: Vec<u32>,
}
/// Named recovered setters. Tutorial entries are exact script requests: the UI
/// resolves their actual target and initializer or rejects them. No mode6->12
/// promotion, arbitrary pose replacement, or guessed fresh/menu history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeOperation {
    SetKind(u32),
    SetTargetWords([u32; 3]),
    SetImmediateWords([u32; 3]),
    ResetYaw,
    WorldResetFinalFields,
    StrategicAfterUiClear,
    TutorialMode { mode: u32, target: u32 },
    TutorialCityView,
    TutorialPlanView,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Explicit lifecycle boundary, acknowledged before any further request.
    /// Caller establishes Frame's lifecycle separately; no implicit clock sync.
    Install {
        capture: Capture,
        admission: Admission,
    },
    Apply {
        operation: NativeOperation,
        admission: Admission,
    },
    Admission(Admission),
    /// UI consumes latched edges/deltas once, mode callback then common once.
    /// UI-owned Build/input gates must be read here, never from stale Status.
    CentralTail {
        admission: Admission,
    },
    CurrentPose,
    /// Actual dispatcher export bits; no clamp, fract, clock, or synthetic tick.
    Sample {
        ratio_bits: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    Install,
    Apply,
    Admission,
    CentralTail,
    CurrentPose,
    Sample(u32),
}
impl Operation {
    fn boundary(&self) -> Boundary {
        match self {
            Self::Install { .. } => Boundary::Install,
            Self::Apply { .. } => Boundary::Apply,
            Self::Admission(_) => Boundary::Admission,
            Self::CentralTail { .. } => Boundary::CentralTail,
            Self::CurrentPose => Boundary::CurrentPose,
            Self::Sample { ratio_bits } => Boundary::Sample(*ratio_bits),
        }
    }
}
/// Only recovered live timer operations can cross the UI callback boundary.
/// Host applies them before the dispatcher reloads the current task's remainder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskEffect {
    SetEnabled { mask: u32, enabled: bool },
    ClearElapsed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub current: Pose,
    /// Present exactly on Sample, from the completed primary sample and stored
    /// up. Save captures the bank separately without requesting another sample.
    pub sampled: Option<Pose>,
    pub task_effects: Vec<TaskEffect>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Ack {
    generation: Generation,
    sequence: Sequence,
    boundary: Boundary,
    observation: Observation,
}
struct Request {
    generation: Generation,
    sequence: Sequence,
    operation: Operation,
    reply: mpsc::Sender<Result<Ack, String>>,
}
/// Deliberately not Clone: one serial requester owns the sequence and observes
/// each completed operation before producing the next dependent operation.
pub struct Worker {
    generation: Generation,
    next: Sequence,
    installed: bool,
    failure: Option<String>,
    sender: mpsc::Sender<Request>,
    retired: Arc<AtomicBool>,
}
/// Move this together with the same bank on Inspection refresh. A true session
/// replacement gets a new generation/channel and an explicit Install instead.
pub struct Inbox {
    generation: Generation,
    next: Sequence,
    installed: bool,
    failure: Option<String>,
    receiver: mpsc::Receiver<Request>,
    retired: Arc<AtomicBool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Idle,
    Completed,
}

pub fn channel(generation: Generation) -> (Worker, Inbox) {
    let (sender, receiver) = mpsc::channel();
    let retired = Arc::new(AtomicBool::new(false));
    (
        Worker {
            generation,
            next: 0,
            installed: false,
            failure: None,
            sender,
            retired: Arc::clone(&retired),
        },
        Inbox {
            generation,
            next: 0,
            installed: false,
            failure: None,
            receiver,
            retired,
        },
    )
}

fn validate_observation(boundary: Boundary, observation: &Observation) -> Result<(), String> {
    if observation.sampled.is_some() != matches!(boundary, Boundary::Sample(_)) {
        return Err("Camera protocol sampled observation does not match boundary".into());
    }
    if boundary != Boundary::CentralTail && !observation.task_effects.is_empty() {
        return Err("Only CentralTail can return live task effects".into());
    }
    Ok(())
}

impl Worker {
    pub fn generation(&self) -> Generation {
        self.generation
    }
    pub fn installed(&self) -> bool {
        self.installed && self.failure.is_none() && !self.retired()
    }
    pub fn retired(&self) -> bool {
        self.retired.load(Ordering::Acquire)
    }
    fn disconnected(&self, error: &str) -> RequestError {
        if self.retired() {
            RequestError::Retired
        } else {
            RequestError::Failed(error.into())
        }
    }
    /// No timeout fallback. Explicit retirement cancels pending/future work;
    /// unexpected disconnect, owner rejection and protocol errors remain fatal.
    /// A validated ack already received is committed even if retirement races
    /// its receipt: in particular its live task effects must not be discarded.
    pub fn request(&mut self, operation: Operation) -> Result<Observation, RequestError> {
        if let Some(error) = &self.failure {
            return Err(RequestError::Failed(error.clone()));
        }
        if self.retired() {
            return Err(RequestError::Retired);
        }
        let result = self.exchange(operation);
        if let Err(RequestError::Failed(error)) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    fn exchange(&mut self, operation: Operation) -> Result<Observation, RequestError> {
        let boundary = operation.boundary();
        if self.installed == matches!(boundary, Boundary::Install) {
            return Err("Camera protocol requires exactly one initial Install".into());
        }
        let following = self
            .next
            .checked_add(1)
            .ok_or("Camera sequence exhausted")?;
        let (reply, response) = mpsc::channel();
        self.sender
            .send(Request {
                generation: self.generation,
                sequence: self.next,
                operation,
                reply,
            })
            .map_err(|_| self.disconnected("Camera event-thread inbox disconnected"))?;
        let ack = response
            .recv()
            .map_err(|_| self.disconnected("Camera event thread dropped acknowledgement"))??;
        if ack.generation != self.generation
            || ack.sequence != self.next
            || ack.boundary != boundary
        {
            return Err("Camera acknowledgement generation/sequence/boundary mismatch".into());
        }
        validate_observation(boundary, &ack.observation)?;
        self.next = following;
        self.installed = true;
        Ok(ack.observation)
    }
}

impl Inbox {
    pub fn generation(&self) -> Generation {
        self.generation
    }
    /// The sole retirement writer. Publish intent BEFORE dropping queued
    /// requests/direct reply senders, which wakes a blocked Worker. Drop alone
    /// deliberately does not set this flag. No queued owner operation is run.
    pub fn retire(self) {
        self.retired.store(true, Ordering::Release);
        drop(self);
    }
    /// Nonblocking receive, synchronous event-thread bank operation, direct ack.
    /// Call repeatedly until Idle alongside auction/audio service on every UI
    /// pass, including prompts/options/results and pending query/save/refresh.
    /// Never wait for FreshRequest or call back into the blocked worker here.
    pub fn try_service(
        &mut self,
        apply: impl FnOnce(Operation) -> Result<Observation, String>,
    ) -> Result<Service, String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.service_inner(None, apply);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    /// A bounded event-loop rendezvous after a completed request. This wait
    /// schedules transport only: it never advances a timer or fabricates work.
    /// UI callers retain an overall service budget before rendering/input.
    pub fn wait_service(
        &mut self,
        timeout: std::time::Duration,
        apply: impl FnOnce(Operation) -> Result<Observation, String>,
    ) -> Result<Service, String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.service_inner(Some(timeout), apply);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    fn service_inner(
        &mut self,
        wait: Option<std::time::Duration>,
        apply: impl FnOnce(Operation) -> Result<Observation, String>,
    ) -> Result<Service, String> {
        let request = if let Some(timeout) = wait {
            match self.receiver.recv_timeout(timeout) {
                Ok(request) => request,
                Err(mpsc::RecvTimeoutError::Timeout) => return Ok(Service::Idle),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Camera worker disconnected".into());
                }
            }
        } else {
            match self.receiver.try_recv() {
                Ok(request) => request,
                Err(mpsc::TryRecvError::Empty) => return Ok(Service::Idle),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Camera worker disconnected".into());
                }
            }
        };
        let boundary = request.operation.boundary();
        let result = (|| {
            if request.generation != self.generation || request.sequence != self.next {
                return Err("Camera request generation/sequence mismatch".into());
            }
            if self.installed == matches!(boundary, Boundary::Install) {
                return Err("Camera protocol requires exactly one initial Install".into());
            }
            let following = self
                .next
                .checked_add(1)
                .ok_or("Camera sequence exhausted")?;
            let observation = apply(request.operation)?;
            validate_observation(boundary, &observation)?;
            self.next = following;
            self.installed = true;
            Ok(Ack {
                generation: self.generation,
                sequence: request.sequence,
                boundary,
                observation,
            })
        })();
        let failure = result.as_ref().err().cloned();
        request
            .reply
            .send(result)
            .map_err(|_| "Camera worker dropped direct reply receiver")?;
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(Service::Completed)
    }
}

#[cfg(test)]
#[path = "camera_bridge/tests.rs"]
mod tests;
