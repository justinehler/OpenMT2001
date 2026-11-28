//! Synthetic regressions for the economy paths run after the tick FPU reset.
use openmt2001::{economy, market, resident, shopping};

fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn player() -> economy::PlayerAccount {
    economy::PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap()
}

#[test]
fn replenishment_retains_cost_through_the_business_store() {
    let large = 2_f32.powi(60);
    // In binary64 the second price disappears before the balance subtraction.
    assert_eq!((f64::from(large) - (f64::from(large) + 1.)) as f32, 0.);
    for (modifier, target, initial, expected_cost) in [
        (0, 1_i16, large, large),
        (1, 1, large * 0.5, large * 0.5),
        (2, 2, large, large),
    ] {
        for free in [false, true] {
            let mut owner = player();
            owner.replenishment_modifier = modifier;
            let mut record = [0; 544];
            word(&mut record, 0x30, initial.to_bits());
            for at in [0x1b8, 0x1ba] {
                record[at..at + 2].copy_from_slice(&target.to_le_bytes());
            }
            let result = economy::replenish_business_record(
                &mut record,
                &[large, 1.],
                i32::from(target) * 2,
                3,
                free,
                Some(&mut owner),
            )
            .unwrap();
            assert_eq!(result.total_stock, 2);
            assert_eq!(result.cost.to_f32().to_bits(), expected_cost.to_bits());
            let residual = if modifier == 1 { -0.5_f32 } else { -1. };
            assert_eq!(
                float(&record, 0x30).to_bits(),
                if free { initial } else { residual }.to_bits()
            );
            let charge = if free { 0. } else { -expected_cost };
            assert_eq!(float(&record, 0x1c).to_bits(), charge.to_bits());
            assert_eq!(owner.cash.to_bits(), charge.to_bits());
            assert_eq!(owner.ledger(0x11).unwrap().to_bits(), charge.to_bits());
            assert_eq!(owner.ledger(0x13).unwrap().to_bits(), 0);
            assert_eq!(
                u16::from_le_bytes(record[0x80..0x82].try_into().unwrap()),
                1
            );
            assert_eq!(
                u16::from_le_bytes(record[0xb4..0xb6].try_into().unwrap()),
                1
            );
        }
    }
}

#[test]
fn replenishment_keeps_per_slot_turnover_ledger_stores() {
    let mut owner = player();
    let mut account = economy::BusinessAccount {
        cost_balance: 0.,
        balance: 0.,
    };
    let mut lines = [
        economy::RestockLine {
            stock: 2,
            target: 1,
            unit_cost: 2_f32.powi(-42),
        },
        economy::RestockLine {
            stock: 8194,
            target: 4097,
            unit_cost: 1. + 2_f32.powi(-12),
        },
    ];
    account
        .replenish(&mut lines, 4098, false, Some(&mut owner))
        .unwrap();
    // Each record call receives an already rounded f32 product. Keeping an
    // extended ledger total across these calls would instead give 0xc5801001.
    assert_eq!(owner.ledger(0x13).unwrap().to_bits(), 0xc580_1000);
    assert_eq!(account.cost_balance.to_bits(), 0);
}

fn catalogs() -> shopping::Catalogs {
    shopping::Catalogs {
        commodities: std::array::from_fn(|_| vec![]),
        fractions: std::array::from_fn(|_| vec![]),
    }
}

#[test]
fn both_market_getters_retain_within_step_products() {
    let tiny = 2_f32.powi(-60);
    let factor = 1. + 2_f32.powi(-12);
    let mut businesses = vec![[0; 544]; 600];
    for business in &mut businesses {
        business[8..10].fill(255);
    }
    businesses[0][8..10].fill(0);
    word(&mut businesses[0], 0x1c4, tiny.to_bits());
    let mut commodities = [resident::Commodity::default(); 32];
    commodities[0].value = factor;
    commodities[0].markup = factor;
    let mut catalogs = catalogs();
    catalogs.commodities[0] = vec![0];
    let mut buildings = vec![[0; 64]; 600];
    for (index, (capacity, rent)) in [(1_u16, 2_f32.powi(-42)), (4097, factor)]
        .into_iter()
        .enumerate()
    {
        word(&mut businesses[index], 0, 1);
        businesses[index][0xe..0x10].copy_from_slice(&capacity.to_le_bytes());
        word(&mut businesses[index], 0x1dc, rent.to_bits());
        buildings[index][0x3f] = 1;
        word(&mut buildings[index], 0x1c, index as u32);
    }
    let inputs = market::Inputs {
        businesses: &businesses,
        buildings: &buildings,
        catalogs: &catalogs,
        commodities: &commodities,
    };
    let old_price = ((f64::from(factor) * f64::from(factor) + f64::from(tiny)) / 2.) as f32;
    let old_rent_sum = (f64::from(factor) * 4097. + f64::from(2_f32.powi(-42))) as f32;
    let old_rent = (f64::from(old_rent_sum) / 4098.) as f32;
    assert_eq!(old_price.to_bits(), 0x3f00_1000);
    assert_eq!(old_rent.to_bits(), 0x3f80_0000);
    for price_first in [true, false] {
        let mut cache = market::Cache {
            dirty: true,
            prices: [0.; 32],
            rent: 0.,
        };
        if price_first {
            cache.price(0, &inputs).unwrap();
        } else {
            cache.residential_rent(&inputs).unwrap();
        }
        assert_eq!(cache.price(0, &inputs).unwrap().to_bits(), 0x3f00_1001);
        assert_eq!(
            cache.residential_rent(&inputs).unwrap().to_bits(),
            0x3f80_0001
        );
        assert!(!cache.dirty);
    }
}

#[test]
fn stock_target_retains_the_fraction_until_ftol() {
    let fraction = 2_f32.powi(-60);
    let mut building = [0; 64];
    building[8] = 1;
    let mut record = [0; 544];
    record[0x1e0..0x1e2].copy_from_slice(&1_i16.to_le_bytes());
    let original = record;
    economy::adjust_stock_target(&mut record, &building, 1., &[0], fraction, 0, 1).unwrap();
    assert_eq!(record, original);
    assert_eq!((f64::from(fraction) - 1.).trunc() as i32, -1);

    // Exercise the world resolver used by both stock commands and AI retail
    // adjustment, including its independent catalog/fraction lookup.
    let mut catalogs = catalogs();
    catalogs.commodities[0] = vec![0];
    catalogs.fractions[0] = vec![fraction];
    let mut settings = economy::BusinessDaySettings {
        capacity_rates: [0.; 45],
        utility_rates: [[0.; 4]; 45],
        catalog_masks: [0; 45],
    };
    settings.capacity_rates[0] = 1.;
    let inputs = economy::BusinessDayInputs {
        buildings: &[building],
        building_categories: &[],
        category_owners: &[0; 52],
        catalogs: &catalogs,
        unit_prices: &[0.; 32],
        settings: &settings,
    };
    inputs.adjust_stock_target(&mut record, 0, 1).unwrap();
    assert_eq!(record, original);
    let mut ai_record = original;
    openmt2001::ai_business::adjust(&mut ai_record, &inputs).unwrap();
    assert_eq!(
        i16::from_le_bytes(ai_record[0x1e0..0x1e2].try_into().unwrap()),
        1
    );
    // AI independently edits the price; that edit legitimately sets dirty.
    assert_eq!(float(&ai_record, 0x1ec).to_bits(), 1_f32.to_bits());
    assert_eq!(ai_record[0x208], 1);
    // A negative request must still bypass unresolved geometry and slot data.
    record[8..10].fill(255);
    let negative = record;
    inputs
        .adjust_stock_target(&mut record, usize::MAX, -1)
        .unwrap();
    assert_eq!(record, negative);
}

#[test]
fn replenishment_preserves_nonfinite_fallback_and_free_accounting() {
    for price in [f32::INFINITY, f32::NAN] {
        for free in [false, true] {
            let mut account = economy::BusinessAccount {
                cost_balance: 7.,
                balance: 9.,
            };
            let mut lines = [economy::RestockLine {
                stock: 0,
                target: 1,
                unit_cost: price,
            }];
            account.replenish(&mut lines, 1, free, None).unwrap();
            assert_eq!(lines[0].stock, 1);
            if free {
                assert_eq!(account.cost_balance, 7.);
                assert_eq!(account.balance, 9.);
            } else if price.is_nan() {
                assert!(account.cost_balance.is_nan());
                assert!(account.balance.is_nan());
            } else {
                assert_eq!(account.cost_balance, f32::NEG_INFINITY);
                assert_eq!(account.balance, f32::NEG_INFINITY);
            }
        }
    }
}
