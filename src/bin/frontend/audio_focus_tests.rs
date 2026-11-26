use super::*;

const ENABLED: Guards = Guards {
    active: true,
    sound_enabled: true,
    music_enabled: true,
};

#[test]
fn constructor_saved_defaults_are_not_overwritten_by_initial_preferences() {
    // Original 4a9500 initializes +8=1 and +c=75; activation/profile level
    // setters only update current factor/music. S4 may admit initial Gain
    // from the restored raw frontend suspended byte before any fresh Loss.
    let audio = SharedAudio::new(25, 25);
    assert_eq!(audio.factor().to_bits(), 0x3e800000);
    assert_eq!(audio.music_level(), 25);
    assert_eq!(audio.0.borrow().saved_factor.to_bits(), 0x3f800000);
    assert_eq!(audio.0.borrow().saved_music.to_bits(), 0x42960000);

    let owner = MusicOwner::new();
    let (mixer, mut output) = rodio::mixer::mixer(1, 22050);
    mixer.add(rodio::source::Zero::new(1, 22050));
    let song = audio.music_sink(&mixer, owner.token().unwrap()).unwrap();
    song.append(rodio::buffer::SamplesBuffer::new(
        1,
        22050,
        vec![0.5; 131072],
    ))
    .unwrap();
    let identity = &*song as *const rodio::Sink;
    let mut window = |expected: f32, initial: bool| {
        let samples: Vec<_> = output.by_ref().take(4096).collect();
        assert_eq!(samples.len(), 4096);
        if initial {
            assert!(
                samples
                    .iter()
                    .all(|sample| *sample == 0. || (*sample - expected).abs() < 1e-6)
            );
        }
        assert!(
            samples[2048..]
                .iter()
                .all(|sample| (*sample - expected).abs() < 1e-6)
        );
    };
    // Fixed nominal commands: music25=-2920, music75=-829. Expectations
    // are independent half-scale PCM literals, not gain-helper readbacks.
    window(0.017336842, true);
    let mut frontend = Frontend::new(audio.clone(), ENABLED);
    frontend.activate(1).unwrap();
    frontend.set_guards(Guards {
        active: false,
        ..ENABLED
    });
    assert_eq!(
        frontend.perform(1, Action::Resume).unwrap(),
        Outcome::GuardedInactive
    );
    window(0.017336842, true);
    assert_eq!(audio.factor().to_bits(), 0x3e800000);
    assert_eq!(audio.music_level(), 25);
    frontend.set_guards(ENABLED);
    let position_before_first_resume = song.get_pos();
    frontend.perform(1, Action::Resume).unwrap();
    assert_eq!(audio.factor().to_bits(), 0x3f800000);
    assert_eq!(audio.music_level(), 75);
    window(0.192517415, false);
    assert_eq!(&*song as *const rodio::Sink, identity);
    assert!(song.get_pos() > position_before_first_resume);
    assert_eq!(song.len(), 1);
    frontend.perform(1, Action::Suspend).unwrap();
    window(0., false);
    let paused_position = song.get_pos();
    window(0., true);
    assert_eq!(song.get_pos(), paused_position);
    frontend.perform(1, Action::Resume).unwrap();
    window(0.192517415, true);
    assert_eq!(&*song as *const rodio::Sink, identity);
    assert!(song.get_pos() > paused_position);
    assert_eq!(song.len(), 1);
}

struct Recorder {
    audio: SharedAudio,
    log: Rc<RefCell<Vec<(&'static str, u32, u8)>>>,
    mutate: bool,
}
impl MusicControl for Recorder {
    fn pause(&self) {
        self.log.borrow_mut().push((
            "pause",
            self.audio.factor().to_bits(),
            self.audio.music_level(),
        ));
        if self.mutate {
            self.audio.0.borrow_mut().music = 17;
        }
    }
    fn run(&self) {
        self.log.borrow_mut().push((
            "run",
            self.audio.factor().to_bits(),
            self.audio.music_level(),
        ));
        if self.mutate {
            self.audio.0.borrow_mut().music = 19;
        }
    }
    fn volume(&self, level: u8) {
        self.log
            .borrow_mut()
            .push(("volume", self.audio.factor().to_bits(), level));
    }
    fn stop(&self) {
        self.log.borrow_mut().push((
            "stop",
            self.audio.factor().to_bits(),
            self.audio.music_level(),
        ));
    }
}

#[test]
fn suspend_restore_state_precedes_member_action_and_volume_reads_late() {
    let audio = SharedAudio::new(75, 75);
    let owner = MusicOwner::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let control = Rc::new(Recorder {
        audio: audio.clone(),
        log: log.clone(),
        mutate: true,
    });
    audio
        .register(owner.token().unwrap(), control.clone())
        .unwrap();
    audio.set_music(75);
    log.borrow_mut().clear();
    assert_eq!(audio.apply(Action::Suspend, ENABLED), Outcome::Applied);
    assert_eq!(audio.factor().to_bits(), 0);
    assert!(audio.music_paused());
    assert_eq!(audio.0.borrow().saved_factor.to_bits(), 0x3f400000);
    assert_eq!(audio.0.borrow().saved_music.to_bits(), 0x42960000);
    assert_eq!(audio.apply(Action::Resume, ENABLED), Outcome::Applied);
    assert!(!audio.music_paused());
    assert_eq!(
        *log.borrow(),
        vec![
            ("pause", 0, 0),
            ("volume", 0, 17),
            ("run", 0x3f400000, 75),
            ("volume", 0x3f400000, 19),
        ]
    );
}

#[test]
fn audio_guards_are_distinct_from_missing_adapter_and_music_disabled() {
    let audio = SharedAudio::new(75, 75);
    for guards in [
        Guards {
            active: false,
            ..ENABLED
        },
        Guards {
            sound_enabled: false,
            ..ENABLED
        },
    ] {
        assert_eq!(
            audio.apply(Action::Suspend, guards),
            Outcome::GuardedInactive
        );
        assert_eq!(audio.factor().to_bits(), 0x3f400000);
        assert_eq!(audio.music_level(), 75);
    }
    let disabled_music = Guards {
        music_enabled: false,
        ..ENABLED
    };
    audio.apply(Action::Suspend, disabled_music);
    assert_eq!(audio.factor(), 0.);
    assert_eq!(audio.music_level(), 0);
    assert!(
        !audio.music_paused(),
        "disabled music does not issue member Pause"
    );
    audio.apply(Action::Resume, disabled_music);
    assert_eq!(audio.factor().to_bits(), 0x3f400000);
    assert_eq!(audio.music_level(), 75);
}

#[test]
fn generations_fence_actual_effects_and_saved_state_survives_reattachment() {
    let audio = SharedAudio::new(75, 75);
    let mut frontend = Frontend::new(audio.clone(), ENABLED);
    assert!(frontend.perform(1, Action::Suspend).is_err());
    frontend.activate(1).unwrap();
    frontend.perform(1, Action::Suspend).unwrap();
    assert!(
        frontend.activate(2).is_err(),
        "cannot overtake a still-attached generation"
    );
    frontend.retire(1).unwrap();
    frontend.activate(2).unwrap();
    assert!(frontend.perform(1, Action::Resume).is_err());
    assert_eq!(
        audio.factor(),
        0.,
        "delayed old action cannot mutate the new owner"
    );
    assert_eq!(audio.music_level(), 0);
    frontend.perform(2, Action::Resume).unwrap();
    assert_eq!(audio.factor().to_bits(), 0x3f400000);
    assert_eq!(audio.music_level(), 75);
    assert!(frontend.retire(1).is_err());
    frontend.retire(2).unwrap();
    assert!(
        frontend.activate(2).is_err(),
        "retired ids cannot alias new ownership"
    );
}

#[test]
fn cancelled_music_generation_cannot_attach_append_or_resurrect() {
    let audio = SharedAudio::new(100, 100);
    let owner = MusicOwner::new();
    let old = owner.token().unwrap();
    let (mixer, mut output) = rodio::mixer::mixer(1, 22050);
    mixer.add(rodio::source::Zero::new(1, 22050));
    let music = audio.music_sink(&mixer, old.clone()).unwrap();
    music
        .append(rodio::buffer::SamplesBuffer::new(
            1,
            22050,
            vec![0.5; 65536],
        ))
        .unwrap();
    for _ in 0..4096 {
        output.next();
    }
    owner.cancel().unwrap();
    assert!(!old.is_current());
    assert!(audio.music_sink(&mixer, old).is_err());
    assert!(
        music
            .append(rodio::buffer::SamplesBuffer::new(1, 22050, vec![0.5; 64]))
            .is_err()
    );
    audio.apply(Action::Suspend, ENABLED); // Pruning stops the cancelled retained source.
    audio.apply(Action::Resume, ENABLED);
    let stopped: Vec<_> = output.by_ref().take(4096).collect();
    assert!(stopped[128..].iter().all(|sample| *sample == 0.));
    owner.retire();
    assert!(owner.token().is_err());
}
