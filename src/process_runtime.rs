//! Missing native process products. The shared handle shares identity, never a
//! flag snapshot. Every method releases its mutex before returning/callbacks.
use crate::process_image::Cells;
use std::sync::{Arc, Mutex};

/// Camera worker generation and request sequence (re-exported by the game's
/// camera bridge).
pub type CameraGeneration = u64;
pub type CameraSequence = u64;
///Opaque identity of the next CentralTail on this serial Worker. Constructed
///from the same checked generation/sequence that the request exchange uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CentralIdentity {
    generation: CameraGeneration,
    sequence: CameraSequence,
}
impl CentralIdentity {
    /// Only the camera worker's checked exchange and test fixtures build these.
    pub fn new(generation: CameraGeneration, sequence: CameraSequence) -> Self {
        Self {
            generation,
            sequence,
        }
    }
}

/// What `Handle::start_sequence4b1800` needs from the city it starts.
pub trait SequenceSession {
    fn clock_mut(&mut self) -> &mut crate::clock::Clock;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Absent,
    Poisoned,
    UnbornSequence,
    MediaProbeUnavailable,
    SequenceCallbackUnavailable,
    TimingModeCallbackUnavailable,
    SourceLoadOrderUnavailable,
    UnsupportedStartupOption,
    CentralAlreadyActive,
    CentralTokenMismatch,
    CentralTokenExhausted,
    NoActiveCentral,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "process source unavailable: {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Byte {
    Media8fca98,
    Suppressed8b0700,
    QuitOnEscape8b23b9,
    Network93bbb9,
    Debug8b23c5,
    Alternative8fca96,
    Controller920888,
    Test8b23bc,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaSite {
    Startup4b94eb,
    Late4b9beb,
    Periodic4b921d,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaOutcome {
    Valid,
    OriginalBypass,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaProducer {
    Loader,
    Awaiting(MediaSite),
    Validated(MediaSite),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitWriter {
    Central,
    Window,
    Menu,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickingWrite {
    DisableAll,
    EnableAll,
    CameraDisable,
    Block(Option<i32>),
    Building(Option<i32>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickingProjection {
    pub common: u8,
    pub camera: u8,
    pub block: u8,
    pub block_id: i32,
    pub building: u8,
    pub building_id: i32,
}
struct Options {
    quit_on_escape: u8,
    debug: u8,
    test: u8,
    test_b: u8,
}
impl Options {
    fn defaults4b9140() -> Self {
        Self {
            quit_on_escape: 0,
            debug: 0,
            test: 0,
            test_b: 0,
        }
    }
    fn parse4c7500(&mut self, command: &str) -> Result<()> {
        // Admitted parser slice: exact source delimiters and ASCII option
        // names. Native ignores unknown tokens. Recognized unported options
        // are explicit errors; no accidental HOST/JOIN policy activation.
        for token in command
            .split('\0')
            .next()
            .unwrap_or("")
            .split(|c: char| matches!(c, ' ' | ',' | '\t' | '\n' | '-' | '+'))
            .filter(|s| !s.is_empty())
        {
            match token.to_ascii_uppercase().as_str() {
                "DEBUGSTUFF" => self.debug = 1,
                "TESTMODE" => {
                    self.test = 1;
                    self.test_b = 1;
                }
                "NOEXCEPTIONHANDLING"
                | "L"
                | "BUILDINGEDITOR"
                | "HOST"
                | "PORT"
                | "JOIN"
                | "SESSIONNAME"
                | "MAXPLAYERS" => return Err(Error::UnsupportedStartupOption),
                _ => {}
            }
        }
        Ok(())
    }
}
struct Network {
    enabled: u8,
}
impl Network {
    fn constructor4e1290() -> Self {
        Self { enabled: 0 }
    }
    fn teardown4e1c50(&mut self) {
        self.enabled = 0;
    }
}
struct Sequence {
    active: u8,
    second: u8,
    bypass: u8,
    bypass_tail: u8,
}
impl Sequence {
    fn constructor4b1790() -> Self {
        Self {
            active: 0,
            second: 0,
            bypass: 1,
            bypass_tail: 0,
        }
    }
}
struct Media {
    byte: u8,
    producer: MediaProducer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CentralId {
    serial: u64,
    operation: CentralIdentity,
}
///Move-only, private construction. Completion/failure consumes the lease
///created for this exact checked Worker operation, not a later World/Host.
pub struct CentralToken(CentralId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TailObligation {
    Pending(CentralId),
    Completed(CentralId),
    Failed(CentralId),
}
struct Quit {
    requested: u8,
    active: Option<CentralId>,
    next_serial: u64,
    obligation: Option<TailObligation>,
}
/// Original API construction policy; unrelated to modern adapter/FPS/detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalGraphicsPolicy {
    NormalD3d,
    SoftwareKind2Or3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerformanceProducer {
    Constructor4b91c3,
    Software4c684a,
    Lua4d6980,
}
struct Performance {
    value: i32,
    producer: PerformanceProducer,
}
impl Performance {
    fn constructor4b9140() -> Self {
        Self {
            value: 2,
            producer: PerformanceProducer::Constructor4b91c3,
        }
    }
    fn graphics4c6810(&mut self, policy: OriginalGraphicsPolicy) {
        // D3D construction retains the current word, including script writes.
        if policy == OriginalGraphicsPolicy::SoftwareKind2Or3 {
            self.value = 0;
            self.producer = PerformanceProducer::Software4c684a;
        }
    }
}
struct ProcessRuntime {
    start_seed8fca84: Option<u32>,
    last_seed940834: u32,
    performance: Performance,
    options: Options,
    network: Network,
    alternative: u8,
    media: Media,
    sequence: Option<Sequence>,
    picking: PickingProjection,
    count: u32,
    quit: Quit,
    // This is only availability/provenance, NEVER a second scene/clock word.
    // Scene822d30 is the already-existing Session.world.clock.timing[5].
    scene_error: Option<Error>,
}
#[derive(Clone)]
pub struct Handle(Arc<Mutex<ProcessRuntime>>);
/// UI-owned pair. Not Clone, not in Host/Session, not rebuilt on World reset.
/// Assets owns it before any graph, and retains it across park/attach/refresh.
pub struct Activity {
    a4: u32,
    a8: u32,
    frontend: Option<FrontendStartup>,
}
/// Plain verified receipt; no Rc/audio/device object enters the shared mutex.
pub struct FrontendStartup {
    pub input: crate::native_input::RetainedCells,
    pub pause: crate::pause::State,
    pub suspended: u8,
    pub disable_sound: u8,
    pub disable_music: u8,
    pub sound: u8,
}
impl Activity {
    pub fn take_frontend_startup(&mut self) -> std::result::Result<FrontendStartup, String> {
        self.frontend
            .take()
            .ok_or_else(|| "Process frontend receipt already consumed".into())
    }
    pub fn a4(&self) -> u32 {
        self.a4
    }
    pub fn a8(&self) -> u32 {
        self.a8
    }
    pub fn store_a4(&mut self, value: u32) {
        self.a4 = value;
    }
    pub fn store_a8(&mut self, value: u32) {
        self.a8 = value;
    }
    ///4c2bdc's actual operand. The source controller uses the very same a4.
    pub fn presentation4c2bdc(&self) -> u32 {
        self.a4
    }
}
/// Construct once at actual Assets startup, from supplied PE loader products,
/// source defaults/options and native offline/profile/sequence constructors.
/// No code from the PE is loaded or executed. No caller-filled flag builder.
pub fn startup(pe: &[u8], command: &str) -> std::result::Result<(Handle, Activity), String> {
    let cells = Cells::process_allocation(pe)?;
    startup_from_loader(&cells, command)
}
fn startup_from_loader(
    cells: &Cells,
    command: &str,
) -> std::result::Result<(Handle, Activity), String> {
    // Each loader preimage is verified, including opaque96 and both activity
    // words. This reads actual supplied virtual-tail zero-fill, not constants
    // returned by getters. Constructor/default stores supersede loader bytes.
    for address in [
        0x8b23b9, 0x8b23c5, 0x8b23bc, 0x8fca96, 0x8fca98, 0x93bbb9, 0x920888, 0x920889, 0x92088a,
        0x920890, 0x93442c,
    ] {
        if cells.byte(address)? != 0 {
            return Err(format!("unexpected process loader preimage {address:08x}"));
        }
    }
    for address in [0x8b26cc, 0x822d38, 0x8fcaa4, 0x8fcaa8] {
        if cells.word(address)? != 0 {
            return Err(format!("unexpected process loader word {address:08x}"));
        }
    }
    let frontend = FrontendStartup::ordinary(cells)?;
    let mut options = Options::defaults4b9140();
    options.parse4c7500(command).map_err(|e| e.to_string())?;
    let mut runtime = ProcessRuntime {
        start_seed8fca84: None,
        last_seed940834: 0,
        performance: Performance::constructor4b9140(),
        options,
        network: Network::constructor4e1290(),
        alternative: cells.byte(0x8fca96)?,
        media: Media {
            byte: cells.byte(0x8fca98)?,
            producer: MediaProducer::Loader,
        },
        sequence: Some(Sequence::constructor4b1790()),
        //4c2b30 first store,4c2b60 constructor and4c2b80 reset: common1,
        //camera1, block0/id-1, building0/id-1. No tutorial-derived negation.
        picking: PickingProjection {
            common: 1,
            camera: 1,
            block: 0,
            block_id: -1,
            building: 0,
            building_id: -1,
        },
        count: cells.word(0x822d38)?,
        quit: Quit {
            requested: cells.byte(0x93442c)?,
            active: None,
            next_serial: 0,
            obligation: None,
        },
        scene_error: None,
    };
    //4b9480 samples the native network constructor product once. Teardown
    //later cannot undo the city-entry b9 writer.
    // Current frontend explicitly adopts the existing NormalD3d original
    // compatibility policy. No WGPU/backend inference or new user setting.
    runtime
        .performance
        .graphics4c6810(OriginalGraphicsPolicy::NormalD3d);
    runtime.city_main_entry();
    //Actual startup clear is committed. The missing probe is represented as
    //awaiting, NOT admitted as known valid0 or silently ignored success.
    runtime.media.byte = 0;
    runtime.media.producer = MediaProducer::Awaiting(MediaSite::Startup4b94eb);
    Ok((
        Handle(Arc::new(Mutex::new(runtime))),
        Activity {
            a4: cells.word(0x8fcaa4)?,
            a8: cells.word(0x8fcaa8)?,
            frontend: Some(frontend),
        },
    ))
}
impl ProcessRuntime {
    fn city_main_entry(&mut self) {
        if self.network.enabled != 0 {
            self.options.quit_on_escape = 1;
        }
    }
}
impl Handle {
    fn with<T>(&self, f: impl FnOnce(&mut ProcessRuntime) -> Result<T>) -> Result<T> {
        let mut state = self.0.lock().map_err(|_| Error::Poisoned)?;
        f(&mut state)
    }
    pub fn performance8b26cc(&self) -> Result<i32> {
        self.with(|r| Ok(r.performance.value))
    }
    pub fn performance_producer(&self) -> Result<PerformanceProducer> {
        self.with(|r| Ok(r.performance.producer))
    }
    pub fn performance_nonzero(&self) -> Result<bool> {
        self.performance8b26cc().map(|v| v != 0)
    }
    pub fn set_performance8b26cc(&self, value: i32) -> Result<()> {
        self.with(|r| {
            r.performance.value = value;
            r.performance.producer = PerformanceProducer::Lua4d6980;
            Ok(())
        })
    }
    /// Explicit construction operation only; never called by reads/rebinding.
    /// Software/Test clears0; later D3D retains that0 (or a later script value).
    pub fn graphics_construction4c6810(&self, policy: OriginalGraphicsPolicy) -> Result<()> {
        self.with(|r| {
            r.performance.graphics4c6810(policy);
            Ok(())
        })
    }
    pub fn byte(&self, cell: Byte) -> Result<u8> {
        self.with(|r| {
            Ok(match cell {
                Byte::Media8fca98 => {
                    if matches!(r.media.producer, MediaProducer::Awaiting(_)) {
                        return Err(Error::MediaProbeUnavailable);
                    }
                    r.media.byte
                }
                Byte::Suppressed8b0700 => r.sequence.as_ref().ok_or(Error::UnbornSequence)?.active,
                Byte::QuitOnEscape8b23b9 => r.options.quit_on_escape,
                Byte::Network93bbb9 => r.network.enabled,
                Byte::Debug8b23c5 => r.options.debug,
                Byte::Test8b23bc => r.options.test,
                Byte::Alternative8fca96 => r.alternative,
                Byte::Controller920888 => r.picking.common,
            })
        })
    }
    pub fn increment_count4ba580(&self) -> Result<()> {
        self.with(|r| {
            r.count = r.count.wrapping_add(1);
            Ok(())
        })
    }
    pub fn count822d38(&self) -> Result<u32> {
        self.with(|r| Ok(r.count))
    }
    ///Called after the actual camera reset final fields, before imported camera
    ///restore. Refresh/Host::new/Admission never invoke this writer.
    pub fn world_reset_post_camera4b9eb1(&self) -> Result<()> {
        self.with(|r| {
            r.count = 0;
            Ok(())
        })
    }
    pub fn world_controller_reset4c2b80(&self) -> Result<()> {
        self.with(|r| {
            r.picking = PickingProjection {
                common: 1,
                camera: 1,
                block: 0,
                block_id: -1,
                building: 0,
                building_id: -1,
            };
            Ok(())
        })
    }
    pub fn picking_write(&self, write: PickingWrite) -> Result<()> {
        self.with(|r| {
            match write {
                PickingWrite::DisableAll => r.picking.common = 0,
                PickingWrite::EnableAll => r.picking.common = 1,
                PickingWrite::CameraDisable => r.picking.camera = 0,
                PickingWrite::Block(id) => {
                    r.picking.common = 1;
                    r.picking.block = 1;
                    r.picking.block_id = id.unwrap_or(-1);
                    r.picking.building = 0;
                }
                PickingWrite::Building(id) => {
                    r.picking.common = 1;
                    r.picking.building = 1;
                    r.picking.building_id = id.unwrap_or(-1);
                    r.picking.block = 0;
                }
            }
            Ok(())
        })
    }
    pub fn picking_projection(&self) -> Result<PickingProjection> {
        self.with(|r| Ok(r.picking))
    }
    pub fn network_teardown4e1c50(&self) -> Result<()> {
        self.with(|r| {
            r.network.teardown4e1c50();
            Ok(())
        })
    }
    pub fn scene_available(&self) -> Result<()> {
        self.with(|r| r.scene_error.map_or(Ok(()), Err))
    }
    pub fn source_load_unjoined(&self) -> Result<()> {
        self.with(|r| {
            r.scene_error = Some(Error::SourceLoadOrderUnavailable);
            Ok(())
        })
    }
    /// Authority selection/received start seed (0x4a229e / 0x450c22).
    pub fn select_connected_seed(&self, seed: u32) -> Result<()> {
        self.with(|r| {
            r.start_seed8fca84 = Some(seed);
            r.network.enabled = 1;
            Ok(())
        })
    }
    #[cfg(test)]
    pub fn last_city_seed(&self) -> Result<u32> {
        self.with(|r| Ok(r.last_seed940834))
    }
    /// 0x4b9fa0 reads the process seed before 0x4e5580 traverses the registry.
    /// The returned states are transferred to live owners by the constructor.
    pub fn reseed_connected_city(&self) -> Result<crate::startup_rng::Snapshot> {
        self.with(|r| {
            let seed = r
                .start_seed8fca84
                .take()
                .ok_or(Error::SourceLoadOrderUnavailable)?;
            let startup = crate::startup_rng::Snapshot::from_seed(seed);
            r.last_seed940834 = startup.last_seed;
            Ok(startup)
        })
    }
    pub fn source_fresh_world(&self) -> Result<()> {
        self.with(|r| {
            r.scene_error = None;
            Ok(())
        })
    }
    pub fn source_timing_mode_unjoined(&self) -> Result<()> {
        self.with(|r| {
            r.scene_error = Some(Error::TimingModeCallbackUnavailable);
            Ok(())
        })
    }
    pub fn source_timing_completed(&self) -> Result<()> {
        self.with(|r| {
            r.scene_error = None;
            Ok(())
        })
    }
    pub fn media_producer(&self) -> Result<MediaProducer> {
        self.with(|r| Ok(r.media.producer))
    }
    ///No lock crosses a media probe (or its external error/cleanup callback).
    pub fn validate_media(
        &self,
        site: MediaSite,
        probe: impl FnOnce() -> Result<MediaOutcome>,
    ) -> Result<()> {
        self.with(|r| {
            r.media.byte = 0;
            r.media.producer = MediaProducer::Awaiting(site);
            Ok(())
        })?;
        let outcome = probe()?;
        self.with(|r| {
            r.media.byte = u8::from(outcome == MediaOutcome::OriginalBypass);
            r.media.producer = MediaProducer::Validated(site);
            Ok(())
        })
    }
    pub fn reached_periodic_media(&self) -> Result<()> {
        self.validate_media(MediaSite::Periodic4b921d, || {
            Err(Error::MediaProbeUnavailable)
        })
    }
    ///The source start callback must own the actual scene/Lua initialization.
    ///This method cannot turn modern result booleans into a source sequence.
    pub fn start_sequence4b1800<S: SequenceSession>(
        &self,
        session: &mut S,
        initialize: impl FnOnce(&mut S) -> Result<()>,
    ) -> Result<()> {
        self.with(|r| {
            r.sequence.as_mut().ok_or(Error::UnbornSequence)?.active = 0;
            Ok(())
        })?;
        session.clock_mut().timing[5] = 3;
        initialize(session)?;
        self.with(|r| {
            r.sequence.as_mut().ok_or(Error::UnbornSequence)?.active = 1;
            Ok(())
        })
    }
    pub fn end_sequence4b1eb0(&self, cleanup: impl FnOnce() -> Result<()>) -> Result<()> {
        cleanup()?;
        self.with(|r| {
            r.sequence.as_mut().ok_or(Error::UnbornSequence)?.active = 0;
            Ok(())
        })
    }
    pub fn reached_sequence4b1980(&self) -> Result<()> {
        self.with(|r| {
            let _ = r.sequence.as_ref().ok_or(Error::UnbornSequence)?;
            Err(Error::SequenceCallbackUnavailable)
        })
    }
    pub fn begin_central(&self, operation: CentralIdentity) -> Result<CentralToken> {
        self.with(|r| {
            if r.quit.active.is_some() {
                return Err(Error::CentralAlreadyActive);
            }
            let next = r
                .quit
                .next_serial
                .checked_add(1)
                .ok_or(Error::CentralTokenExhausted)?;
            let id = CentralId {
                serial: r.quit.next_serial,
                operation,
            };
            r.quit.next_serial = next;
            r.quit.active = Some(id);
            Ok(CentralToken(id))
        })
    }
    pub fn request_quit(&self, writer: QuitWriter) -> Result<()> {
        self.with(|r| {
            if writer == QuitWriter::Central && r.quit.active.is_none() {
                return Err(Error::NoActiveCentral);
            }
            r.quit.requested = 1;
            if r.quit.obligation.is_none() {
                if let Some(id) = r.quit.active {
                    r.quit.obligation = Some(TailObligation::Pending(id));
                }
            }
            Ok(())
        })
    }
    pub fn complete_central_tail(&self, token: CentralToken) -> Result<()> {
        self.with(|r| {
            if r.quit.active != Some(token.0) {
                return Err(Error::CentralTokenMismatch);
            }
            r.quit.active = None;
            if r.quit.obligation == Some(TailObligation::Pending(token.0)) {
                r.quit.obligation = Some(TailObligation::Completed(token.0));
            }
            Ok(())
        })
    }
    pub fn fail_central(&self, token: CentralToken) -> Result<()> {
        self.with(|r| {
            if r.quit.active != Some(token.0) {
                return Err(Error::CentralTokenMismatch);
            }
            r.quit.active = None;
            if r.quit.obligation == Some(TailObligation::Pending(token.0)) {
                r.quit.obligation = Some(TailObligation::Failed(token.0));
            }
            Ok(())
        })
    }
    pub fn quit_requested(&self) -> Result<u8> {
        self.with(|r| Ok(r.quit.requested))
    }
    pub fn exit_ready(&self) -> Result<bool> {
        self.with(|r| {
            Ok(r.quit.requested != 0
                && r.quit.active.is_none()
                && matches!(r.quit.obligation, None | Some(TailObligation::Completed(_))))
        })
    }
    #[doc(hidden)]
    pub fn mutex_available(&self) -> bool {
        self.0.try_lock().is_ok()
    }
}
/// Test support: a runtime born from a synthetic loader image.
#[doc(hidden)]
pub fn test_fixture() -> Handle {
    startup(&synthetic_loader(), "").unwrap().0
}
/// Test support: the smallest i386 PE whose process cells pass `startup`.
#[doc(hidden)]
pub fn synthetic_loader() -> Vec<u8> {
    let mut pe = vec![0; 0x200];
    pe[..2].copy_from_slice(b"MZ");
    let put =
        |pe: &mut [u8], at: usize, value: u32| pe[at..at + 4].copy_from_slice(&value.to_le_bytes());
    put(&mut pe, 0x3c, 0x80);
    pe[0x80..0x84].copy_from_slice(b"PE\0\0");
    pe[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    pe[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    pe[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
    pe[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut pe, 0xb4, 0x400000);
    put(&mut pe, 0x180, 0x150000);
    put(&mut pe, 0x184, 0x400000);
    pe
}
#[cfg(test)]
mod tests;

impl FrontendStartup {
    fn ordinary(cells: &Cells) -> std::result::Result<Self, String> {
        use crate::native_input::{LogicalPointer, MousePacket, RetainedCells};
        for (address, length) in [
            (0x8af23a, 2),
            (0x8b23ba, 2),
            (0x8b22d0, 9),
            (0x8af401, 1),
            (0x8b2374, 1),
            (0x8b22dd, 1),
            (0x93c5c0, 594),
        ] {
            if cells.read(address, length)?.iter().any(|&b| b != 0) {
                return Err(format!(
                    "Unexpected ordinary frontend loader preimage {address:08x}"
                ));
            }
        }
        //4ad020 writes3a only;4b9140 writes sound/music1;4bab70 enabled1/transition0.
        //Heap CHAR slots/event count/recording use explicit modern zero priors.
        Ok(Self {
            input: RetainedCells {
                mouse_current: MousePacket([0; 16]),
                mouse_prior: MousePacket([0; 16]),
                keyboard_current: [0; 256],
                keyboard_prior: [0; 256],
                logical_pointer: LogicalPointer { x: 0, y: 0 },
                left_override: 0,
                right_override: 0,
                double_click: 0,
                keyboard_poll_tick: 0,
                device_event_count: 0,
                input_enabled: 1,
                focus_transition: 0,
                recording: 0,
                space_recorded: 0,
            },
            pause: crate::pause::State {
                bytes: cells.read(0x8b22d0, 9)?.try_into().unwrap(),
            },
            suspended: 0,
            disable_sound: 0,
            disable_music: cells.byte(0x8af23b)?,
            sound: 1,
        })
    }
}

/// Test support: a runtime and its single-use frontend receipt.
#[doc(hidden)]
pub fn test_frontend_startup() -> (Handle, FrontendStartup) {
    let (process, mut activity) = startup(&synthetic_loader(), "").unwrap();
    (process, activity.take_frontend_startup().unwrap())
}
