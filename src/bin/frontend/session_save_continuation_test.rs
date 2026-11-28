//! Load-boundary effects applied independently to the uninterrupted control.
//! The subsequent comparisons still check complete continued state.
use super::*;

pub(crate) fn load_boundary(
    session: &mut Session<'_>,
    loaded: &Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    save: &SaveFile<'_>,
    native_allocation_next: u32,
) {
    allocation_cursor(session, loaded, native_allocation_next);
    let city = &mut session.world.city;
    resident_poses(
        &mut city.ledgers.residents,
        &mut city.creation.auxiliary,
        &assets.board.category_geometry().centers,
    );
    if save.clock().unwrap().timing[5] != 2 && city.auction.controller.record[0x7d] != 0 {
        // 465810/41db40 reopens announcements with whole signed seconds.
        // The queued room-start deadline and controller snapshot stay exact.
        let ticks = word(&city.auction.controller.record, 0x80) as i32;
        assert_eq!(city.auction.presentation.countdown, ticks as u32);
        city.auction.presentation.countdown = (ticks / 50).wrapping_mul(50) as u32;
    }
    let extension = extension::Extension::parse(save).unwrap().unwrap();
    if !extension.records.contains_key(&14) {
        // These controls are Scenario3 without opponents. Auction snapshots
        // retain their separate AI continuation policy; normal loads rebuild
        // metrics/demand and invalidate market prices (4b9f00/47f8e0).
        assert!(!session.roster.status.has_ai);
        let players = session.roster.manager.players.iter().collect::<Vec<_>>();
        openmt2001::player_metrics::reset(
            &mut city.ai_neighbors,
            &mut city.ownership_refresh_counter,
            &mut session.population.business_counts,
            &mut session.population.market,
            &players,
            &city.categories,
            &city.ledgers.businesses,
            &city.ledgers.residents,
            assets.image,
        )
        .unwrap();
        city.ai_demand
            .rebuild(
                &city.ledgers.residents,
                &city.ledgers.businesses,
                &city.buildings,
                &city.catalog.info,
                &city.catalog.products,
                &city.commodities.rows,
                assets.image,
            )
            .unwrap();
    }
    // A new process restarts accumulators instead of importing record12.
    // No saved owner is held by these control runs; owner retirement itself
    // is covered by pause_test and the load-menu regression.
    assert!(!session.world.pause.has_reason());
    for timer in &mut session.hooks.timers {
        timer[12..16].fill(0);
    }
    assert_eq!(session.world.pause, loaded.world.pause);
    assert_eq!(session.hooks.timers, loaded.hooks.timers);
    for (slot, (expected, actual)) in session
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .zip(&loaded.world.ledgers.residents.nodes)
        .enumerate()
    {
        assert_eq!(
            expected.as_bytes(),
            actual.as_bytes(),
            "load resident {slot}"
        );
        assert_eq!(expected.active_byte(), actual.active_byte());
        assert_eq!(expected.auxiliary_reference(), actual.auxiliary_reference());
    }
    assert_eq!(
        session.world.creation.auxiliary.slots(),
        loaded.world.creation.auxiliary.slots(),
        "load actor pose and all omitted model/controller bytes"
    );
    assert_eq!(
        session.road_traffic.admission.random, loaded.road_traffic.admission.random,
        "load movement does not draw from the traffic RNG"
    );
}

pub(super) fn allocation_cursor(
    control: &mut Session<'_>,
    loaded: &Session<'_>,
    native_allocation_next: u32,
) {
    // Observe the native arena before extension restoration. Its rebuilt AI
    // lists must not overlap later message allocations. Applying the same
    // floor to the control keeps future message references comparable too.
    let expected = control.allocation_next.max(native_allocation_next);
    assert_eq!(loaded.allocation_next, expected);
    control.allocation_next = expected;
}

fn resident_poses(
    residents: &mut openmt2001::economy::ResidentAccounts,
    actors: &mut openmt2001::resident::AuxiliaryPool,
    centers: &[[f32; 2]; 52],
) {
    for resident in residents.nodes.iter_mut().filter(|r| r.is_active()) {
        let reference = resident.auxiliary_reference();
        // Native construction starts with zero yaw. Only the pose and visible
        // bit survive into the saved actor; model/controller bytes do not reset.
        let mut native_actor = [0; 284];
        resident
            .restore_saved_movement(reference, &mut native_actor, centers)
            .unwrap();
        let actor = actors.get_mut((reference - 1) as usize).unwrap();
        actor[4..16].copy_from_slice(&native_actor[4..16]);
        actor[0x14..0x18].copy_from_slice(&native_actor[0x14..0x18]);
        actor[0xb8] = (actor[0xb8] & !1) | (native_actor[0xb8] & 1);
    }
}

#[test]
fn control_load_step_changes_only_resident_pose() {
    use openmt2001::resident::{AuxiliaryPool, Resident};
    let mut residents = openmt2001::economy::ResidentAccounts::new_pool();
    let mut actors = AuxiliaryPool::for_residents();
    actors.allocate(&[0xa5; 284]).unwrap();
    let mut bytes = *residents.nodes[0].as_bytes();
    bytes[0x7c] = 0;
    bytes[0x7e] = 0; // Even a clear moving flag must advance once on load.
    put(&mut bytes, 0xf4, 480_f32.to_bits());
    put(&mut bytes, 0xfc, 460_f32.to_bits());
    put(&mut bytes, 0x100, 500_f32.to_bits());
    put(&mut bytes, 0x108, 460_f32.to_bits());
    put(&mut bytes, 0x110, 1_f32.to_bits());
    put(&mut bytes, 0x11c, 20_f32.to_bits());
    put(&mut bytes, 0x120, 2_f32.to_bits());
    residents.nodes[0] = Resident::from_runtime_bytes(bytes, 1, 1);
    residents.active_count = 1;
    let inactive = *residents.nodes[1].as_bytes();
    resident_poses(&mut residents, &mut actors, &[[7., 8.]; 52]);
    let resident = residents.nodes[0].as_bytes();
    assert_eq!(word(resident, 0xf4), 482_f32.to_bits());
    assert_eq!(word(resident, 0x11c), 18_f32.to_bits());
    assert_eq!(resident[0x7e] & 4, 4);
    assert_eq!(word(resident, 0xac), 5_f32.to_bits());
    assert_eq!(word(resident, 0xb4), 2_f32.to_bits());
    for (at, (&before, &after)) in bytes.iter().zip(resident).enumerate() {
        if !matches!(at, 0x7e | 0xac..=0xb7 | 0xf0..=0xf7 | 0x11c..=0x11f) {
            assert_eq!(before, after, "non-pose resident byte {at:x}");
        }
    }
    for (at, &byte) in actors.slots()[0].iter().enumerate() {
        if !matches!(at, 4..=15 | 0x14..=0x17 | 0xb8) {
            assert_eq!(byte, 0xa5, "non-pose actor byte {at:x}");
        }
    }
    assert_eq!(residents.nodes[0].active_byte(), 1);
    assert_eq!(residents.nodes[0].auxiliary_reference(), 1);
    assert_eq!(actors.slots()[0][0xb8] & !1, 0xa4);
    assert_eq!(residents.nodes[1].as_bytes(), &inactive);
}
