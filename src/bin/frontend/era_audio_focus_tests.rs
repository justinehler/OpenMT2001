use super::*;
use crate::audio_bus::focus::{Action, Guards, SharedAudio};
const ENABLED: Guards = Guards {
    active: true,
    sound_enabled: true,
    music_enabled: true,
};

fn owner() -> (Player, SharedAudio, rodio::mixer::MixerSource) {
    let assets = Assets {
        songs: std::array::from_fn(|_| PathBuf::new()),
        before_six: vec![0.5; 65536],
        ffmpeg: PathBuf::new(),
    };
    let audio = SharedAudio::new(75, 25);
    let mut player = Player::new(assets);
    player.bind_audio(audio.clone()).unwrap();
    let (mixer, output) = rodio::mixer::mixer(2, 44100);
    mixer.add(rodio::source::Zero::new(2, 44100));
    player.music_bus.connect_test_output(&mixer, 2, 44100);
    player.effects_bus.connect_test_output(&mixer, 2, 44100);
    (player, audio, output)
}
fn pending(player: &mut Player) -> mpsc::Sender<Result<Vec<f32>, String>> {
    let (sender, receiver) = mpsc::channel();
    player.pending_token = Some(player.music_owner.token().unwrap());
    player.pending = Some((true, receiver));
    sender
}
fn window(output: &mut rodio::mixer::MixerSource, expected: f32, initial: bool) {
    let samples: Vec<_> = output.by_ref().take(8192).collect();
    assert_eq!(samples.len(), 8192);
    if initial {
        assert!(
            samples
                .iter()
                .all(|sample| *sample == 0. || (*sample - expected).abs() < 1e-6)
        );
    }
    assert!(
        samples[4096..]
            .iter()
            .all(|sample| (*sample - expected).abs() < 1e-6)
    );
}

#[test]
fn pending_decode_attaches_with_current_level_and_desired_pause_before_samples() {
    let (mut player, audio, mut output) = owner();
    let sender = pending(&mut player);
    audio.apply(Action::Suspend, ENABLED);
    audio.set_music(25); // CURRENT state changes while decode is in flight.
    sender.send(Ok(vec![0.5; 262144])).unwrap();
    player.poll().unwrap();
    assert!(player.pending.is_none());
    let song = player.song.as_ref().unwrap();
    assert!(song.is_paused());
    assert!((song.volume() - 0.034673683).abs() < 1e-7);
    window(&mut output, 0., true);
    assert_eq!(
        player.song.as_ref().unwrap().get_pos(),
        std::time::Duration::ZERO
    );
    audio.apply(Action::Resume, ENABLED); // Restore the saved75, not preference25.
    window(&mut output, 0.192517415, true);
    assert_eq!(audio.music_level(), 75);
    assert_eq!(player.song.as_ref().unwrap().len(), 1);
}

#[test]
fn retained_song_pauses_and_resumes_without_redecode_restart_or_position_loss() {
    let (mut player, audio, mut output) = owner();
    pending(&mut player).send(Ok(vec![0.5; 262144])).unwrap();
    player.poll().unwrap();
    window(&mut output, 0.192517415, true);
    let identity = &**player.song.as_ref().unwrap() as *const rodio::Sink;
    audio.apply(Action::Suspend, ENABLED);
    window(&mut output, 0., false);
    let position = player.song.as_ref().unwrap().get_pos();
    window(&mut output, 0., true);
    assert_eq!(player.song.as_ref().unwrap().get_pos(), position);
    let transferred = player;
    audio.apply(Action::Resume, ENABLED);
    window(&mut output, 0.192517415, true);
    let song = transferred.song.as_ref().unwrap();
    assert_eq!(&**song as *const rodio::Sink, identity);
    assert!(song.get_pos() > position);
    assert_eq!(song.len(), 1);
}

#[test]
fn cancelled_replaced_and_retired_decode_generations_never_attach() {
    let (mut player, audio, mut output) = owner();
    let ready_before_stop = pending(&mut player);
    ready_before_stop.send(Ok(vec![0.5; 128])).unwrap();
    player.apply(Operation::Stop).unwrap();
    player.poll().unwrap();
    assert!(player.song.is_none());
    window(&mut output, 0., true);
    let stale = pending(&mut player);
    player.music_owner.cancel().unwrap(); // Adversarial old receiver reaches poll.
    stale.send(Ok(vec![0.5; 128])).unwrap();
    assert_eq!(player.poll().unwrap_err(), "Stale era decode generation");
    assert!(player.song.is_none());
    window(&mut output, 0., true);
    let retired = pending(&mut player);
    drop(player);
    assert!(retired.send(Ok(vec![0.5; 128])).is_err());
    audio.apply(Action::Suspend, ENABLED);
    audio.apply(Action::Resume, ENABLED);
    window(&mut output, 0., true);
}

#[test]
fn pre_six_wav_uses_shared_factor_but_is_not_paused_or_rewritten_as_music() {
    let (mut player, audio, mut output) = owner();
    player.apply(Operation::BeforeSixCreate).unwrap();
    window(&mut output, 0.017336842, true);
    audio.apply(Action::Suspend, ENABLED);
    window(&mut output, 0.017336842, true);
    audio.apply(Action::Resume, ENABLED);
    window(&mut output, 0.017336842, true);
}
