//! Card command consumer through the same account feedback handler as live play.
use super::*;
use openmt2001::{
    category_ownership::Seam,
    economy::{AccountPosting, PlayerAccount, ResidentAccounts, Utilities},
    event_cards::{Deck, effects, lifecycle::Command, targeted},
    live_commands,
    pe::PeImage,
};

#[path = "../../../tests/support/building_image.rs"]
mod building_image;

#[derive(Debug, PartialEq)]
enum Observed {
    CardSound(bool),
    Posting(usize, f32),
    Ui(Seam),
    Appearance,
    Refresh,
    TakeoverMessage,
}

fn pulse(credit: bool) -> Observed {
    Observed::Ui(Seam::NativeUiCall {
        address: if credit { 0x4c6290 } else { 0x4c62d0 },
        args: [0; 6],
        argument_count: 0,
    })
}
fn sound(credit: bool) -> Observed {
    Observed::Ui(Seam::NativeUiCall {
        address: 0x4ad760,
        args: [if credit { 0x541cc8 } else { 0x541cb4 }, 0, 0, 0, 1, 60],
        argument_count: 6,
    })
}
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn account_bytes(account: &PlayerAccount) -> Vec<u8> {
    let mut bytes = vec![0; 0x1b8];
    account.write_runtime(&mut bytes).unwrap();
    bytes
}

fn execute(kind: u32, raw: u32, current: usize, zero_price: bool) -> Vec<Observed> {
    let rebate = (17..=20).contains(&kind);
    let mut bytes = building_image::fixture();
    let mut image_put = |address: u32, value: &[u8]| {
        let at = 0x200 + (address - 0x525000) as usize;
        bytes[at..at + value.len()].copy_from_slice(value);
    };
    image_put(0x5250e8, &0.5f64.to_le_bytes());
    image_put(0x534200, &1f32.to_le_bytes());
    // Authored row in the original table layout: target subtype8 is buyout.
    let card = 73;
    image_put(0x534208 + card * 20 + 8, &raw.to_le_bytes());
    image_put(0x534208 + card * 20 + 12, &kind.to_le_bytes());
    image_put(0x534208 + card * 20 + 16, &1u32.to_le_bytes());
    let image = PeImage::parse(&bytes).unwrap();
    let mut accounts: Vec<_> = [1000., 300., 500.]
        .map(|cash| {
            let mut account = PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap();
            account.cash = cash;
            account
        })
        .into();
    let mut expected = accounts.clone();
    let price = if zero_price { 0. } else { 250. };
    let postings = match kind {
        6 => vec![(1, price, 0x16), (0, -price, 3)],
        0 => vec![(0, raw as f32, 0x24)],
        7 => vec![(0, -(raw as f32), 0x12)],
        1 | 8 => {
            let other = if kind == 1 { -(raw as f32) } else { raw as f32 };
            vec![
                (1, other, if kind == 1 { 0x12 } else { 0x24 }),
                (0, -other, if kind == 1 { 0x24 } else { 0x12 }),
            ]
        }
        17..=20 if raw == 0 => vec![],
        17..=20 if zero_price => vec![(0, -0., 0x12)],
        17..=20 => vec![
            (1, 2. * raw as f32, 0x24),
            (2, raw as f32, 0x24),
            (0, -3. * raw as f32, 0x12),
        ],
        _ => panic!("unsupported test kind"),
    };
    for &(player, amount, category) in &postings {
        expected[player].post(amount, category).unwrap();
    }
    let mut business = [0; 544];
    business[0xa] = 2;
    if !zero_price {
        put(&mut business, 0x58 + 3 * 4, 100f32.to_bits());
        put(&mut business, 0x20, 400f32.to_bits());
    }
    let mut businesses = if rebate {
        // Two consuming businesses owned by player 1, one by inactive player 2,
        // and one by the utility owner (which receives no rebate).
        [2, 3, 2, 1]
            .map(|owner| {
                let mut business = [0; 544];
                business[0xa] = owner;
                business[0x21c] = 1;
                business
            })
            .to_vec()
    } else {
        vec![business]
    };
    let mut building = [0; 0x394];
    put(&mut building, 0x24, 1);
    put(&mut building, 0x4c, 1);
    if !zero_price {
        put(&mut building, 0x38, 200f32.to_bits());
    }
    let mut buildings = [building];
    let mut catalog_rows = [[0; 0x74]; 45];
    let mut categories = [[0; 0x17c]; 52];
    if rebate {
        let utility = (kind - 17) as usize;
        categories[[39, 42, 35, 2][utility]][5] = 1;
        if !zero_price {
            put(
                &mut catalog_rows[0],
                [0x68, 0x6c, 0x64, 0x60][utility],
                1f32.to_bits(),
            );
        }
    }
    let catalog = openmt2001::business_pool::BusinessCatalog::from_rows(catalog_rows);
    let mut utilities = Utilities::new([0; 4]);
    let mut deck = Deck::new([1, 0x343fd, 0x269ec3]);
    deck.used[card as usize] = 1;
    let mut counts = openmt2001::player_metrics::BusinessCounts {
        age: 0,
        values: [0; 6],
    };
    let mut residents = ResidentAccounts {
        active_count: 0,
        nodes: vec![],
        accounts: vec![],
    };
    let mut calls = vec![];
    let mut posted = 0;
    let mut running_cash = [1000., 300., 500.];
    let notice = live_commands::event_card(
        &Command {
            card,
            player: 0,
            target: 0,
        },
        &mut deck,
        &mut effects::World {
            accounts: &mut accounts,
            active: &[1, 1, 0],
            businesses: &mut businesses,
            buildings: &mut buildings,
            categories: &categories,
            counts: &mut counts,
            current,
        },
        &mut targeted::Context {
            image: &image,
            catalog: &catalog,
            products: &[[[0; 2]; 6]; 45],
            commodities: &[[0; 0x1c]; 32],
            residents: &mut residents,
            tick: 42,
            accounting: true,
            network: false,
            network_override: false,
        },
        &mut utilities,
        |event, world| {
            match event {
                live_commands::Event::EventCardEffect(effects::Call::AccountPosted(posting)) => {
                    let (player, amount, _) = postings[posted];
                    posted += 1;
                    assert_eq!(posting, AccountPosting { player, amount });
                    running_cash[player] += amount;
                    assert_eq!(
                        world.accounts.iter().map(|a| a.cash).collect::<Vec<_>>(),
                        running_cash
                    );
                    if kind == 6 {
                        assert_eq!(world.businesses[0][0xa], 2, "post before transfer");
                    }
                    calls.push(Observed::Posting(player, amount));
                    account_ui_post(posting, Some(current as u8), |amount| {
                        ledger_ui_post_with(true, amount, 0., |call| {
                            calls.push(Observed::Ui(call));
                            Ok(0)
                        })
                    })?;
                }
                live_commands::Event::EventCardTargeted(call) => calls.push(match call {
                    targeted::Call::Sound { actor_is_current } => {
                        Observed::CardSound(actor_is_current)
                    }
                    targeted::Call::BuildingAppearance { building: 0 } => Observed::Appearance,
                    targeted::Call::RefreshInterface => {
                        assert_eq!(world.businesses[0][0xa], 1);
                        Observed::Refresh
                    }
                    targeted::Call::Message {
                        from: 1,
                        to: 0,
                        building: 0,
                    } => Observed::TakeoverMessage,
                    _ => panic!("unexpected targeted call {call:?}"),
                }),
                event => panic!("unexpected command event {event:?}"),
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(posted, postings.len());
    assert_eq!(
        accounts.iter().map(account_bytes).collect::<Vec<_>>(),
        expected.iter().map(account_bytes).collect::<Vec<_>>(),
        "cash and both ledger banks equal single-post accounting"
    );
    assert_eq!(deck.used[card as usize], 0);
    assert_eq!(notice.enabled, current != 0);
    if rebate {
        let affected = raw != 0 && !zero_price && current != 0;
        assert_eq!(notice.affected, affected);
        assert_eq!(
            notice.amount,
            if affected {
                raw as f32 * if current == 1 { 2. } else { 1. }
            } else {
                0.
            }
        );
        let mut enabled = [true; 4];
        if raw == 0 {
            enabled[[2, 3, 1, 0][(kind - 17) as usize]] = false;
        }
        assert_eq!(utilities.enabled, enabled);
    }
    if kind == 6 {
        assert_eq!(buildings[0][0x2d], 1);
        assert_eq!(counts.age, 100);
    }
    calls
}

#[test]
fn buyout_card_local_buyer_feedback_after_charge() {
    assert_eq!(
        execute(6, 8, 0, false),
        vec![
            Observed::CardSound(true),
            Observed::Posting(1, 250.),
            Observed::Posting(0, -250.),
            pulse(false),
            sound(false),
            Observed::Appearance,
            Observed::Refresh,
            Observed::TakeoverMessage,
        ]
    );
}

#[test]
fn buyout_card_local_seller_feedback_before_buyer_charge() {
    assert_eq!(
        execute(6, 8, 1, false),
        vec![
            Observed::CardSound(false),
            Observed::Posting(1, 250.),
            sound(true),
            pulse(true),
            Observed::Posting(0, -250.),
            Observed::Appearance,
            Observed::Refresh,
            Observed::TakeoverMessage,
        ]
    );
}

#[test]
fn buyout_card_zero_payments_and_other_players_have_no_feedback() {
    for (current, zero) in [(0, true), (1, true), (2, false)] {
        assert!(
            !execute(6, 8, current, zero)
                .iter()
                .any(|call| matches!(call, Observed::Ui(_)))
        );
    }
}

#[test]
fn cash_cards_report_each_payment_without_netting_or_reposting() {
    for (kind, amount, feedback) in [
        (0, 50., vec![sound(true), pulse(true)]),
        (7, -50., vec![pulse(false), sound(false)]),
    ] {
        let mut expected = vec![Observed::Posting(0, amount)];
        expected.extend(feedback);
        assert_eq!(execute(kind, 50, 0, false), expected);
    }
    assert_eq!(
        execute(1, 50, 1, false),
        vec![
            Observed::Posting(1, -50.),
            pulse(false),
            sound(false),
            Observed::Posting(0, 50.)
        ]
    );
    assert_eq!(
        execute(8, 50, 0, false),
        vec![
            Observed::Posting(1, 50.),
            Observed::Posting(0, -50.),
            pulse(false),
            sound(false)
        ]
    );
}

#[test]
fn utility_rebate_cards_report_recipients_in_slot_order_then_owner_debit() {
    for kind in 17..=20 {
        assert_eq!(
            execute(kind, 50, 1, false),
            vec![
                Observed::Posting(1, 100.),
                sound(true),
                pulse(true),
                Observed::Posting(2, 50.),
                Observed::Posting(0, -150.),
            ]
        );
        assert_eq!(
            execute(kind, 50, 2, false),
            vec![
                Observed::Posting(1, 100.),
                Observed::Posting(2, 50.),
                sound(true),
                pulse(true),
                Observed::Posting(0, -150.),
            ]
        );
        assert_eq!(
            execute(kind, 50, 0, false),
            vec![
                Observed::Posting(1, 100.),
                Observed::Posting(2, 50.),
                Observed::Posting(0, -150.),
                pulse(false),
                sound(false),
            ]
        );
    }
}

#[test]
fn utility_rebate_cards_keep_zero_amounts_silent() {
    for kind in 17..=20 {
        for current in 0..3 {
            assert!(execute(kind, 0, current, false).is_empty());
            assert_eq!(
                execute(kind, 50, current, true),
                vec![Observed::Posting(0, -0.)]
            );
        }
    }
}
