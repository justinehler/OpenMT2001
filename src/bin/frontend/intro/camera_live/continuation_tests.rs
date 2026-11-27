//! Authored synthetic order/mutation/error mechanics, using
//! real public StagedInput polling/CHAR and real Ring. No device/native runtime,
//! actual callback semantic, whole-game or four-timer acceptance is claimed.
use super::*;
fn run(c: &mut Synthetic) -> std::result::Result<BeforeTail, Interrupted> {
    super::run(c, super::super::group_dispatch::PrefixComplete::synthetic())
}
use openmt2001::native_input::{
    CharacterQueue, KeyEvent, LogicalPointer, MousePacket, RetainedCells, WindowInputEffect,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};
#[derive(Clone, Debug, PartialEq, Eq)]
enum Log {
    Byte(Byte),
    Word(Word),
    Write(Word, u32),
    Busy,
    Resolve(u32, u32),
    Device(u32, u32),
    Mouse,
    Keyboard,
    Buffered,
    RecordAfter(u32, Poll),
    Secondary,
    Chat,
    Active(Group),
    Mode(i32),
    Current,
    Kind,
    Player,
    Flags(i32),
    Lookup(i32),
    Publish,
    Effect(Effect),
    Quit,
}
#[derive(Clone)]
enum Change {
    Byte(Byte, u8),
    Word(Word, u32),
    Busy(u8),
    Mode(i32),
    Secondary(u8),
    Activity([u8; 2]),
    Fail,
    DoubleClick,
    Character(u32),
}
struct Controls {
    bytes: BTreeMap<Byte, u8>,
    words: BTreeMap<Word, u32>,
    receiver: u32,
    vtable: u32,
    buffering: u32,
    replay: u8,
    recording: u8,
    busy: u8,
    secondary: u8,
    mode: i32,
}
struct Device {
    control: Rc<RefCell<Controls>>,
    log: Rc<RefCell<Vec<Log>>>,
    keys: [u8; 256],
    mouse_hresult: i32,
    key_hresult: i32,
    key_error: bool,
    replace_on_mouse: bool,
    suppress_on_mouse: bool,
    replay_after_mouse: u8,
    recording_after_mouse: u8,
    vtable_after_mouse: Option<u32>,
    recording_after_keyboard: u8,
}
impl PortableDeviceAdapter for Device {
    fn mouse_state(&mut self, d: &mut [u8; 16]) -> std::result::Result<i32, BoundaryError> {
        self.log.borrow_mut().push(Log::Mouse);
        d[0] = 0x55;
        d[12] = 0xee;
        d[13] = 0xdd;
        let mut c = self.control.borrow_mut();
        if self.replace_on_mouse {
            c.receiver = 0x2222;
            c.buffering = 1;
            c.busy = 2;
        }
        if self.suppress_on_mouse {
            c.bytes.insert(Byte::Suppressed8b0700, 1);
        }
        c.replay = self.replay_after_mouse;
        c.recording = self.recording_after_mouse;
        if let Some(v) = self.vtable_after_mouse {
            c.vtable = v;
        }
        Ok(self.mouse_hresult)
    }
    fn keyboard_state(&mut self, d: &mut [u8; 256]) -> std::result::Result<i32, BoundaryError> {
        self.log.borrow_mut().push(Log::Keyboard);
        if self.key_error {
            d[2] = 0x77;
            return Err(BoundaryError::AdapterUnavailable(
                "synthetic keyboard absent",
            ));
        }
        *d = self.keys;
        self.control.borrow_mut().recording = self.recording_after_keyboard;
        Ok(self.key_hresult)
    }
    fn keyboard_events(&mut self, n: u32) -> std::result::Result<BufferedKeyboard, BoundaryError> {
        assert_eq!(n, 16);
        self.log.borrow_mut().push(Log::Buffered);
        let records = self
            .keys
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .map(|(i, v)| KeyEvent {
                native_index: i as u32,
                data: u32::from(*v),
            })
            .collect::<Vec<_>>();
        BufferedKeyboard::admit(self.key_hresult, records.len() as u32, &records)
    }
}
struct Synthetic {
    input: StagedInput,
    control: Rc<RefCell<Controls>>,
    device: Device,
    log: Rc<RefCell<Vec<Log>>>,
    ring: Option<Ring>,
    active: [u8; 2],
    modes: VecDeque<i32>,
    current: Option<Selected>,
    player: u32,
    flags: Vec<u8>,
    transient: bool,
    auxiliary: openmt2001::resident::AuxiliaryPool,
    registry: super::super::resident_descriptors::Registry,
    script: VecDeque<(Effect, Vec<Change>)>,
}
fn selected(raw: u32, kind: i16) -> Selected {
    Selected {
        key: SelectionKey::NativeObject(raw),
        kind,
        category: 0,
        hit: None,
    }
}
impl Synthetic {
    fn new() -> Self {
        let input = StagedInput::from_process_cells(
            RetainedCells {
                mouse_current: MousePacket([0; 16]),
                mouse_prior: MousePacket([0; 16]),
                keyboard_current: [0; 256],
                keyboard_prior: [0; 256],
                logical_pointer: LogicalPointer { x: 7, y: 9 },
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
            CharacterQueue::initialize([0x91; 16]),
        );
        let mut bytes = BTreeMap::new();
        for b in [
            Byte::Media8fca98,
            Byte::Suppressed8b0700,
            Byte::PauseLatch8b22d8,
            Byte::QuitOnEscape8b23b9,
            Byte::Network93bbb9,
            Byte::Debug8b23c5,
            Byte::Alternative8fca96,
            Byte::Guard57e794Plus345,
            Byte::Controller920888,
            Byte::Test8b23bc,
            Byte::SignedReason3_8b22d3,
        ] {
            bytes.insert(b, 0);
        }
        let mut words = BTreeMap::new();
        for w in [
            Word::Count822d38,
            Word::Scene822d30,
            Word::Activity8fcaa4,
            Word::Activity8fcaa8,
            Word::ParticipantCount7a18ec,
            Word::TargetZ93af9c,
            Word::TargetY93af98,
            Word::TargetX93af94,
        ] {
            words.insert(w, 0);
        }
        words.insert(Word::Scene822d30, 1);
        words.insert(Word::ParticipantCount7a18ec, 3);
        let control = Rc::new(RefCell::new(Controls {
            bytes,
            words,
            receiver: 0x1111,
            vtable: 0x52ae80,
            buffering: 0,
            replay: 0,
            recording: 0,
            busy: 0,
            secondary: 0,
            mode: 0,
        }));
        let log = Rc::new(RefCell::new(Vec::new()));
        let device = Device {
            control: control.clone(),
            log: log.clone(),
            keys: [0; 256],
            mouse_hresult: 0,
            key_hresult: 0,
            key_error: false,
            replace_on_mouse: false,
            suppress_on_mouse: false,
            replay_after_mouse: 0,
            recording_after_mouse: 0,
            vtable_after_mouse: None,
            recording_after_keyboard: 0,
        };
        let auxiliary =
            openmt2001::resident::AuxiliaryPool::from_slots(vec![[0; 284]; 3], 3).unwrap();
        let mut registry = super::super::resident_descriptors::Registry::new().unwrap();
        registry
            .install(
                &crate::live_host::camera_bridge::ResidentSource::synthetic(880, &auxiliary),
                880,
            )
            .unwrap();
        Self {
            auxiliary,
            registry,
            input,
            control,
            device,
            log,
            ring: Some(Ring::construct452040([[0x66; 264]; 11])),
            active: [0, 0],
            modes: VecDeque::new(),
            current: None,
            player: 0,
            flags: vec![0; 3],
            transient: false,
            script: VecDeque::new(),
        }
    }
    // Actual issued identities even in the synthetic continuation-order harness.
    fn resident(&mut self, index: usize, kind: i16) -> ParticipantDescriptor {
        self.registry
            .selected(self.auxiliary.bind(index).unwrap(), kind, index as u32)
            .unwrap()
    }
    fn byte_set(&mut self, b: Byte, v: u8) {
        self.control.borrow_mut().bytes.insert(b, v);
    }
    fn word_set(&mut self, w: Word, v: u32) {
        self.control.borrow_mut().words.insert(w, v);
    }
    fn key(&mut self, i: usize) {
        self.device.keys[i] = 0x80;
    }
    fn script(&mut self, e: Effect, v: Vec<Change>) {
        self.script.push_back((e, v));
    }
    fn effects(&self) -> Vec<Effect> {
        self.log
            .borrow()
            .iter()
            .filter_map(|l| {
                if let Log::Effect(e) = l {
                    Some(*e)
                } else {
                    None
                }
            })
            .collect()
    }
    fn logs(&self) -> Vec<Log> {
        self.log.borrow().clone()
    }
    fn count(&self, l: Log) -> usize {
        self.logs().iter().filter(|v| **v == l).count()
    }
}
impl Source for Synthetic {
    fn input(&self) -> Result<&StagedInput> {
        Ok(&self.input)
    }
    fn input_mut(&mut self) -> Result<&mut StagedInput> {
        Ok(&mut self.input)
    }
    fn byte(&mut self, b: Byte) -> Result<u8> {
        self.log.borrow_mut().push(Log::Byte(b));
        self.control
            .borrow()
            .bytes
            .get(&b)
            .copied()
            .ok_or("unseeded synthetic byte".into())
    }
    fn word(&mut self, w: Word) -> Result<u32> {
        self.log.borrow_mut().push(Log::Word(w));
        self.control
            .borrow()
            .words
            .get(&w)
            .copied()
            .ok_or("unseeded synthetic word".into())
    }
    fn write_word(&mut self, w: Word, v: u32) -> Result<()> {
        self.log.borrow_mut().push(Log::Write(w, v));
        self.word_set(w, v);
        Ok(())
    }
    fn write_quit93442c(&mut self) -> Result<()> {
        self.log.borrow_mut().push(Log::Quit);
        Ok(())
    }
    fn busy48dbf0(&mut self) -> Result<u8> {
        self.log.borrow_mut().push(Log::Busy);
        Ok(self.control.borrow().busy)
    }
    fn resolve_receiver8b2278(&mut self) -> Result<Receiver> {
        let c = self.control.borrow();
        self.log
            .borrow_mut()
            .push(Log::Resolve(c.receiver, c.vtable));
        Receiver::from_source(c.receiver, c.vtable)
    }
    fn live_device_input(
        &mut self,
        r: Receiver,
        poll: Poll,
    ) -> Result<(&mut StagedInput, LiveDevice<'_>)> {
        let c = self.control.borrow();
        let mode = match poll {
            Poll::Mouse => DeviceMode::Mouse,
            Poll::Keyboard => DeviceMode::KeyboardBuffering(c.buffering),
        };
        self.log.borrow_mut().push(Log::Device(
            r.address(),
            match mode {
                DeviceMode::Mouse => u32::MAX,
                DeviceMode::KeyboardBuffering(w) => w,
            },
        ));
        let (replay, recording) = (c.replay, c.recording);
        drop(c);
        Ok((
            &mut self.input,
            LiveDevice {
                receiver: r,
                replay,
                recording,
                mode,
                adapter: &mut self.device,
            },
        ))
    }
    fn recording_after_device(&mut self, r: Receiver, poll: Poll) -> Result<u8> {
        self.log
            .borrow_mut()
            .push(Log::RecordAfter(r.address(), poll));
        Ok(self.control.borrow().recording)
    }
    fn secondary_enabled(&mut self) -> Result<u8> {
        self.log.borrow_mut().push(Log::Secondary);
        Ok(self.control.borrow().secondary)
    }
    fn chat_ring68b4c0(&mut self) -> Result<&mut Ring> {
        self.log.borrow_mut().push(Log::Chat);
        self.ring.as_mut().ok_or("actual ring not supplied".into())
    }
    fn active43c5c0(&mut self, g: Group) -> Result<u8> {
        self.log.borrow_mut().push(Log::Active(g));
        match g {
            Group::Primary => Ok(self.active[0]),
            Group::Secondary => Ok(self.active[1]),
            _ => Err("unexpected tertiary".into()),
        }
    }
    fn mode4b52f0(&mut self) -> Result<i32> {
        let v = self.modes.pop_front().unwrap_or(self.control.borrow().mode);
        self.log.borrow_mut().push(Log::Mode(v));
        Ok(v)
    }
    fn current822d50(&mut self) -> Result<Option<SelectionKey>> {
        self.log.borrow_mut().push(Log::Current);
        Ok(self.current.map(|v| v.key))
    }
    fn selected_kind30(&mut self, k: SelectionKey) -> Result<i16> {
        self.log.borrow_mut().push(Log::Kind);
        let v = self.current.ok_or("synthetic current absent")?;
        assert_eq!(v.key, k);
        Ok(v.kind)
    }
    fn participant_player38(&mut self, _k: SelectionKey) -> Result<u32> {
        self.log.borrow_mut().push(Log::Player);
        Ok(self.player)
    }
    fn participant_flags336(&mut self, i: i32) -> Result<u8> {
        self.log.borrow_mut().push(Log::Flags(i));
        self.flags
            .get(i as usize)
            .copied()
            .ok_or("synthetic resident prefix absent".into())
    }
    fn participant6899a0(&mut self, i: i32) -> Result<ParticipantDescriptor> {
        self.log.borrow_mut().push(Log::Lookup(i));
        self.registry.selected(
            self.auxiliary
                .bind(usize::try_from(i).map_err(|_| "negative resident lookup")?)?,
            1,
            i as u32,
        )
    }
    fn publish_current822d50(&mut self, v: ParticipantDescriptor) -> Result<()> {
        self.log.borrow_mut().push(Log::Publish);
        self.current = Some(v.selected());
        Ok(())
    }
    fn transient823168_present(&mut self) -> Result<bool> {
        Ok(self.transient)
    }
    fn effect(&mut self, e: Effect) -> Result<()> {
        self.log.borrow_mut().push(Log::Effect(e));
        if self.script.front().is_some_and(|(at, _)| *at == e) {
            for a in self.script.pop_front().unwrap().1 {
                match a {
                    Change::Byte(b, v) => self.byte_set(b, v),
                    Change::Word(w, v) => self.word_set(w, v),
                    Change::Busy(v) => self.control.borrow_mut().busy = v,
                    Change::Mode(v) => self.control.borrow_mut().mode = v,
                    Change::Secondary(v) => self.control.borrow_mut().secondary = v,
                    Change::Activity(v) => self.active = v,
                    Change::Fail => return Err("synthetic missing actual callback".into()),
                    Change::DoubleClick => self
                        .input
                        .window_input_effect(WindowInputEffect::LeftDoubleClick),
                    Change::Character(v) => self
                        .input
                        .window_input_effect(WindowInputEffect::Character { wparam: v }),
                }
            }
        }
        Ok(())
    }
}
#[test]
fn ordinary_order_and_completion_stops_before_tail() {
    let mut c = Synthetic::new();
    let done = run(&mut c).unwrap();
    assert_eq!(done.phase(), Phase::ContinuationComplete);
    assert!(done.requires_final_join());
    assert_eq!(done.exit(), Exit::Normal);
    let logs = c.logs();
    let pos = |l| logs.iter().position(|x| *x == l).unwrap();
    assert!(pos(Log::Mouse) < pos(Log::Keyboard));
    assert!(pos(Log::Keyboard) < pos(Log::Chat));
    assert!(pos(Log::Chat) < pos(Log::Active(Group::Primary)));
    assert!(pos(Log::Write(Word::Activity8fcaa4, 0)) < pos(Log::Write(Word::Activity8fcaa8, 0)));
    assert_eq!(c.input.cells().keyboard_poll_tick, 1);
    assert_eq!(c.count(Log::Busy), 1);
}
#[test]
fn media_counts_and_probe_mutation_do_not_reenter_input() {
    for (count, probe) in [(124, false), (125, true), (0, true), (u32::MAX, false)] {
        let mut c = Synthetic::new();
        c.byte_set(Byte::Media8fca98, 1);
        c.word_set(Word::Count822d38, count);
        c.script(
            Effect::Media4b9210,
            vec![Change::Byte(Byte::Media8fca98, 0)],
        );
        assert_eq!(run(&mut c).unwrap().exit(), Exit::MediaBranch);
        assert_eq!(c.effects().contains(&Effect::Media4b9210), probe);
        assert_eq!(c.count(Log::Busy), 0);
    }
}
#[test]
fn input_disabled_skips_receivers_and_controls() {
    let mut c = Synthetic::new();
    c.input.focus_loss_cells_after_device_calls();
    assert_eq!(run(&mut c).unwrap().exit(), Exit::InputDisabled);
    assert_eq!(c.count(Log::Busy), 0);
}
#[test]
fn receiver_vtable_reloaded_and_poll1_mode_is_fresh() {
    let mut c = Synthetic::new();
    c.device.replace_on_mouse = true;
    c.key(0x0f);
    run(&mut c).unwrap();
    assert!(c.logs().contains(&Log::Resolve(0x2222, 0x52ae80)));
    assert!(c.logs().contains(&Log::Device(0x2222, 1)));
    assert_eq!(c.count(Log::Buffered), 1);
    assert_eq!(c.count(Log::Keyboard), 0);
    assert_eq!(c.count(Log::Busy), 1);
    assert_eq!(c.count(Log::RecordAfter(0x1111, Poll::Mouse)), 1);
    assert_eq!(c.count(Log::RecordAfter(0x2222, Poll::Keyboard)), 1);
    //poll0 changed busy to2; captured0 still admits chat and Tab.
    assert!(c.effects().contains(&Effect::Graph4baf10));
}
#[test]
fn null_and_unknown_receiver_do_not_fake_poll_success() {
    for (r, v) in [(0, 0x52ae80), (0x1111, 0x1234)] {
        let mut c = Synthetic::new();
        c.control.borrow_mut().receiver = r;
        c.control.borrow_mut().vtable = v;
        let e = run(&mut c).unwrap_err();
        assert_eq!(e.phase, Phase::PrefixComplete);
        assert_eq!(c.count(Log::Mouse), 0);
    }
}
#[test]
fn recording_replay_reject_without_device_substitution() {
    for replay in [false, true] {
        let mut c = Synthetic::new();
        if replay {
            c.control.borrow_mut().replay = 1;
        } else {
            c.control.borrow_mut().recording = 1;
        }
        let e = run(&mut c).unwrap_err();
        assert!(
            e.reason
                .contains(if replay { "replay" } else { "recording" })
        );
        assert_eq!(c.count(Log::Mouse), 0);
        assert_eq!(c.input.cells().keyboard_poll_tick, 0);
    }
}
#[test]
fn negative_hresult_preserves_device_partial_bytes_and_actual_overrides() {
    let mut c = Synthetic::new();
    c.device.mouse_hresult = -1;
    c.device.key_hresult = -2;
    c.key(3);
    c.input
        .window_input_effect(WindowInputEffect::ButtonMessage { wparam: 1 });
    run(&mut c).unwrap();
    assert_eq!(c.input.cells().mouse_current.0[0], 0x55);
    assert_eq!(c.input.cells().mouse_current.left(), 1);
    assert_eq!(c.input.cells().mouse_current.right(), 0);
    assert_eq!(c.input.cells().keyboard_current[3], 0x80);
    assert_eq!(c.input.cells().keyboard_poll_tick, 1);
}
#[test]
fn portable_poll1_error_retains_poll0_prior_and_partial_keyboard() {
    let mut c = Synthetic::new();
    c.device.key_error = true;
    let e = run(&mut c).unwrap_err();
    assert!(e.at.contains("poll1"));
    assert!(e.reason.contains("not HRESULT"));
    assert_eq!(c.input.cells().mouse_current.0[0], 0x55);
    assert_eq!(c.input.cells().keyboard_current[2], 0x77);
    assert_eq!(c.input.cells().keyboard_poll_tick, 0);
    assert_eq!(c.input.cells().device_event_count, 16);
    assert_eq!(c.count(Log::Chat), 0);
}
#[test]
fn postpoll_suppression_has_only_required_sequence_callback() {
    let mut c = Synthetic::new();
    c.device.suppress_on_mouse = true;
    c.key(1);
    c.key(0x3b);
    assert_eq!(run(&mut c).unwrap().exit(), Exit::Suppressed);
    assert_eq!(c.effects(), vec![Effect::Sequence4b1980]);
    assert_eq!(c.count(Log::Chat), 0);
}
#[test]
fn busy_al_nonzero_and_secondary_gate_chat_but_not_activity() {
    for busy in [1, 2] {
        let mut c = Synthetic::new();
        c.control.borrow_mut().busy = busy;
        c.key(0x0f);
        run(&mut c).unwrap();
        assert_eq!(c.count(Log::Chat), 0);
        assert!(c.effects().is_empty());
        assert_eq!(c.count(Log::Active(Group::Primary)), 1);
    }
    let mut c = Synthetic::new();
    c.control.borrow_mut().secondary = 1;
    c.key(0x0f);
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Chat), 0);
    assert!(!c.effects().contains(&Effect::Graph4baf10));
}
#[test]
fn missing_actual_ring_is_partial_error_after_polls() {
    let mut c = Synthetic::new();
    c.ring = None;
    let e = run(&mut c).unwrap_err();
    assert!(e.at.contains("chat"));
    assert_eq!(c.input.cells().keyboard_poll_tick, 1);
    assert_eq!(c.count(Log::Active(Group::Primary)), 0);
}
#[test]
fn graph_callback_changes_later_actual_activity() {
    let mut c = Synthetic::new();
    c.key(0x0f);
    c.script(
        Effect::Graph4baf10,
        vec![
            Change::Secondary(1),
            Change::Mode(4),
            Change::Activity([1, 1]),
        ],
    );
    assert_eq!(run(&mut c).unwrap().exit(), Exit::ActivityLocked);
    assert_eq!(c.effects(), vec![Effect::Graph4baf10]);
    assert_eq!(c.count(Log::Active(Group::Secondary)), 0);
    assert_eq!(c.control.borrow().secondary, 1);
}
#[test]
fn primary_activity_short_circuits_secondary_and_screenshot_changes_fresh_lock() {
    let mut c = Synthetic::new();
    c.active = [2, 1];
    c.key(0x38);
    c.key(0x3f);
    c.key(0x1f);
    c.byte_set(Byte::Test8b23bc, 1);
    c.script(
        Effect::Capture458af0Zero,
        vec![Change::Word(Word::Activity8fcaa4, 0)],
    );
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Active(Group::Secondary)), 0);
    assert_eq!(c.control.borrow().words[&Word::Activity8fcaa8], 1);
    assert_eq!(
        c.effects(),
        vec![Effect::Capture458af0Zero, Effect::Script4bc9d0Map]
    );
}
#[test]
fn screenshot_can_lock_world_before_selection() {
    let mut c = Synthetic::new();
    c.key(0x38);
    c.key(0x3f);
    c.key(0x1f);
    c.byte_set(Byte::Test8b23bc, 1);
    c.script(
        Effect::Capture458af0Zero,
        vec![Change::Word(Word::Activity8fcaa4, 0x100)],
    );
    assert_eq!(run(&mut c).unwrap().exit(), Exit::ActivityLocked);
    assert_eq!(c.effects(), vec![Effect::Capture458af0Zero]);
}
#[test]
fn escape_quit_keeps_processing_pause_and_latch_blocks_menu() {
    let mut c = Synthetic::new();
    c.key(1);
    c.key(0x3b);
    c.byte_set(Byte::QuitOnEscape8b23b9, 1);
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Quit), 1);
    assert_eq!(c.effects(), vec![Effect::Pause43e5f0]);
    let mut c = Synthetic::new();
    c.key(1);
    c.byte_set(Byte::PauseLatch8b22d8, 1);
    run(&mut c).unwrap();
    assert!(!c.effects().contains(&Effect::Menu491720));
}
#[test]
fn f1_bypasses_guard_p_requires_guard_and_menu_changes_later_network() {
    let mut c = Synthetic::new();
    c.key(0x3b);
    c.byte_set(Byte::Guard57e794Plus345, 1);
    run(&mut c).unwrap();
    assert_eq!(c.effects(), vec![Effect::Pause43e5f0]);
    assert_eq!(c.count(Log::Byte(Byte::Guard57e794Plus345)), 0);
    let mut c = Synthetic::new();
    c.key(0x19);
    c.byte_set(Byte::Guard57e794Plus345, 1);
    run(&mut c).unwrap();
    assert!(c.effects().is_empty());
    let mut c = Synthetic::new();
    c.key(1);
    c.key(0x3b);
    c.script(
        Effect::Menu491720,
        vec![Change::Byte(Byte::Network93bbb9, 1)],
    );
    run(&mut c).unwrap();
    assert_eq!(c.effects(), vec![Effect::Menu491720]);
}
#[test]
fn normal_debug_held_and_alternative_with_distinct_network_guard() {
    for alternative in [false, true] {
        let mut c = Synthetic::new();
        c.byte_set(Byte::Debug8b23c5, 1);
        if alternative {
            c.byte_set(Byte::Alternative8fca96, 1);
        } else {
            c.key(0x10);
            c.key(0x2a);
            let mut cells = c.input.cells().clone();
            cells.keyboard_current[0x10] = 0x80;
            cells.keyboard_current[0x2a] = 0x80;
            c.input =
                StagedInput::from_process_cells(cells, CharacterQueue::initialize([0x91; 16]));
        }
        run(&mut c).unwrap();
        assert_eq!(c.effects(), vec![Effect::DebugCommand4ba9f0_450490]);
    }
    let mut c = Synthetic::new();
    c.byte_set(Byte::Debug8b23c5, 1);
    c.byte_set(Byte::Alternative8fca96, 1);
    c.byte_set(Byte::Network93bbb9, 1);
    run(&mut c).unwrap();
    assert!(c.effects().is_empty());
}
#[test]
fn participant_second_mode_query_is_live_and_zero_count_is_error() {
    let mut c = Synthetic::new();
    c.key(0x22);
    c.modes = VecDeque::from([0, 1]);
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Current), 0);
    let mut c = Synthetic::new();
    c.key(0x22);
    c.word_set(Word::ParticipantCount7a18ec, 0);
    assert!(run(&mut c).unwrap_err().reason.contains("count<=0"));
}
#[test]
fn participant_wrap_eligibility_publish_then_callbacks_and_partial_error() {
    let mut c = Synthetic::new();
    c.key(0x22);
    c.current = Some(c.resident(2, 2).selected());
    c.player = 2;
    c.flags = vec![1, 0, 1];
    let next = c.resident(1, 1);
    c.script(Effect::Participant481a50(next), vec![Change::Busy(2)]);
    c.script(Effect::FreshA41f290Mode5, vec![Change::Fail]);
    let e = run(&mut c).unwrap_err();
    assert!(e.at.contains("participant"));
    assert_eq!(c.current, Some(next.selected()));
    assert_eq!(
        c.effects(),
        vec![
            Effect::Participant481a50(next),
            Effect::SetKind4e06c0(9),
            Effect::FreshA41f290Mode5
        ]
    );
    let logs = c.logs();
    assert!(
        logs.iter().position(|x| *x == Log::Publish).unwrap()
            < logs
                .iter()
                .position(|x| *x == Log::Effect(Effect::Participant481a50(next)))
                .unwrap()
    );
}
#[test]
fn participant_all_ineligible_reads_final_flag_without_lookup() {
    let mut c = Synthetic::new();
    c.key(0x22);
    c.flags = vec![1; 3];
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Flags(0)), 2);
    assert_eq!(c.count(Log::Lookup(0)), 0);
    assert!(c.current.is_none());
}
#[test]
fn target_words_read_zyx_and_set_kind_after_target() {
    let mut c = Synthetic::new();
    c.key(0x38);
    c.key(0x21);
    c.word_set(Word::TargetX93af94, 0x11);
    c.word_set(Word::TargetY93af98, 0x22);
    c.word_set(Word::TargetZ93af9c, 0x33);
    run(&mut c).unwrap();
    assert_eq!(
        c.effects(),
        vec![
            Effect::Target4dfca0([0x11, 0x22, 0x33]),
            Effect::SetKind4e06c0(0)
        ]
    );
    let log = c.logs();
    let i = log
        .iter()
        .position(|x| *x == Log::Word(Word::TargetZ93af9c))
        .unwrap();
    assert_eq!(
        &log[i..i + 3],
        &[
            Log::Word(Word::TargetZ93af9c),
            Log::Word(Word::TargetY93af98),
            Log::Word(Word::TargetX93af94)
        ]
    );
}
#[test]
fn full_right_override_and_prior_mouse_gate_selection() {
    for (right, prior, expected) in [(0, 0, true), (0x100, 0, false), (0, 1, false)] {
        let mut c = Synthetic::new();
        let mut cells = c.input.cells().clone();
        cells.right_override = right;
        cells.left_override = 1;
        cells.mouse_current.0[12] = prior;
        c.input = StagedInput::from_process_cells(cells, CharacterQueue::initialize([0x91; 16]));
        c.transient = true;
        c.byte_set(Byte::Controller920888, 1);
        run(&mut c).unwrap();
        assert_eq!(c.effects().contains(&Effect::Selection4c3a20Zero), expected);
    }
}
#[test]
fn doubleclick_persists_and_signed_reason_only_blocks_action() {
    for reason in [0, 1, 0xff, 0x80] {
        let mut c = Synthetic::new();
        c.input
            .window_input_effect(WindowInputEffect::LeftDoubleClick);
        c.byte_set(Byte::SignedReason3_8b22d3, reason);
        run(&mut c).unwrap();
        assert_eq!(
            c.effects().contains(&Effect::Action4c3b80),
            (reason as i8) <= 0
        );
        assert_eq!(c.input.cells().double_click, 1);
    }
}
#[test]
fn script_callback_mutates_live_later_gate_and_keeps_char_owner() {
    let mut c = Synthetic::new();
    c.key(0x1f);
    c.key(0x26);
    c.byte_set(Byte::Test8b23bc, 1);
    c.input
        .window_input_effect(WindowInputEffect::Character { wparam: 65 });
    c.script(
        Effect::Script4bc9d0Map,
        vec![
            Change::Byte(Byte::Test8b23bc, 0),
            Change::Character(66),
            Change::DoubleClick,
        ],
    );
    run(&mut c).unwrap();
    assert_eq!(c.effects(), vec![Effect::Script4bc9d0Map]);
    assert_eq!(c.input.characters().controls().count, 2);
    assert_eq!(c.input.take_character(), 66);
    assert_eq!(c.input.take_character(), 66);
    assert_eq!(c.input.cells().double_click, 1);
}
#[test]
fn missing_callback_keeps_prior_writes_and_no_successful_receipt() {
    let mut c = Synthetic::new();
    c.key(1);
    c.script(
        Effect::Menu491720,
        vec![Change::Word(Word::Activity8fcaa4, 7), Change::Fail],
    );
    let e = run(&mut c).unwrap_err();
    assert_eq!(e.phase, Phase::PrefixComplete);
    assert_eq!(c.control.borrow().words[&Word::Activity8fcaa4], 7);
    assert_eq!(c.input.cells().keyboard_poll_tick, 1);
    assert_eq!(c.effects(), vec![Effect::Menu491720]);
}
#[test]
fn poll0_enables_recording_at_late_gate_or_replay_at_next_entry() {
    for replay in [false, true] {
        let mut c = Synthetic::new();
        if replay {
            c.device.replay_after_mouse = 1;
        } else {
            c.device.recording_after_mouse = 1;
        }
        let e = run(&mut c).unwrap_err();
        assert!(e.at.contains(if replay { "poll1" } else { "poll0" }));
        assert_eq!(c.count(Log::Mouse), 1);
        assert_eq!(c.count(Log::Keyboard), 0);
        assert_eq!(c.input.cells().mouse_current.0[0], 0x55);
        assert_eq!(c.input.cells().keyboard_poll_tick, 0);
    }
}
#[test]
fn poll0_vtable_change_is_observed_before_poll1() {
    let mut c = Synthetic::new();
    c.device.vtable_after_mouse = Some(0x12345678);
    let e = run(&mut c).unwrap_err();
    assert!(e.at.contains("poll1"));
    assert!(e.reason.contains("vtable12345678"));
    assert_eq!(c.count(Log::Mouse), 1);
    assert_eq!(c.count(Log::Keyboard), 0);
}
#[test]
fn held_pause_key_is_not_a_rising_scan_and_tab_alt_does_not_toggle() {
    let mut c = Synthetic::new();
    c.key(0x3b);
    let mut cells = c.input.cells().clone();
    cells.keyboard_current[0x3b] = 0x80;
    c.input = StagedInput::from_process_cells(cells, CharacterQueue::initialize([0x19; 16]));
    run(&mut c).unwrap();
    assert!(c.effects().is_empty());
    let mut c = Synthetic::new();
    c.key(0x0f);
    c.key(0x38);
    run(&mut c).unwrap();
    assert!(!c.effects().contains(&Effect::Graph4baf10));
    assert_eq!(c.count(Log::Chat), 1);
}
#[test]
fn debug_callback_mutates_busy_but_captured_busy_still_admits_participant() {
    let mut c = Synthetic::new();
    c.key(0x22);
    c.byte_set(Byte::Debug8b23c5, 1);
    c.byte_set(Byte::Alternative8fca96, 1);
    c.script(Effect::DebugCommand4ba9f0_450490, vec![Change::Busy(2)]);
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Busy), 1);
    assert!(c.logs().contains(&Log::Lookup(0)));
}
#[test]
fn participant_domain_rejects_native_current_and_index_outside_actual_count() {
    let mut c = Synthetic::new();
    c.key(0x22);
    c.current = Some(selected(0, 1));
    assert!(run(&mut c).unwrap_err().reason.contains("actual6899a0"));
    let mut c = Synthetic::new();
    c.key(0x22);
    c.current = Some(c.resident(0, 1).selected());
    c.player = 3;
    assert!(
        run(&mut c)
            .unwrap_err()
            .reason
            .contains("descriptor+38 outside")
    );
    assert_eq!(c.count(Log::Flags(0)), 0);
    assert!(SelectionKey::NativeObject(0).require_resident().is_err());
}
#[test]
fn owner_target_words_preserve_raw_nonfinite_words_without_eye_or_sample_alias() {
    use openmt2001::camera::bank::{Bank, Capture};
    let mut raw = [0x23; 196];
    let values = [0x7fc12345u32, 0xff800000, 0x80000000];
    for (i, v) in values.iter().enumerate() {
        raw[0x34 + i * 4..0x38 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    let owner = super::super::owner::Owner::new(
        Bank::from_capture(Capture::from_records([raw, [0x91; 196]])),
        7,
    );
    for (i, v) in values.iter().enumerate() {
        assert_eq!(owner.continuation_target_word(i).unwrap(), *v);
    }
    assert!(owner.continuation_target_word(3).is_err());
    assert_eq!(owner.capture().records()[0], raw);
    assert_eq!(owner.kind_dirty(), 7);
}

#[test]
fn keyboard_callback_enables_recording_and_stops_before_poll_tick() {
    let mut c = Synthetic::new();
    c.key(7);
    c.device.recording_after_keyboard = 1;
    let e = run(&mut c).unwrap_err();
    assert!(e.at.contains("poll1"));
    assert!(e.reason.contains("before completion/tick"));
    assert_eq!(c.input.cells().keyboard_current[7], 0x80);
    assert_eq!(c.input.cells().keyboard_poll_tick, 0);
    assert_eq!(c.count(Log::Chat), 0);
}
#[test]
fn scene_not_one_skips_chat_and_tab_but_keeps_activity() {
    let mut c = Synthetic::new();
    c.word_set(Word::Scene822d30, 2);
    c.key(0x0f);
    run(&mut c).unwrap();
    assert_eq!(c.count(Log::Chat), 0);
    assert!(!c.effects().contains(&Effect::Graph4baf10));
    assert_eq!(c.count(Log::Active(Group::Primary)), 1);
}
