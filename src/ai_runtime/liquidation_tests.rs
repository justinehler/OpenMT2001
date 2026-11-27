//! Exercise cash-recovery initiation through the frontend's controller entry.
use super::*;
use crate::player_manager::{Auxiliary, put};

fn image_bytes() -> Vec<u8> {
    // Synthetic PE containing only the constants and action table we use.
    let mut bytes = vec![0; 0x150000];
    bytes[..2].copy_from_slice(b"MZ");
    put(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut bytes, 0xb4, 0x400000);
    put(&mut bytes, 0xc8, 0x14f000);
    put(&mut bytes, 0xcc, 0x1000);
    let offset = |address: usize| address - 0x400000 + 0x1000;
    for address in [0x52502c, 0x525248, 0x525b1c, 0x525b20, 0x525c50] {
        put(&mut bytes, offset(address), 1.0f32.to_bits());
    }
    put(&mut bytes, offset(0x525058), 500.0f32.to_bits());
    // Cash-recovery action table: its mode word enables target production.
    put(&mut bytes, offset(0x53fc28 + 7 * 264 + 0x100), 7);
    put(&mut bytes, offset(0x500004), 0x12345678);
    bytes
}

fn cash_recovery(links: &[i32]) -> Result<Vec<[u32; 3]>, String> {
    let bytes = image_bytes();
    let image = PeImage::parse(&bytes)?;
    let parameters = crate::building_parameters::BuildingKinds::from_image(&image)?;
    let mut buildings = vec![[0; 916]; links.len()];
    let mut businesses = [[0; 544]; 3];
    for (id, b) in businesses.iter_mut().enumerate() {
        b[10] = if id < 2 { 2 } else { 3 };
        b[0x18..0x1a].copy_from_slice(&[100u16, 50, 25][id].to_le_bytes());
    }
    for (index, (building, &id)) in buildings.iter_mut().zip(links).enumerate() {
        crate::building_pool::initialize_with_parameters(
            building,
            &crate::building_pool::Initialization {
                rectangle: [0, 0, 1, 1],
                height: 1,
                center: [0.0; 2],
                variation: 0,
                kind: 0,
                quality: 0,
                total_cost: 1000.0f32.to_bits(),
                date: [0; 2],
            },
            &parameters,
        )?;
        assert_eq!(word(building, 0x1c), u32::MAX);
        if id >= 0 && (id as usize) < businesses.len() {
            let business = &mut businesses[id as usize];
            business[8..10].copy_from_slice(&(index as i16).to_le_bytes());
            put(building, 0x14, 4); // Completed, with an opened business.
        } else {
            // Construction retains the no-business link from initialization.
            crate::pending_world::State::default().construct(0, building, 0, 2)?;
        }
        put(building, 0x1c, id as u32);
        if index + 1 < links.len() {
            put(building, 0x44, 0x591318 + (index as u32 + 1) * 916);
        }
    }
    let mut categories = [[0; 380]; 52];
    categories[0][5] = 2;
    put(&mut categories[0], 0x84, 1000.0f32.to_bits());
    // Prevent the low-property rent addback from cancelling the rent penalty.
    put(&mut categories[0], 0x7c, 10);
    if !links.is_empty() {
        put(&mut categories[0], 0x15c, 0x591318);
    }
    let mut player = Player {
        reference: 0x10010000,
        bytes: vec![0; 0x1e50],
        auxiliaries: vec![Auxiliary {
            reference: 1,
            bytes: [0; 48],
        }],
    };
    put(&mut player.bytes, 0, 0x500000);
    put(&mut player.bytes, 4, (-100.0f32).to_bits());
    put(&mut player.bytes, 8, 1); // No pending offers to service this tick.
    put(&mut player.bytes, 0x21c, 1);
    put(&mut player.bytes, 0x240, 2);
    // Retain the owned category in both controller ownership views.
    for group in player.bytes[0xaac..0xb3c].as_chunks_mut::<12>().0 {
        group[1..5].fill(0xff);
    }
    player.bytes[0xaad] = 0;
    put(&mut player.bytes, 0xab4, 1);
    // Enter with cash recovery already selected; this tick initiates it.
    put(&mut player.bytes, 0xf3c, 2);
    put(&mut player.bytes, 0x1040, 6);
    player.bytes[0x10] = 1;
    player.bytes[0x178] = 1;
    player.bytes[0x1bc] = 1;
    player.bytes[0x1401] = 1;
    player.bytes[0x1402] = 1;
    let mut manager = Manager {
        bytes: [0; 40],
        players: vec![player],
    };
    let catalog = crate::business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
    let residents = crate::economy::ResidentAccounts {
        active_count: 0,
        nodes: vec![],
        accounts: vec![],
    };
    let mut world = World {
        building_parameters: parameters,
        neighbor_cache: &mut Default::default(),
        ownership_refresh_counter: &mut 0,
        utility_enabled: [false; 4],
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
        strategy_time: Some(crate::ai_selection::Time { hour: 12, year: 1 }),
        ownership_tick: None,
        strategy_mode: 0,
        business_counts: None,
        categories: &mut categories,
        buildings: &buildings,
        businesses: &mut businesses,
        ownership: &mut Default::default(),
        allocation_next: &mut 0x10000000,
        message_routing: None,
        global_reset: &mut 0,
        income: crate::ai_finance::IncomeTotals {
            railroads: [0.0; 4],
            utilities: [0.0; 4],
        },
        auction: None,
        auction_blocked: None,
        auction_guard: None,
        offer_auction: None,
        auction_state: None,
        cash_statuses: &[0; 6],
    };
    let mut completed_tick = false;
    advance(
        0,
        &mut manager,
        &mut [[0; 0x650]],
        &mut ai_tick::Controls {
            normal_tick: 0,
            day_tick: 0,
            event_gate: false,
        },
        &mut crate::ai_script::AiScript::new()?,
        &image,
        &mut world,
        |call, _, _, _| match call {
            Call::Decision(ai_tick::Call {
                address: 0x12345678,
                ..
            }) => {
                completed_tick = true;
                Ok(Default::default())
            }
            other => Err(format!("unexpected live controller callback: {other:?}")),
        },
    )?;
    assert!(completed_tick);
    let player = &manager.players[0];
    assert_eq!(word(&player.bytes, 0x240), 3);
    // Selection clears the category; production restores it after traversal.
    assert_eq!(word(&player.bytes, 0xb68), u32::MAX);
    let base = word(&player.bytes, 0x13e8);
    Ok((0..word(&player.bytes, 0x13ec))
        .map(|i| {
            let row = &world.liquidation_targets[&(base + i * 12)];
            [word(row, 0), word(row, 4), word(row, 8)]
        })
        .collect())
}

#[test]
fn cash_recovery_with_unfinished_buildings_completes_live_tick() {
    assert_eq!(cash_recovery(&[-1]).unwrap(), vec![[300, 1, 0]]);
    assert_eq!(
        cash_recovery(&[-1, -1]).unwrap(),
        cash_recovery(&[]).unwrap()
    );
}

#[test]
fn cash_recovery_preserves_completed_rent_and_target_order() {
    // Owned rents 100+50 and other rent 25 reduce the land score by 175.
    // Equal business scores also expose changes to their traversal order.
    let completed = cash_recovery(&[0, 1, 2]).unwrap();
    assert_eq!(completed, vec![[240, 2, 0], [240, 2, 1], [125, 1, 0]]);
    for links in [
        &[-1, 0, 1, 2][..],
        &[0, -1, 1, 2],
        &[0, 1, -1, 2],
        &[0, 1, 2, -1],
        &[-1, 0, -1, 1, 2, -1],
    ] {
        assert_eq!(cash_recovery(links).unwrap(), completed, "{links:?}");
    }
}

#[test]
fn cash_recovery_still_rejects_other_invalid_business_links() {
    for id in [-2, 3] {
        assert!(
            cash_recovery(&[id])
                .unwrap_err()
                .contains("native liquidation rent invalid business")
        );
    }
}
