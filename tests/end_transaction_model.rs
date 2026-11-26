//! Pure End state/progress/retry model only: no live Host, UI, teardown or I/O.
use openmt2001::{
    end_transaction::{
        Error, Identity, Origin, Pair, Phase, ResultGate, ReturnContext, ReturnTarget, Transaction,
    },
    level_catalog::{Catalog, Labels, Registration},
    outcome_notice::Sequence,
};
use std::path::Path;

fn catalog(ids: &[i32], selected: usize) -> Catalog {
    let mut catalog = Catalog::default();
    for &id in ids {
        catalog
            .register(
                Registration {
                    id,
                    directory: b"fixture".to_vec(),
                    labels: Labels::Literal {
                        title: vec![],
                        description: vec![],
                    },
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
    }
    catalog.selected = Some(selected);
    catalog.difficulty = 1;
    catalog
}

fn identity(catalog: &Catalog) -> Identity {
    Identity::capture(17, catalog, Some(Path::new("owned/profile-a"))).unwrap()
}

fn non_tutorial_route(transaction: &mut Transaction, live: &Identity, catalog: &Catalog) {
    transaction
        .acknowledge_pre_routing(live, catalog, None)
        .unwrap();
    transaction
        .acknowledge_route_presentation(live, catalog, ReturnContext::default(), |_, _| Ok(()))
        .unwrap();
}

#[test]
fn native_pair_controls_attempts_chosen_cup_and_actual_successor() {
    for (completed, secondary, attempts, awarded) in [
        (false, 1, 0, false),
        (true, 0, 1, false),
        (true, 2, 1, false),
        (true, -1, 1, false),
        (true, 1, 1, true),
    ] {
        let mut catalog = catalog(&[200003, 100001, 200004], 0);
        let live = identity(&catalog);
        let mut transaction = Transaction::new(
            live.clone(),
            Pair {
                completed,
                secondary,
            },
            Origin::Direct,
        );
        assert_eq!(
            transaction.acknowledge_tail(&live, &catalog),
            Err(Error::WrongPhase)
        );
        non_tutorial_route(&mut transaction, &live, &catalog);
        if completed {
            transaction
                .apply_non_tutorial_award(&live, &mut catalog)
                .unwrap();
        } else {
            assert_eq!(
                transaction.apply_non_tutorial_award(&live, &mut catalog),
                Err(Error::WrongAwardBranch)
            );
        }
        let selected = &catalog.levels[0];
        assert_eq!(selected.attempts(), attempts);
        assert_eq!(selected.completed(), [false, awarded, false]);
        assert!(!catalog.levels[1].unlocked(), "skip tutorial successor");
        assert_eq!(catalog.levels[2].unlocked(), awarded);
    }
}

#[test]
fn tutorial_completion_ignores_secondary_without_nontutorial_attempts() {
    let mut catalog = catalog(&[100001, 100002, 200003], 0);
    let live = identity(&catalog);
    let mut transaction = Transaction::new(
        live.clone(),
        Pair {
            completed: true,
            secondary: 2,
        },
        Origin::Tutorial,
    );
    assert_eq!(
        transaction
            .return_target(&live, &catalog, ReturnContext::default())
            .unwrap(),
        ReturnTarget::Panel(0x88d384)
    );
    transaction
        .acknowledge_pre_routing(&live, &catalog, None)
        .unwrap();
    let mut observations = 0;
    assert_eq!(
        transaction.acknowledge_route_presentation(
            &live,
            &catalog,
            ReturnContext::default(),
            |_, _| { panic!("Tutorial opened before native48dbd0 completion publication") }
        ),
        Err(Error::WrongPhase)
    );
    transaction
        .apply_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(transaction.phase(), Phase::TutorialAwardApplied);
    transaction
        .acknowledge_route_presentation(
            &live,
            &catalog,
            ReturnContext::default(),
            |target, observed| {
                observations += 1;
                assert_eq!(target, ReturnTarget::Panel(0x88d384));
                // Independent49c890 observer reads literal published completion bytes,
                // not a prediction of the model's next phase or the old tutorial test.
                assert_eq!(observed.levels[0].completed(), [false, true, false]);
                assert_eq!(observed.levels[0].attempts(), 0);
                assert_eq!(
                    observed
                        .levels
                        .iter()
                        .position(|level| level.tutorial() && level.completed() == [false; 3]),
                    Some(1)
                );
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(observations, 1);
    assert_eq!(transaction.phase(), Phase::RoutePresented);
    assert_eq!(
        transaction.apply_non_tutorial_award(&live, &mut catalog),
        Err(Error::WrongAwardBranch)
    );
    assert_eq!(catalog.levels[0].attempts(), 0);
    assert_eq!(catalog.levels[0].completed(), [false, true, false]);
    assert!(!catalog.levels[2].unlocked());
    transaction.acknowledge_tail(&live, &catalog).unwrap();
}

#[test]
fn graph_presentation_observes_preaward_state_before_nontutorial_progress() {
    let mut catalog = catalog(&[200003, 100001, 200004], 0);
    let live = identity(&catalog);
    let mut transaction = Transaction::new(
        live.clone(),
        Pair {
            completed: true,
            secondary: 1,
        },
        Origin::Direct,
    );
    transaction
        .acknowledge_pre_routing(&live, &catalog, None)
        .unwrap();
    assert_eq!(
        transaction.apply_non_tutorial_award(&live, &mut catalog),
        Err(Error::WrongPhase)
    );
    let mut observed = vec![];
    transaction
        .acknowledge_route_presentation(
            &live,
            &catalog,
            ReturnContext::default(),
            |target, catalog| {
                assert_eq!(target, ReturnTarget::Panel(0x8898e4));
                // Native4ba2d9 Graph initialization precedes4ba2e9 attempt store and
                //4ba302/4ba30c cup/unlock. This is a literal state observer at the
                // presentation boundary, not an expected value from Catalog::finish.
                observed.push((
                    catalog.levels[0].attempts(),
                    catalog.levels[0].completed(),
                    catalog.levels[2].unlocked(),
                ));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(observed, [(0, [false; 3], false)]);
    assert_eq!(transaction.phase(), Phase::RoutePresented);
    assert_eq!(
        transaction.acknowledge_tail(&live, &catalog),
        Err(Error::WrongPhase)
    );
    transaction
        .apply_non_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(transaction.phase(), Phase::NonTutorialAwardApplied);
    assert_eq!(catalog.levels[0].attempts(), 1);
    assert_eq!(catalog.levels[0].completed(), [false, true, false]);
    assert!(catalog.levels[2].unlocked());
    transaction
        .acknowledge_route_presentation(&live, &catalog, ReturnContext::default(), |_, _| {
            panic!("retry presented Graph twice")
        })
        .unwrap();
    transaction
        .apply_non_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(catalog.levels[0].attempts(), 1);
}

#[test]
fn tutorial_route_failure_retains_published_award_and_retries_only_presentation() {
    let mut catalog = catalog(&[100001, 100002], 0);
    let live = identity(&catalog);
    let mut transaction = Transaction::new(
        live.clone(),
        Pair {
            completed: true,
            secondary: 0,
        },
        Origin::Tutorial,
    );
    transaction
        .acknowledge_pre_routing(&live, &catalog, None)
        .unwrap();
    transaction
        .apply_tutorial_award(&live, &mut catalog)
        .unwrap();
    let published = catalog.levels.clone();
    assert_eq!(
        transaction.acknowledge_route_presentation(
            &live,
            &catalog,
            ReturnContext::default(),
            |_, observed| {
                assert_eq!(observed.levels[0].completed(), [false, true, false]);
                Err("retained route refusal".into())
            }
        ),
        Err(Error::Presentation("retained route refusal".into()))
    );
    assert_eq!(transaction.phase(), Phase::TutorialAwardApplied);
    assert_eq!(
        transaction.acknowledge_tail(&live, &catalog),
        Err(Error::WrongPhase)
    );
    transaction
        .apply_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(catalog.levels, published);
    transaction
        .acknowledge_route_presentation(
            &live,
            &catalog,
            ReturnContext::default(),
            |target, observed| {
                assert_eq!(target, ReturnTarget::Panel(0x88d384));
                assert_eq!(observed.levels[0].completed(), [false, true, false]);
                Ok(())
            },
        )
        .unwrap();
    transaction.acknowledge_tail(&live, &catalog).unwrap();
    assert_eq!(catalog.levels[0].attempts(), 0);
}

#[test]
fn result_requires_existing_request_finished_presentation_and_actual_pair() {
    let mut catalog = catalog(&[200003, 200004], 0);
    let live = identity(&catalog);
    let pair = Pair {
        completed: true,
        secondary: 1,
    };
    let mut transaction = Transaction::new(live.clone(), pair, Origin::Result(Sequence::Win));
    let context = ReturnContext::default();
    assert_eq!(
        transaction.acknowledge_pre_routing(&live, &catalog, None),
        Err(Error::ResultNotReady)
    );
    let mut gate = ResultGate {
        request: Sequence::Win,
        presentation_finished: false,
        authored_end: Some(pair),
    };
    assert_eq!(
        transaction.acknowledge_pre_routing(&live, &catalog, Some(&gate)),
        Err(Error::ResultNotReady)
    );
    gate.presentation_finished = true;
    gate.authored_end = Some(Pair {
        completed: true,
        secondary: 2,
    });
    assert_eq!(
        transaction.acknowledge_pre_routing(&live, &catalog, Some(&gate)),
        Err(Error::ResultEndMismatch)
    );
    gate.authored_end = Some(pair);
    transaction
        .acknowledge_pre_routing(&live, &catalog, Some(&gate))
        .unwrap();
    transaction
        .acknowledge_route_presentation(&live, &catalog, context, |target, observed| {
            assert_eq!(target, ReturnTarget::Panel(0x8898e4));
            assert_eq!(observed.levels[0].attempts(), 0);
            assert_eq!(observed.levels[0].completed(), [false; 3]);
            Ok(())
        })
        .unwrap();
    transaction
        .apply_non_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(catalog.levels[0].attempts(), 1);
    assert_eq!(catalog.levels[0].completed(), [false, true, false]);
}

#[test]
fn identity_rejects_profile_session_difficulty_and_duplicate_registration_changes() {
    let mut catalog = catalog(&[200003, 200003], 0);
    let live = identity(&catalog);
    let mut transaction = Transaction::new(
        live.clone(),
        Pair {
            completed: true,
            secondary: 1,
        },
        Origin::Direct,
    );
    for other in [
        Identity::capture(18, &catalog, Some(Path::new("owned/profile-a"))).unwrap(),
        Identity::capture(17, &catalog, Some(Path::new("owned/profile-b"))).unwrap(),
    ] {
        assert_eq!(
            transaction.apply_non_tutorial_award(&other, &mut catalog),
            Err(Error::IdentityChanged)
        );
    }
    catalog.difficulty = 2;
    assert_eq!(
        transaction.apply_non_tutorial_award(&live, &mut catalog),
        Err(Error::IdentityChanged)
    );
    catalog.difficulty = 1;
    catalog.selected = Some(1); // Same level ID, different registered row.
    assert_eq!(
        transaction.apply_non_tutorial_award(&live, &mut catalog),
        Err(Error::IdentityChanged)
    );
    assert!(
        catalog
            .levels
            .iter()
            .all(|l| l.attempts() == 0 && l.completed() == [false; 3])
    );
    assert_eq!(transaction.phase(), Phase::Captured);
}

#[test]
fn failed_persistence_retries_only_owned_write_and_completion_is_once_only() {
    let mut catalog = catalog(&[200003, 200004], 0);
    let live = identity(&catalog);
    let pair = Pair {
        completed: true,
        secondary: 1,
    };
    let mut transaction = Transaction::new(live.clone(), pair, Origin::Direct);
    assert_eq!(
        transaction.acknowledge_tail(&live, &catalog),
        Err(Error::WrongPhase)
    );
    non_tutorial_route(&mut transaction, &live, &catalog);
    transaction
        .apply_non_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(
        transaction.persist(&live, &catalog, |_, _| panic!("early persistence")),
        Err(Error::WrongPhase)
    );
    transaction.acknowledge_tail(&live, &catalog).unwrap();
    assert_eq!(
        transaction.persist(&live, &catalog, |_, path| {
            assert_eq!(path, Path::new("owned/profile-a"));
            Err("retained write failure".into())
        }),
        Err(Error::Persistence("retained write failure".into()))
    );
    assert_eq!(transaction.phase(), Phase::TailAcknowledged);
    transaction
        .apply_non_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(catalog.levels[0].attempts(), 1);
    let completion = transaction
        .persist(&live, &catalog, |saved, path| {
            assert_eq!(saved.levels[0].attempts(), 1);
            assert_eq!(path, Path::new("owned/profile-a"));
            Ok(())
        })
        .unwrap();
    assert_eq!(completion.identity, live);
    assert_eq!(completion.pair, pair);
    assert_eq!(completion.target, ReturnTarget::Panel(0x8898e4));
    assert_eq!(transaction.phase(), Phase::Complete);
    assert_eq!(
        transaction
            .persist(&live, &catalog, |_, _| panic!("duplicate write"))
            .unwrap(),
        completion
    );
    transaction
        .apply_non_tutorial_award(&live, &mut catalog)
        .unwrap();
    assert_eq!(catalog.levels[0].attempts(), 1);
}

#[test]
fn abort_return_targets_match_native_flags_without_fabricating_result() {
    for (id, context, expected) in [
        (
            100001,
            ReturnContext::default(),
            ReturnTarget::Panel(0x88d384),
        ),
        (
            200003,
            ReturnContext::default(),
            ReturnTarget::Panel(0x885e7c),
        ),
        (
            500001,
            ReturnContext::default(),
            ReturnTarget::Panel(0x886338),
        ),
        (
            300002,
            ReturnContext::default(),
            ReturnTarget::Background491720,
        ),
        (
            300002,
            ReturnContext {
                network_enabled: true,
                ..Default::default()
            },
            ReturnTarget::Panel(0x86da48),
        ),
        (
            100001,
            ReturnContext {
                sequence_active_8b0700: true,
                ..Default::default()
            },
            ReturnTarget::Background491720,
        ),
        (
            200003,
            ReturnContext {
                transition_8fca9c: 1,
                ..Default::default()
            },
            ReturnTarget::Background491720,
        ),
    ] {
        let catalog = catalog(&[id], 0);
        let live = identity(&catalog);
        let transaction = Transaction::new(
            live.clone(),
            Pair {
                completed: false,
                secondary: 1,
            },
            Origin::Direct,
        );
        assert_eq!(
            transaction.return_target(&live, &catalog, context).unwrap(),
            expected
        );
    }
}

#[test]
fn skirmish_without_campaign_profile_returns_without_creating_progress_file() {
    let mut catalog = catalog(&[300002], 0);
    let live = Identity::capture(17, &catalog, None).unwrap();
    let mut transaction = Transaction::new(
        live.clone(),
        Pair {
            completed: false,
            secondary: 0,
        },
        Origin::Direct,
    );
    non_tutorial_route(&mut transaction, &live, &catalog);
    transaction.acknowledge_tail(&live, &catalog).unwrap();
    let completion = transaction
        .persist(&live, &catalog, |_, _| {
            panic!("created unowned campaign progress")
        })
        .unwrap();
    assert!(!completion.pair.completed);
    assert_eq!(completion.target, ReturnTarget::Background491720);
    assert_eq!(catalog.levels[0].attempts(), 0);
}
