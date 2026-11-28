fn spatial_owner() -> (Player, rodio::mixer::MixerSource) {
    let (mut player, _, _) = owner();
    player.assets.spatial_enabled = true;
    player.set_level(100);
    let (mixer, output) = rodio::mixer::mixer(2, 22_050);
    mixer.add(rodio::source::Zero::new(2, 22_050));
    player.bus.connect_test_output(&mixer, 2, 22_050);
    (player, output)
}

fn current_listener(player: &mut Player, eye: [f32; 3], target: [f32; 3]) {
    crate::intro::camera_live::context::publish_listener(
        player,
        crate::intro::camera_live::owner::Pose {
            eye: eye.map(f32::to_bits),
            target: target.map(f32::to_bits),
            // Audio uses world up even if the camera rolls.
            up: [1f32, 0., 0.].map(f32::to_bits),
        },
    );
}
fn stereo_window(output: &mut rodio::mixer::MixerSource, expected: [f32; 2]) {
    let samples: Vec<_> = output.by_ref().take(4096).collect();
    assert_eq!(samples.len(), 4096);
    for frame in samples[2048..].as_chunks::<2>().0 {
        for (sample, expected_sample) in frame.iter().zip(expected) {
            assert!(
                (*sample - expected_sample).abs() < 1e-6,
                "got {frame:?}, expected {expected:?}"
            );
        }
    }
}

#[test]
fn spatial_current_camera_listener_rotates_and_moves_a_retained_source() {
    let (mut player, mut output) = spatial_owner();
    current_listener(&mut player, [0.; 3], [0., 0., 10.]);
    assert_eq!(player.listener.forward, [0., 0., 1.]);
    let mut groups = [[None; 2]; 4];
    insert_construction_source(&mut groups, 2, [160., 0., 0.], [0.; 3]).unwrap();
    player.apply(Operation::Construction(groups));
    let sink = player.construction[4].as_ref().unwrap().clone();
    // min / (min + rolloff*(distance-min)) = 80/(80+3*80).
    assert_eq!(sink.spatial_gains(), [0., 0.25]);
    let nominal = player.audio.source_gain(70) * 0.5;
    stereo_window(&mut output, [0., nominal * 0.25]);
    current_listener(&mut player, [0.; 3], [0., 0., -20.]);
    assert_eq!(sink.spatial_gains(), [0.25, 0.]);
    stereo_window(&mut output, [nominal * 0.25, 0.]);
    current_listener(&mut player, [80., 0., 0.], [80., 0., 1.]);
    assert_eq!(player.listener_position(), [80., 0., 0.]);
    assert_eq!(sink.spatial_gains(), [0., 1.]);
    stereo_window(&mut output, [0., nominal]);
    // Listener updates affect even loops that receive no new simulation cue.
    current_listener(&mut player, [-91., 0., 0.], [-91., 0., 1.]);
    assert_eq!(sink.spatial_gains(), [0.; 2]);
    stereo_window(&mut output, [0.; 2]);
    assert!(!sink.empty(), "3D cutoff mutes without rewinding");
}

#[test]
fn spatial_frame_visits_both_construction_sources_and_mutes_far_candidates() {
    let (mut player, _) = spatial_owner();
    let mut city = city_inputs(100);
    city.categories = [[0; 0x17c]; 52];
    city.construction = [[None; 2]; 4];
    insert_construction_source(&mut city.construction, 0, [0., 0., 40.], [0.; 3]).unwrap();
    insert_construction_source(&mut city.construction, 0, [0., 0., 160.], [0.; 3]).unwrap();
    player.apply(Operation::City(Box::new(city)));
    player.service_frame([0.; 3]);
    assert_eq!(player.construction.iter().flatten().count(), 2);
    assert_eq!(
        player.construction[0].as_ref().unwrap().spatial_gains(),
        [1.; 2]
    );
    assert_eq!(
        player.construction[1].as_ref().unwrap().spatial_gains(),
        [0.25; 2]
    );
    current_listener(&mut player, [0., 0., -300.], [0., 0., -299.]);
    player.service_frame([0., 0., -300.]);
    for sink in player.construction.iter().flatten() {
        assert_eq!(sink.volume(), 0.);
        assert!(!sink.empty());
    }
}

#[test]
fn spatial_environment_keeps_ship_handles_and_uses_each_original_range() {
    let (mut player, _) = spatial_owner();
    current_listener(&mut player, [0.; 3], [0., 0., 1.]);
    let ship = |z| Operation::Environment {
        call: EnvironmentCall::Ship(1),
        distance: z,
        position: [0., 0., z],
    };
    player.apply(ship(170.));
    let engine = player.ship_engine.as_ref().unwrap().clone();
    assert_eq!(engine.spatial_gains(), [80. / 350.; 2]);
    assert_eq!(player.ship_horn.as_ref().unwrap().spatial_gains(), [1.; 2]);
    player.apply(ship(300.));
    assert!(std::ptr::eq(
        &**player.ship_engine.as_ref().unwrap(),
        &*engine
    ));
    assert_eq!(engine.spatial_gains(), [80. / 740.; 2]);
    player.apply(ship(301.));
    assert_eq!(engine.spatial_gains(), [0.; 2]);
    assert_eq!(
        player.ship_engine.as_ref().unwrap().volume(),
        player.audio.source_gain(100)
    );
    player.apply(ship(801.));
    assert!(player.ship_engine.is_none());
    assert!(player.ship_horn.is_none());
    player.apply(Operation::Environment {
        call: EnvironmentCall::Cue(29),
        distance: 160.,
        position: [0., 0., 160.],
    });
    let lift = player.crane_lift.as_ref().unwrap();
    assert_eq!(lift.spatial_gains(), [0.25; 2]);
    assert_eq!(lift.volume(), player.audio.source_gain(70));
    player.apply(Operation::Environment {
        call: EnvironmentCall::Refresh,
        distance: 160.,
        position: [0.; 3],
    });
    assert_eq!(
        player.crane_lift.as_ref().unwrap().volume(),
        player.audio.source_gain(70)
    );
    current_listener(&mut player, [0., 0., -41.], [0., 0., -40.]);
    assert_eq!(player.crane_lift.as_ref().unwrap().spatial_gains(), [0.; 2]);
}

#[test]
fn spatial_block_admission_and_nonspatial_transport_layers_stay_distinct() {
    let (mut player, _) = spatial_owner();
    current_listener(&mut player, [-160., 40., 0.], [-160., 40., 1.]);
    let mut snapshot = block(2, 16, 0);
    if let Operation::Blocks(snapshot) = &mut snapshot {
        snapshot.camera = [-160., 40., 0.];
    }
    player.apply(snapshot);
    let factory = player.block_sinks[2].as_ref().unwrap();
    assert_eq!(factory.volume(), player.audio.source_gain(100));
    assert_ne!(factory.spatial_gains(), [1.; 2]);
    player.apply(route(7));
    player.apply(Operation::Coast { camera_z: -300. });
    let train = player.train_motion.as_ref().unwrap().clone();
    let coast = player.coast.as_ref().unwrap().clone();
    let levels = [train.volume(), coast.volume()];
    current_listener(&mut player, [1000.; 3], [-1000.; 3]);
    assert_eq!(train.spatial_gains(), [1.; 2]);
    assert_eq!(coast.spatial_gains(), [1.; 2]);
    assert_eq!([train.volume(), coast.volume()], levels);
    // Admission remains the gameplay cutoff of 300, even though its 3D
    // buffer's max distance is 800.
    let mut far = block_audio::Snapshot {
        centers: [None; 52],
        camera: [1000.; 3],
        hour: 16,
        tick: 1,
        previous_tick: 0,
        initial_random_state: 1,
    };
    far.centers[2] = Some([0.; 2]);
    player.apply(Operation::Blocks(Box::new(far)));
    assert!(player.block_sinks[2].is_none());
    assert!(submit(&None, Operation::Reset).is_ok());
}

#[test]
fn spatial_loop_recomputes_channels_after_the_pcm_has_wrapped() {
    let (mut player, mut output) = spatial_owner();
    player.assets.construction_saw = vec![0.5; 8];
    current_listener(&mut player, [0.; 3], [0., 0., 1.]);
    let mut groups = [[None; 2]; 4];
    insert_construction_source(&mut groups, 2, [40., 0., 0.], [0.; 3]).unwrap();
    player.apply(Operation::Construction(groups));
    let level = player.audio.source_gain(70) * 0.5;
    stereo_window(&mut output, [0., level]);
    current_listener(&mut player, [0.; 3], [0., 0., -1.]);
    stereo_window(&mut output, [level, 0.]);
}

#[test]
fn spatial_gain_uses_full_3d_distance_and_normalized_camera_basis() {
    let listener = Listener::camera([0.; 3], [0., 0., 100.]);
    let gains = Emitter {
        position: [96., 0., 128.],
        min_distance: 80.,
        max_distance: 250.,
    }
    .gains(listener);
    // 3-4-5 triangle: distance 160, pan 0.6, attenuation 0.25.
    assert!((gains[0] - 0.1).abs() < 1e-6);
    assert_eq!(gains[1], 0.25);
    let elevated = Emitter {
        position: [0., 96., 128.],
        min_distance: 80.,
        max_distance: 250.,
    }
    .gains(listener);
    assert_eq!(elevated, [0.25; 2]);
}

#[test]
fn spatial_mono_output_preserves_sources_on_either_side() {
    let (mut player, _, mut output) = owner();
    player.assets.spatial_enabled = true;
    current_listener(&mut player, [0.; 3], [0., 0., 1.]);
    let mut groups = [[None; 2]; 4];
    insert_construction_source(&mut groups, 2, [40., 0., 0.], [0.; 3]).unwrap();
    player.apply(Operation::Construction(groups));
    let gain = player.audio.source_gain(70) * 0.5;
    window(&mut output, gain, true);
    current_listener(&mut player, [0.; 3], [0., 0., -1.]);
    window(&mut output, gain, false);
}
