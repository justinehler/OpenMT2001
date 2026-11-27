use super::*;
use crate::intro::process_frontend::{Owner, ProcessFrontend, Provider};
use crate::{audio_bus::focus as audio, intro::FreshRequest};
use openmt2001::{pause, timers};
use std::{sync::mpsc, thread, time::Duration};
use winit::window::WindowId;

struct Snapshot {
    pause: pause::State,
    timers: Vec<timers::Task>,
    speed: u32,
}
struct Harness {
    input: Input,
    route: focus::Route,
    tx: mpsc::Sender<FreshRequest>,
    snapshots: mpsc::Receiver<Snapshot>,
    mix: audio::SharedAudio,
    process: Owner,
    worker: Option<thread::JoinHandle<()>>,
}
impl Harness {
    fn new(network: bool, reason: Option<usize>) -> Self {
        let (process, receipt) = crate::live_host::process_runtime::test_frontend_startup();
        let process = ProcessFrontend::new(
            receipt,
            process,
            WindowId::from(7),
            crate::settings_menu::Levels {
                music: 75,
                effects: 80,
            },
        )
        .unwrap();
        process.borrow_mut().observe(true).unwrap();
        let _custody = process.borrow_mut().take_custody(false).unwrap();
        let mix = process.borrow().audio.clone();
        let mut route = focus::Route::default();
        route.install(Box::new(Provider(process.clone()))).unwrap();
        route.observe(true).unwrap();
        let (tx, rx) = mpsc::channel();
        let (snapshots_tx, snapshots) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut host = crate::live_host::Host::new([0.; 3]);
            host.policy.scheduler.network_enabled = network;
            host.set_running(true);
            let mut pause = pause::State::default();
            let mut timers = vec![[0; 20]; 2];
            for (i, task) in timers.iter_mut().enumerate() {
                task[0] = 1;
                task[4..8].copy_from_slice(&(if i == 0 { 2u32 } else { 1 }).to_le_bytes());
            }
            if let Some(reason) = reason {
                pause
                    .pause(reason, |change, _| {
                        change.apply(&mut timers);
                        Ok(())
                    })
                    .unwrap();
            }
            host.restore_player_pause(&pause);
            while let Ok(request) = rx.recv() {
                match request {
                    FreshRequest::Focus(event) => {
                        event.apply(&mut host, &mut pause, &mut timers).unwrap()
                    }
                    FreshRequest::Options { open, reply } => {
                        reply
                            .send(host.options(&mut pause, &mut timers, open))
                            .unwrap();
                    }
                    FreshRequest::ToggleSpeed => host.toggle_speed(),
                    _ => panic!("unexpected input request"),
                }
                snapshots_tx
                    .send(Snapshot {
                        pause: pause.clone(),
                        timers: timers.clone(),
                        speed: host.policy.scheduler.iterations,
                    })
                    .unwrap();
            }
        });
        route.attach(tx.clone()).unwrap();
        process.borrow_mut().note_sent(route.sent());
        let mut h = Self {
            input: Input::default(),
            route,
            tx,
            snapshots,
            mix,
            process,
            worker: Some(worker),
        };
        if reason == Some(3) {
            h.route.wait_prefix();
            assert!(h.route.service_one().unwrap());
        }
        h.finish(true);
        h
    }
    fn key(&mut self, code: KeyCode, down: bool, repeat: bool, gates: Gates) {
        let sample = PhysicalSample::Key {
            key: PhysicalKey::Code(code),
            down,
            repeat,
        };
        self.process
            .borrow_mut()
            .sample(
                WindowId::from(7),
                sample,
                self.route.sent(),
                self.route.completed(),
            )
            .unwrap();
        let samples = self.process.borrow_mut().take_samples();
        for sample in samples {
            self.input.dispatch(sample, gates, &mut self.route).unwrap();
        }
    }
    fn finish(&mut self, audio: bool) -> Snapshot {
        if audio {
            self.route.wait_prefix();
            assert!(self.route.service_one().unwrap());
        }
        let snapshot = self.snapshots.recv_timeout(Duration::from_secs(5)).unwrap();
        self.route.poll().unwrap();
        self.process
            .borrow_mut()
            .complete_physical(self.route.completed());
        snapshot
    }
    fn options(&mut self, open: bool) -> Snapshot {
        let (reply, answer) = mpsc::channel();
        self.tx.send(FreshRequest::Options { open, reply }).unwrap();
        answer
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        self.finish(false)
    }
    fn no_request(&self) {
        assert!(self.snapshots.try_recv().is_err());
        assert_eq!(self.route.sent(), self.route.completed());
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.route.detach();
        // Retire every sender before joining the worker.
        let (replacement, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.tx, replacement));
        self.worker.take().unwrap().join().unwrap();
    }
}
fn active() -> Gates {
    Gates {
        active: true,
        ..Default::default()
    }
}

#[test]
fn pause_keys_toggle_once_per_press_and_resume_audio_and_timers() {
    for code in [KeyCode::F1, KeyCode::KeyP] {
        let mut h = Harness::new(false, None);
        h.key(code, true, false, active());
        let paused = h.finish(true);
        assert_eq!(&paused.pause.bytes[..4], &[0, 0, 0, 1]);
        assert_eq!(
            paused.timers.iter().map(|t| t[0]).collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(h.mix.music_paused());
        assert_eq!(h.mix.factor(), 0.);
        h.key(code, true, true, active());
        h.key(code, true, false, active());
        h.no_request();
        h.key(code, false, false, active());
        h.key(code, true, false, active());
        let resumed = h.finish(true);
        assert_eq!(resumed.pause.bytes, [0; 9]);
        assert_eq!(resumed.timers[0][0], 1);
        assert!(!h.mix.music_paused());
        assert_eq!(h.mix.factor(), crate::music_gain::normalized_percentage(80));
        assert_eq!(
            h.process.borrow().input.cells().input_enabled,
            1,
            "player pause must not change device admission"
        );
    }
}
#[test]
fn pause_keys_ignore_network_menus_modals_and_p_in_text_entry() {
    let mut network = Harness::new(true, None);
    for code in [KeyCode::F1, KeyCode::KeyP] {
        network.key(code, true, false, active());
        assert_eq!(network.finish(false).pause.bytes, [0; 9]);
    }
    assert!(!network.mix.music_paused());
    let mut h = Harness::new(false, None);
    for gates in [
        Gates::default(),
        Gates {
            modal: true,
            ..active()
        },
        Gates {
            suppressed: true,
            ..active()
        },
    ] {
        for code in [KeyCode::F1, KeyCode::KeyP] {
            h.key(code, true, false, gates);
            // Dismissing the modal while held cannot synthesize a new press.
            h.key(code, true, true, active());
            h.key(code, false, false, gates);
        }
    }
    h.key(
        KeyCode::KeyP,
        true,
        false,
        Gates {
            text_entry: true,
            ..active()
        },
    );
    h.no_request();
    h.key(
        KeyCode::F1,
        true,
        false,
        Gates {
            text_entry: true,
            ..active()
        },
    );
    assert_eq!(
        h.finish(true).pause.bytes[3],
        1,
        "F1 bypasses the native P-only text guard"
    );
}
#[test]
fn pause_keys_preserve_options_script_reasons_and_clickable_speed() {
    let mut h = Harness::new(false, Some(1));
    h.key(KeyCode::F1, true, false, active());
    assert_eq!(&h.finish(true).pause.bytes[..4], &[0, 1, 0, 1]);
    assert_eq!(&h.options(true).pause.bytes[..4], &[1, 1, 0, 1]);
    // Even stale UI admission cannot toggle through an open Options owner.
    h.key(KeyCode::F1, false, false, active());
    h.key(KeyCode::F1, true, false, active());
    assert_eq!(&h.finish(false).pause.bytes[..4], &[1, 1, 0, 1]);
    assert_eq!(&h.options(false).pause.bytes[..4], &[0, 1, 0, 1]);
    h.tx.send(FreshRequest::ToggleSpeed).unwrap();
    let fast = h.finish(false);
    assert_eq!(fast.speed, 4);
    assert_eq!(fast.pause.bytes[3], 1);
    h.key(KeyCode::F1, false, false, active());
    h.key(KeyCode::F1, true, false, active());
    let resumed = h.finish(true);
    assert_eq!(&resumed.pause.bytes[..4], &[0, 1, 0, 0]);
    assert_eq!(
        resumed.timers[0][0], 0,
        "script reason still owns the logic group"
    );
    assert_eq!(resumed.speed, 4);
}
#[test]
fn pause_keys_release_and_focus_loss_retire_press_latches() {
    let mut h = Harness::new(false, None);
    h.key(KeyCode::KeyP, true, false, Gates::default());
    h.input
        .dispatch(PhysicalSample::Unfocus, active(), &mut h.route)
        .unwrap();
    h.key(KeyCode::KeyP, true, true, active());
    h.no_request();
    h.key(
        KeyCode::KeyP,
        false,
        false,
        Gates {
            modal: true,
            ..active()
        },
    );
    h.key(KeyCode::KeyP, true, false, active());
    assert_eq!(h.finish(true).pause.bytes[3], 1);
}

#[test]
fn pause_keys_restore_saved_player_latch_and_keep_focus_ownership_separate() {
    let mut h = Harness::new(false, Some(3));
    assert!(h.mix.music_paused());
    h.process.borrow_mut().observe(false).unwrap();
    h.route.observe(false).unwrap();
    h.process.borrow_mut().note_sent(h.route.sent());
    let lost = h.finish(true);
    assert_eq!((lost.pause.bytes[3], lost.pause.bytes[4]), (1, 1));
    h.process.borrow_mut().observe(true).unwrap();
    h.route.observe(true).unwrap();
    h.process.borrow_mut().note_sent(h.route.sent());
    let gained = h.finish(true);
    assert_eq!((gained.pause.bytes[3], gained.pause.bytes[4]), (1, 0));
    assert!(
        h.mix.music_paused(),
        "focus gain must not resume player-paused audio"
    );
    h.key(KeyCode::F1, true, false, active());
    let resumed = h.finish(true);
    assert_eq!(resumed.pause.bytes, [0; 9]);
    assert_eq!(resumed.timers[0][0], 1);
    assert!(!h.mix.music_paused());
}
