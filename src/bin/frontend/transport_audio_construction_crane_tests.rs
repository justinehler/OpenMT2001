#[test]
fn construction_pending_pair_plays_one_fallback_bed_per_frame() {
    use openmt2001::pending_world::{Controls, Event, State};

    let (mut player, _, _) = owner();
    let mut pending = State::default();
    let mut buildings = [[0; 0x394]; 2];
    for (index, building) in buildings.iter_mut().enumerate() {
        building[..4].copy_from_slice(&(index as u32).to_le_bytes());
        building[8] = 1;
        building[0x58..0x5c].copy_from_slice(&(40. + index as f32 * 40.).to_le_bytes());
        pending.construct(0, building, 0, 1).unwrap();
    }
    let mut groups = [[None; 2]; 4];
    pending
        .advance(
            &mut buildings,
            &mut Controls {
                network_enabled: false,
                network_override: false,
                effect_model_reference: 0,
            },
            |event, _, buildings, _| {
                let Event::Progress { group, building } = event else {
                    panic!("unexpected pending event: {event:?}");
                };
                let position = std::array::from_fn(|axis| {
                    let at = 0x58 + axis * 4;
                    f32::from_le_bytes(buildings[building][at..at + 4].try_into().unwrap())
                });
                insert_construction_source(&mut groups, group, position, [0.; 3])?;
                Ok(0)
            },
        )
        .unwrap();
    assert_eq!(groups[0].map(|source| source.unwrap().distance), [40., 80.]);
    let mut city = city_inputs(101);
    city.construction = groups;
    player.apply(Operation::City(Box::new(city)));
    for _ in 0..2 {
        player.service_frame([0.; 3]);
        assert_eq!(player.construction.iter().flatten().count(), 1);
        let sink = player.construction[0].as_ref().unwrap();
        assert_eq!(sink.volume(), player.audio.source_gain(75));
    }
}

#[test]
fn construction_fallback_includes_boundary_and_stops_each_phase() {
    let (mut player, _, _) = owner();
    let source = |distance| {
        Some(ConstructionSource {
            distance,
            position: [distance, 0., 0.],
        })
    };
    // An admitted boundary source gets a retained (zero-gain) voice. A
    // second, nearer source must not be visited, even in another phase.
    player.apply(Operation::Construction([[source(250.), source(20.)]; 4]));
    for phase in player.construction.as_chunks::<2>().0 {
        assert_eq!(phase[0].as_ref().unwrap().volume(), 0.);
        assert!(phase[1].is_none());
    }
    player.apply(Operation::Construction([[None; 2]; 4]));
    assert!(player.construction.iter().all(Option::is_none));
}

#[test]
fn construction_fallback_failed_allocation_does_not_try_next_candidate() {
    let (mut player, _, _) = owner();
    let source = Some(ConstructionSource {
        distance: 20.,
        position: [20., 0., 0.],
    });
    // Seed only the second voice. With an unavailable allocator the first
    // admitted candidate still ends the phase, leaving this voice untouched.
    let mut groups = [[None; 2]; 4];
    groups[0][1] = source;
    player.apply(Operation::Construction(groups));
    player.construction[1].as_ref().unwrap().set_volume(0.123);
    groups[0][0] = source;
    player.construction_fallback(groups, None);
    assert!(player.construction[0].is_none());
    assert_eq!(player.construction[1].as_ref().unwrap().volume(), 0.123);
    // A zero-count phase releases both slots, including the unvisited one.
    player.apply(Operation::Construction([[None; 2]; 4]));
    assert!(player.construction.iter().all(Option::is_none));
}

#[test]
fn construction_fallback_mutes_far_handles_and_skips_to_near_source() {
    let (mut player, _, _) = owner();
    let source = |distance| {
        Some(ConstructionSource {
            distance,
            position: [distance, 0., 0.],
        })
    };
    let mut groups = [[None; 2]; 4];
    groups[0][1] = source(20.);
    player.apply(Operation::Construction(groups));
    groups[0][0] = source(10.);
    player.apply(Operation::Construction(groups));
    let retained: [_; 2] = std::array::from_fn(|slot| {
        &**player.construction[slot].as_ref().unwrap() as *const rodio::Sink
    });
    groups[0] = [source(250.001), source(400.)];
    player.apply(Operation::Construction(groups));
    for (slot, retained) in retained.into_iter().enumerate() {
        let sink = player.construction[slot].as_ref().unwrap();
        assert_eq!(&**sink as *const rodio::Sink, retained);
        assert_eq!(sink.volume(), 0.);
    }
    groups[0][1] = source(20.);
    player.apply(Operation::Construction(groups));
    assert_eq!(player.construction[0].as_ref().unwrap().volume(), 0.);
    assert!(player.construction[1].as_ref().unwrap().volume() > 0.);
}

fn crane_call(player: &mut Player, call: EnvironmentCall, distance: f32) {
    player.apply(Operation::Environment {
        call,
        distance,
        position: [distance, 0., 0.],
    });
}

fn assert_crane_beds_cleared(player: &Player) {
    assert!(player.crane_start.is_none());
    assert!(player.crane_lift.is_none());
    assert!(player.crane_loop.is_none());
}

#[test]
fn crane_strategic_cues_clear_beds_before_dispatch_and_preserve_end() {
    for mode in [1, 3] {
        // Exercise creation, release and unknown cue IDs: cleanup precedes
        // all dispatch, rather than being special to a particular new bed.
        for cue in [27, 28, 29, 30, 31, 32, 99] {
            let (mut player, _, _) = owner();
            for id in [27, 28, 29, 31] {
                crane_call(&mut player, EnvironmentCall::Cue(id), 50.);
            }
            crane_call(&mut player, EnvironmentCall::Refresh, 50.);
            let end = &**player.crane_end.as_ref().unwrap() as *const rodio::Sink;
            let end_gain = player.crane_end.as_ref().unwrap().volume();
            player.set_interface_mode(mode);
            crane_call(&mut player, EnvironmentCall::Cue(cue), 50.);
            assert_crane_beds_cleared(&player);
            assert_eq!(
                &**player.crane_end.as_ref().unwrap() as *const rodio::Sink,
                end
            );
            assert_eq!(player.crane_end.as_ref().unwrap().volume(), end_gain);
            crane_call(&mut player, EnvironmentCall::Refresh, 20.);
            assert_crane_beds_cleared(&player);
            assert_eq!(
                &**player.crane_end.as_ref().unwrap() as *const rodio::Sink,
                end
            );
            assert!(player.crane_end.as_ref().unwrap().volume() > end_gain);
            player.set_interface_mode(0);
            crane_call(&mut player, EnvironmentCall::Refresh, 20.);
            assert_crane_beds_cleared(&player);
            crane_call(&mut player, EnvironmentCall::Cue(29), 20.);
            assert!(player.crane_lift.is_some());
        }
    }
}

#[test]
fn crane_sequence_resumes_from_next_cue_after_strategic_view() {
    for mode in [1, 3] {
        let (mut player, _, _) = owner();
        crane_call(&mut player, EnvironmentCall::Cue(27), 50.);
        // Normal 44ec50 counter boundaries and cue ordering.
        for (count, cues) in [
            (2, &[29][..]),
            (300, &[31, 30][..]),
            (800, &[29, 32][..]),
            (1400, &[30, 31][..]),
            (1790, &[32, 28][..]),
        ] {
            if count == 300 {
                player.set_interface_mode(mode);
            } else if count == 800 {
                player.set_interface_mode(0);
            }
            for &cue in cues {
                crane_call(&mut player, EnvironmentCall::Cue(cue), 50.);
            }
            crane_call(&mut player, EnvironmentCall::Refresh, 50.);
            assert_eq!(player.crane_start.is_some(), count == 2);
            assert_eq!(player.crane_lift.is_some(), matches!(count, 2 | 800));
            assert_eq!(player.crane_loop.is_some(), count == 1400);
            assert_eq!(player.crane_end.is_some(), count == 1790);
        }
    }
}

#[test]
fn crane_far_refresh_still_clears_only_start_lift_and_loop() {
    let (mut player, _, _) = owner();
    for id in [27, 28, 29, 31] {
        crane_call(&mut player, EnvironmentCall::Cue(id), 50.);
    }
    let end = &**player.crane_end.as_ref().unwrap() as *const rodio::Sink;
    crane_call(&mut player, EnvironmentCall::Refresh, 400.);
    assert_crane_beds_cleared(&player);
    assert_eq!(
        &**player.crane_end.as_ref().unwrap() as *const rodio::Sink,
        end
    );
    crane_call(&mut player, EnvironmentCall::Refresh, 50.);
    assert_crane_beds_cleared(&player);
}
