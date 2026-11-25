use super::*;

// Synthetic, constant PCM measures the actual Player/Bus/Sink route. The
// independent original-derived nominal oracle is a mandatory separate input.
fn assets() -> Assets {
    let samples = vec![0.5; 65_536];
    Assets {
        weapon: samples.clone(), spell: samples.clone(),
        auction_murmur: samples.clone(), positive_pluck: samples.clone(),
        negative_boing: samples.clone(), firework_launch1: samples.clone(),
        firework_launch2: samples.clone(), firework_echo_crackle: samples.clone(),
        firework_echo_single: samples.clone(), firework_whistle: samples.clone(),
        auction_voices: vec![[samples.clone(), samples.clone(), samples.clone()]; 10],
        auction_applause: samples.clone(), buy: samples.clone(), sell: samples.clone(),
        ui_light_click: samples, script_directory: PathBuf::new(),
    }
}

fn owner(effects: u8) -> (Player, rodio::mixer::MixerSource) {
    let (output, source) = rodio::mixer::mixer(1, 22_050);
    output.add(rodio::source::Zero::new(1, 22_050));
    let mut player = Player::new(assets());
    player.set_level(effects);
    player.bus.connect_test_output(&output, 1, 22_050);
    (player, source)
}

fn window(output: &mut rodio::mixer::MixerSource) -> Vec<f32> {
    (0..4096).map(|_| output.next().expect("retained same-Bus output")).collect()
}
fn tail(samples: &[f32]) -> &[f32] { &samples[2048..] }

#[test]
fn ui_click_initial_mute_has_no_full_gain_prefix() {
    let (mut player, mut output) = owner(0);
    assert_eq!(player.apply(Operation::UiLightClick).unwrap(), 0);
    assert_eq!(player.ui_click_cues.len(), 1);
    assert_eq!(player.ui_click_cues[0].source_level, 100);
    assert_eq!(player.ui_click_cues[0].sink.volume(), 0.);
    assert!(player.one_shots.is_empty(), "click must use the existing neutral output path");
    assert!(window(&mut output).iter().all(|sample| *sample == 0.),
        "initial mute applies before the first queued sample");
}

#[test]
fn ui_click_actual_owner_pcm_rejects_old_linear_preference() {
    let (mut player, mut output) = owner(25);
    player.apply(Operation::UiLightClick).unwrap();
    let samples = window(&mut output);
    // Original formula at p25/q100: -3000 < D < -2000, hence amplitude
    // between 10^(-3/2) and 1/10. Half-scale PCM is strictly below .05.
    // These deliberately broad bounds are independent of the production helper.
    assert!(samples.iter().all(|sample| *sample >= 0. && *sample < 0.05),
        "neither a full-gain prefix nor old linear .125 PCM is admissible");
    assert!(tail(&samples).iter().all(|sample| *sample > 0.01 && *sample < 0.05));
}

#[test]
fn ui_click_new_creation_consumes_preference_without_rewriting_prior_click() {
    let (mut player, mut output) = owner(100);
    player.apply(Operation::UiLightClick).unwrap();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.5));
    let prior_gain = player.ui_click_cues[0].sink.volume();

    player.set_level(0);
    assert_eq!(player.effects_factor, 0.);
    assert_eq!(player.ui_click_cues[0].source_level, 100);
    assert_eq!(player.ui_click_cues[0].sink.volume(), prior_gain);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.5),
        "factor setter does not rewrite the existing click");
    player.apply(Operation::UiLightClick).unwrap();
    assert_eq!(player.ui_click_cues.len(), 2);
    assert_eq!(player.ui_click_cues[1].source_level, 100);
    assert_eq!(player.ui_click_cues[1].sink.volume(), 0.);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.5));

    player.ui_click_cues[0].sink.stop();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));
    player.set_level(100);
    assert_eq!(player.ui_click_cues.last().unwrap().source_level, 100);
    assert_eq!(player.ui_click_cues.last().unwrap().sink.volume(), 0.);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.),
        "the prior muted click cannot revive when preference is restored");
    player.apply(Operation::UiLightClick).unwrap();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.5),
        "a subsequent click consumes the restored factor");
}

#[test]
fn ui_click_prune_reset_and_drop_release_the_actual_children() {
    let (mut player, mut output) = owner(100);
    player.assets.ui_light_click = vec![0.5; 64];
    player.apply(Operation::UiLightClick).unwrap();
    window(&mut output);
    assert!(player.ui_click_cues[0].sink.empty());
    player.assets.ui_light_click = vec![0.5; 65_536];
    player.apply(Operation::UiLightClick).unwrap();
    assert_eq!(player.ui_click_cues.len(), 1, "next operation prunes completed click");
    assert_eq!(player.ui_click_cues[0].source_level, 100);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.5));
    player.apply(Operation::Reset).unwrap();
    assert!(player.ui_click_cues.is_empty());
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));
    player.apply(Operation::UiLightClick).unwrap();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.5));
    drop(player);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.),
        "Player drop stops its non-detached neutral-path children");
}

#[test]
fn ui_click_and_ledger_share_existing_bus_output_without_rewriting_ledger() {
    let (mut player, mut output) = owner(100);
    player.apply(Operation::Ledger { sale: false }).unwrap();
    let ledger_pcm = *tail(&window(&mut output)).last().unwrap();
    let ledger_gain = player.ledger_cues[0].sink.volume();
    assert!(ledger_pcm > 0. && ledger_pcm < 0.25);
    assert_eq!(player.ledger_cues[0].source_level, 60);
    player.apply(Operation::UiLightClick).unwrap();
    assert_eq!(player.ui_click_cues[0].source_level, 100);
    assert_eq!(player.ledger_cues[0].source_level, 60);
    assert_eq!(player.ledger_cues[0].sink.volume(), ledger_gain);
    let both = ledger_pcm + 0.5;
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == both),
        "both actual children reach the same already-owned output mixer");
    player.set_level(0);
    assert_eq!(player.ledger_cues[0].sink.volume(), ledger_gain);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == both));
    player.apply(Operation::Reset).unwrap();
    assert!(player.ui_click_cues.is_empty() && player.ledger_cues.is_empty());
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));
}

#[test]
fn ui_click_migration_preserves_both_targeted_variants_linear_live_gain() {
    for actor_is_current in [false, true] {
        let (mut player, mut output) = owner(25);
        player.apply(Operation::Targeted { actor_is_current }).unwrap();
        assert!(player.ui_click_cues.is_empty() && player.ledger_cues.is_empty());
        assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.125));
        player.set_level(50);
        assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.25));
        player.set_level(0);
        assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));
    }
}

#[test]
#[ignore = "requires independently supplied q100 command/gain TSV"]
fn ui_click_independent_original_fixture_through_actual_owner_pcm() {
    let fixture = std::env::var_os("OPENMT2001_UI_CLICK_GAIN_ORACLE")
        .expect("independent oracle path is mandatory; no generated fallback");
    let rows = std::fs::read_to_string(fixture).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for line in rows.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')) {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 5, "p q combined_bits command nominal_amplitude");
        let effects: u8 = fields[0].parse().unwrap();
        let source_level: i32 = fields[1].parse().unwrap();
        let bits = u32::from_str_radix(fields[2].trim_start_matches("0x"), 16).unwrap();
        let command: i32 = fields[3].parse().unwrap();
        let expected_gain: f32 = fields[4].parse().unwrap();
        assert_eq!(source_level, 100);
        assert!(matches!(effects, 0 | 25 | 50 | 60 | 100));
        assert!((-10_000..=0).contains(&command));
        assert!(expected_gain.is_finite() && (0.0..=1.0).contains(&expected_gain));
        assert!(seen.insert(effects), "duplicate independent fixture");
        let (mut player, mut output) = owner(effects);
        let combined = fixed_cue_normalized(player.effects_factor, source_level);
        assert_eq!(combined.to_bits(), bits, "original final binary32 argument");
        assert_eq!(crate::music_gain::normalized_command(combined), command);
        player.apply(Operation::UiLightClick).unwrap();
        assert_eq!(player.ui_click_cues[0].source_level, source_level);
        assert!((player.ui_click_cues[0].sink.volume() - expected_gain).abs()
            <= 8. * f32::EPSILON);
        let samples = window(&mut output);
        let expected_pcm = 0.5 * expected_gain;
        let tolerance = 8. * f32::EPSILON;
        assert!(samples.iter().all(|sample|
            (*sample - expected_pcm).abs() <= tolerance || *sample == 0.),
            "prefix must contain neither full gain nor a wrong curve");
        assert!(tail(&samples).iter().all(|sample|
            (*sample - expected_pcm).abs() <= tolerance),
            "actual owner PCM must match independent nominal expectation");
    }
    assert_eq!(seen, [0, 25, 50, 60, 100].into_iter().collect());
}
