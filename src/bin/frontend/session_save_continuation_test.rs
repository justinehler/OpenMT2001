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
        rebuilt_metrics(session, assets);
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
    residents_match(session, loaded);
    // #537: a load clears every transport effect row and restarts the ship
    // frame at one, so a ship that was mid-route at the save does not return.
    // Everything else in the transport record must still continue exactly.
    session.transport.clear_effects_for_load();
    assert_eq!(
        session.transport.hourly.effects,
        loaded.transport.hourly.effects
    );
    assert_eq!(
        session.transport.environment.frame,
        loaded.transport.environment.frame
    );
    assert_eq!(
        session.road_traffic.admission.random, loaded.road_traffic.admission.random,
        "load movement does not draw from the traffic RNG"
    );
}

/// Normal loads rebuild player metrics and AI demand and invalidate market
/// prices (4b9f00/47f8e0). Apply the same rebuild to the control.
fn rebuilt_metrics(session: &mut Session<'_>, assets: &crate::fresh_city::CommandAssets<'_, '_>) {
    let city = &mut session.world.city;
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

/// #247: load advances every active resident's saved movement once. Apply the
/// same step to the control, then require identical residents and actors.
fn resident_movement(
    control: &mut Session<'_>,
    loaded: &Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
) {
    let city = &mut control.world.city;
    resident_poses(
        &mut city.ledgers.residents,
        &mut city.creation.auxiliary,
        &assets.board.category_geometry().centers,
    );
    residents_match(control, loaded);
}

fn residents_match(session: &Session<'_>, loaded: &Session<'_>) {
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
}

/// Opponent bytes that native load rebuilds instead of restoring. The save
/// keeps only each player's 0x1b8-byte account. 4bbd20 -> 478ea0 -> 466870
/// then resets the AI controller, and 4d7a80(false) skips the fresh-game
/// build-delay draw, so the next deadline stays zero.
const REBUILT_OPPONENT_FIELDS: [std::ops::Range<usize>; 6] = [
    0x8..0xc,       // offer-service countdown, reset to 15
    0x240..0x244,   // controller state, reset to idle (2)
    0xb4c..0xb50,   // strategy and finance deadlines, reset to 0
    0xc98..0xcd4,   // own finance snapshot, cleared
    0xcd8..0xe1c,   // rival finance rows, outside the saved account
    0x1e1c..0x1e24, // timetable event pointers, cleared
];

/// Apply native load's opponent rebuild to the uninterrupted control. Every
/// other player and profile byte must already match, so a genuine save/load
/// loss still fails here. Afterwards both sessions must continue identically.
fn loaded_opponents(control: &mut Session<'_>, loaded: &Session<'_>) {
    assert_eq!(
        control.roster.manager.players.len(),
        loaded.roster.manager.players.len()
    );
    for (slot, (expected, actual)) in control
        .roster
        .manager
        .players
        .iter_mut()
        .zip(&loaded.roster.manager.players)
        .enumerate()
    {
        if word(&actual.bytes, 0x154) != 1 {
            assert_eq!(expected.bytes, actual.bytes, "loaded player {slot}");
            continue;
        }
        assert_eq!(
            word(&actual.bytes, 8),
            15,
            "loaded opponent {slot} countdown"
        );
        assert_eq!(
            word(&actual.bytes, 0x240),
            2,
            "loaded opponent {slot} state"
        );
        assert_eq!(
            &actual.bytes[0xb4c..0xb50],
            &[0; 4],
            "loaded opponent {slot} deadlines"
        );
        assert_eq!(expected.bytes.len(), actual.bytes.len());
        for (at, (before, after)) in expected.bytes.iter().zip(&actual.bytes).enumerate() {
            if before != after {
                assert!(
                    REBUILT_OPPONENT_FIELDS
                        .iter()
                        .any(|range| range.contains(&at)),
                    "opponent {slot} +{at:#x} changed across load: {before} -> {after}"
                );
            }
        }
        expected.bytes.clone_from(&actual.bytes);
    }
    // Load also reruns ai.lua for the rebuilt opponents. Its timetable setup
    // zeroes each opponent profile's primary and secondary cursors (+0x14,
    // +0x18), and the first poll after 466870 cleared the opponent's
    // timetable pointers selects from there. The rest of every profile is
    // rebuilt identically.
    let opponents = loaded
        .roster
        .manager
        .players
        .iter()
        .filter(|player| word(&player.bytes, 0x154) == 1)
        .map(|player| word(&player.bytes, 0x150) as usize)
        .collect::<Vec<_>>();
    assert_eq!(control.roster.profiles.len(), loaded.roster.profiles.len());
    for (index, (expected, actual)) in control
        .roster
        .profiles
        .iter_mut()
        .zip(&loaded.roster.profiles)
        .enumerate()
    {
        if opponents.contains(&index) {
            assert_eq!(
                &actual[0x14..0x1c],
                &[0; 8],
                "loaded profile {index} cursors"
            );
            expected[0x14..0x1c].copy_from_slice(&actual[0x14..0x1c]);
        }
        assert_eq!(expected, actual, "loaded profile {index}");
    }
}

/// Native load effects for a city with opponents: the resident movement step
/// (#247), the metrics and demand rebuild, and the rebuilt opponents and
/// ownership lists (#254, #351). Apply them all to the uninterrupted control.
pub(crate) fn opponent_load_boundary(
    control: &mut Session<'_>,
    loaded: &Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
) {
    assert!(loaded.roster.status.has_ai);
    resident_movement(control, loaded, assets);
    // 4b9f00 rebuilds metrics and demand before 4bbd20 rebuilds the AI.
    rebuilt_metrics(control, assets);
    loaded_opponents(control, loaded);
    // 4706e0 copies each commodity name with its terminator only, so the
    // bytes after it keep whatever the sorted rows held before the rebuild.
    let rows = &mut control.world.ai_demand.rows;
    for (index, (row, rebuilt)) in rows
        .iter_mut()
        .zip(&loaded.world.ai_demand.rows)
        .enumerate()
    {
        let name = 8 + rebuilt[8..].iter().position(|&b| b == 0).unwrap() + 1;
        assert_eq!(row[..name], rebuilt[..name], "demand row {index}");
        row[name..].copy_from_slice(&rebuilt[name..]);
    }
    // Each 466870 reset clears the shared demand-ready flag again.
    assert_eq!(loaded.world.ai_demand.ready, 0);
    control.world.ai_demand.ready = 0;
    // The rebuilt ownership lists keep each list's identity and businesses;
    // 4708a0 recomputes every entry's value (+20) from the current city.
    let lists = &mut control.roster.ai_ownership.lists;
    assert_eq!(
        lists.keys().collect::<Vec<_>>(),
        loaded.roster.ai_ownership.lists.keys().collect::<Vec<_>>()
    );
    for (reference, rebuilt) in &loaded.roster.ai_ownership.lists {
        let list = lists.get_mut(reference).unwrap();
        assert_eq!(list.len(), rebuilt.len(), "ownership list {reference:#x}");
        for (entry, rebuilt) in list.iter_mut().zip(rebuilt) {
            assert_eq!(entry[..20], rebuilt[..20], "ownership list {reference:#x}");
            assert_eq!(entry[24..], rebuilt[24..], "ownership list {reference:#x}");
            entry[20..24].copy_from_slice(&rebuilt[20..24]);
        }
    }
    // Both price caches are pending a refresh, so their stale values are never
    // read; a cold process simply starts with different stale values.
    let market = &mut control.population.market;
    assert!(market.dirty && loaded.population.market.dirty);
    market.prices = loaded.population.market.prices;
    market.rent = loaded.population.market.rent;
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
