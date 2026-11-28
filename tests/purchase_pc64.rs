//! Synthetic purchase and AI-rent regressions after the logic tick FPU reset.
use openmt2001::{ai_rent, economy, resident::*, shopping};

fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn resident() -> Resident {
    let mut bytes = [0; 330];
    bytes[0xe..0x10].copy_from_slice(&(-1_i16).to_le_bytes());
    bytes[0x2c] = 5; // Two eligible categories.
    word(&mut bytes, 0x6c, 1);
    word(&mut bytes, 0x70, 1_f32.to_bits());
    Resident::from_bytes(bytes)
}

fn account(cell: f32) -> SpendingAccount {
    let mut bytes = [0; 144];
    word(&mut bytes, 4, cell.to_bits());
    word(&mut bytes, 0x84, 1);
    word(&mut bytes, 0x8c, cell.to_bits());
    SpendingAccount::from_bytes(bytes)
}

#[test]
fn purchase_candidate_cancellation_survives_both_composed_choices() {
    let mut resident = resident();
    let accounts = [account(1.25)];
    let mut business = [0; 544];
    word(&mut business, 0x218, 1);
    word(&mut business, 0x70, 1);
    word(&mut business, 0x1c4, 1_f32.to_bits());
    let weights = [-33. * 2_f32.powi(50), 1., 100. * 2_f32.powi(50)];
    let inputs = PurchaseCandidateInputs {
        commodity: 0,
        value: 1.,
        markup: 1.,
        building_attribute: 0,
        weights,
    };
    let mut calls = Vec::new();
    let score = resident
        .score_purchase_candidate(
            &business,
            &accounts,
            inputs,
            |_, _| {
                calls.push("price");
                Ok(1.)
            },
            |_| Ok(1.),
        )
        .unwrap();
    assert_eq!(calls, ["price", "price"]);
    assert_eq!(score.value.ftol_low_word(), 20);
    assert_eq!(score.ratio, Some(100.));
    assert_eq!(score.category_factor, Some(20.));
    let old = 100. * f64::from(weights[0]) + (33. * f64::from(weights[2]) + 20.);
    assert_eq!(old, 0.);

    let businesses = [business];
    let mut chains = std::array::from_fn(|_| vec![]);
    chains[0] = vec![Some(0)];
    let mut commodities = [Commodity::default(); 32];
    commodities[0].value = 1.;
    commodities[0].markup = 1.;
    let world = PurchaseWorld {
        businesses: &businesses,
        catalogs: &[vec![0]],
        chains: &chains,
        category_masks: &[1; 52],
        category_factors: &[1.; 52],
        centers: &[[0.; 2]; 52],
        building_categories: &[0],
        building_attributes: &[0],
        commodities: &commodities,
        weights: &[[weights[0], weights[1], weights[2], 0.]],
        distance_disabled: true,
    };
    assert!(
        resident
            .choose_purchase_category(&world, &accounts, [0.; 2], &[None; 52], &mut [])
            .unwrap()
    );
    assert_eq!(resident.as_bytes()[0x44], 0);
    assert!(
        resident
            .choose_purchase_business(
                &world,
                &accounts,
                PurchaseBusinessInputs {
                    preferences: &[0],
                    account_mask: 1,
                    nodes: &[PurchaseNode {
                        building: 0,
                        position: [0.; 2],
                        business_mask: Some(1)
                    }],
                    auxiliary_position: [0.; 2],
                    distance_weight: 0.,
                },
                &[[7., 99., 9.]],
            )
            .unwrap()
    );
    assert_eq!(&resident.as_bytes()[0x46..0x48], &[0, 0]);
    assert_eq!(float(resident.as_bytes(), 0x48), 7.);
    assert_eq!(float(resident.as_bytes(), 0x50), 9.);
}

#[test]
fn purchase_distance_retains_return_through_total_and_best_comparison() {
    let mut resident = resident();
    let mut account_bytes = *account(1001. / 80.).as_bytes();
    word(&mut account_bytes, 8, 12.5_f32.to_bits());
    word(&mut account_bytes, 0x84, 3);
    let accounts = [SpendingAccount::from_bytes(account_bytes)];
    let mut resident_bytes = *resident.as_bytes();
    word(&mut resident_bytes, 0x6c, 3);
    resident = Resident::from_bytes(resident_bytes);
    let mut centers = [[0.; 2]; 52];
    centers[0] = [3., 4.];
    let source = [3., 4.];
    let value = resident
        .purchase_distance_adjustment(1, &centers, source, -2., false)
        .unwrap();
    assert_eq!(value.ftol_low_word(), -198);
    // Serializing the return to binary64 hides the residual needed by choice.
    assert_eq!(value.to_f64(), -199.);
    assert_eq!((100. - 5_f64 * 0.1) * -2., -199.);
    let mut businesses = [[0; 544]; 2];
    let mut chains = std::array::from_fn(|_| vec![]);
    let mut category_masks = [0; 52];
    for index in 0..2 {
        word(&mut businesses[index], 4, index as u32);
        businesses[index][8..10].copy_from_slice(&(index as i16).to_le_bytes());
        word(&mut businesses[index], 0x218, 1);
        word(&mut businesses[index], 0x70, 1 << index);
        word(&mut businesses[index], 0x1c4, 1_f32.to_bits());
        chains[index] = vec![Some(index)];
        category_masks[index] = 1 << index;
    }
    let world = PurchaseWorld {
        businesses: &businesses,
        catalogs: &[vec![0], vec![1]],
        chains: &chains,
        category_masks: &category_masks,
        category_factors: &[0.; 52],
        centers: &centers,
        building_categories: &[0, 1],
        building_attributes: &[0, 0],
        commodities: &[Commodity {
            active: true,
            value: 1.,
            markup: 1.,
        }; 32],
        weights: &[[1., 0., 0., -2.]],
        distance_disabled: false,
    };
    assert!(
        resident
            .choose_purchase_category(&world, &accounts, source, &[None; 52], &mut [])
            .unwrap()
    );
    // Category 0 stores exactly 801. Category 1 returns slightly above 801;
    // the old binary64 chain tied and kept category 0 instead.
    assert_eq!(resident.as_bytes()[0x44], 1);
    assert_eq!(1000_f64 + (100. - 5_f64 * 0.1) * -2., 801.);
    assert_eq!(
        resident
            .purchase_distance_adjustment(usize::MAX, &centers, [f32::NAN; 2], f32::NAN, true)
            .unwrap()
            .to_f32(),
        0.
    );

    // A resident with a current building uses its category center instead of
    // the auxiliary position, with the same retained result.
    let mut bytes = *resident.as_bytes();
    bytes[0xe..0x10].fill(0);
    bytes[0xc] = 0;
    let resident = Resident::from_bytes(bytes);
    assert_eq!(
        resident
            .purchase_distance_adjustment(1, &centers, [999.; 2], -2., false)
            .unwrap()
            .ftol_low_word(),
        -198
    );
}

#[test]
fn purchase_business_compares_retained_difference_before_distance_bonus() {
    let mut resident_bytes = *resident().as_bytes();
    word(&mut resident_bytes, 0x70, 2_f32.to_bits());
    let mut resident = Resident::from_bytes(resident_bytes);
    let accounts = [account(3.75)];
    let mut businesses = [[0; 544]; 2];
    for (index, price) in [1_f32, 100. / 67.].into_iter().enumerate() {
        businesses[index][8..10].copy_from_slice(&(index as i16).to_le_bytes());
        word(&mut businesses[index], 0x218, 1);
        word(&mut businesses[index], 0x70, 1);
        word(&mut businesses[index], 0x1c4, price.to_bits());
    }
    let mut chains = std::array::from_fn(|_| vec![]);
    chains[0] = vec![Some(0), Some(1)];
    let world = PurchaseWorld {
        businesses: &businesses,
        catalogs: &[vec![0]],
        chains: &chains,
        category_masks: &[1; 52],
        category_factors: &[1.; 52],
        centers: &[[0.; 2]; 52],
        building_categories: &[0, 0],
        building_attributes: &[0, 0],
        commodities: &[Commodity {
            active: true,
            value: 1.,
            markup: 1.,
        }; 32],
        weights: &[[1., -2_f32.powi(-60), 0., 0.]],
        distance_disabled: true,
    };
    let nodes = std::array::from_fn::<_, 2, _>(|index| PurchaseNode {
        building: index as i32,
        position: [0.; 2],
        business_mask: Some(1),
    });
    assert!(
        resident
            .choose_purchase_business(
                &world,
                &accounts,
                PurchaseBusinessInputs {
                    preferences: &[0],
                    account_mask: 1,
                    nodes: &nodes,
                    auxiliary_position: [0.; 2],
                    distance_weight: -1.,
                },
                &[[0.; 3]; 2]
            )
            .unwrap()
    );
    assert_eq!(&resident.as_bytes()[0x46..0x48], &[0, 0]);
    // First candidate stores best=200. The second retains just below 201,
    // so difference<1 adds the -100 bonus and keeps the first business.
    // Binary64 loses the small factor term, skips that bonus and chooses 1.
    assert_eq!(201_f64 + 20. * f64::from(-2_f32.powi(-60)), 201.);
}

#[test]
fn purchase_debit_keeps_product_until_each_account_store() {
    let mut resident = resident();
    let price = 1. + 2_f32.powi(-12);
    let mut business = [0; 544];
    business[0x80..0x82].copy_from_slice(&4097_u16.to_le_bytes());
    word(&mut business, 0x1c4, price.to_bits());
    let mut bytes = *account(f32::from_bits(0x4580_1001)).as_bytes();
    word(&mut bytes, 0x88, (-2_f32.powi(-42)).to_bits());
    let mut account = SpendingAccount::from_bytes(bytes);
    let quantity = resident
        .purchase_resolved(
            &mut business,
            &mut account,
            RetailInputs {
                slot: 0,
                preference_limit: 4097.,
                interaction_delay: 2.,
                unit_cost: 0.,
                tick: 0,
            },
            None,
        )
        .unwrap();
    assert_eq!(quantity, Some(4097));
    assert_eq!(float(account.as_bytes(), 0x88).to_bits(), 0xc580_1001);
    assert_eq!(float(account.as_bytes(), 4), 2_f32.powi(-12));
    assert_eq!(float(account.as_bytes(), 0x8c), 2_f32.powi(-12));
    assert_eq!(&account.as_bytes()[0x84..0x88], &1_u32.to_le_bytes());
    assert_eq!(
        ((-2_f64.powi(-42)) - 4097. * f64::from(price)) as f32,
        f32::from_bits(0xc580_1000)
    );
    assert_eq!(&business[0x80..0x82], &[0, 0]);
}

#[test]
fn ai_rent_truncates_retained_cost_before_minimum_comparison() {
    let mut building = [0; 64];
    building[8] = 1;
    let mut settings = economy::BusinessDaySettings {
        capacity_rates: [0.; 45],
        utility_rates: [[0.; 4]; 45],
        catalog_masks: [0; 45],
    };
    settings.utility_rates[0] = [1., -2_f32.powi(-60), 0., 0.];
    let catalogs = shopping::Catalogs {
        commodities: std::array::from_fn(|_| vec![]),
        fractions: std::array::from_fn(|_| vec![]),
    };
    let business_inputs = economy::BusinessDayInputs {
        buildings: &[building],
        building_categories: &[],
        category_owners: &[0; 52],
        catalogs: &catalogs,
        unit_prices: &[0.; 32],
        settings: &settings,
    };
    let utilities = economy::Utilities {
        owners: [0; 4],
        enabled: [true; 4],
        totals: [0.; 4],
    };
    let mut business = [0; 544];
    word(&mut business, 0, 1);
    business[10] = 1;
    business[14..16].copy_from_slice(&1_u16.to_le_bytes());
    word(&mut business, 0x1dc, 1_f32.to_bits());
    assert_eq!(
        openmt2001::ai_business::estimate_cost(&business, &business_inputs, &utilities).unwrap(),
        1.
    );
    let mut entry = [0; 28];
    entry[0] = 1;
    let list = openmt2001::ai_ownership::BusinessList {
        entries: vec![entry],
        active_count: 1,
    };
    let mut businesses = [business];
    assert!(
        ai_rent::adjust_owned(
            &list,
            &mut businesses,
            &ai_rent::Inputs {
                business: &business_inputs,
                utilities: &utilities,
                increase: 0,
            }
        )
        .unwrap()
    );
    // The old cost/minimum was 1, so minimum < old was false and no edit ran.
    assert_eq!(float(&businesses[0], 0x204).to_bits(), 0.1_f32.to_bits());
    assert_eq!(businesses[0][0x208], 1);
}
