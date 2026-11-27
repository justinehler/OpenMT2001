//! Inert4ba5eb..BEFORE4ba9d5. No focus, camera tick, scheduler, save or ack.
//! All getters are at the actual source point; callbacks are synchronous and
//! fallible. Error means prior effects RETAINED, never restart/replay/rollback.
use super::{
    chat_ring::Ring,
    navigation::Group,
    ui_selection::{Selected, SelectionKey},
};
use openmt2001::native_input::{
    BoundaryError, BufferedKeyboard, KeyboardMode, PollReport, PortableDeviceAdapter, StagedInput,
};
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Byte {
    Media8fca98,
    Suppressed8b0700,
    PauseLatch8b22d8,
    QuitOnEscape8b23b9,
    Network93bbb9,
    Debug8b23c5,
    Alternative8fca96,
    Guard57e794Plus345,
    Controller920888,
    Test8b23bc,
    SignedReason3_8b22d3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Word {
    Count822d38,
    Scene822d30,
    Activity8fcaa4,
    Activity8fcaa8,
    ParticipantCount7a18ec,
    TargetZ93af9c,
    TargetY93af98,
    TargetX93af94,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    MediaBranch,
    InputDisabled,
    Suppressed,
    ActivityLocked,
    Normal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    PrefixComplete,
    ContinuationComplete,
}
///Not Clone/Copy and cannot construct a tail/camera/save acknowledgement.
///Only run() can create a production receipt. The final join is REQUIRED.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Only continuation completed; join SAME focus then camera tail before central/save acknowledgment"]
pub struct BeforeTail {
    exit: Exit,
}
impl BeforeTail {
    pub fn phase(&self) -> Phase {
        Phase::ContinuationComplete
    }
    pub fn exit(&self) -> Exit {
        self.exit
    }
    pub fn requires_final_join(&self) -> bool {
        true
    }
    #[cfg(test)]
    pub(super) fn synthetic() -> Self {
        Self { exit: Exit::Normal }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Interrupted {
    pub phase: Phase,
    pub at: &'static str,
    pub reason: String,
}
impl std::fmt::Display for Interrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "continuation interrupted at {}; prefix completed, prior effects retained; NO tail/central/save completion: {}",
            self.at, self.reason
        )
    }
}
///Actual receiver identity AND vtable, reread before each call. Unknown
///vtable/null is a reached unsupported native dereference/provider boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receiver {
    address: u32,
    vtable: u32,
}
impl Receiver {
    pub fn from_source(address: u32, vtable: u32) -> Result<Self> {
        if address == 0 {
            return Err("central8b2278 null receiver unsupported".into());
        }
        if vtable != 0x52ae80 {
            return Err(format!("central input vtable{vtable:08x} unsupported"));
        }
        Ok(Self { address, vtable })
    }
    pub fn address(self) -> u32 {
        self.address
    }
    pub fn vtable(self) -> u32 {
        self.vtable
    }
}
///No new device/clock/input authority: borrowed actual resolved receiver's
///provider. Mode/flags are read at each poll entry, AFTER earlier callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    Mouse,
    Keyboard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceMode {
    Mouse,
    KeyboardBuffering(u32),
}
pub struct LiveDevice<'a> {
    pub receiver: Receiver,
    pub replay: u8,
    pub recording: u8,
    pub mode: DeviceMode,
    pub adapter: &'a mut dyn PortableDeviceAdapter,
}
///6899a0 participant lookup product, never688740/property-pool conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticipantDescriptor(Selected);
impl ParticipantDescriptor {
    pub(super) fn from_registered(
        value: Selected,
        registry: &super::resident_descriptors::Registry,
    ) -> Result<Self> {
        registry.validate_selected(value)?;
        Ok(Self(value))
    }
    pub fn selected(self) -> Selected {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Media4b9210,
    Sequence4b1980,
    Graph4baf10,
    Menu491720,
    Pause43e5f0,
    ///Provider must really construct4ba9f0(1), install5265dc/+14=2 and
    ///submit450490(...,0); unknown command bytes keep their provenance.
    DebugCommand4ba9f0_450490,
    Participant481a50(ParticipantDescriptor),
    SetKind4e06c0(u32),
    ///Must reload actual A57e760 HERE, after setkind9, then41f290(5).
    FreshA41f290Mode5,
    Capture458af0Zero,
    Target4dfca0([u32; 3]),
    Selection4c3a20Zero,
    Action4c3b80,
    Script4bc9d0Map,
    Script4d62b0MapOne,
}
///No successful missing-provider defaults or source snapshots. The SAME public
///StagedInput must back input() and live_device_input(), groups, CHAR and the
///later camera adapter. Input packets are never rebuilt or cleared here.
pub trait Source {
    fn input(&self) -> Result<&StagedInput>;
    fn input_mut(&mut self) -> Result<&mut StagedInput>;
    fn byte(&mut self, cell: Byte) -> Result<u8>;
    fn word(&mut self, cell: Word) -> Result<u32>;
    fn write_word(&mut self, cell: Word, value: u32) -> Result<()>;
    fn write_quit93442c(&mut self) -> Result<()>;
    fn busy48dbf0(&mut self) -> Result<u8>;
    fn resolve_receiver8b2278(&mut self) -> Result<Receiver>;
    fn live_device_input(
        &mut self,
        receiver: Receiver,
        poll: Poll,
    ) -> Result<(&mut StagedInput, LiveDevice<'_>)>;
    ///Read captured call receiver+19218 AFTER device work, before poll1 tick.
    ///This is not a fresh central receiver resolution or cached entry flag.
    fn recording_after_device(&mut self, receiver: Receiver, poll: Poll) -> Result<u8>;
    fn secondary_enabled(&mut self) -> Result<u8>;
    fn chat_ring68b4c0(&mut self) -> Result<&mut Ring>;
    fn active43c5c0(&mut self, group: Group) -> Result<u8>;
    fn mode4b52f0(&mut self) -> Result<i32>;
    fn current822d50(&mut self) -> Result<Option<SelectionKey>>;
    fn selected_kind30(&mut self, key: SelectionKey) -> Result<i16>;
    fn participant_player38(&mut self, key: SelectionKey) -> Result<u32>;
    fn participant_flags336(&mut self, index: i32) -> Result<u8>;
    fn participant6899a0(&mut self, index: i32) -> Result<ParticipantDescriptor>;
    fn publish_current822d50(&mut self, value: ParticipantDescriptor) -> Result<()>;
    fn transient823168_present(&mut self) -> Result<bool>;
    fn effect(&mut self, effect: Effect) -> Result<()>;
}
struct Adapter<'a>(&'a mut dyn PortableDeviceAdapter);
impl PortableDeviceAdapter for Adapter<'_> {
    fn mouse_state(&mut self, d: &mut [u8; 16]) -> std::result::Result<i32, BoundaryError> {
        self.0.mouse_state(d)
    }
    fn keyboard_state(&mut self, d: &mut [u8; 256]) -> std::result::Result<i32, BoundaryError> {
        self.0.keyboard_state(d)
    }
    fn keyboard_events(&mut self, n: u32) -> std::result::Result<BufferedKeyboard, BoundaryError> {
        self.0.keyboard_events(n)
    }
}
fn poll<C: Source>(c: &mut C, index: u8) -> Result<PollReport> {
    let receiver = c.resolve_receiver8b2278()?; //Reload receiver AND vtable0/1.
    let call = if index == 0 {
        Poll::Mouse
    } else {
        Poll::Keyboard
    };
    let (report, pending) = {
        let (input, device) = c.live_device_input(receiver, call)?;
        if device.receiver != receiver {
            return Err("resolved device does not match current receiver/vtable".into());
        }
        if device.replay != 0 {
            return Err(format!(
                "poll{index}: actual replay requires stream/cursor/tag provider; live substitution refused"
            ));
        }
        if device.recording != 0 || input.cells().recording != 0 {
            return Err(format!(
                "poll{index}: actual recording requires serialization provider; live substitution refused"
            ));
        }
        let mut adapter = Adapter(device.adapter);
        match (call, device.mode) {
            (Poll::Mouse, DeviceMode::Mouse) => {
                let report = input.poll0_live(&mut adapter).map_err(|error| {
                    format!("poll{index} portable provider incomplete (not HRESULT): {error:?}")
                })?;
                (report, None)
            }
            (Poll::Keyboard, DeviceMode::KeyboardBuffering(word)) => {
                let mode = KeyboardMode::from_receiver_buffering_word(word);
                let pending = input
                    .poll1_live_before_recording(mode, &mut adapter)
                    .map_err(|error| {
                        format!("poll{index} portable provider incomplete (not HRESULT): {error:?}")
                    })?;
                (pending.report(), Some(pending))
            }
            _ => {
                return Err(
                    "actual poll/provider mode mismatch; no substituted device call".into(),
                );
            }
        }
    };
    //Native records after the actual device work. If a callback changed this
    //gate, stop at its reached missing provider; retain bytes and DON'T tick.
    if c.recording_after_device(receiver, call)? != 0 || c.input()?.cells().recording != 0 {
        return Err(format!(
            "poll{index}: recording became active after device work; serialization provider required before completion/tick"
        ));
    }
    if let Some(pending) = pending {
        Ok(c.input_mut()?.finish_poll1_after_recording(pending))
    } else {
        Ok(report)
    }
}
fn held<C: Source>(c: &C, i: usize) -> Result<bool> {
    Ok(c.input()?.cells().keyboard_current[i] != 0)
}
fn edge<C: Source>(c: &C, i: usize) -> Result<bool> {
    Ok(held(c, i)? && c.input()?.cells().keyboard_prior[i] == 0)
}
fn cycle<C: Source>(c: &mut C) -> Result<()> {
    let selected = c.current822d50()?; //PE pointer first, then count, then kind.
    let count = c.word(Word::ParticipantCount7a18ec)? as i32;
    if count <= 0 {
        return Err(
            "participant count<=0: native division/resident dereference outside portable domain"
                .into(),
        );
    }
    let (mut index, mut visited) = (0i32, 0i32);
    if let Some(key) = selected {
        let kind = c.selected_kind30(key)?;
        if kind == 1 || kind == 2 {
            key.require_resident()?;
            let player = c.participant_player38(key)?;
            //PE INC+CDQ+IDIV, not unsigned modulo. Bound actual resident domain
            //before signed overflow/negative remainder/native invalid indexing.
            if player >= count as u32 {
                return Err(
                    "current descriptor+38 outside signed-positive resident count domain".into(),
                );
            }
            visited = 1;
            index = ((player + 1) as i32) % count;
        }
    }
    //Native loads the flags BEFORE its visited<count guard, incl final index.
    while c.participant_flags336(index)? & 1 != 0 && visited < count {
        visited += 1;
        index = (index + 1) % count;
    }
    if visited != count {
        let next = c.participant6899a0(index)?;
        c.publish_current822d50(next)?;
        c.effect(Effect::Participant481a50(next))?;
        c.effect(Effect::SetKind4e06c0(9))?;
        c.effect(Effect::FreshA41f290Mode5)?;
    }
    Ok(())
}
///Call ONLY after the actual group prefix completed on this operation. This
///returns before43e3b0, never a transport Observation or successful central.
pub fn run<C: Source>(
    c: &mut C,
    _prefix: super::group_dispatch::PrefixComplete,
) -> std::result::Result<BeforeTail, Interrupted> {
    let mut at = "4ba5eb media";
    let result = (|| -> Result<Exit> {
        if c.byte(Byte::Media8fca98)? != 0 {
            at = "4ba9be periodic count/probe";
            if c.word(Word::Count822d38)? % 125 == 0 {
                c.effect(Effect::Media4b9210)?;
            }
            return Ok(Exit::MediaBranch);
        }
        at = "4ba5f8 enabled";
        if c.input()?.cells().input_enabled == 0 {
            return Ok(Exit::InputDisabled);
        }
        at = "4ba605 busy";
        let busy = c.busy48dbf0()?; //Capture AL ONCE.
        at = "4ba60f poll0";
        poll(c, 0)?;
        at = "4ba61e poll1";
        poll(c, 1)?;
        at = "4ba62b suppression";
        if c.byte(Byte::Suppressed8b0700)? != 0 {
            c.effect(Effect::Sequence4b1980)?;
            return Ok(Exit::Suppressed);
        }
        at = "4ba643 chat/Tab";
        if c.secondary_enabled()? == 0 && busy == 0 && c.word(Word::Scene822d30)? == 1 {
            c.chat_ring68b4c0()?.tick452110();
            if edge(c, 0x0f)? && c.mode4b52f0()? != 1 && !held(c, 0x38)? {
                c.effect(Effect::Graph4baf10)?;
            }
        }
        at = "4ba68d activity";
        let active = if c.active43c5c0(Group::Primary)? != 0 {
            1
        } else {
            u32::from(c.active43c5c0(Group::Secondary)? != 0)
        };
        c.write_word(Word::Activity8fcaa4, active)?;
        c.write_word(Word::Activity8fcaa8, active)?;
        at = "4ba6bc Escape";
        if edge(c, 1)? && c.byte(Byte::PauseLatch8b22d8)? == 0 {
            if c.byte(Byte::QuitOnEscape8b23b9)? != 0 {
                c.write_quit93442c()?;
            } else if c.mode4b52f0()? != 4 {
                c.effect(Effect::Menu491720)?;
            }
        }
        at = "4ba6f8 pause";
        if (edge(c, 0x3b)? || (edge(c, 0x19)? && c.byte(Byte::Guard57e794Plus345)? == 0))
            && c.byte(Byte::Network93bbb9)? == 0
            && c.mode4b52f0()? != 4
        {
            c.effect(Effect::Pause43e5f0)?;
        }
        at = "4ba748 debug";
        if c.byte(Byte::Debug8b23c5)? != 0
            && ((held(c, 0x10)? && held(c, 0x2a)?) || c.byte(Byte::Alternative8fca96)? != 0)
            && c.byte(Byte::Network93bbb9)? == 0
        {
            c.effect(Effect::DebugCommand4ba9f0_450490)?;
        }
        at = "4ba79b participant";
        if edge(c, 0x22)?
            && c.byte(Byte::Guard57e794Plus345)? == 0
            && c.mode4b52f0()? == 0
            && busy == 0
            && c.mode4b52f0()? != 1
        {
            cycle(c)?;
        }
        at = "4ba88d screenshot";
        if held(c, 0x38)? && edge(c, 0x3f)? {
            c.effect(Effect::Capture458af0Zero)?;
        }
        at = "4ba8b2 fresh activity";
        if c.word(Word::Activity8fcaa4)? != 0 {
            return Ok(Exit::ActivityLocked);
        }
        at = "4ba8bf Alt-F";
        if held(c, 0x38)?
            && edge(c, 0x21)?
            && c.byte(Byte::Guard57e794Plus345)? == 0
            && busy == 0
            && c.mode4b52f0()? == 0
        {
            let z = c.word(Word::TargetZ93af9c)?;
            let y = c.word(Word::TargetY93af98)?;
            let x = c.word(Word::TargetX93af94)?;
            c.effect(Effect::Target4dfca0([x, y, z]))?;
            c.effect(Effect::SetKind4e06c0(0))?;
        }
        at = "4ba91a selection";
        if c.input()?.cells().mouse_current.left() != 0
            && c.input()?.cells().mouse_prior.left() == 0
            && c.input()?.cells().right_override == 0
            && c.transient823168_present()?
            && c.byte(Byte::Controller920888)? != 0
        {
            c.effect(Effect::Selection4c3a20Zero)?;
        }
        at = "4ba951 doubleclick";
        if c.input()?.cells().double_click != 0 && (c.byte(Byte::SignedReason3_8b22d3)? as i8) <= 0
        {
            c.effect(Effect::Action4c3b80)?;
        }
        at = "4ba968 script4bc9d0";
        if edge(c, 0x1f)? && c.byte(Byte::Test8b23bc)? != 0 {
            c.effect(Effect::Script4bc9d0Map)?;
        }
        at = "4ba990 script4d62b0";
        if edge(c, 0x26)? && c.byte(Byte::Test8b23bc)? != 0 {
            c.effect(Effect::Script4d62b0MapOne)?;
        }
        Ok(Exit::Normal)
    })();
    result
        .map(|exit| BeforeTail { exit })
        .map_err(|reason| Interrupted {
            phase: Phase::PrefixComplete,
            at,
            reason,
        })
}
#[cfg(test)]
#[path = "continuation_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn test_resident_cycle(source: &mut impl Source) -> Result<()> {
    cycle(source)
}
