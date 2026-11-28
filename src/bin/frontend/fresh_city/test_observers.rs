use super::Session;
use openmt2001::economy::AccountPosting;

pub(super) fn cash(session: &Session<'_>) -> Vec<f32> {
    session
        .world
        .ledgers
        .players
        .iter()
        .map(|p| p.cash)
        .collect()
}

pub(super) fn assert_account_postings(before: &[f32], after: &[f32], postings: &[AccountPosting]) {
    let mut expected = before.to_vec();
    for posting in postings {
        let balance = expected
            .get_mut(posting.player)
            .expect("account posting player outside loaded roster");
        // Each native post rounds the new cash balance independently.
        *balance = (f64::from(*balance) + f64::from(posting.amount)) as f32;
    }
    assert_eq!(after, expected, "account postings differ from cash changes");
}

#[test]
fn account_posting_observer_checks_each_player_and_rounds_each_post() {
    assert_account_postings(
        &[16_777_216., 20.],
        &[16_777_216., 15.],
        &[
            AccountPosting {
                player: 0,
                amount: 1.,
            },
            AccountPosting {
                player: 1,
                amount: -5.,
            },
            AccountPosting {
                player: 0,
                amount: 1.,
            },
        ],
    );
}

#[test]
#[should_panic(expected = "account postings differ from cash changes")]
fn account_posting_observer_rejects_incorrect_amount() {
    assert_account_postings(
        &[100.],
        &[109.],
        &[AccountPosting {
            player: 0,
            amount: 8.,
        }],
    );
}

#[test]
#[should_panic(expected = "account posting player outside loaded roster")]
fn account_posting_observer_rejects_unknown_player() {
    assert_account_postings(
        &[100.],
        &[100.],
        &[AccountPosting {
            player: 1,
            amount: 0.,
        }],
    );
}
