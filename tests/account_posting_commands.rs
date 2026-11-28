use openmt2001::{
    business_sale, business_takeover, category_ownership,
    economy::{AccountPosting, PlayerAccount},
    pe::PeImage,
    player_manager::Player,
};
#[path = "support/building_image.rs"]
mod building_image;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn account() -> PlayerAccount {
    let mut account = PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap();
    account.cash = 1000.;
    account
}
fn image_bytes() -> Vec<u8> {
    let mut bytes = building_image::fixture();
    bytes.resize(0x200 + 0x15000, 0);
    put(&mut bytes, 0xc8, 0x15000);
    for address in [0x525c50, 0x525af4] {
        put(&mut bytes, 0x200 + (address - 0x525000), 1f32.to_bits());
    }
    bytes
}

#[test]
fn sale_post_is_synchronous_before_owner_transfer_and_propagates_feedback_failure() {
    for amount in [250f32, 0., -25.] {
        let bytes = image_bytes();
        let image = PeImage::parse(&bytes).unwrap();
        let mut businesses = [[0; 544]];
        businesses[0][10] = 1;
        let mut buildings = [[0; 916]];
        put(&mut buildings[0], 0x38, amount.to_bits());
        let mut players = [account()];
        let catalog = openmt2001::business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
        let mut age = 0;
        let mut observed = Vec::new();
        let result = business_sale::apply(
            &business_sale::Command {
                business: 0,
                player: 0,
            },
            &mut business_sale::World {
                businesses: &mut businesses,
                buildings: &mut buildings,
                players: &mut players,
                catalog: &catalog,
                products: &[[[0; 2]; 6]; 45],
                commodities: &[[0; 28]; 32],
                age: &mut age,
            },
            &business_sale::Controls {
                current_player: None,
                accounting: false,
                ownership_tick: 0,
                timestamp: 0,
                hour: 0,
            },
            &image,
            |call, world| {
                let business_sale::Call::AccountPosted(posting) = call else {
                    panic!("effect preceded posting: {call:?}")
                };
                observed.push(posting);
                assert_eq!(world.businesses[0][10], 1);
                assert_eq!(world.players[0].cash, 1000. + amount);
                Err("feedback unavailable".into())
            },
        );
        assert_eq!(result, Err("feedback unavailable".into()));
        assert_eq!(observed, [AccountPosting { player: 0, amount }]);
        assert_eq!(businesses[0][10], 1);
    }
}

#[test]
fn takeover_keeps_both_posts_before_transfer_even_for_the_same_account() {
    for (seller, value) in [(1, 250f32), (2, 250.), (2, 0.)] {
        let bytes = image_bytes();
        let image = PeImage::parse(&bytes).unwrap();
        let mut categories = [[0; 380]; 52];
        categories[0][5] = 1;
        let mut buildings = [[0; 916]];
        put(&mut buildings[0], 0x40, 0x617628);
        put(&mut buildings[0], 0x38, value.to_bits());
        let mut businesses = [[0; 544]];
        businesses[0][10] = seller;
        let mut players = [account(), account()];
        let mut age = 0;
        let mut postings = Vec::new();
        let result = business_takeover::apply(
            &business_takeover::Command { building: 0 },
            &mut business_takeover::World {
                categories: &categories,
                buildings: &mut buildings,
                businesses: &mut businesses,
                players: &mut players,
                player_references: &[0x1000, 0x2000],
                player_slots: &[0, 1],
                age: &mut age,
            },
            &business_takeover::Controls {
                accounting: false,
                ownership_tick: 0,
                network: [false; 2],
                current_player: None,
            },
            &image,
            |call, world| {
                let business_takeover::Call::AccountPosted(posting) = call else {
                    panic!("effect preceded both postings: {call:?}")
                };
                postings.push(posting);
                assert_eq!(world.businesses[0][10], seller);
                if postings.len() == 2 {
                    Err("stop before transfer".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result, Err("stop before transfer".into()));
        assert_eq!(
            postings,
            [
                AccountPosting {
                    player: usize::from(seller) - 1,
                    amount: value
                },
                AccountPosting {
                    player: 0,
                    amount: -value
                }
            ]
        );
        assert_eq!(
            players[0].cash,
            if seller == 1 { 1000. } else { 1000. - value }
        );
    }
}

#[test]
fn property_posts_precede_owner_stores_and_keep_each_payment_separate() {
    for (seller, value) in [(1, 250f32), (2, 250.), (2, 0.)] {
        let mut categories = [[0; 380]; 52];
        categories[0][5] = seller;
        let mut players: Vec<_> = (0..2)
            .map(|slot| {
                let mut player = Player {
                    reference: slot as u32 + 1,
                    bytes: vec![0; 548],
                    auxiliaries: vec![],
                };
                player.bytes[0x1bc] = slot;
                put(&mut player.bytes, 4, 1000f32.to_bits());
                player
            })
            .collect();
        let mut world = category_ownership::World {
            categories: &mut categories,
            buildings: &mut [],
            businesses: &mut [],
            players: &mut players,
            chains: &std::array::from_fn(|_| vec![]),
            normal_tick: 0,
            category_tick: 0,
            refresh_counter: 0,
            accounting_enabled: false,
            current_player: None,
            notification_threshold: 0.,
        };
        let mut postings = Vec::new();
        let result = world.apply(0, 0, value, &mut |world, call| {
            let category_ownership::Seam::AccountPosted(posting) = call else {
                panic!("effect preceded both postings: {call:?}")
            };
            postings.push(posting);
            assert_eq!(
                world.categories[0][5],
                if postings.len() == 1 { seller } else { 1 }
            );
            assert_eq!(
                f32::from_bits(word(&world.players[posting.player].bytes, 4)),
                if posting.player == 0 && seller == 1 && postings.len() == 2 {
                    1000.
                } else {
                    1000. + posting.amount
                }
            );
            if postings.len() == 2 {
                Err("stop after buyer feedback".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("stop after buyer feedback".into()));
        assert_eq!(
            postings,
            [
                AccountPosting {
                    player: usize::from(seller) - 1,
                    amount: value
                },
                AccountPosting {
                    player: 0,
                    amount: -value
                }
            ]
        );
    }
}

#[test]
fn sale_initial_stock_post_precedes_pricing_when_appearance_changes_the_owner() {
    let bytes = image_bytes();
    let image = PeImage::parse(&bytes).unwrap();
    let mut businesses = [[0; 544]];
    businesses[0][10] = 1;
    put(&mut businesses[0], 0x1c4, 123f32.to_bits());
    let mut buildings = [[0; 916]];
    buildings[0][8] = 1;
    put(&mut buildings[0], 0x4c, 1);
    let mut players = [account()];
    let mut rows = [[0; 116]; 45];
    rows[0][0x4c] = 1;
    put(&mut rows[0], 0x58, 10f32.to_bits());
    let catalog = openmt2001::business_pool::BusinessCatalog::from_rows(rows);
    let mut products = [[[0; 2]; 6]; 45];
    products[0][0][1] = 1f32.to_bits();
    let mut commodities = [[0; 28]; 32];
    put(&mut commodities[0], 20, 2f32.to_bits());
    let mut age = 0;
    let mut postings = Vec::new();
    let result = business_sale::apply(
        &business_sale::Command {
            business: 0,
            player: 0,
        },
        &mut business_sale::World {
            businesses: &mut businesses,
            buildings: &mut buildings,
            players: &mut players,
            catalog: &catalog,
            products: &products,
            commodities: &commodities,
            age: &mut age,
        },
        &business_sale::Controls {
            current_player: Some(0),
            accounting: false,
            ownership_tick: 0,
            timestamp: 0,
            hour: 9,
        },
        &image,
        |call, world| match call {
            business_sale::Call::BuildingAppearance(_) => {
                world.businesses[0][10] = 1;
                Ok(())
            }
            business_sale::Call::AccountPosted(posting) => {
                postings.push(posting);
                if postings.len() == 2 {
                    assert_eq!(posting.amount, -20.);
                    assert_eq!(world.players[0].cash, 980.);
                    assert_eq!(word(&world.businesses[0], 0x1c4), 123f32.to_bits());
                    Err("stop before pricing".into())
                } else {
                    Ok(())
                }
            }
            other => panic!("effect preceded stock posting: {other:?}"),
        },
    );
    assert_eq!(result, Err("stop before pricing".into()));
    assert_eq!(postings.len(), 2);
}
