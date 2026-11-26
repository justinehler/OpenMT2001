//! Proposed boundary only; no production queue/Host recovery wiring.
use openmt2001::{
    outcome_notice::{self, Call, PresentationError, UnresolvedOutcome, World},
    pe::PeImage,
    player_manager::Player,
    scenario_objective::State,
};

// Header-only PE, with no image-backed data. Rejected commands must never
// reach portrait/resource reads or invoke any UI callback at this boundary.
fn header_only_pe() -> Vec<u8> {
    let mut bytes = vec![0; 184];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&128u32.to_le_bytes());
    bytes[128..132].copy_from_slice(b"PE\0\0");
    bytes[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[148..150].copy_from_slice(&32u16.to_le_bytes());
    bytes[152..154].copy_from_slice(&0x10bu16.to_le_bytes());
    bytes[180..184].copy_from_slice(&0x400000u32.to_le_bytes());
    bytes
}

#[test]
fn native_tie_is_typed_before_any_ui_effect() {
    let bytes = header_only_pe();
    let image = PeImage::parse(&bytes).unwrap();
    let players: Vec<_> = (0..2)
        .map(|_| Player {
            reference: 0,
            bytes: vec![],
            auxiliaries: vec![],
        })
        .collect();
    let objective = State {
        kind: 4,
        target: 1940.,
        ..State::default()
    };
    let world = World {
        image: &image,
        players: &players,
        current_reference: 0,
        objective: &objective,
    };
    let result = outcome_notice::present_checked(-1, 2, &world, |call| {
        panic!("unresolved tie emitted UI {call:?}")
    });
    assert_eq!(
        result,
        Err(PresentationError::Unresolved(UnresolvedOutcome {
            winner: -1,
            reason: 2,
            roster_len: 2,
            objective_kind: 4,
            target_bits: 1940f32.to_bits(),
        }))
    );
    // No fabricated winner, changed target or evaluator state.
    assert_eq!(objective.reached, [0; 6]);
    assert_eq!(objective.excess, [0; 6]);
}

#[test]
fn invalid_participants_preserve_raw_descriptor_instead_of_aliasing_slots() {
    let objective = State {
        kind: 5,
        target: 6250.,
        ..State::default()
    };
    for (winner, roster_len) in [(-2, 2), (2, 2), (0, 0), (6, 7)] {
        let error = outcome_notice::preflight(winner, 2, roster_len, &objective).unwrap_err();
        assert_eq!(
            (error.winner, error.reason, error.roster_len),
            (winner, 2, roster_len)
        );
        assert_eq!(
            (error.objective_kind, error.target_bits),
            (5, 6250f32.to_bits())
        );
    }
}

#[test]
fn valid_participant_delegates_to_existing_presentation_without_relabeling() {
    let bytes = header_only_pe();
    let image = PeImage::parse(&bytes).unwrap();
    let players = [Player {
        reference: 0,
        bytes: vec![],
        auxiliaries: vec![],
    }];
    let objective = State::default();
    let world = World {
        image: &image,
        players: &players,
        current_reference: 0,
        objective: &objective,
    };
    assert_eq!(outcome_notice::preflight(0, 4, 1, &objective), Ok(0));
    // Stop at the real first callback; this is delegation/error propagation,
    // not a claim that this deliberately incomplete World can show a dialog.
    let mut calls = vec![];
    let result = outcome_notice::present_checked(0, 4, &world, |call| {
        calls.push(call);
        Err("controlled first-boundary refusal".into())
    });
    assert_eq!(calls, [Call::Show]);
    assert_eq!(
        result,
        Err(PresentationError::Boundary(
            "controlled first-boundary refusal".into()
        ))
    );
}

#[test]
fn local_queue_preserves_unresolved_descriptor_when_preflight_refusal_propagates() {
    let mut queue = openmt2001::local_commands::Queue::default();
    queue
        .submit((-1i32, 2u32), 100, true, |_| {
            panic!("early outcome dispatch")
        })
        .unwrap();
    let objective = State {
        kind: 4,
        target: 1940.,
        ..State::default()
    };
    let mut unresolved = None;
    let result = queue.drain(&mut [125, 0], |&(winner, reason)| {
        match outcome_notice::preflight(winner, reason, 2, &objective) {
            Ok(_) => panic!("native tie acquired a fabricated participant"),
            Err(error) => {
                unresolved = Some(error);
                // Queue error transport is String today. Preserve the typed
                // descriptor separately; a future Host adapter must do both.
                Err("unresolved outcome; disposition pending".into())
            }
        }
    });
    assert_eq!(
        result,
        Err("unresolved outcome; disposition pending".into())
    );
    assert_eq!(unresolved.unwrap().winner, -1);
    let retained = queue.snapshot().unwrap();
    assert_eq!(retained.len(), 1);
    assert!(matches!(
        retained[0].0,
        openmt2001::scheduler::ClockDomain::Total
    ));
    assert_eq!(
        (retained[0].1, retained[0].2, retained[0].3),
        (125, 125, false)
    );
    assert_eq!(*retained[0].4, (-1, 2));
}
