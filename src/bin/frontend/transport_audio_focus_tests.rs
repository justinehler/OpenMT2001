use super::*;
use crate::audio_bus::focus::{Action, Guards, SharedAudio};

fn owner() -> (Player, SharedAudio, rodio::mixer::MixerSource) {
    let samples = vec![0.5; 65536];
    let assets = Assets {
        train_motion: samples.clone(),
        train_toot: samples.clone(),
        road_ambient: samples.clone(),
        road_motor: samples.clone(),
        resident_crowd: samples.clone(),
        near_aircraft: samples.clone(),
        ship_engine: samples.clone(),
        ship_horn: samples.clone(),
        ocean_liner: samples.clone(),
        crane_start: samples.clone(),
        crane_end: samples.clone(),
        crane_lift: samples.clone(),
        crane_loop: samples.clone(),
        construction_scaffold: samples.clone(),
        construction_saw: samples.clone(),
        construction_hammer: samples.clone(),
        block_factory: samples.clone(),
        block_school: samples.clone(),
        block_park_one: samples.clone(),
        block_park_two: samples.clone(),
        block_owl: samples,
    };
    let mut player = Player::new(assets);
    let audio = SharedAudio::new(75, 25);
    player.bind_audio(audio.clone());
    let (mixer, output) = rodio::mixer::mixer(1, 22050);
    mixer.add(rodio::source::Zero::new(1, 22050));
    player.bus.connect_test_output(&mixer, 1, 22050);
    (player, audio, output)
}
fn window(output: &mut rodio::mixer::MixerSource, expected: f32, creation: bool) {
    let samples: Vec<_> = output.by_ref().take(4096).collect();
    assert_eq!(samples.len(), 4096);
    if creation {
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
fn route(id: u32) -> Operation {
    Operation::Route(RouteSound {
        id,
        distance: 0.,
        arguments: [1f32.to_bits(), 0],
    })
}
fn block(category: usize, hour: u32, delta: u32) -> Operation {
    let mut centers = [Some([1000., 1000.]); 52];
    centers[category] = Some([0., 0.]);
    Operation::Blocks(Box::new(block_audio::Snapshot {
        centers,
        camera: [0., 40., 0.],
        hour,
        tick: 100 + delta,
        previous_tick: 100,
        initial_random_state: 205,
    }))
}

#[test]
fn transport_creation_and_explicit_update_consume_current_factor_for_all_beds() {
    let far = [101f32.to_bits(), 0, 0];
    let mut motor = [far; 3];
    motor[0] = [10f32.to_bits(), 0, 1f32.to_bits()];
    let mut groups = [[None; 2]; 4];
    groups[0][0] = Some(ConstructionSource {
        distance: 0.,
        position: [0.; 3],
    });
    // Expected half-scale PCM comes from independent source words at p25.
    // q10/40/49/59/69/89/99 -> -4153/-3714/-3588/-3451/-3317/-3058/-2932.
    let mut cases = vec![
        (
            "road",
            Operation::RoadProximity {
                reciprocal_sum: 0.1,
                nearest: [far; 3],
            },
            1,
            0.0069497633,
        ),
        (
            "motor",
            Operation::RoadProximity {
                reciprocal_sum: 0.,
                nearest: motor,
            },
            2,
            0.008034706,
        ),
        ("crowd", Operation::ResidentCrowd(40.), 1, 0.0041924706),
        ("aircraft", route(0), 2, 0.0094074085),
        ("motion", route(7), 1, 0.010976654),
        ("toot", route(8), 1, 0.010976654),
        (
            "construction",
            Operation::Construction(groups),
            1,
            0.014790063,
        ),
        ("factory", block(2, 16, 0), 1, 0.017098973),
        ("school", block(41, 8, 0), 1, 0.017098973),
    ];
    for group in 1..4 {
        let mut other = [[None; 2]; 4];
        other[group][0] = groups[0][0];
        cases.push((
            "construction",
            Operation::Construction(other),
            1,
            0.014790063,
        ));
    }
    let guards = Guards {
        active: true,
        sound_enabled: true,
        music_enabled: true,
    };
    for (name, operation, calls, expected) in cases {
        let (mut player, audio, mut output) = owner();
        if name == "motor" {
            player.assets.road_ambient.fill(0.);
        }
        for invocation in 0..calls {
            player.apply(operation.clone());
            if calls == 2 && invocation == 0 {
                window(&mut output, 0., true);
            }
        }
        window(&mut output, expected, true);
        audio.apply(Action::Suspend, guards);
        window(&mut output, expected, true); // Factor store alone does not rewrite source.
        player.apply(operation.clone());
        window(&mut output, 0., false); // Explicit source update consumes current zero.
        audio.apply(Action::Resume, guards);
        window(&mut output, 0., true); // Restore alone must not revive retained source.
        player.apply(operation);
        window(&mut output, expected, false);
    }
}

#[test]
fn ship_sources_use_factor_at_creation_and_refresh() {
    let guards = Guards {
        active: true,
        sound_enabled: true,
        music_enabled: true,
    };
    for call in [
        EnvironmentCall::Ship(0),
        EnvironmentCall::Ship(1),
        EnvironmentCall::Ship(2),
    ] {
        let (mut player, audio, mut output) = owner();
        let expected = 0.034197945; // Two independently mapped q99 sources at p25.
        player.apply(Operation::Environment { call, distance: 0. });
        window(&mut output, expected, true);
        audio.apply(Action::Suspend, guards);
        window(&mut output, expected, true);
        let update = call;
        player.apply(Operation::Environment {
            call: update,
            distance: 0.,
        });
        window(&mut output, 0., false);
        audio.apply(Action::Resume, guards);
        window(&mut output, 0., true);
        player.apply(Operation::Environment {
            call: update,
            distance: 0.,
        });
        window(&mut output, expected, false);
    }
}

fn crane(player: &Player, id: u32) -> &rodio::Sink {
    match id {
        27 => player.crane_start.as_ref(),
        28 => player.crane_end.as_ref(),
        29 => player.crane_lift.as_ref(),
        31 => player.crane_loop.as_ref(),
        _ => unreachable!(),
    }
    .unwrap()
}

#[test]
fn crane_creation_stays_zero_until_actual_refresh_consumes_current_factor() {
    let guards = Guards {
        active: true,
        sound_enabled: true,
        music_enabled: true,
    };
    for id in [27, 28, 29, 31] {
        let (mut player, audio, mut output) = owner();
        let cue = Operation::Environment {
            call: EnvironmentCall::Cue(id),
            distance: 0.,
        };
        let refresh = Operation::Environment {
            call: EnvironmentCall::Refresh,
            distance: 0.,
        };
        player.apply(cue.clone());
        let identity = crane(&player, id) as *const rodio::Sink;
        window(&mut output, 0., true); // Native create word0, with no positional setter.
        let initial_position = crane(&player, id).get_pos();
        assert!(
            initial_position > std::time::Duration::ZERO,
            "muted creation still advances"
        );
        player.apply(cue.clone());
        window(&mut output, 0., true);
        assert_eq!(crane(&player, id) as *const rodio::Sink, identity);
        assert!(crane(&player, id).get_pos() > initial_position);

        audio.set_effects(100);
        window(&mut output, 0., true); // Factor store cannot activate the old zero source.
        player.apply(cue.clone());
        window(&mut output, 0., true); // Repeated Cue is also not Refresh.
        player.apply(refresh.clone());
        // Independent q99/p100 D=-31 -> half PCM .482469648, not linear .495.
        window(&mut output, 0.48246965, false);
        audio.set_effects(25);
        player.apply(cue.clone());
        window(&mut output, 0.48246965, true); // Repeated Cue must not consume p25.
        player.apply(refresh.clone());
        // Independent q99/p25 D=-2932 -> half PCM .017098973.
        window(&mut output, 0.017098973, false);
        audio.apply(Action::Suspend, guards);
        player.apply(cue);
        window(&mut output, 0.017098973, true);
        player.apply(refresh.clone());
        window(&mut output, 0., false);
        audio.apply(Action::Resume, guards);
        window(&mut output, 0., true);
        player.apply(refresh);
        window(&mut output, 0.017098973, false);
        assert_eq!(crane(&player, id) as *const rodio::Sink, identity);
        assert!(crane(&player, id).get_pos() > initial_position);
    }
}

#[test]
fn retained_train_toot_uses_non_id8_update_and_far_zero_without_restart() {
    let guards = Guards {
        active: true,
        sound_enabled: true,
        music_enabled: true,
    };
    for far in [f32::from_bits(500f32.to_bits() + 1), 700.] {
        let (mut player, audio, mut output) = owner();
        player.assets.train_motion.fill(0.); // Isolate toot PCM without omitting motion work.
        player.apply(route(8));
        let toot = player.train_toot.as_ref().unwrap();
        let identity = toot as *const rodio::Sink;
        // Independent q69/p25 D=-3317 -> half PCM .010976654.
        window(&mut output, 0.010976654, true);
        let created_position = player.train_toot.as_ref().unwrap().get_pos();
        audio.apply(Action::Suspend, guards);
        window(&mut output, 0.010976654, true);
        player.apply(route(7)); // Actual non-id8 retained-toot setter before motion.
        window(&mut output, 0., false);
        assert_eq!(
            player.train_toot.as_ref().unwrap() as *const rodio::Sink,
            identity
        );
        let muted_position = player.train_toot.as_ref().unwrap().get_pos();
        assert!(
            muted_position > created_position,
            "muting did not rewind/pause the toot"
        );
        audio.apply(Action::Resume, guards);
        window(&mut output, 0., true);
        let near_update = Operation::Route(RouteSound {
            id: 7,
            distance: 100.,
            arguments: [1f32.to_bits(), 0],
        });
        player.apply(near_update.clone());
        // Distance100 changes toot q to64; independent p25 command=-3384.
        window(&mut output, 0.010161785, false);
        assert_eq!(
            player.train_toot.as_ref().unwrap() as *const rodio::Sink,
            identity
        );
        // Original fcomp/test AH admits equality: q44 toot and q19 motion.
        player.apply(Operation::Route(RouteSound {
            id: 7,
            distance: 500.,
            arguments: [1f32.to_bits(), 0],
        }));
        // Independent q44/p25 command=-3658, nominal half-scale PCM.
        window(&mut output, 0.0074125904, false);
        assert!(player.train_motion.is_some());
        assert_eq!(
            player.train_toot.as_ref().unwrap() as *const rodio::Sink,
            identity
        );
        assert!(player.train_toot.as_ref().unwrap().get_pos() > muted_position);
        player.apply(Operation::Route(RouteSound {
            id: 7,
            distance: far,
            arguments: [1f32.to_bits(), 0],
        }));
        window(&mut output, 0., false); // Native out-of-range source0, retained handle.
        assert!(player.train_motion.is_none());
        assert_eq!(
            player.train_toot.as_ref().unwrap() as *const rodio::Sink,
            identity
        );
        let far_position = player.train_toot.as_ref().unwrap().get_pos();
        assert!(far_position > muted_position);
        audio.set_effects(100);
        window(&mut output, 0., true);
        player.apply(near_update);
        // Same retained q64 toot at p100: D=-1233, half PCM .120912164.
        window(&mut output, 0.120912164, false);
        let toot = player.train_toot.as_ref().unwrap();
        assert_eq!(toot as *const rodio::Sink, identity);
        assert!(toot.get_pos() > far_position);
        assert!(!toot.empty());
    }
}

#[test]
fn authored_park_and_owl_one_shots_keep_creation_gain() {
    for (category, hour) in [(14, 8), (19, 8), (14, 21)] {
        let (mut player, audio, mut output) = owner();
        player.apply(block(category, hour, 1500));
        window(&mut output, 0.017098973, true);
        audio.set_effects(0);
        window(&mut output, 0.017098973, true);
        audio.set_effects(100);
        window(&mut output, 0.017098973, true);
    }
}
