use super::*;
use crate::audio_bus::focus::{Action, Guards, SharedAudio};

include!("transport_audio_construction_crane_tests.rs");

fn owner() -> (Player, SharedAudio, rodio::mixer::MixerSource) {
    let samples = vec![0.5; 65536];
    let assets = Assets {
        train_motion: samples.clone(),
        train_toot: samples.clone(),
        road_ambient: samples.clone(),
        road_motor: samples.clone(),
        resident_crowd: samples.clone(),
        coast: Some(samples.clone()),
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
        (
            "coast",
            Operation::Coast { camera_z: -300. },
            1,
            0.012955972,
        ),
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
        // ID8 requests another unique voice. ID12 is the retained toot's
        // ordinary setter path, which this gain test needs after creation.
        let operation = if name == "toot" { route(12) } else { operation };
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
fn ship_suppressed_engine_keeps_creation_factor_while_horn_refreshes() {
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
        assert!(player.ship_engine.is_none());
        window(&mut output, expected / 2., false);
        audio.apply(Action::Resume, guards);
        window(&mut output, expected / 2., true);
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
        let identity = &**toot as *const rodio::Sink;
        // Independent q69/p25 D=-3317 -> half PCM .010976654.
        window(&mut output, 0.010976654, true);
        let created_position = player.train_toot.as_ref().unwrap().get_pos();
        audio.apply(Action::Suspend, guards);
        window(&mut output, 0.010976654, true);
        player.apply(route(7)); // Actual non-id8 retained-toot setter before motion.
        window(&mut output, 0., false);
        assert_eq!(
            &**player.train_toot.as_ref().unwrap() as *const rodio::Sink,
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
            &**player.train_toot.as_ref().unwrap() as *const rodio::Sink,
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
            &**player.train_toot.as_ref().unwrap() as *const rodio::Sink,
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
            &**player.train_toot.as_ref().unwrap() as *const rodio::Sink,
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
        assert_eq!(&**toot as *const rodio::Sink, identity);
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

#[test]
fn coast_camera_z_threshold_clipping_and_source_levels() {
    let (mut player, audio, mut output) = owner();
    audio.set_effects(100);
    for z in [-150., 0., 500.] {
        player.apply(Operation::Coast { camera_z: z });
        assert!(player.coast.is_none(), "no inland creation at Z={z}");
    }
    // Independent source words from 4adbe0's clipped distance and f32 gain
    // store. The first two coastal positions still create a zero-level loop.
    for (z, level) in [
        (f32::from_bits((-150f32).to_bits() + 1), 0),
        (-151., 0),
        (-151.875, 1),
        (-153.75, 2),
        (-163.125, 6),  // A second f32 store before truncation would yield 7.
        (-226.875, 40), // Likewise 40, not 41.
        (-225., 40),
        (f32::from_bits((-300f32).to_bits() - 1), 79),
        (-300., 80),
        (-1000., 80),
        (-f32::MAX, 80),
        (-150., 0),
        (1000., 0),
    ] {
        player.apply(Operation::Coast { camera_z: z });
        let sink = player.coast.as_ref().unwrap();
        assert_eq!(sink.volume(), audio.source_gain(level), "Z={z}");
        assert!(!sink.is_paused());
        assert!(!sink.empty());
    }
    player.apply(Operation::Coast { camera_z: -300. });
    // Independent q80/p100 D=-654 maps half-scale PCM to this amplitude.
    window(&mut output, 0.23548867, true);
}

#[test]
fn coast_mutes_and_resumes_same_advancing_loop() {
    let (mut player, audio, mut output) = owner();
    let (sender, receiver) = mpsc::channel();
    submit(&Some(sender), Operation::Coast { camera_z: -300. }).unwrap();
    player.apply(receiver.recv().unwrap().operation);
    let identity = &**player.coast.as_ref().unwrap() as *const rodio::Sink;
    // Independent q80/p25 D=-3173, with half-scale PCM.
    window(&mut output, 0.012955972, true);
    let near_position = player.coast.as_ref().unwrap().get_pos();
    for z in [-150., 0.] {
        player.apply(Operation::Coast { camera_z: z });
        window(&mut output, 0., false);
        let sink = player.coast.as_ref().unwrap();
        assert_eq!(&**sink as *const rodio::Sink, identity);
        assert!(sink.get_pos() > near_position, "muted loop advances");
    }
    let muted_position = player.coast.as_ref().unwrap().get_pos();
    audio.set_effects(100);
    player.apply(Operation::Coast { camera_z: -225. });
    assert_eq!(
        player.coast.as_ref().unwrap().volume(),
        audio.source_gain(40)
    );
    for _ in 0..4096 {
        let _ = output.next();
    }
    let sink = player.coast.as_ref().unwrap();
    assert_eq!(&**sink as *const rodio::Sink, identity);
    assert!(sink.get_pos() > muted_position, "returning never rewinds");
    player.apply(Operation::Coast { camera_z: -300. });
    window(&mut output, 0.23548867, false);
}

#[test]
fn coast_releases_in_strategic_inspection_and_reset() {
    for mode in [1, 3] {
        let (mut player, _, mut output) = owner();
        player.apply(Operation::Coast { camera_z: -300. });
        window(&mut output, 0.012955972, true);
        player.set_interface_mode(mode);
        player.apply(Operation::Coast { camera_z: -300. });
        assert!(player.coast.is_none());
        window(&mut output, 0., false);
        player.set_interface_mode(0);
        player.apply(Operation::Coast { camera_z: -300. });
        window(&mut output, 0.012955972, true);
        player.apply(Operation::Reset);
        assert!(player.coast.is_none());
        window(&mut output, 0., false);
    }
}

#[test]
fn coast_missing_sample_is_silent_and_supplied_mixed_case_pcm_loads() {
    let root = openmt2001::fixtures::scratch("COAST_AUDIO");
    let directory = root.join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let (mut player, _, mut output) = owner();
    player.assets.coast = load_coast(&directory);
    assert!(player.assets.coast.is_none());
    for z in [-300., -225., -150., -300.] {
        player.apply(Operation::Coast { camera_z: z });
        assert!(player.coast.is_none());
        window(&mut output, 0., true);
    }
    // Other city audio still works when the optional coast WAV is absent.
    player.apply(Operation::ResidentCrowd(40.));
    window(&mut output, 0.0041924706, true);
    player.apply(Operation::Reset);
    window(&mut output, 0., false);

    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&164u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&22_050u32.to_le_bytes());
    wav.extend_from_slice(&44_100u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&128u32.to_le_bytes());
    for _ in 0..64 {
        wav.extend_from_slice(&16_384i16.to_le_bytes());
    }
    std::fs::write(directory.join("WAVCRASH2.WAV"), wav).unwrap();
    player.assets.coast = load_coast(&directory);
    assert_eq!(player.assets.coast.as_deref(), Some([0.5; 64].as_slice()));
    player.apply(Operation::Coast { camera_z: -300. });
    window(&mut output, 0.012955972, true);
    window(&mut output, 0.012955972, false); // Loops beyond the WAV's 64 samples.
    std::fs::remove_dir_all(directory).unwrap();
}

fn city_inputs(tick: u32) -> CityInputs {
    let mut categories = [[0; 0x17c]; 52];
    for category in &mut categories {
        category[4] = 1;
        category[0xe0..0xe4].copy_from_slice(&1000f32.to_le_bytes());
        category[0xe4..0xe8].copy_from_slice(&1000f32.to_le_bytes());
    }
    // A nearby factory and resident category let the frame update both beds.
    categories[2][0xe0..0xe4].copy_from_slice(&0f32.to_le_bytes());
    categories[2][0xe4..0xe8].copy_from_slice(&(-300f32).to_le_bytes());
    categories[2][0x88..0x8c].copy_from_slice(&200i32.to_le_bytes());
    let mut construction = [[None; 2]; 4];
    insert_construction_source(&mut construction, 0, [0., 40., -300.], [0., 40., -300.]).unwrap();
    CityInputs {
        reciprocal_sum: 0.1,
        nearest: [[101f32.to_bits(), 0, 0]; 3],
        categories,
        construction,
        hour: 8,
        tick,
        previous_tick: 100,
        initial_random_state: 205,
    }
}

#[test]
fn city_frame_keeps_construction_during_pause_and_repositions_before_resume() {
    let (mut player, _, mut output) = owner();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut host = crate::live_host::Host::new([0., 40., -300.]);
    host.transport_audio = Some(sender.clone());
    host.set_running(true);
    submit(
        &Some(sender.clone()),
        Operation::City(Box::new(city_inputs(101))),
    )
    .unwrap();
    player.drain_requests(Some(&receiver));
    assert!(player.construction[0].is_none()); // Receiving inputs does not service a frame.
    player.service_frame([0., 40., -300.]);
    let sink = player.construction[0].as_ref().unwrap();
    let retained = &**sink as *const rodio::Sink;
    let near_gain = sink.volume();
    assert!(near_gain > 0.);
    assert!(player.coast.as_ref().unwrap().volume() > 0.);
    assert!(player.resident_crowd.as_ref().unwrap().volume() > 0.);
    assert!(player.block_sinks[2].as_ref().unwrap().volume() > 0.);
    let pcm: Vec<_> = output.by_ref().take(4096).collect();
    assert!(pcm[2048..].iter().all(|sample| *sample > 0.));
    let position = sink.get_pos();

    host.set_running(false);
    assert!(matches!(
        receiver.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    for camera in [[200., 40., -300.], [400., 40., 0.], [0., 40., -300.]] {
        player.drain_requests(Some(&receiver));
        player.service_frame(camera);
        let sink = player.construction[0].as_ref().unwrap();
        assert_eq!(&**sink as *const rodio::Sink, retained);
        if camera[0] == 200. {
            assert!(sink.volume() > 0. && sink.volume() < near_gain);
        } else if camera[0] == 400. {
            assert_eq!(sink.volume(), 0.);
            assert_eq!(player.coast.as_ref().unwrap().volume(), 0.);
            assert!(player.resident_crowd.is_none());
            assert!(player.block_sinks[2].is_none());
        } else {
            assert_eq!(sink.volume(), near_gain);
            assert!(player.coast.as_ref().unwrap().volume() > 0.);
        }
        output.by_ref().take(4096).for_each(drop);
        assert_eq!(player.city.as_ref().unwrap().tick, 101);
    }
    assert!(player.construction[0].as_ref().unwrap().get_pos() > position);
    host.set_running(true);
    let mut finished = city_inputs(102);
    finished.construction = [[None; 2]; 4];
    submit(&Some(sender), Operation::City(Box::new(finished))).unwrap();
    player.drain_requests(Some(&receiver));
    assert!(player.construction[0].is_some());
    player.service_frame([0., 40., -300.]);
    assert!(player.construction[0].is_none());
    player.apply(Operation::Reset);
    player.service_frame([0., 40., -300.]);
    assert!(player.city.is_none() && player.coast.is_none());
}

#[test]
fn city_frame_render_frequency_preserves_simulation_inputs_and_shared_park_delta() {
    let run = |renders_per_tick| {
        let (mut player, _, mut output) = owner();
        let (sender, receiver) = std::sync::mpsc::channel();
        for tick in 101..=110 {
            let input = city_inputs(tick);
            let categories = input.categories;
            let construction = input.construction;
            submit(&Some(sender.clone()), Operation::City(Box::new(input))).unwrap();
            player.drain_requests(Some(&receiver));
            for _ in 0..renders_per_tick {
                // Extra request polls must not consume a simulation delta.
                player.drain_requests(Some(&receiver));
                player.service_frame([0., 40., -300.]);
                let retained = player.city.as_ref().unwrap();
                assert_eq!(retained.categories, categories);
                assert_eq!(retained.construction, construction);
                assert_eq!(retained.reciprocal_sum, 0.1);
                assert_eq!(retained.nearest, [[101f32.to_bits(), 0, 0]; 3]);
                assert_eq!(retained.previous_tick, tick);
            }
            output.by_ref().take(4096).for_each(drop);
        }
        player.block_controller.take().unwrap()
    };
    let every_tick = run(1);
    assert_eq!(run(2), every_tick);
    assert_eq!(run(5), every_tick);

    // Several simulation updates can arrive before the next rendered frame.
    // Keep the previous frame tick, not the immediately preceding producer tick.
    let (mut player, _, _) = owner();
    for tick in 101..=110 {
        let mut input = city_inputs(tick);
        input.previous_tick = tick - 1;
        player.apply(Operation::City(Box::new(input)));
    }
    assert_eq!(player.city.as_ref().unwrap().previous_tick, 100);
    player.service_frame([0., 40., -300.]);
    assert_eq!(player.block_controller.as_ref().unwrap(), &every_tick);
}

#[test]
fn train_route_dwell_departure_and_end_reach_player_with_retained_cursor() {
    use openmt2001::traffic::{HourlyEffects, RouteGeometry, scene, spawn_route, train_motion};
    let (mut player, audio, mut output) = owner();
    player.assets.train_toot.fill(0.);
    let mut scene = scene::Scene {
        hourly: HourlyEffects {
            tier: 0,
            random: [1, 0x343fd, 0x269ec3],
            model_choice: 0,
            variant: 0,
            auxiliaries: [[0; 300]; 2],
            effects: [[0; 20]; 9],
            models: [0; 4],
        },
        aircraft: [[0; 304]; 8],
        trains: [[0; 60]; 10],
        environment: scene::Environment::default(),
        first_frame: false,
    };
    let (sender, receiver) = mpsc::channel();
    let sender = Some(sender);
    let mut events = Vec::new();
    spawn_route(
        &mut scene.trains,
        0,
        0,
        0,
        |_| {
            Ok(RouteGeometry {
                start: [0.; 3],
                end: [200., 0., 0.],
            })
        },
        [0.; 3],
        |call, _| {
            events.push(call.id);
            submit(&sender, Operation::Route(call))
        },
    )
    .unwrap();
    player.drain_requests(Some(&receiver));
    let constants = train_motion::Constants {
        speed_scale: 50.,
        step: 0.06,
        zero: 0.,
        max_distance: 2.,
        origin: [0.; 3],
    };
    let mut identity = None;
    let mut dwell_position = std::time::Duration::ZERO;
    let mut ended = false;
    for tick in 1..2000 {
        let mut event = None;
        train_motion::advance(
            0,
            &mut scene.trains[0],
            constants,
            &mut train_motion::Callbacks {
                audio: &mut |call, _| {
                    event = Some(call.kind);
                    events.push(call.kind);
                    submit(
                        &sender,
                        Operation::Route(RouteSound {
                            id: call.kind,
                            distance: f32::from_bits(call.arguments[1]),
                            arguments: [0; 2],
                        }),
                    )
                },
            },
        )
        .unwrap();
        player.drain_requests(Some(&receiver));
        if event == Some(11) {
            let sink = player.train_motion.as_ref().unwrap();
            assert_eq!(sink.volume(), 0.);
            assert_eq!(Some(&**sink as *const rodio::Sink), identity);
            let before = sink.get_pos();
            window(&mut output, 0., false);
            dwell_position = player.train_motion.as_ref().unwrap().get_pos();
            assert!(
                dwell_position > before,
                "dwell mutes without pausing or rewinding"
            );
        } else if event == Some(12) {
            let sink = player.train_motion.as_ref().unwrap();
            assert!(sink.volume() > 0., "departure runs the common level tail");
            assert!(sink.get_pos() > dwell_position);
        } else if event == Some(9) {
            assert!(player.train_motion.is_none());
            assert!(scene.trains.iter().all(|row| row[..4] == [0; 4]));
            ended = true;
            break;
        }
        scene
            .proximity_sounds(&mut { tick }, &mut [0.; 3], |call, _, _, _| {
                submit(&sender, Operation::Route(call))
            })
            .unwrap();
        player.drain_requests(Some(&receiver));
        if let Some(sink) = &player.train_motion {
            let retained = &**sink as *const rodio::Sink;
            assert_eq!(*identity.get_or_insert(retained), retained);
        }
        output.by_ref().take(128).for_each(drop);
    }
    assert!(
        ended,
        "train did not finish its route: events={events:?}, row={:?}",
        scene.trains[0]
    );
    assert_eq!(events, [8, 11, 12, 9]);
    window(&mut output, 0., false);
    // No active train produces another proximity request. The UI still drains
    // requests and services retained city inputs on subsequent rendered frames.
    scene
        .proximity_sounds(&mut 2001, &mut [0.; 3], |_, _, _, _| {
            panic!("last train is gone")
        })
        .unwrap();
    player.apply(Operation::City(Box::new(city_inputs(2001))));
    player.drain_requests(Some(&receiver));
    player.service_frame([0., 40., -300.]);
    assert!(player.train_motion.is_none());
    assert!(player.coast.as_ref().unwrap().volume() > 0.);
    player.service_frame([0., 40., 0.]);
    assert_eq!(
        player.coast.as_ref().unwrap().volume(),
        audio.source_gain(0)
    );
    assert!(output.by_ref().take(4096).any(|sample| sample > 0.));
}

#[test]
fn train_normal_event_range_updates_toot_and_finishes_far_cleanup() {
    let (mut player, audio, mut output) = owner();
    player.assets.train_toot = vec![0.5; 128];
    player.apply(route(8));
    output.by_ref().take(4096).for_each(drop);
    assert!(player.train_toot.as_ref().unwrap().empty());
    player.apply(route(11));
    assert!(
        player.train_toot.is_none(),
        "dwell still retires a finished toot"
    );
    player.assets.train_toot = vec![0.5; 65536];
    player.apply(route(8));
    for id in 7..=18 {
        if id == 8 {
            continue;
        }
        player.apply(Operation::Route(RouteSound {
            id,
            distance: 100.,
            arguments: [0; 2],
        }));
        assert_eq!(
            player.train_toot.as_ref().unwrap().volume(),
            audio.source_gain(64)
        );
    }
    player.apply(route(7));
    player.apply(Operation::Route(RouteSound {
        id: 9,
        distance: 700.,
        arguments: [0; 2],
    }));
    assert!(player.train_motion.is_none());
    assert_eq!(player.train_toot.as_ref().unwrap().volume(), 0.);
    output.by_ref().take(70000).for_each(drop);
    player.apply(Operation::Route(RouteSound {
        id: 18,
        distance: 700.,
        arguments: [0; 2],
    }));
    assert!(
        player.train_toot.is_none(),
        "far calls retire finished toots too"
    );
}

#[test]
fn ship_unique_fallback_loses_handle_without_changing_or_stopping_active_voice() {
    for exit_mode in [0, 1, 3] {
        let (mut player, audio, mut output) = owner();
        player.assets.ship_horn.fill(0.);
        player.assets.block_factory.fill(0.);
        let ship = |distance| Operation::Environment {
            call: EnvironmentCall::Ship(1),
            distance,
        };
        player.apply(ship(0.));
        let gain = audio.source_gain(99);
        window(&mut output, gain * 0.5, true);
        let first_position = player.ship_engine.as_ref().unwrap().get_pos();
        let voice = player.ship_engine.as_ref().unwrap().clone();
        let horn = &**player.ship_horn.as_ref().unwrap() as *const rodio::Sink;
        for distance in [100., 400., 800., 0.] {
            player.apply(ship(distance));
            assert!(
                player.ship_engine.is_none(),
                "unique request returned no handle"
            );
            assert_eq!(voice.volume(), gain, "distance cannot update a lost handle");
            assert!(!voice.empty());
            assert_eq!(
                &**player.ship_horn.as_ref().unwrap() as *const rodio::Sink,
                horn
            );
            assert_eq!(
                player.ship_horn.as_ref().unwrap().volume(),
                audio.source_gain(positional_level(distance, 800.))
            );
            window(&mut output, gain * 0.5, false);
        }
        assert!(voice.get_pos() > first_position);
        assert_eq!(player.pending_ship_random, 5);
        // Apply the pending draws through the actual block consumer and compare
        // the whole controller, including the shared RNG, to five ship draws.
        let snapshot = block_audio::Snapshot {
            centers: [None; 52],
            camera: [0.; 3],
            hour: 0,
            tick: 100,
            previous_tick: 100,
            initial_random_state: 205,
        };
        player.apply(Operation::Blocks(Box::new(snapshot)));
        let mut expected = block_audio::Controller::new(205, 100);
        for _ in 0..5 {
            expected.ship_random();
        }
        expected.advance(snapshot, 0, [false; 52]);
        assert_eq!(player.block_controller.as_ref().unwrap(), &expected);
        assert_eq!(player.pending_ship_random, 0);
        player.set_interface_mode(exit_mode);
        player.apply(ship(if exit_mode == 0 { 801. } else { 0. }));
        assert!(player.ship_engine.is_none() && player.ship_horn.is_none());
        assert_eq!(
            player.block_controller.as_ref().unwrap(),
            &expected,
            "rejected calls do not draw RNG"
        );
        window(&mut output, gain * 0.5, false);
        player.set_interface_mode(0);
        player.apply(ship(400.));
        expected.ship_random();
        assert_eq!(player.block_controller.as_ref().unwrap(), &expected);
        assert!(
            player.ship_engine.is_none(),
            "returning still finds the unique sample active"
        );
        window(&mut output, gain * 0.5, false);
    }
}

#[test]
fn ship_leaving_before_suppression_releases_returned_engine_handle() {
    let (mut player, _, mut output) = owner();
    player.apply(Operation::Environment {
        call: EnvironmentCall::Ship(0),
        distance: 0.,
    });
    window(&mut output, 0.034197945, true);
    player.apply(Operation::Environment {
        call: EnvironmentCall::Ship(0),
        distance: 801.,
    });
    assert!(player.ship_engine.is_none() && player.ship_horn.is_none());
    window(&mut output, 0., false);
    player.apply(Operation::Environment {
        call: EnvironmentCall::Ship(0),
        distance: 0.,
    });
    assert!(player.ship_engine.is_some());
    window(&mut output, 0.034197945, true);
}

#[test]
fn ship_finished_horn_is_not_retriggered_by_admitted_updates() {
    let (mut player, _, mut output) = owner();
    player.assets.ship_engine.fill(0.);
    player.assets.ship_horn = vec![0.5; 128];
    let ship = Operation::Environment {
        call: EnvironmentCall::Ship(1),
        distance: 0.,
    };
    player.apply(ship.clone());
    let horn = &**player.ship_horn.as_ref().unwrap() as *const rodio::Sink;
    output.by_ref().take(4096).for_each(drop);
    assert!(player.ship_horn.as_ref().unwrap().empty());
    for _ in 0..32 {
        player.apply(ship.clone());
        assert_eq!(
            &**player.ship_horn.as_ref().unwrap() as *const rodio::Sink,
            horn
        );
        assert!(player.ship_horn.as_ref().unwrap().empty());
    }
    assert_eq!(player.pending_ship_random, 33);
    window(&mut output, 0., false);
}

#[test]
fn voice_admission_factory_categories_retain_only_one_shared_loop() {
    let (mut player, _, mut output) = owner();
    let mut centers = [Some([1000., 1000.]); 52];
    centers[2] = Some([0., 0.]);
    centers[35] = Some([0., 0.]);
    let snapshot = block_audio::Snapshot {
        centers,
        camera: [0., 40., 0.],
        hour: 16,
        tick: 100,
        previous_tick: 100,
        initial_random_state: 205,
    };
    for _ in 0..10 {
        player.apply(Operation::Blocks(Box::new(snapshot)));
    }
    assert!(player.block_sinks[2].is_some());
    assert!(
        player.block_sinks[35].is_none(),
        "suppressed unique request has no handle"
    );
    window(&mut output, 0.017098973, true);
    let mut moved = snapshot;
    moved.centers[2] = Some([1000., 1000.]);
    player.apply(Operation::Blocks(Box::new(moved)));
    assert!(player.block_sinks[2].is_none());
    assert!(
        player.block_sinks[35].is_some(),
        "released loop can be admitted by another category"
    );
    window(&mut output, 0.017098973, false);
}

#[test]
fn voice_admission_cached_motor_group_has_four_cells_and_keeps_earlier_equal_age() {
    let (_, shared, _) = owner();
    let mut players = Vec::new();
    let mut handles = Vec::new();
    let mut outputs = Vec::new();
    let mut nearest = [[101f32.to_bits(), 0, 0]; 3];
    nearest[0] = [10f32.to_bits(), 0, 1f32.to_bits()];
    for _ in 0..4 {
        let (mut player, _, mut output) = owner();
        player.bind_audio(shared.clone());
        let mixer = player.bus.combined_mixer().cloned();
        player.update_road_motors(nearest, mixer.as_ref());
        player.update_road_motors(nearest, mixer.as_ref());
        window(&mut output, 0.008034706, true);
        handles.push(player.road_motors[0].as_ref().unwrap().clone());
        outputs.push(output);
        players.push(player);
    }
    shared.service_effects();
    let (mut fifth, _, output) = owner();
    fifth.bind_audio(shared);
    let mixer = fifth.bus.combined_mixer().cloned();
    fifth.update_road_motors(nearest, mixer.as_ref());
    assert!(
        handles[0].empty(),
        "cached equal ages replace the first cell"
    );
    assert!(handles[1..].iter().all(|handle| !handle.empty()));
    assert!(!fifth.road_motors[0].as_ref().unwrap().empty());
    outputs.push(output);
    // Replacement stops the displaced backend, not merely its bookkeeping.
    window(&mut outputs[0], 0., false);
    window(&mut outputs[1], 0.008034706, false);
}

#[test]
fn voice_admission_repeated_train_toot_returns_no_handle_and_keeps_existing_pcm() {
    let (mut player, _, mut output) = owner();
    player.apply(route(8));
    window(&mut output, 0.010976654, true);
    let voice = player.train_toot.as_ref().unwrap().clone();
    player.set_level(0);
    player.apply(route(8));
    assert!(player.train_toot.is_none());
    assert!(!voice.empty());
    window(&mut output, 0.010976654, false);
    player.apply(Operation::Reset);
    assert!(voice.empty());
    window(&mut output, 0., false);
}
