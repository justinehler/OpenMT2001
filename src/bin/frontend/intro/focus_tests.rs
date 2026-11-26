//! Literal native expectations and deterministic real broker/worker interleaves.
use super::*;
use openmt2001::{native_input::focus::AudioOutcome, pause::State};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call { Activate(u64), Retire(u64), Effect(u64, bool, i8), Begin(u64), End(u64) }
#[derive(Default)]
struct Process { active: Option<AttachmentId>, calls: Vec<Call>, focused: Option<bool> }
struct EffectsFixture { process: Arc<Mutex<Process>>, result: Result<Prefix, String> }
impl EffectsFixture {
    fn complete(process: Arc<Mutex<Process>>) -> Self {
        Self { process, result: Ok(Prefix::Complete(Some(AudioOutcome::Applied))) }
    }
}
impl Effects for EffectsFixture {
    fn activate(&mut self, id: AttachmentId) -> Result<(), String> {
        let mut p = self.process.lock().unwrap();
        assert!(p.active.is_none()); p.active = Some(id); p.calls.push(Call::Activate(id.value())); Ok(())
    }
    fn retire(&mut self, id: AttachmentId) -> Result<(), String> {
        let mut p = self.process.lock().unwrap();
        assert_eq!(p.active, Some(id)); p.active = None; p.calls.push(Call::Retire(id.value())); Ok(())
    }
    fn before_pause(&mut self, id: AttachmentId, focused: bool, reason3: i8) -> Result<Prefix, String> {
        let mut p = self.process.lock().unwrap();
        assert_eq!(p.active, Some(id));
        p.calls.push(Call::Effect(id.value(), focused, reason3));
        if self.result.is_ok() { p.focused = Some(focused); }
        self.result.clone()
    }
}
fn route(process: &Arc<Mutex<Process>>) -> Route {
    let mut route = Route::default();
    route.install(Box::new(EffectsFixture::complete(Arc::clone(process)))).unwrap();
    route
}
fn next(rx: &mpsc::Receiver<FreshRequest>) -> Event {
    let FreshRequest::Focus(event) = rx.recv_timeout(Duration::from_secs(5)).unwrap()
        else { panic!("expected focus FIFO event") };
    event
}
struct Applied { result: Result<(), String>, pause: State, tasks: Vec<openmt2001::timers::Task>, running: bool }
fn start(event: Event, mut pause: State, mut tasks: Vec<openmt2001::timers::Task>, network: bool)
    -> std::thread::JoinHandle<Applied> {
    std::thread::spawn(move || {
        let mut host = crate::live_host::Host::new([0.; 3]);
        host.set_running(true); host.policy.scheduler.network_enabled = network;
        let result = event.apply(&mut host, &mut pause, &mut tasks);
        Applied { result, pause, tasks, running: host.running }
    })
}
fn apply(route: &mut Route, event: Event, pause: State, tasks: Vec<openmt2001::timers::Task>, network: bool) -> Applied {
    let mailbox = Arc::clone(&event.mailbox);
    let worker = start(event, pause, tasks, network);
    mailbox.wait_pending();
    let _ = route.service_one(); // prefix errors are also delivered to worker
    worker.join().unwrap()
}
fn tasks() -> Vec<openmt2001::timers::Task> { openmt2001::timers::startup_tasks([0x95; 20]) }

#[test]
fn loading_snapshot_once_then_events_share_command_fifo_and_ack_after_effects() {
    let process = Arc::new(Mutex::new(Process::default()));
    let mut route = route(&process);
    for focused in [true, false, true, false] { route.observe(focused).unwrap(); }
    let (tx, rx) = mpsc::channel(); route.attach(tx.clone()).unwrap();
    route.observe(true).unwrap(); route.observe(true).unwrap();
    tx.send(FreshRequest::Run(false)).unwrap();
    assert_eq!(process.lock().unwrap().calls, [Call::Activate(1)]);
    route.poll().unwrap(); assert_eq!(route.pending.len(), 3);
    let event = next(&rx); assert!(event.is_attachment()); assert!(!event.focused);
    let original = tasks();
    let mut state = apply(&mut route, event, State::default(), original.clone(), false);
    state.result.unwrap(); assert!(state.running);
    assert_eq!(state.pause.bytes, [0, 0, 0, 0, 1, 0, 0, 0, 1]);
    let mut expected = original.clone(); expected[3][0] = 0; assert_eq!(state.tasks, expected);
    route.poll().unwrap(); assert_eq!(route.pending.len(), 2);
    for _ in 0..2 {
        let event = next(&rx); assert!(!event.is_attachment()); assert!(event.focused);
        state = apply(&mut route, event, state.pause, state.tasks, false); state.result.unwrap();
    }
    assert!(matches!(rx.try_recv().unwrap(), FreshRequest::Run(false)));
    assert_eq!(state.pause.bytes, [0; 9]); assert_eq!(state.tasks, original);
    assert_eq!(process.lock().unwrap().calls, [Call::Activate(1), Call::Effect(1, false, 0),
        Call::Effect(1, true, 0), Call::Effect(1, true, 0)]);
    route.poll().unwrap(); assert!(route.pending.is_empty());
}

#[test]
fn cold_attachment_preserves_signed_counts_ordinary_word_and_task_padding() {
    for (focused, count, latch, reason3, after, active, enabled) in [
        (false, 0x80, 0, 0, 0x81, 0, 7), (false, 0xff, 0, 0, 0, 0, 7),
        (false, 0, 0, 0, 1, 1, 0), (false, 1, 0, 0, 1, 0, 7),
        (false, 2, 1, 0, 2, 1, 7), (false, 127, 1, 0, 127, 1, 7),
        (true, 0x80, 1, 0, 0x80, 1, 7), (true, 0xff, 1, 0, 0xff, 1, 7),
        (true, 0, 1, 0, 0, 1, 7), (true, 1, 1, 0, 0, 0, 1),
        (true, 2, 1, 0, 1, 0, 1), (true, 127, 1, 0, 126, 0, 1),
        (true, 1, 1, 1, 0, 1, 7), (true, 1, 1, 0x80, 0, 1, 7),
    ] {
        let process = Arc::new(Mutex::new(Process::default())); let mut route = route(&process);
        route.observe(!focused).unwrap(); route.observe(focused).unwrap();
        let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
        let pause = State { bytes: [0, 0, 0, reason3, count, 0x91, 0xa2, 0xb3, latch] };
        let mut row = [0x95; 20]; row[0] = 7; row[4..8].copy_from_slice(&10u32.to_le_bytes());
        let state = apply(&mut route, next(&rx), pause, vec![row], false); state.result.unwrap();
        assert_eq!(state.pause.bytes, [0, 0, 0, reason3, after, 0x91, 0xa2, 0xb3, active]);
        row[0] = enabled; assert_eq!(state.tasks, [row]);
        assert_eq!(process.lock().unwrap().calls, [Call::Activate(1), Call::Effect(1, focused, reason3 as i8)]);
        assert!(rx.try_recv().is_err()); route.poll().unwrap();
    }
}

#[test]
fn prefix_error_or_receiver_absence_cannot_apply_later_pause_effects() {
    for result in [Ok(Prefix::ReceiverAbsent), Err("actual audio unavailable".into())] {
        let process = Arc::new(Mutex::new(Process::default())); let mut route = Route::default();
        route.install(Box::new(EffectsFixture { process: Arc::clone(&process), result: result.clone() })).unwrap();
        route.observe(false).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
        let pause = State { bytes: [0, 0, 0, 0x80, 0, 0x91, 0xa2, 0xb3, 0] }; let before = tasks();
        let state = apply(&mut route, next(&rx), pause.clone(), before.clone(), false);
        assert_eq!(state.result.is_err(), result.is_err()); assert_eq!(state.pause, pause);
        assert_eq!(state.tasks, before); assert_eq!(state.running, result.is_ok());
        assert_eq!(process.lock().unwrap().calls, [Call::Activate(1), Call::Effect(1, false, -128)]);
    }
}

#[test]
fn network_gate_follows_ui_prefix_without_pause_changes() {
    let process = Arc::new(Mutex::new(Process::default())); let mut route = route(&process);
    route.observe(false).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
    let before = tasks(); let state = apply(&mut route, next(&rx), State::default(), before.clone(), true);
    state.result.unwrap(); assert_eq!(state.pause.bytes, [0; 9]); assert_eq!(state.tasks, before);
    assert_eq!(process.lock().unwrap().calls, [Call::Activate(1), Call::Effect(1, false, 0)]);
}

#[test]
fn delayed_old_event_apply_after_new_world_cannot_reach_process_effects() {
    let process = Arc::new(Mutex::new(Process::default())); let mut route = route(&process);
    route.observe(false).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx.clone()).unwrap();
    let old = next(&rx); assert!(route.attach(tx).is_err()); route.detach();
    route.observe(true).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
    apply(&mut route, next(&rx), State::default(), tasks(), false).result.unwrap();
    let mut host = crate::live_host::Host::new([0.; 3]); let mut pause = State::default(); let mut rows = tasks();
    assert!(old.apply(&mut host, &mut pause, &mut rows).is_err());
    assert_eq!(pause.bytes, [0; 9]); assert_eq!(rows, tasks());
    let p = process.lock().unwrap(); assert_eq!(p.focused, Some(true));
    assert_eq!(p.calls, [Call::Activate(1), Call::Retire(1), Call::Activate(2), Call::Effect(2, true, 0)]);
}

#[test]
fn retire_wakes_admitted_prefix_without_waiting_for_worker_and_revokes_old_event() {
    let process = Arc::new(Mutex::new(Process::default())); let mut route = route(&process);
    route.observe(false).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
    let old = next(&rx); let mailbox = Arc::clone(&old.mailbox);
    let worker = start(old, State::default(), tasks(), false); mailbox.wait_pending();
    route.detach(); // prefix admitted/enqueued; UI never serviced it
    assert!(worker.join().unwrap().result.is_err());
    route.observe(true).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
    apply(&mut route, next(&rx), State::default(), tasks(), false).result.unwrap();
    let p = process.lock().unwrap(); assert_eq!(p.focused, Some(true));
    assert_eq!(p.calls, [Call::Activate(1), Call::Retire(1), Call::Activate(2), Call::Effect(2, true, 0)]);
}

#[test]
fn actual_inflight_ui_prefix_finishes_before_retire_and_new_attachment() {
    struct Blocking { base: EffectsFixture, entered: mpsc::Sender<()>, release: mpsc::Receiver<()> }
    impl Effects for Blocking {
        fn activate(&mut self, id: AttachmentId) -> Result<(), String> { self.base.activate(id) }
        fn retire(&mut self, id: AttachmentId) -> Result<(), String> { self.base.retire(id) }
        fn before_pause(&mut self, id: AttachmentId, focused: bool, reason3: i8) -> Result<Prefix, String> {
            if id.value() == 1 {
                self.base.process.lock().unwrap().calls.push(Call::Begin(1));
                self.entered.send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(5)).unwrap(); // deterministic fixture only
                self.base.process.lock().unwrap().calls.push(Call::End(1));
            }
            self.base.before_pause(id, focused, reason3)
        }
    }
    let process = Arc::new(Mutex::new(Process::default())); let p = Arc::clone(&process);
    let (events, received) = mpsc::channel(); let (commands, command) = mpsc::channel();
    let (entered, admitted) = mpsc::channel(); let (release, released) = mpsc::channel();
    let ui = std::thread::spawn(move || {
        // UI-only Route is constructed and stays on this thread, never moved
        // with a !Send provider. Real callbacks cannot wait on the worker.
        let mut route = Route::default();
        route.install(Box::new(Blocking { base: EffectsFixture::complete(p), entered, release: released })).unwrap();
        route.observe(false).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
        events.send(next(&rx)).unwrap();
        command.recv_timeout(Duration::from_secs(5)).unwrap(); route.service_one().unwrap();
        command.recv_timeout(Duration::from_secs(5)).unwrap();
        route.detach(); route.observe(true).unwrap(); let (tx, rx) = mpsc::channel(); route.attach(tx).unwrap();
        events.send(next(&rx)).unwrap();
        command.recv_timeout(Duration::from_secs(5)).unwrap(); route.service_one().unwrap();
        command.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let old = received.recv_timeout(Duration::from_secs(5)).unwrap(); let old_mailbox = Arc::clone(&old.mailbox);
    let worker = start(old, State::default(), tasks(), false); old_mailbox.wait_pending();
    commands.send(()).unwrap(); admitted.recv_timeout(Duration::from_secs(5)).unwrap();
    commands.send(()).unwrap(); // retirement queued while actual prefix is in flight
    assert!(received.try_recv().is_err(), "new attachment cannot overtake admitted actual effects");
    release.send(()).unwrap(); worker.join().unwrap().result.unwrap();
    let new = received.recv_timeout(Duration::from_secs(5)).unwrap(); let mailbox = Arc::clone(&new.mailbox);
    let worker = start(new, State::default(), tasks(), false); mailbox.wait_pending();
    commands.send(()).unwrap(); worker.join().unwrap().result.unwrap();
    let p = process.lock().unwrap(); assert_eq!(p.focused, Some(true));
    assert_eq!(p.calls, [Call::Activate(1), Call::Begin(1), Call::End(1), Call::Effect(1, false, 0),
        Call::Retire(1), Call::Activate(2), Call::Effect(2, true, 0)]); drop(p);
    commands.send(()).unwrap(); ui.join().unwrap();
}

#[test]
fn options_and_reason3_keep_native_shared_latch_and_frontend_predicates() {
    use openmt2001::native_input::focus::{Frontend, Action};
    let mut frontend = Frontend::from_process_byte(0); let mut calls = Vec::new();
    let mut host = crate::live_host::Host::new([0.; 3]); host.set_running(true);
    let mut pause = State::default(); let mut rows = tasks();
    host.options(&mut pause, &mut rows, true).unwrap();
    host.focus(&mut pause, &mut rows, false, |reason| Ok(Prefix::Complete(frontend.apply(false, true, reason,
        |call| { calls.push(call); Ok(AudioOutcome::Applied) })?))).unwrap();
    assert_eq!(pause.bytes, [1, 0, 0, 0, 1, 0, 0, 0, 1]); assert!(!host.running);
    host.options(&mut pause, &mut rows, false).unwrap();
    assert_eq!(pause.bytes, [0, 0, 0, 0, 1, 0, 0, 0, 0]); assert!(host.running);
    pause.pause(3, |change, _| { change.apply(&mut rows); Ok(()) }).unwrap();
    host.focus(&mut pause, &mut rows, true, |reason| Ok(Prefix::Complete(frontend.apply(true, true, reason,
        |call| { calls.push(call); Ok(AudioOutcome::Applied) })?))).unwrap();
    assert_eq!(pause.bytes, [0, 0, 0, 1, 0, 0, 0, 0, 1]); assert_eq!(calls, [Action::Suspend]);
    pause.unpause(3, |change, _| { change.apply(&mut rows); Ok(()) }).unwrap();
    host.focus(&mut pause, &mut rows, true, |reason| Ok(Prefix::Complete(frontend.apply(true, true, reason,
        |call| { calls.push(call); Ok(AudioOutcome::Applied) })?))).unwrap();
    assert_eq!(pause.bytes, [0; 9]); assert_eq!(calls, [Action::Suspend, Action::Resume]);
}
