use super::*;

fn owner(level: u8) -> (Player, rodio::mixer::MixerSource) {
    let assets = Assets {
        songs: std::array::from_fn(|i| PathBuf::from(SONGS[i].1)),
        before_six: Vec::new(),
        ffmpeg: PathBuf::new(),
    };
    let mut player = Player::new(assets);
    player.set_levels(level, 100);
    let (mixer, output) = rodio::mixer::mixer(2, 44_100);
    mixer.add(rodio::source::Zero::new(2, 44_100));
    player.music_bus.connect_test_output(&mixer, 2, 44_100);
    select(&mut player, 0, true)
        .send(Ok(vec![0.5; 32768]))
        .unwrap();
    player.poll().unwrap();
    (player, output)
}

// Only WMA decoding is supplied by the fixture; admission, generations, fade,
// replacement and rodio source attachment use the production Select owner.
fn select(player: &mut Player, era: u32, repeat: bool) -> mpsc::Sender<Result<Vec<f32>, String>> {
    let (sender, receiver) = mpsc::channel();
    player
        .select_with(era, repeat, |index, _, path| {
            assert_eq!(index, era as usize);
            assert_eq!(path, PathBuf::from(SONGS[index].1));
            Ok(receiver)
        })
        .unwrap();
    sender
}

fn window(output: &mut rodio::mixer::MixerSource, expected: f32) {
    let samples: Vec<_> = output.by_ref().take(8192).collect();
    assert_eq!(samples.len(), 8192);
    assert!(
        samples[4096..]
            .iter()
            .all(|sample| (*sample - expected).abs() < 1e-6)
    );
}

#[test]
fn fade_replaces_on_services_35_2_1_independently_of_simulation_pause_and_speed() {
    use openmt2001::{clock::Clock, pause, scheduler};
    for (paused, speed) in [(true, 1), (false, 1), (false, 8)] {
        for (level, replacement_service) in [(63, 35), (30, 2), (29, 1)] {
            let (mut player, mut output) = owner(level);
            window(&mut output, 0.5 * crate::music_gain::amplitude(level));
            let old = &**player.song.as_ref().unwrap() as *const rodio::Sink;
            let ready = select(&mut player, 1, true);
            ready.send(Ok(vec![-0.25; 32768])).unwrap();
            player.poll().unwrap();
            assert!(player.prepared.is_some());
            assert_eq!(&**player.song.as_ref().unwrap() as *const rodio::Sink, old);
            // Exercise the actual scheduler at zero, one and eight iterations
            // between audio services. Audio does not consume its clocks/gates.
            let mut pause = pause::State::default();
            if paused {
                pause.pause(0, |_, _| Ok(())).unwrap();
            }
            let mut clock = Clock {
                calendar: [0; 5],
                timing: [0, 0, 0, 0, speed, 1],
            };
            let mut controls = scheduler::Controls {
                iterations: if pause.has_reason() { 0 } else { speed },
                abort: false,
                alternate_script_active: false,
                network_enabled: false,
                network_override: false,
                deadline: u32::MAX,
                gate_32: false,
                gate_32_offset: false,
                before_six_sound: 0,
            };
            for service in 1..=replacement_service {
                scheduler::advance(&mut clock, &mut controls, |_, _, _| Ok(0)).unwrap();
                // Multiple camera request drains must neither fade nor attach
                // an already-prepared replacement before this frame's service.
                for _ in 0..5 {
                    player.poll().unwrap();
                }
                player.service().unwrap();
                let song = player.song.as_ref().unwrap();
                assert!(
                    !song.is_paused(),
                    "ordinary simulation pause leaves music running"
                );
                assert_eq!(
                    player.audio.music_level(),
                    level,
                    "fade must not edit the target"
                );
                if service < replacement_service {
                    let source_level = level - service as u8;
                    assert_eq!(song.source_level(), source_level);
                    assert_eq!(&**song as *const rodio::Sink, old);
                    window(
                        &mut output,
                        0.5 * crate::music_gain::amplitude(source_level),
                    );
                } else {
                    assert!(!player.change_queued);
                    assert!(player.prepared.is_none());
                    assert_eq!(song.source_level(), level);
                    window(&mut output, -0.25 * crate::music_gain::amplitude(level));
                }
            }
            assert_eq!(clock.timing[0], replacement_service * controls.iterations);
        }
    }
}

#[test]
fn slow_decode_keeps_old_song_at_29_then_starts_with_current_target() {
    let (mut player, mut output) = owner(30);
    window(&mut output, 0.5 * crate::music_gain::amplitude(30));
    let pending = select(&mut player, 1, true);
    let old = &**player.song.as_ref().unwrap() as *const rodio::Sink;
    for _ in 0..10 {
        player.poll().unwrap();
        player.service().unwrap();
        assert_eq!(&**player.song.as_ref().unwrap() as *const rodio::Sink, old);
        assert_eq!(player.song.as_ref().unwrap().source_level(), 29);
        assert_eq!(player.audio.music_level(), 30);
        window(&mut output, 0.5 * crate::music_gain::amplitude(29));
    }
    // Settings affect the retained member too, so the next fade step reads25.
    player.set_levels(25, 100);
    assert_eq!(player.song.as_ref().unwrap().source_level(), 25);
    pending.send(Ok(vec![-0.25; 32768])).unwrap();
    player.poll().unwrap();
    assert_eq!(&**player.song.as_ref().unwrap() as *const rodio::Sink, old);
    player.service().unwrap();
    assert_eq!(player.song.as_ref().unwrap().source_level(), 25);
    // Fixed nominal amplitude for level25, through actual mixed PCM.
    window(&mut output, -0.008668421);
}

#[test]
fn later_selection_stop_and_decoder_failure_cannot_revive_a_queued_song() {
    let (mut player, mut output) = owner(29);
    let stale = select(&mut player, 1, true);
    let stale_token = player.pending_token.as_ref().unwrap().clone();
    let latest = select(&mut player, 2, true);
    assert!(!stale_token.is_current());
    assert!(stale.send(Ok(vec![-0.5; 32])).is_err());
    latest.send(Ok(vec![-0.25; 32768])).unwrap();
    player.poll().unwrap();
    player.service().unwrap();
    window(&mut output, -0.25 * crate::music_gain::amplitude(29));

    let failure = select(&mut player, 3, true);
    failure
        .send(Err("synthetic decode failure".into()))
        .unwrap();
    assert_eq!(player.poll().unwrap_err(), "synthetic decode failure");
    assert!(!player.change_queued);
    window(&mut output, -0.25 * crate::music_gain::amplitude(29));

    select(&mut player, 4, true)
        .send(Ok(vec![0.5; 32]))
        .unwrap();
    player.poll().unwrap();
    player.apply(Operation::Stop).unwrap();
    player.service().unwrap();
    player.poll().unwrap();
    assert!(player.song.is_none());
    assert!(player.prepared.is_none());
    assert!(!player.change_queued);
    window(&mut output, 0.);
}

#[test]
fn immediate_selection_and_first_song_do_not_wait_for_fade_services() {
    let (mut player, mut output) = owner(63);
    select(&mut player, 1, false)
        .send(Ok(vec![-0.25; 32768]))
        .unwrap();
    assert!(player.song.is_none());
    player.poll().unwrap();
    assert!(!player.change_queued);
    assert_eq!(player.song.as_ref().unwrap().source_level(), 63);
    window(&mut output, -0.25 * crate::music_gain::amplitude(63));
}

#[test]
fn queued_decode_keeps_outgoing_member_registered_for_focus_actions() {
    use crate::audio_bus::focus::{Action, Guards};
    let (mut player, mut output) = owner(63);
    window(&mut output, 0.5 * crate::music_gain::amplitude(63));
    let _pending = select(&mut player, 1, true);
    let guards = Guards {
        active: true,
        sound_enabled: true,
        music_enabled: true,
    };
    player.audio.apply(Action::Suspend, guards);
    assert!(player.song.as_ref().unwrap().is_paused());
    assert_eq!(player.song.as_ref().unwrap().source_level(), 0);
    window(&mut output, 0.);
    player.audio.apply(Action::Resume, guards);
    assert!(!player.song.as_ref().unwrap().is_paused());
    player.service().unwrap();
    assert_eq!(player.song.as_ref().unwrap().source_level(), 62);
    assert_eq!(player.audio.music_level(), 63);
    window(&mut output, 0.5 * crate::music_gain::amplitude(62));
}
