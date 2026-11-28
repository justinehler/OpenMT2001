//! Exercise the normal AI runtime with the callback bound by the live host.
use super::*;
use openmt2001::{
    ai_messages,
    ai_runtime::{self, World},
    economy,
    pe::PeImage,
    player_manager::{Auxiliary, Manager, Player},
};

#[test]
fn synthetic_warnings_accumulate_and_replace_the_last_message() {
    let mut host = Host::new([0.; 3]);
    assert_eq!(host.ai_warnings.count, 0);
    assert_eq!(host.ai_warnings.last_message, None);
    host.expect_ai_warnings(2);
    host.running = true;
    for diagnostic in [
        openmt2001::ai_diagnostic::Diagnostic::warning(0x53da30),
        openmt2001::ai_diagnostic::Diagnostic::timetable(false, 99),
    ] {
        assert_eq!(
            ai_callback(
                ai_runtime::Call::Diagnostic(diagnostic),
                &mut host.ai_warnings
            ),
            Ok(0)
        );
        assert_eq!(host.ai_warnings.last_message, Some(diagnostic.to_string()));
        assert!(host.running && host.failure.is_none());
    }
    assert_eq!(host.ai_warnings.count, 2);
    // Dropping the host accepts only the declared count.
}

#[test]
#[should_panic(expected = "unexpected AI warning count")]
fn unexpected_ai_warning_fails_when_host_is_dropped() {
    let mut host = Host::new([0.; 3]);
    ai_callback(
        ai_runtime::Call::Diagnostic(openmt2001::ai_diagnostic::Diagnostic::warning(0x53da30)),
        &mut host.ai_warnings,
    )
    .unwrap();
    drop(host);
}

#[test]
#[should_panic(expected = "unexpected AI warning count")]
fn expected_ai_warning_must_actually_occur() {
    let mut host = Host::new([0.; 3]);
    host.expect_ai_warnings(1);
    drop(host);
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

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
    put(&mut bytes, offset(0x500004), 0x469210);
    bytes
}

fn with_world(run: impl FnOnce(&mut World<'_>, &PeImage<'_>)) {
    let bytes = image_bytes();
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
    let catalog = openmt2001::business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
    let residents = economy::ResidentAccounts {
        active_count: 0,
        nodes: vec![],
        accounts: vec![],
    };
    let mut world = World {
        building_parameters: openmt2001::building_parameters::BuildingKinds::from_image(&image)
            .unwrap(),
        neighbor_cache: &mut openmt2001::ai_neighbors::Cache::default(),
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
        income: openmt2001::ai_finance::IncomeTotals::from_economy(&properties, &utilities),
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
        reference: 0x10010000,
        bytes: vec![0; 0x1e50],
        auxiliaries: vec![Auxiliary {
            reference: 0x20000000,
            bytes: [0; 48],
        }],
    };
    put(&mut player.bytes, 0, 0x500000);
    put(&mut player.bytes, 8, 100);
    put(&mut player.bytes, 0x210, 0x20000000);
    put(&mut player.bytes, 0x21c, 0x20000000);
    put(&mut player.bytes, 0xb68, u32::MAX);
    for index in 0..10 {
        put(&mut player.bytes, 0x1408 + index * 4, u32::MAX);
    }
    player.bytes[0x10] = 1;
    player.bytes[0x1bc] = 1;
    player.bytes[0x1401] = 1;
    player.bytes[0xb4c..0xb4e].copy_from_slice(&u16::MAX.to_le_bytes());
    player
}

fn tick(
    host: &mut Host,
    manager: &mut Manager,
    profiles: &mut [[u8; 0x650]],
    world: &mut World<'_>,
    image: &PeImage<'_>,
    seen: &mut Vec<openmt2001::ai_diagnostic::Diagnostic>,
) {
    if !host.running || host.failure.is_some() {
        return;
    }
    let result = ai_runtime::advance(
        0,
        manager,
        profiles,
        &mut openmt2001::ai_tick::Controls {
            normal_tick: 1,
            day_tick: 10,
            event_gate: false,
        },
        &mut openmt2001::ai_script::AiScript::new().unwrap(),
        image,
        world,
        |call, _, _, _| {
            if let ai_runtime::Call::Diagnostic(diagnostic) = call {
                seen.push(diagnostic);
            }
            ai_callback(call, &mut host.ai_warnings)
                .map(|value| ai_messages::Reply { value, offer: 0 })
        },
    );
    if let Err(error) = result {
        host.fail(error);
    }
}

#[test]
fn ai_warnings_advance_both_live_timetables_and_keep_ticking() {
    with_world(|world, image| {
        let mut manager = Manager {
            bytes: [0; 40],
            players: vec![player()],
        };
        let mut profiles = [[0; 0x650]];
        for (base, cursor, count, pointer, pending, enabled) in [
            (0x410, 0x14, 0x20, 0x1e1c, 0x1e11, 0x239),
            (0x280, 0x18, 0x24, 0x1e20, 0x1e12, 0x23a),
        ] {
            put(&mut profiles[0], cursor, 0);
            put(&mut profiles[0], count, 2);
            put(&mut profiles[0], base, 10);
            put(&mut profiles[0], base + 4, 99);
            put(&mut profiles[0], base + 8, 1000);
            put(&mut profiles[0], base + 12, 0);
            put(
                &mut manager.players[0].bytes,
                pointer,
                0x7f92f8 + base as u32,
            );
            manager.players[0].bytes[pending] = 1;
            manager.players[0].bytes[enabled] = 1;
        }
        let mut host = Host::new([0.; 3]);
        // This fixture deliberately injects two unsupported timetable codes.
        host.expect_ai_warnings(2);
        host.running = true;
        let mut seen = vec![];
        tick(
            &mut host,
            &mut manager,
            &mut profiles,
            world,
            image,
            &mut seen,
        );
        assert_eq!(
            seen,
            vec![
                openmt2001::ai_diagnostic::Diagnostic::timetable(true, 99),
                openmt2001::ai_diagnostic::Diagnostic::timetable(false, 99),
            ]
        );
        assert!(host.failure.is_none(), "{:?}", host.failure);
        assert!(host.running);
        for (base, cursor, pointer, pending) in
            [(0x410, 0x14, 0x1e1c, 0x1e11), (0x280, 0x18, 0x1e20, 0x1e12)]
        {
            assert_eq!(word(&profiles[0], cursor), 1);
            assert_eq!(
                word(&manager.players[0].bytes, pointer),
                0x7f92f8 + base + 8
            );
            assert_eq!(manager.players[0].bytes[pending], 0);
        }
        let age = word(&manager.players[0].bytes, 0x1e48);
        tick(
            &mut host,
            &mut manager,
            &mut profiles,
            world,
            image,
            &mut seen,
        );
        assert_eq!(word(&manager.players[0].bytes, 0x1e48), age + 1);
        assert_eq!(seen.len(), 2);
        assert!(host.running && host.failure.is_none());
        assert_eq!(host.ai_warnings.count, 2);
        assert_eq!(
            host.ai_warnings.last_message,
            seen.last().map(ToString::to_string)
        );
    });
}

#[test]
fn ai_warning_for_missing_offer_preserves_cleanup_and_keeps_ticking() {
    with_world(|world, image| {
        let mut player = player();
        let mut packet = [0; 48];
        put(&mut packet, 4, 0x26);
        put(&mut packet, 0x18, 5);
        put(&mut packet, 0x28, 123);
        put(&mut player.bytes, 0xc, 42);
        player.bytes[0x1406] = 1;
        put(&mut player.bytes, 0x1408, 5);
        player.bytes[0xb86] = 1;
        let mut arena = openmt2001::player_messages::Arena { next: 0x30000000 };
        assert!(
            openmt2001::player_messages::enqueue(&mut player, &packet, || arena.allocate())
                .unwrap()
        );
        let mut manager = Manager {
            bytes: [0; 40],
            players: vec![player],
        };
        let mut host = Host::new([0.; 3]);
        // This fixture deliberately injects completion for a missing offer.
        host.expect_ai_warnings(1);
        host.running = true;
        let mut seen = vec![];
        let mut profiles = [[0; 0x650]];
        tick(
            &mut host,
            &mut manager,
            &mut profiles,
            world,
            image,
            &mut seen,
        );
        assert_eq!(
            seen,
            vec![openmt2001::ai_diagnostic::Diagnostic::warning(0x53da30)]
        );
        assert!(host.running && host.failure.is_none(), "{:?}", host.failure);
        assert_eq!(word(&manager.players[0].bytes, 0xc), u32::MAX);
        assert_eq!(word(&manager.players[0].bytes, 0x214), 0);
        assert_eq!(manager.players[0].bytes[0x1406], 0);
        assert_eq!(word(&manager.players[0].bytes, 0x1408), u32::MAX);
        assert_eq!(manager.players[0].bytes[0xb74], 1);
        assert_eq!(manager.players[0].bytes[0xb86], 1); // Missing identity does not cancel another offer.
        tick(
            &mut host,
            &mut manager,
            &mut profiles,
            world,
            image,
            &mut seen,
        );
        assert!(host.running && host.failure.is_none());
        assert_eq!(seen.len(), 1);
    });
}

#[test]
fn ai_fatal_kind_zero_stops_live_host() {
    with_world(|world, image| {
        world.auction = Some(ai_messages::Auction {
            participant: 0,
            winner: -1,
            price: 0,
        });
        let mut player = player();
        let mut packet = [0; 48];
        put(&mut packet, 4, 0x29);
        put(&mut packet, 0x18, u32::MAX);
        put(&mut packet, 0x1c, u32::MAX);
        let mut arena = openmt2001::player_messages::Arena { next: 0x30000000 };
        assert!(
            openmt2001::player_messages::enqueue(&mut player, &packet, || arena.allocate())
                .unwrap()
        );
        let mut manager = Manager {
            bytes: [0; 40],
            players: vec![player],
        };
        let mut host = Host::new([0.; 3]);
        host.running = true;
        let mut seen = vec![];
        let mut profiles = [[0; 0x650]];
        tick(
            &mut host,
            &mut manager,
            &mut profiles,
            world,
            image,
            &mut seen,
        );
        assert_eq!(
            seen,
            vec![openmt2001::ai_diagnostic::Diagnostic::original(0, 0x53cc80).unwrap()]
        );
        assert!(!host.running);
        assert_eq!(host.ai_warnings.count, 0);
        assert_eq!(host.ai_warnings.last_message, None);
        assert!(
            host.failure
                .as_ref()
                .unwrap()
                .contains("AI fatal diagnostic")
        );
        let stopped = manager.players[0].bytes.clone();
        tick(
            &mut host,
            &mut manager,
            &mut profiles,
            world,
            image,
            &mut seen,
        );
        assert_eq!(manager.players[0].bytes, stopped);
        host.set_running(true);
        assert!(!host.running);
    });
}

#[test]
fn ai_unsupported_gameplay_operation_still_stops_live_host() {
    with_world(|world, image| {
        let mut player = player();
        // A vtable target without a runtime implementation must still reach
        // the host's explicit unsupported-operation failure.
        put(&mut player.bytes, 0, 0x500008);
        let mut manager = Manager {
            bytes: [0; 40],
            players: vec![player],
        };
        let mut host = Host::new([0.; 3]);
        host.running = true;
        let mut seen = vec![];
        tick(
            &mut host,
            &mut manager,
            &mut [[0; 0x650]],
            world,
            image,
            &mut seen,
        );
        assert!(seen.is_empty());
        assert!(!host.running);
        assert!(
            host.failure
                .as_ref()
                .unwrap()
                .contains("Live session requires Ai(Decision")
        );
    });
}
