use super::*;

fn candidate(capacity: u16, rent: f32, attribute_2c: i8) -> HousingCandidate {
    HousingCandidate {
        residential: true,
        capacity,
        occupied: 0,
        attribute_2c,
        rent,
    }
}

#[test]
fn housing_average_keeps_the_product_until_each_f32_store() {
    let candidates = [
        candidate(1, 2f32.powi(-42), 0),
        candidate(4097, 1. + 2f32.powi(-12), 0),
    ];
    assert_eq!(average_residential_rent(&candidates).to_bits(), 0x3f80_0001);
    let binary64_sum = candidates.iter().fold(0f32, |sum, c| {
        (f64::from(c.capacity) * f64::from(c.rent) + f64::from(sum)) as f32
    });
    assert_eq!((f64::from(binary64_sum) / 4098.) as f32, 1.);
    assert!(average_residential_rent(&[]).is_nan());
    assert_eq!(
        average_residential_rent(&[candidate(0, 2., 0), candidate(1, 3., 0)]),
        3.
    );
    assert_eq!(
        average_residential_rent(&[
            candidate(1, 2f32.powi(24), 0),
            candidate(1, 1., 0),
            candidate(1, -2f32.powi(24), 0),
        ]),
        0.
    );
}

#[test]
fn housing_score_retains_the_rent_term_through_ftol() {
    let inputs = HousingScoreInputs {
        affordability_factor: 1.,
        welfare: 1.,
        rent_weight: -2f32.powi(-60),
        district_weight: 0.,
        district_value: 0.,
        average_rent: 1.,
        current_category: false,
    };
    let mut calls = 0;
    let score = score_housing_category_cached(&[candidate(100, 1., 0)], None, inputs, || {
        calls += 1;
        Ok(1.)
    })
    .unwrap();
    assert_eq!(score, 99);
    assert_eq!(calls, 1);
    let binary64 = 100. * f64::from(inputs.rent_weight) + 100.;
    assert_eq!(binary64.trunc() as i32, 100);
    assert_eq!(
        score_housing_category_cached(&[], None, inputs, || panic!("eager rent lookup")).unwrap(),
        -99_999_999
    );

    // The earlier per-building multiply-add must also retain its product.
    let candidates = [
        candidate(1, 2f32.powi(-42), 0),
        candidate(4097, 1. + 2f32.powi(-12), 0),
    ];
    let inputs = HousingScoreInputs {
        welfare: 2.,
        rent_weight: 1e6,
        ..inputs
    };
    assert_eq!(
        score_housing_category(&candidates, None, inputs).unwrap(),
        100_000_084
    );
    let old_sum = candidates.iter().fold(0f32, |sum, c| {
        (f64::from(c.capacity) * f64::from(c.rent) + f64::from(sum)) as f32
    });
    let old_scaled = (f64::from(old_sum) * 100.) as f32;
    let old_score = ((200. - f64::from(old_scaled) / 4098.) * 1e6 + 100.).trunc() as i32;
    assert_eq!(old_score, 100_000_100);
}

#[test]
fn residence_selection_retains_attribute_across_the_rent_conversion() {
    let weight = -2f32.powi(-60);
    assert_eq!((33. * f64::from(weight) + 1.).trunc() as i32, 1);
    // The first residence scores exactly zero. The witness also scores zero
    // at PC64, so first-on-equal wins; binary64 instead selects the second.
    let candidates = [candidate(1, 0., -1), candidate(1, 1., 0)];
    assert_eq!(
        select_residence(&candidates, None, 1., 1., 1., weight).unwrap(),
        Some(0)
    );
    // With two unit rents, only attribute -1 reaches score one at PC64.
    let candidates = [candidate(1, 1., 0), candidate(1, 1., -1)];
    assert_eq!(
        select_residence(&candidates, None, 1., 1., 1., weight).unwrap(),
        Some(1)
    );
    assert_eq!(
        select_residence(&candidates, Some(0), 1., 1., 1., weight).unwrap(),
        Some(0)
    );
    assert_eq!(
        select_residence(&[candidates[1]; 2], None, 1., 1., 1., weight).unwrap(),
        Some(0)
    );
}

fn put_float(resident: &mut Resident, at: usize, value: f32) {
    resident.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn bits(resident: &Resident, at: usize) -> u32 {
    u32::from_le_bytes(resident.bytes[at..at + 4].try_into().unwrap())
}

#[test]
fn resident_movement_keeps_each_product_until_the_position_store() {
    let mut resident = Resident::from_bytes([0; 0x14a]);
    let speed = 1. + 2f32.powi(-12);
    put_float(&mut resident, 0x11c, 5.);
    put_float(&mut resident, 0x120, speed);
    put_float(&mut resident, 0x110, speed);
    put_float(&mut resident, 0xf4, 2f32.powi(-60));
    let mut auxiliary = [0; 284];
    assert!(!resident.advance_movement(&mut auxiliary));
    assert_eq!(bits(&resident, 0xf4), 0x3f80_1001);
    assert_eq!(&auxiliary[4..16], &resident.bytes[0xf4..0x100]);
    assert_eq!(
        ((f64::from(speed) * f64::from(speed)) + 2f64.powi(-60)) as f32,
        f32::from_bits(0x3f80_1000)
    );
    put_float(&mut resident, 0x11c, 0.);
    assert!(!resident.advance_movement(&mut auxiliary));
    assert_eq!(resident.bytes[0x10c], 1);
    assert!(resident.advance_movement(&mut auxiliary));
}

#[test]
fn resident_movement_retains_x_delta_but_reloads_the_z_spill() {
    let mut resident = Resident::from_bytes([0; 0x14a]);
    resident.bytes[0x10c] = 1;
    for at in [0x124, 0x12c] {
        put_float(&mut resident, at, 2f32.powi(60));
    }
    for at in [0x130, 0x138] {
        put_float(&mut resident, at, 1.);
    }
    assert!(!resident.advance_movement(&mut [0; 284]));
    assert_eq!(bits(&resident, 0x124), 1f32.to_bits());
    assert_eq!(bits(&resident, 0x12c), 0);
    assert_eq!(bits(&resident, 0x110), 1f32.to_bits());
    assert_eq!(bits(&resident, 0x118), 0);
    assert_eq!(bits(&resident, 0x11c), 1f32.to_bits());
    assert_eq!((1. - 2f64.powi(60)) + 2f64.powi(60), 0.);

    let mut resident = Resident::from_bytes([0; 0x14a]);
    resident.bytes[0x10c] = 1;
    put_float(&mut resident, 0x130, 3.);
    put_float(&mut resident, 0x138, 4.);
    assert!(!resident.advance_movement(&mut [0; 284]));
    assert_eq!(bits(&resident, 0x110), 0.6f32.to_bits());
    assert_eq!(bits(&resident, 0x118), 0.8f32.to_bits());
    assert_eq!(bits(&resident, 0x11c), 5f32.to_bits());
}

#[test]
fn resident_speed_initialization_distinguishes_post_reset_and_early_startup() {
    let movement = MovementSettings {
        speed_by_age: [2f32.powi(-65); 5],
        variation: 1.25,
    };
    let preferences = Preferences::empty();
    let objectives = DailyObjectives {
        rows: std::array::from_fn(|_| vec![0]),
    };
    let timetables = Timetables {
        rows: [[0; 24]; 100],
    };
    let commodities = [Commodity::default(); 32];
    for (precision, expected) in [
        (MovementSpeedPrecision::AfterReset, 0x3c4c_cccd),
        (MovementSpeedPrecision::BeforeFirstReset, 0x3c4c_cccc),
    ] {
        let mut resident = Resident::from_bytes([0; 0x14a]);
        let mut account = SpendingAccount::from_bytes([0; 0x90]);
        let mut rng = InitializationRng {
            constructor: 10,
            movement: 205,
            objective: 30,
        };
        let inputs = InitializationInputs {
            movement: &movement,
            movement_precision: precision,
            preferences: &preferences,
            objectives: &objectives,
            timetables: &timetables,
            commodities: &commodities,
            commodity_masks: [0; 5],
            hour: 0,
        };
        resident
            .initialize_personal([2, 0, 0], &mut account, &inputs, &mut rng)
            .unwrap();
        assert_eq!(bits(&resident, 0x120), expected);
        let next = |seed: u32| seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        assert_eq!(rng.movement, next(205));
        assert_eq!(((rng.movement >> 16) & 0x7fff) % 101, 1);
        assert_eq!(rng.objective, next(30));
        assert_eq!(rng.constructor, next(next(next(10))));
    }
}
