//! Ordered camera ownership boundary. Worker services same-operation calls on the direct reply;
//! the event loop must drain Inbox during modal UI and pending worker requests.
//! This transport never owns a second bank, advances a clock, or invents an ack.
use openmt2001::camera::bank::Capture;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[path="camera_bridge/resident_source.rs"]
mod resident_source;
pub use resident_source::{ResidentSource,ResidentRead};

pub type Generation = u64;
pub type Sequence = u64;
///Opaque identity of the next CentralTail on this serial Worker. Constructed
///from the same checked generation/sequence that the request exchange uses.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct CentralIdentity { generation:Generation,sequence:Sequence }
impl CentralIdentity {
    #[cfg(test)]
    pub(crate) fn synthetic(generation:Generation,sequence:Sequence)->Self{Self{generation,sequence}}
}


/// Synthetic identities exist ONLY in binary test fixtures. Production values
/// can be obtained only from World::fresh's checked topology writer.
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]
pub enum PropertyEpoch {
    World(openmt2001::level_startup::SourceEpoch),
    #[cfg(test)]
    Synthetic(u64),
}
impl PropertyEpoch {
    pub fn serial(self)->u64{match self{
        Self::World(epoch)=>epoch.serial(),
        #[cfg(test)] Self::Synthetic(value)=>value,
    }}
    pub fn follows(self,old:Self)->bool{match(self,old){
        (Self::World(a),Self::World(b))=>a>b,
        #[cfg(test)] (Self::Synthetic(a),Self::Synthetic(b))=>a>b,
        #[cfg(test)] _=>false,
    }}
    #[cfg(test)]
    pub fn synthetic(value:u64)->Self{Self::Synthetic(value)}
}

/// An immutable install-only copy of the completed Session's actual World.
/// No Manager, C/F tables, Scene, camera or status fallback is captured here.
/// This is a bounded modern admission source, NOT proof of native44faf0
/// production ordering: native topology/44fe60/44f8b0/44faf0 precedes later
/// startup/load writers. Kind13/removal/renderer and that exact source stage
/// remain gated; no full native callback is acknowledged by this payload.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct PropertySource {
    resident:Option<ResidentSource>,
    epoch:PropertyEpoch,
    resource:crate::selection::pavement_resource::LocalPavementHandle,
    rows:[[u8;380];52],
    count:i32,
}
impl PropertySource {
    pub fn capture(session:&crate::fresh_city::Session<'_>,resource:crate::selection::pavement_resource::LocalPavementHandle)->Self{
        Self{resident:Some(ResidentSource::capture(session)),epoch:PropertyEpoch::World(session.world.topology.epoch()),resource,
            rows:session.world.categories,count:session.world.topology.count617624()}
    }
    pub fn resident(&self)->Result<&ResidentSource,String>{self.resident.as_ref().ok_or_else(||"actual Session resident source absent".into())}
    /// Only explicit cfg(test) property-only fixtures may omit resident authority.
    pub fn resident_for_install(&self)->Result<Option<&ResidentSource>,String>{
        if let Some(source)=&self.resident{return Ok(Some(source));}
        #[cfg(test)] {Ok(None)}
        #[cfg(not(test))] {Err("actual completed Session resident source absent".into())}
    }
    #[cfg(test)]
    pub(crate) fn test_with_resident(mut self,source:ResidentSource)->Self{self.resident=Some(source);self}
    pub fn epoch(&self)->PropertyEpoch{self.epoch}
    pub fn resource(&self)->crate::selection::pavement_resource::LocalPavementHandle{self.resource}
    pub fn rows(&self)->&[[u8;380];52]{&self.rows}
    pub fn count(&self)->i32{self.count}
    #[cfg(test)]
    pub fn synthetic(epoch:u64,rows:[[u8;380];52],count:i32,assets:&crate::selection::Assets)->Self{
        Self{resident:None,epoch:PropertyEpoch::synthetic(epoch),resource:assets.pavement_resource().binding().unwrap().handle(),rows,count}
    }
}

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
    pub upper:Option<std::sync::Arc<crate::navigator::upper::Inputs>>,
    pub strategic:Option<std::sync::Arc<crate::locator::strategic::Inputs>>,
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
        property: Option<Arc<PropertySource>>,
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
    RawCapture,
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
    RawCapture,
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
            Self::RawCapture => Boundary::RawCapture,
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
pub struct SaveCapture {
    pub cameras:Capture,
    /// UI-owned view, captured without running inputs or providers.
    pub interface_mode:Result<u32,String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub raw_capture:Option<SaveCapture>,
    /// Core boundary completed; Host must apply TaskEffects before surfacing
    /// this real fatal error. No save success was published for this boundary.
    pub finalization_fault:Option<String>,
    pub strategic_writeback:Option<crate::locator::strategic::Writeback>,
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
    subcalls: u64,
}
struct Request {
    generation: Generation,
    sequence: Sequence,
    operation: Operation,
    reply: mpsc::Sender<Reply>,
}
/// Only source-established Session primitives are admitted. These are NOT an
/// implementation of the larger43e5f0/UI/menu/script callback families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseByte { Latch8b22d8, SignedReason3_8b22d3 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseReason { Script0, Action3 }
impl PauseReason {
    pub fn index(self)->usize { match self { Self::Script0=>0, Self::Action3=>3 } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingFamily {
    ProcessControls, ActivityWords, Participant336And6899a0,
    Media, Sequence, Graph, Menu, Pause43e5f0, DebugCommand,
    Participant481a50, Screenshot, Selection, Action, Script,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionCall {
    Resident { source:ResidentSource, read:ResidentRead },
    ProcessByte(super::process_runtime::Byte),
    Count822d38,
    IncrementCount4ba580,
    Scene822d30,
    Quit93442c,
    Media4b9210,
    Sequence4b1980,
    PauseByte(PauseByte),
    ///Read the actual selected record+85c, zero for null; no equality1 gate.
    Busy48dbf0,
    Pause4bac90(PauseReason),
    Unpause4bacc0(PauseReason),
    ///An explicit reached unsupported source join, never a successful stub.
    Missing(MissingFamily),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionReply { Byte(u8), Word(u32), ResidentBinding(openmt2001::resident::AuxiliaryBinding), Completed }
impl SessionCall {
    fn accepts(&self,reply:&SessionReply)->bool {
        matches!((self,reply),
            (Self::Resident{read:ResidentRead::Count7a18ec|ResidentRead::Kind30(_)|ResidentRead::Index38(_),..},SessionReply::Word(_)) |
            (Self::Resident{read:ResidentRead::Flags336(_),..},SessionReply::Byte(_)) |
            (Self::Resident{read:ResidentRead::Lookup6899a0(_),..},SessionReply::ResidentBinding(_)) |
            (Self::Resident{read:ResidentRead::Validate(_)|ResidentRead::Participant481a50(_),..},SessionReply::Completed) |
            (Self::PauseByte(_)|Self::Busy48dbf0|Self::ProcessByte(_),SessionReply::Byte(_)) |
            (Self::Count822d38|Self::Scene822d30,SessionReply::Word(_)) |
            (Self::IncrementCount4ba580|Self::Quit93442c|Self::Media4b9210|Self::Sequence4b1980,SessionReply::Completed) |
            (Self::Pause4bac90(_)|Self::Unpause4bacc0(_),SessionReply::Completed))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CallId { generation:Generation, sequence:Sequence, boundary:Boundary, subcall:u64 }
#[derive(Debug)]
struct SessionRequest {
    id:CallId,
    call:SessionCall,
    reply:mpsc::Sender<SessionResponse>,
}
#[derive(Debug)]
struct SessionResponse { id:CallId, result:Result<SessionReply,RequestError> }
#[derive(Debug)]
enum Reply { Complete(Result<Ack,RequestError>), Session(SessionRequest) }

/// Stack-borrowed, non-Clone port lent ONLY during Inbox's actual operation.
/// call() suspends the UI closure until the waiting Host has finished this
/// callback on its owned Session. No FreshRequest, captured Session, timeout,
/// async effect success, callback replay, or post-ack port can exist here.
/// &mut self serializes subcalls and prevents safe same-port reentrancy.
pub struct SessionServices<'a> {
    id:CallId,
    next:u64,
    sender:&'a mpsc::Sender<Reply>,
    retired:&'a AtomicBool,
    failure:Option<RequestError>,
}
impl SessionServices<'_> {
    /// Check before entering the next UI phase or publishing an operation.
    /// Previously committed Host effects are never revoked by this check.
    pub fn check(&mut self)->Result<(),RequestError> {
        if let Some(error)=&self.failure { return Err(error.clone()); }
        if self.retired.load(Ordering::Acquire) {
            self.failure=Some(RequestError::Retired);return Err(RequestError::Retired);
        }
        Ok(())
    }
    pub fn call(&mut self,call:SessionCall)->Result<SessionReply,RequestError> {
        if let Some(error)=&self.failure { return Err(error.clone()); }
        let result=self.exchange(call);
        if let Err(error)=&result { self.failure=Some(error.clone()); }
        result
    }
    fn disconnected(&self,message:&str)->RequestError {
        if self.retired.load(Ordering::Acquire) { RequestError::Retired }
        else { RequestError::Failed(message.into()) }
    }
    fn exchange(&mut self,call:SessionCall)->Result<SessionReply,RequestError> {
        if self.retired.load(Ordering::Acquire) { return Err(RequestError::Retired); }
        if self.id.boundary!=Boundary::CentralTail {
            return Err("Session callback outside admitted central operation".into());
        }
        let next=self.next.checked_add(1).ok_or("Session subcall sequence exhausted")?;
        let id=CallId{subcall:self.next,..self.id};
        let (reply,response)=mpsc::channel();
        self.sender.send(Reply::Session(SessionRequest{id,call:call.clone(),reply}))
            .map_err(|_|self.disconnected("Host dropped operation reply stream"))?;
        let response=response.recv().map_err(|_|self.disconnected("Host dropped Session callback reply"))?;
        // Validate even when a retirement races receipt. No stale/cross-op
        // result, including an error, is allowed to authorize continuation.
        if response.id!=id { return Err("Session callback reply generation/sequence/boundary/subcall mismatch".into()); }
        let value=match response.result {
            Err(RequestError::Retired) if !self.retired.load(Ordering::Acquire)=>
                return Err("Session reply claimed retirement without explicit generation retirement".into()),
            result=>result?,
        };
        if !call.accepts(&value) { return Err("Session callback reply type mismatch".into()); }
        self.next=next;
        Ok(value)
    }
    pub fn pause_byte(&mut self,byte:PauseByte)->Result<u8,RequestError> {
        match self.call(SessionCall::PauseByte(byte))? { SessionReply::Byte(v)=>Ok(v), _=>unreachable!() }
    }
    pub fn busy48dbf0(&mut self)->Result<u8,RequestError> {
        match self.call(SessionCall::Busy48dbf0)? { SessionReply::Byte(v)=>Ok(v), _=>unreachable!() }
    }
    pub fn pause4bac90(&mut self,reason:PauseReason)->Result<(),RequestError> {
        self.call(SessionCall::Pause4bac90(reason)).map(|_|())
    }
    pub fn unpause4bacc0(&mut self,reason:PauseReason)->Result<(),RequestError> {
        self.call(SessionCall::Unpause4bacc0(reason)).map(|_|())
    }
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
    #[cfg(test)]
    enqueued: Option<(Option<Boundary>,mpsc::Sender<()>)>,
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
            #[cfg(test)]
            enqueued: None,
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
    if observation.raw_capture.is_some() != matches!(boundary,Boundary::RawCapture) {
        return Err("Camera raw capture reply does not match boundary".into());
    }
    if observation.finalization_fault.is_some() &&
        (!matches!(boundary,Boundary::Install|Boundary::CentralTail)||observation.strategic_writeback.is_some()){
        return Err("Camera finalization fault outside protected completion or with unfinalized writeback".into());
    }
    if observation.strategic_writeback.is_some() && !matches!(boundary,Boundary::Install|Boundary::CentralTail){
        return Err("C writeback outside actual protected Install/CentralTail boundary".into());
    }

    if observation.sampled.is_some() != matches!(boundary, Boundary::Sample(_)) {
        return Err("Camera protocol sampled observation does not match boundary".into());
    }
    if boundary != Boundary::CentralTail && !observation.task_effects.is_empty() {
        return Err("Only CentralTail can return live task effects".into());
    }
    Ok(())
}

impl Worker {
    /// Read-only observer of the actual retirement flag, not a mirror state.
    #[cfg(test)]
    pub(crate) fn test_retirement_flag(&self)->Arc<AtomicBool>{Arc::clone(&self.retired)}
    /// One-shot notification after the actual successful request enqueue.
    #[cfg(test)]
    pub(crate) fn test_notify_next_enqueue(&mut self,sender:mpsc::Sender<()>){self.enqueued=Some((None,sender));}
    /// Same actual-enqueue witness, filtered so CurrentPose does not consume a
    /// RawCapture retirement observer. This changes no non-test transport code.
    #[cfg(test)]
    fn test_notify_enqueue_at(&mut self,boundary:Boundary,sender:mpsc::Sender<()>){
        self.enqueued=Some((Some(boundary),sender));
    }
    pub(crate) fn central_identity(&self)->Result<CentralIdentity,RequestError>{
        if let Some(error)=&self.failure{return Err(RequestError::Failed(error.clone()))}
        if self.retired(){return Err(RequestError::Retired)}
        if !self.installed{return Err("Central identity requires completed Install".into())}
        Ok(CentralIdentity{generation:self.generation,sequence:self.next})
    }
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
        self.request_with_handler(operation, |_|Err("Session callback handler absent on camera-only request".into()))
    }
    /// The caller keeps the real Session on THIS Host stack. Each admitted
    /// callback runs once here before its reply permits the UI's next read.
    /// A callback failure is sticky even if UI ignores it and returns an ack.
    pub fn request_with_handler(
        &mut self, operation:Operation,
        mut handle:impl FnMut(SessionCall)->Result<SessionReply,RequestError>,
    )->Result<Observation,RequestError> {
        if let Some(error) = &self.failure {
            return Err(RequestError::Failed(error.clone()));
        }
        if self.retired() {
            return Err(RequestError::Retired);
        }
        let result = self.exchange(operation,&mut handle);
        match &result {
            Err(RequestError::Failed(error))=>self.failure=Some(error.clone()),
            Ok(observation)=>if let Some(error)=&observation.finalization_fault{
                // Return THIS completed ack intact, then reject further work.
                self.failure=Some(error.clone());
            },
            _=>{},
        }
        result
    }
    fn exchange(&mut self, operation: Operation,
        handle:&mut impl FnMut(SessionCall)->Result<SessionReply,RequestError>,
    ) -> Result<Observation, RequestError> {
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
        #[cfg(test)]
        if self.enqueued.as_ref().is_some_and(|(expected,_)|expected.is_none_or(|at|at==boundary)){
            let (_,enqueued)=self.enqueued.take().unwrap();
            enqueued.send(()).expect("request enqueue observer disconnected");
        }
        let mut subcalls=0u64;
        let mut callback_failure:Option<RequestError>=None;
        let ack=loop {
            let frame=response.recv().map_err(|_|callback_failure.clone()
                .unwrap_or_else(||self.disconnected("Camera event thread dropped acknowledgement")))?;
            match frame {
                Reply::Complete(result)=>{
                    if let Some(error)=callback_failure { return Err(error); }
                    let ack=match result {
                        Err(RequestError::Retired) if !self.retired()=>
                            return Err("Camera completion claimed retirement without explicit generation retirement".into()),
                        result=>result?,
                    };
                    if ack.generation!=self.generation || ack.sequence!=self.next || ack.boundary!=boundary
                        || ack.subcalls!=subcalls {
                        return Err("Camera acknowledgement generation/sequence/boundary/subcall mismatch".into());
                    }
                    break ack;
                }
                Reply::Session(request)=>{
                    let expected=CallId{generation:self.generation,sequence:self.next,boundary,subcall:subcalls};
                    // An identity error takes priority over cancellation; forged
                    // messages never invoke even an otherwise admitted handler.
                    let result=if request.id!=expected || boundary!=Boundary::CentralTail {
                        Err(RequestError::Failed("Session callback request generation/sequence/boundary/subcall mismatch".into()))
                    }else if let Some(error)=&callback_failure { Err(error.clone()) }
                    else if self.retired() { Err(RequestError::Retired) }
                    else if let Some(next)=subcalls.checked_add(1) {
                        match handle(request.call.clone()) {
                            Ok(value) if request.call.accepts(&value)=>{subcalls=next;Ok(value)},
                            Ok(_)=>Err("Session handler reply type mismatch".into()),
                            Err(RequestError::Retired) if !self.retired()=>
                                Err("Session handler claimed retirement without explicit generation retirement".into()),
                            Err(error)=>Err(error),
                        }
                    }else { Err("Session subcall sequence exhausted".into()) };
                    if let Err(error)=&result {
                        if callback_failure.is_none() { callback_failure=Some(error.clone()); }
                    }
                    request.reply.send(SessionResponse{id:request.id,result})
                        .map_err(|_|callback_failure.clone()
                            .unwrap_or_else(||self.disconnected("UI dropped Session callback receiver")))?;
                }
            }
        };
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
    /// Never wait for FreshRequest. Use SessionServices on this direct reply
    /// for admitted synchronous Host calls; never queue them to the worker.
    pub fn try_service(
        &mut self,
        apply: impl FnOnce(Operation) -> Result<Observation, String>,
    ) -> Result<Service, String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.service_inner(None, |operation,_services|apply(operation));
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
        let result = self.service_inner(Some(timeout), |operation,_services|apply(operation));
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    /// Same-operation synchronous service port. The UI closure can retain its
    /// ordinary mutable Assets/commonFonts loans; only typed owned arguments
    /// cross this stream, never Assets, Owner, Lua or Session itself.
    pub fn try_service_with_handler(
        &mut self,apply:impl FnOnce(Operation,&mut SessionServices<'_>)->Result<Observation,String>,
    )->Result<Service,String> { self.service_with_handler(None,apply) }
    pub fn wait_service_with_handler(
        &mut self,timeout:std::time::Duration,
        apply:impl FnOnce(Operation,&mut SessionServices<'_>)->Result<Observation,String>,
    )->Result<Service,String> { self.service_with_handler(Some(timeout),apply) }
    fn service_with_handler(
        &mut self,wait:Option<std::time::Duration>,
        apply:impl FnOnce(Operation,&mut SessionServices<'_>)->Result<Observation,String>,
    )->Result<Service,String> {
        if let Some(error)=&self.failure { return Err(error.clone()); }
        let result=self.service_inner(wait,apply);
        if let Err(error)=&result { self.failure=Some(error.clone()); }
        result
    }
    fn service_inner(
        &mut self,
        wait: Option<std::time::Duration>,
        apply: impl FnOnce(Operation,&mut SessionServices<'_>) -> Result<Observation, String>,
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
        let mut service_error=None;
        let result: Result<Ack,String> = (|| {
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
            let mut services=SessionServices{
                id:CallId{generation:self.generation,sequence:request.sequence,boundary,subcall:0},
                next:0,sender:&request.reply,retired:&self.retired,failure:None,
            };
            let applied=apply(request.operation,&mut services);
            // Ignore-and-success cannot turn a failed service into a completed
            // camera operation or publish its TaskEffects. Prior Host writes stay.
            if let Some(error)=services.failure {
                service_error=Some(error.clone());return Err(error.to_string());
            }
            let observation=applied?;
            let subcalls=services.next;
            validate_observation(boundary, &observation)?;
            self.next = following;
            self.installed = true;
            Ok(Ack {
                generation: self.generation,
                sequence: request.sequence,
                boundary,
                observation,
                subcalls,
            })
        })();
        let failure = match &result {
            Err(error)=>Some(error.clone()),
            Ok(ack)=>ack.observation.finalization_fault.clone(),
        };
        request
            .reply
            .send(Reply::Complete(result.map_err(|error|service_error.unwrap_or_else(||error.into()))))
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

#[cfg(test)]
#[path = "camera_bridge/session_service_tests.rs"]
mod session_service_tests;

#[cfg(test)]
#[path = "camera_bridge/autosave_producer_failure_tests.rs"]
mod autosave_producer_failure_tests;
