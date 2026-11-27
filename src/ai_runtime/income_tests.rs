//! Synthetic financial snapshots; no original executable or captures are needed.
use super::*;
use crate::{ai_finance::IncomeTotals, economy, player_manager::put};

fn income_image(final_utility: bool) -> Vec<u8> {
    // One synthetic PE section contains only the tables these queries read.
    let mut bytes = vec![0; 0x150000];
    bytes[..2].copy_from_slice(b"MZ");
    put(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut bytes, 0xb4, 0x400000);
    put(&mut bytes, 0xc4, 0);
    put(&mut bytes, 0xc8, 0x14f000);
    put(&mut bytes, 0xcc, 0x1000);
    let offset = |address: usize| address - 0x400000 + 0x1000;
    put(&mut bytes, offset(0x525048), 0.25f32.to_bits());
    for (group, table, categories) in [
        (10, 0x541ec8, [44, 43, 17, 4]),
        (11, 0x541eb8, [2, 35, 42, 39]),
    ] {
        for (index, category) in categories.into_iter().enumerate() {
            bytes[offset(0x5385a4 + category * 88)] = group;
            put(&mut bytes, offset(table - index * 4), category as u32);
        }
    }
    if final_utility {
        // Force a utility traversal last so its otherwise-discarded subtotal
        // is observable, without changing the reverse tables or their order.
        bytes[offset(0x5385a4 + 51 * 88)] = 11;
    }
    bytes
}

fn with_world(final_utility: bool, run: impl FnOnce(&mut World<'_>, &PeImage<'_>)) {
    let bytes = income_image(final_utility);
    let image = PeImage::parse(&bytes).unwrap();
    let properties = economy::PropertyIncome {
        owners: [1, 0, 0, 1],
        totals: [10.125, 20.5, 30.75, 40.25],
    };
    let mut utilities = economy::Utilities::new([1, 1, 0, 0]);
    utilities.totals = [101.5, 303.25, 707.0, 909.0];
    let mut categories = [[0; 380]; 52];
    for category in [4, 44, 2, 35] {
        categories[category][5] = 1;
    }
    let catalog = crate::business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
    let residents = economy::ResidentAccounts {
        active_count: 0,
        nodes: vec![],
        accounts: vec![],
    };
    let mut world = World {
        building_parameters: crate::building_parameters::BuildingKinds::from_image(&image).unwrap(),
        neighbor_cache: &mut crate::ai_neighbors::Cache::default(),
        ownership_refresh_counter: &mut 0,
        utility_enabled: utilities.enabled,
        liquidation_targets: &mut Default::default(),
        colors: None,
        category_counts: &mut [0; 52 * 45],
        ai_demand_rows: &[[0; 264]; 33],
        active_player_count: 1,
        construction_components: &mut [0.0; 4],
        construction_allowance: 0,
        construction_discount: 0.0,
        affordability_override: false,
        acquisition_checks: false,
        proposal_identity: &mut 0,
        residents: &residents,
        commodity_active: [false; 32],
        catalog: &catalog,
        products: &[[[0; 2]; 6]; 45],
        commodities: &[[0; 28]; 32],
        category_footprints: &mut [[0; 2]; 52],
        strategy_time: None,
        ownership_tick: None,
        strategy_mode: 0,
        business_counts: None,
        categories: &mut categories,
        buildings: &[],
        businesses: &mut vec![[0; 544]; 600],
        ownership: &mut Default::default(),
        allocation_next: &mut 0,
        message_routing: None,
        global_reset: &mut 0,
        // Both retained and frontend worlds use this same typed conversion.
        income: IncomeTotals::from_economy(&properties, &utilities),
        auction: None,
        auction_blocked: None,
        auction_guard: None,
        offer_auction: None,
        auction_state: None,
        cash_statuses: &[0; 6],
    };
    run(&mut world, &image);
}

fn player() -> Player {
    let mut player = Player {
        reference: 1,
        bytes: vec![0; 0x1e50],
        auxiliaries: vec![],
    };
    player.bytes[0x10] = 1;
    player
}

#[test]
fn ai_financial_snapshot_uses_separate_income_banks() {
    for (final_utility, expected) in [(false, 201.0f32), (true, 1821.0)] {
        with_world(final_utility, |world, image| {
            let mut player = player();
            finance(&mut player, &[None], &[[0; 0x650]], image, world, 0, false).unwrap();
            // Count accumulates two owned entries per traversal, but only the
            // final subtotal survives: 16*.25*(10.125+40.25) truncates to201;
            // the extra utility traversal gives 18*.25*(101.5+303.25) ->1821.
            assert_eq!(f32::from_bits(word(&player.bytes, 0xca8)), expected);
            assert_eq!(world.income.railroads, [10.125, 20.5, 30.75, 40.25]);
            assert_eq!(world.income.utilities, [707.0, 909.0, 303.25, 101.5]);
        });
    }
}

#[test]
fn ai_railroad_investment_uses_only_railroad_income() {
    with_world(false, |world, image| {
        let player = player();
        let investment = |world: &World<'_>| {
            crate::ai_valuation_queries::utility_investment(
                &player,
                17, // An unowned railroad, in addition to owned slots0 and3.
                world.categories,
                world.buildings,
                world.businesses,
                world.catalog,
                &world.income.railroads,
                image,
            )
            .unwrap()
        };
        // (.25 * (10.125 + 40.25 + 20.5)) truncates to17.
        assert_eq!(investment(world), 17);
        world.income.utilities = [10000.0; 4];
        assert_eq!(investment(world), 17);
        world.income.railroads[1] += 8.0;
        assert_eq!(investment(world), 19);
    });
}
