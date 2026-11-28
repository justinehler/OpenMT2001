use super::*;
use crate::audio_bus::focus::{Action, Guards, SharedAudio};

fn owner(level: u8) -> (Player, SharedAudio, rodio::mixer::MixerSource) {
    let samples = vec![0.5; 65536];
    let assets = Assets {
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
        ui_light_click: samples.clone(),
        script_directory: PathBuf::new(),
    };
    let audio = SharedAudio::new(75, level);
    let mut player = Player::new(assets);
    player.bind_audio(audio.clone());
    player.scripted.insert("synthetic.wav".into(), samples);
    let (mixer, output) = rodio::mixer::mixer(1, 22050);
    mixer.add(rodio::source::Zero::new(1, 22050));
    player.bus.connect_test_output(&mixer, 1, 22050);
    (player, audio, output)
}
fn window(output: &mut rodio::mixer::MixerSource, expected: f32, prefix: bool) {
    let samples: Vec<_> = output.by_ref().take(4096).collect();
    assert_eq!(samples.len(), 4096);
    if prefix {
        assert!(
            samples
                .iter()
                .all(|sample| *sample == 0. || (*sample - expected).abs() < 1e-6)
        );
    }
    assert!(
        samples[2048..]
            .iter()
            .all(|sample| (*sample - expected).abs() < 1e-6),
        "expected {expected}"
    );
}

#[test]
fn every_gameplay_wav_family_consumes_independent_source_gain_without_live_master() {
    use crate::{auction_scene::ActorCue, fresh_city::result::fireworks::Cue};
    // Independent p25 command/PCM literals: q100=-2920, q80=-3173, q60=-3438.
    let mut cases = vec![
        (
            Operation::Targeted {
                actor_is_current: false,
            },
            0.017336842,
        ),
        (
            Operation::Targeted {
                actor_is_current: true,
            },
            0.017336842,
        ),
        (Operation::AuctionAmbientCreate, 0.017336842),
        (Operation::CardOpen { positive: false }, 0.017336842),
        (Operation::CardOpen { positive: true }, 0.017336842),
        (
            Operation::Script {
                name: "synthetic.wav".into(),
            },
            0.017336842,
        ),
        (Operation::Ledger { sale: false }, 0.009549267),
        (Operation::Ledger { sale: true }, 0.009549267),
        (Operation::UiLightClick, 0.017336842),
        (
            Operation::AuctionActor(ActorCue::Bid {
                participant: 0,
                character: 0,
                suppressed: false,
            }),
            0.017336842,
        ),
        (
            Operation::AuctionActor(ActorCue::Withdraw {
                participant: 0,
                character: 0,
                suppressed: false,
            }),
            0.017336842,
        ),
        (
            Operation::AuctionActor(ActorCue::Win { character: 0 }),
            0.017336842,
        ),
        (Operation::AuctionActor(ActorCue::Applause), 0.017336842),
    ];
    for cue in [
        Cue::Launch1,
        Cue::Launch2,
        Cue::EchoCrackle,
        Cue::EchoSingle,
    ] {
        cases.push((Operation::ResultFirework(cue), 0.017336842));
    }
    cases.push((Operation::ResultFirework(Cue::LaunchWhistle), 0.012955972));
    let guards = Guards {
        active: true,
        sound_enabled: true,
        music_enabled: true,
    };
    for (operation, expected) in cases {
        let (mut player, audio, mut output) = owner(25);
        player.apply(operation.clone()).unwrap();
        window(&mut output, expected, true);
        player.set_level(50);
        window(&mut output, expected, true);
        audio.apply(Action::Suspend, guards);
        window(&mut output, expected, true);
        audio.apply(Action::Resume, guards);
        window(&mut output, expected, true);

        let (mut muted, muted_audio, mut muted_output) = owner(0);
        muted.apply(operation).unwrap();
        window(&mut muted_output, 0., true);
        muted_audio.set_effects(100);
        window(&mut muted_output, 0., true);
    }
}

#[test]
fn process_factor_survives_player_transfer_and_next_view_reads_current_state() {
    let (mut player, audio, mut output) = owner(25);
    player
        .apply(Operation::Targeted {
            actor_is_current: true,
        })
        .unwrap();
    window(&mut output, 0.017336842, true);
    audio.apply(
        Action::Suspend,
        Guards {
            active: true,
            sound_enabled: true,
            music_enabled: true,
        },
    );
    let mut transferred = player;
    window(&mut output, 0.017336842, true);
    transferred.apply(Operation::UiLightClick).unwrap();
    window(&mut output, 0.017336842, true); // New click muted; old source unchanged.
    let (mut next, _, mut next_output) = owner(100);
    next.bind_audio(audio.clone());
    next.apply(Operation::Targeted {
        actor_is_current: false,
    })
    .unwrap();
    window(&mut next_output, 0., true);
    audio.apply(
        Action::Resume,
        Guards {
            active: true,
            sound_enabled: true,
            music_enabled: true,
        },
    );
    window(&mut next_output, 0., true); // A retained zero-gain WAV cannot revive.
}

#[test]
fn voice_admission_repeated_cash_and_clicks_do_not_layer_and_finished_cues_readmit() {
    for operation in [
        Operation::Ledger { sale: false },
        Operation::Ledger { sale: true },
        Operation::UiLightClick,
    ] {
        let (mut player, _, mut output) = owner(100);
        for _ in 0..20 {
            player.apply(operation.clone()).unwrap();
        }
        if matches!(operation, Operation::UiLightClick) {
            assert_eq!(player.ui_click_cues.len(), 1);
            window(&mut output, 0.5, true);
        } else {
            assert_eq!(player.ledger_cues.len(), 1);
            let gain = player.ledger_cues[0].sink.volume();
            window(&mut output, gain * 0.5, true);
        }
        output.by_ref().take(70_000).for_each(drop);
        player.apply(operation.clone()).unwrap();
        assert!(output.by_ref().take(4096).any(|sample| sample > 0.));
    }
}

#[test]
fn voice_admission_streamed_pool_is_shared_across_production_gameplay_families() {
    let (mut player, audio, mut output) = owner(100);
    for _ in 0..10 {
        player
            .apply(Operation::Targeted {
                actor_is_current: true,
            })
            .unwrap();
    }
    let handles: Vec<_> = player.one_shots.to_vec();
    audio.service_effects(); // All ten have the same positive age.
    player.assets.positive_pluck.fill(0.25);
    player
        .apply(Operation::CardOpen { positive: true })
        .unwrap();
    assert!(handles[..9].iter().all(|handle| !handle.empty()));
    assert!(
        handles[9].empty(),
        "streamed equal ages replace the last row"
    );
    window(&mut output, 4.75, true);
}

#[test]
fn voice_admission_unique_requests_return_no_new_auction_handle() {
    let (mut first, shared, mut output) = owner(100);
    let (mut second, _, _second_output) = owner(100);
    second.bind_audio(shared);
    let handle = first.apply(Operation::AuctionAmbientCreate).unwrap();
    assert_ne!(handle, 0);
    assert_eq!(second.apply(Operation::AuctionAmbientCreate).unwrap(), 0);
    assert!(second.ambient.is_none());
    window(&mut output, 0.5, true);
    first
        .apply(Operation::AuctionAmbientRelease(handle))
        .unwrap();
    assert_ne!(second.apply(Operation::AuctionAmbientCreate).unwrap(), 0);
}

#[test]
fn voice_admission_fireworks_use_four_cached_cells_before_the_emitter_handle_cap() {
    let (mut player, audio, mut output) = owner(100);
    let cue = Operation::ResultFirework(crate::fresh_city::result::fireworks::Cue::Launch1);
    for _ in 0..4 {
        player.apply(cue.clone()).unwrap();
    }
    let handles = player.firework_bursts.clone();
    audio.service_effects();
    player.apply(cue).unwrap();
    assert!(handles[0].empty());
    assert!(handles[1..].iter().all(|handle| !handle.empty()));
    window(&mut output, 2., true);
}
