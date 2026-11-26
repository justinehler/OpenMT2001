use super::*;

fn owner(effects: u8) -> (Player, rodio::mixer::MixerSource) {
    let samples = vec![0.5; 65_536];
    let assets = Assets {
        weapon: samples.clone(), spell: samples.clone(),
        auction_murmur: samples.clone(), positive_pluck: samples.clone(),
        negative_boing: vec![0.25; 65_536], firework_launch1: samples.clone(),
        firework_launch2: samples.clone(), firework_echo_crackle: samples.clone(),
        firework_echo_single: samples.clone(), firework_whistle: samples.clone(),
        auction_voices: vec![[samples.clone(), samples.clone(), samples.clone()]; 10],
        auction_applause: samples.clone(), buy: samples.clone(), sell: samples.clone(),
        ui_light_click: samples, script_directory: PathBuf::new(),
    };
    let (mixer, output) = rodio::mixer::mixer(1, 22_050);
    mixer.add(rodio::source::Zero::new(1, 22_050));
    let mut player = Player::new(assets);
    player.set_level(effects);
    player.bus.connect_test_output(&mixer, 1, 22_050);
    (player, output)
}

fn window(output: &mut rodio::mixer::MixerSource, expected: f32) {
    let samples: Vec<_> = (0..4096)
        .map(|_| output.next().expect("retained output mixer")).collect();
    // Bounded rodio startup silence is allowed; a full/linear/double-master
    // gain prefix is not. Absolute tolerance is below every interior defect.
    assert!(samples.iter().all(|sample|
        *sample == 0. || (*sample - expected).abs() < 1e-6),
        "unexpected PCM before settled output {expected}");
    assert!(samples[2048..].iter().all(|sample| (*sample - expected).abs() < 1e-6));
}

fn silence_after_stop(output: &mut rodio::mixer::MixerSource, prior_pcm: f32) {
    let samples: Vec<_> = (0..4096)
        .map(|_| output.next().expect("retained output mixer")).collect();
    // Sink::stop/Drop set an atomic; rodio 0.21 observes it in its 5ms
    // periodic_access callback (110 mono samples at 22050Hz). This direct
    // same-rate output allows at most 128 samples, including lookahead.
    // This is a stop-control bound, not permission for a wrong startup gain.
    assert!(samples.iter().all(|sample|
        *sample == 0. || (*sample - prior_pcm).abs() < 1e-6));
    assert!(samples[128..].iter().all(|sample| *sample == 0.),
        "stopped source outlived the bounded control observation window");
}

#[test]
fn card_open_both_cues_emit_independent_q100_pcm() {
    // Native 41b436 supplies q100 for BOTH filenames. Literal expectations
    // derive from the retained independent rational/Decimal q100 oracle:
    // p25 -> D=-2920, p50 -> -1787, p60 -> -1386 hundredths dB.
    // No production gain helper or Sink.volume() computes expected output.
    for (effects, positive_pcm, negative_pcm) in [
        (0, 0., 0.),
        (25, 0.017336842, 0.008668421),
        (50, 0.063895464, 0.031947732),
        (60, 0.10138413, 0.050692067),
        (100, 0.5, 0.25),
    ] {
        for (positive, expected) in [(true, positive_pcm), (false, negative_pcm)] {
            let (mut player, mut output) = owner(effects);
            assert_eq!(player.apply(Operation::CardOpen { positive }).unwrap(), 0);
            assert_eq!(player.one_shots.len(), 1);
            window(&mut output, expected);
            assert!(!player.one_shots[0].empty());
        }
    }
}

#[test]
fn card_open_factor_store_affects_only_subsequent_cues() {
    for (positive, initial_pcm, full_pcm) in [
        (true, 0.017336842, 0.5), (false, 0.008668421, 0.25),
    ] {
        let (mut player, mut output) = owner(25);
        player.apply(Operation::CardOpen { positive }).unwrap();
        window(&mut output, initial_pcm);
        player.set_level(0);
        window(&mut output, initial_pcm);
        player.apply(Operation::CardOpen { positive }).unwrap();
        window(&mut output, initial_pcm);
        player.one_shots[0].stop();
        silence_after_stop(&mut output, initial_pcm);
        player.set_level(100);
        window(&mut output, 0.); // The old muted source cannot revive.
        player.apply(Operation::CardOpen { positive }).unwrap();
        window(&mut output, full_pcm);
        player.apply(Operation::Reset).unwrap();
        assert!(player.one_shots.is_empty());
        silence_after_stop(&mut output, full_pcm);
    }
}

#[test]
fn card_open_completed_cues_prune_and_drop_stops_neutral_output() {
    let (mut player, mut output) = owner(100);
    player.assets.positive_pluck = vec![0.5; 64];
    player.apply(Operation::CardOpen { positive: true }).unwrap();
    for _ in 0..4096 { output.next(); }
    assert!(player.one_shots[0].empty());
    player.apply(Operation::CardOpen { positive: false }).unwrap();
    assert_eq!(player.one_shots.len(), 1, "completed source pruned on next operation");
    window(&mut output, 0.25);
    drop(player);
    silence_after_stop(&mut output, 0.25);
}
