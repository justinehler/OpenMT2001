use super::*;

// Synthetic PCM isolates the real owner/routing. No original assets or device
// are needed; command and gain expectations come from an independent oracle.
fn assets() -> Assets {
    let samples = vec![0.5; 32_768];
    Assets {
        weapon: samples.clone(),
        spell: samples.clone(),
        auction_murmur: samples.clone(),
        positive_pluck: samples.clone(),
        negative_boing: samples.clone(),
        firework_launch1: samples.clone(),
        firework_launch2: samples.clone(),
        firework_echo_crackle: samples.clone(),
        firework_echo_single: samples.clone(),
        firework_whistle: samples.clone(),
        auction_voices: vec![[samples.clone(), samples.clone(), samples.clone()]; 10],
        auction_applause: samples.clone(),
        buy: samples.clone(),
        sell: samples.clone(),
        ui_light_click: samples,
        script_directory: PathBuf::new(),
    }
}

fn owner(effects: u8) -> (Player, rodio::mixer::MixerSource) {
    let (mixer, output) = rodio::mixer::mixer(1, 22_050);
    mixer.add(rodio::source::Zero::new(1, 22_050));
    let mut player = Player::new(assets());
    // The actual preference setter runs before the actual Bus connects.
    player.set_level(effects);
    player.bus.connect_test_output(&mixer, 1, 22_050);
    (player, output)
}

fn window(output: &mut rodio::mixer::MixerSource) -> Vec<f32> {
    (0..4096).map(|_| output.next().expect("retained output mixer")).collect()
}

fn tail(samples: &[f32]) -> &[f32] { &samples[2048..] }

#[test]
fn ledger_initial_mute_has_no_full_gain_prefix_for_buy_or_sell() {
    for sale in [false, true] {
        let (mut player, mut output) = owner(0);
        assert_eq!(player.apply(Operation::Ledger { sale }).unwrap(), 0);
        assert_eq!(player.ledger_cues.len(), 1);
        assert_eq!(player.ledger_cues[0].source_level, 60);
        assert_eq!(player.ledger_cues[0].sink.volume(), 0.);
        assert!(window(&mut output).iter().all(|sample| *sample == 0.),
            "even the first samples must be muted");
    }
}

#[test]
fn ledger_pcm_distinguishes_old_linear_source_gain_at_full_preference() {
    let (mut player, mut output) = owner(100);
    player.apply(Operation::Ledger { sale: false }).unwrap();
    let samples = window(&mut output);
    // Independent broad bound from the original formula:
    // q60/p100 => -2000 < D <= -1000; nominal amplitude < 1/2.
    // Source PCM is 1/2. Old linear PCM is 3/10 and violates this bound.
    assert!(samples.iter().all(|sample| *sample < 0.25),
        "no full-gain or old linear prefix");
    assert!(tail(&samples).iter().all(|sample| *sample > 0. && *sample < 0.25));
}

#[test]
fn ledger_factor_store_preserves_old_cue_and_only_new_cue_consumes_mute() {
    let (mut player, mut output) = owner(100);
    player.apply(Operation::Ledger { sale: false }).unwrap();
    let before = window(&mut output);
    let old_pcm = *tail(&before).last().unwrap();
    let old_gain = player.ledger_cues[0].sink.volume();
    assert!(old_pcm > 0.);

    player.set_level(0);
    assert_eq!(player.effects_factor, 0.);
    assert_eq!(player.ledger_cues[0].source_level, 60);
    assert_eq!(player.ledger_cues[0].sink.volume(), old_gain);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == old_pcm),
        "factor store must not rewrite the old one-shot");
    player.apply(Operation::Ledger { sale: true }).unwrap();
    assert_eq!(player.ledger_cues.len(), 2);
    assert_eq!(player.ledger_cues[1].source_level, 60);
    assert_eq!(player.ledger_cues[1].sink.volume(), 0.);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == old_pcm),
        "the subsequent cue consumes zero without changing the old cue");

    player.ledger_cues[0].sink.stop();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));
    player.set_level(100);
    assert_eq!(player.ledger_cues.last().unwrap().source_level, 60);
    assert_eq!(player.ledger_cues.last().unwrap().sink.volume(), 0.);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.),
        "restoring the factor must not revive the muted old cue");
    player.apply(Operation::Ledger { sale: false }).unwrap();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample > 0.));
}

#[test]
fn ledger_prune_reset_and_player_drop_stop_real_children() {
    let (mut player, mut output) = owner(100);
    player.assets.buy = vec![0.5; 64];
    player.apply(Operation::Ledger { sale: false }).unwrap();
    window(&mut output);
    assert!(player.ledger_cues[0].sink.empty());
    player.apply(Operation::Ledger { sale: true }).unwrap();
    assert_eq!(player.ledger_cues.len(), 1, "next operation prunes completed cue");
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample > 0.));
    player.apply(Operation::Reset).unwrap();
    assert!(player.ledger_cues.is_empty());
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));

    player.apply(Operation::Ledger { sale: true }).unwrap();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample > 0.));
    drop(player);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.),
        "dropping the real owner must stop its neutral-path child");
}

#[test]
fn ledger_migration_preserves_unmigrated_targeted_family_linear_output() {
    let (mut player, mut output) = owner(25);
    player.apply(Operation::Targeted { actor_is_current: true }).unwrap();
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.125));
    player.set_level(50);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.25),
        "unmigrated families keep the same live linear master");
    player.set_level(0);
    assert!(tail(&window(&mut output)).iter().all(|sample| *sample == 0.));
}

#[test]
#[ignore = "requires independently supplied q60 command/gain TSV"]
fn ledger_independent_original_fixture_through_actual_owner_pcm() {
    let fixture = std::env::var_os("OPENMT2001_LEDGER_GAIN_ORACLE")
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
        assert_eq!(source_level, 60);
        assert!(matches!(effects, 0 | 25 | 50 | 60 | 100));
        assert!((-10_000..=0).contains(&command));
        assert!(expected_gain.is_finite() && (0.0..=1.0).contains(&expected_gain));
        assert!(seen.insert(effects), "duplicate independent fixture");
        for sale in [false, true] {
            let (mut player, mut output) = owner(effects);
            let normalized = fixed_cue_normalized(player.effects_factor, source_level);
            assert_eq!(normalized.to_bits(), bits, "single final RN32 store");
            if effects == 60 {
                // LG-01: the fixture's correct bits are external expectations.
                // This deliberately wrong staging is only a sensitivity guard;
                // it never computes the correct expected bits or emitted PCM.
                let percent = f64::from(f32::from_bits(0x3c23_d70a));
                let prematurely_stored = (f64::from(source_level) * percent) as f32;
                let early_spill = (f64::from(player.effects_factor)
                    * f64::from(prematurely_stored)) as f32;
                assert_ne!(bits, early_spill.to_bits(),
                    "mandatory p60/q60 fixture must reject intermediate RN32(q*c)");
            }
            assert_eq!(crate::music_gain::normalized_command(normalized), command);
            player.apply(Operation::Ledger { sale }).unwrap();
            assert_eq!(player.ledger_cues[0].source_level, source_level);
            let samples = window(&mut output);
            let expected = 0.5 * expected_gain;
            let tolerance = 8. * f32::EPSILON;
            assert!(samples.iter().all(|sample|
                (*sample - expected).abs() <= tolerance || *sample == 0.),
                "initial prefix contains neither full gain nor a wrong curve");
            assert!(tail(&samples).iter().all(|sample|
                (*sample - expected).abs() <= tolerance),
                "actual Player/Bus child PCM must match independent expectation");
        }
    }
    assert_eq!(seen, [0, 25, 50, 60, 100].into_iter().collect());
}
