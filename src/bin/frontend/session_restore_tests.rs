use super::super::test_observers::{assert_account_postings, cash};
use super::*;
use std::{fs, path::Path};
#[test]
fn immediate_terms_future_gate_matches_native_unsigned_boundaries() {
    // Native 450a30 uses unsigned JA, not signed or modular distance;
    // 480900 recursively dispatches a future -1 packet deadline.
    for (clock, due, queued) in [
        (50, 51, true),
        (50, 50, false),
        (50, 0, false),
        (50, u32::MAX, false),
        (0x7fff_ffff, 0x8000_0000, true),
        (0x8000_0000, 0x7fff_ffff, false),
        (0x8000_0000, 0x8000_0001, true),
        (0xffff_fffe, 1, false),
        (0xffff_fffe, u32::MAX, false),
        (u32::MAX, u32::MAX, false),
    ] {
        assert_eq!(
            native_future_packet_due(due, clock),
            queued,
            "clock {clock:08x}, packet deadline {due:08x}"
        );
    }
}
#[test]
fn decodes_native_command18_saved_packet() {
    // 464060 submits 20-byte transport header plus the 44-byte
    // business-settings payload; preserve signed targets and float bits.
    let mut packet = [0u8; 64];
    packet[4..8].copy_from_slice(&0x18u32.to_le_bytes());
    packet[8..12].copy_from_slice(&64u32.to_le_bytes());
    packet[20..24].copy_from_slice(&17u32.to_le_bytes());
    for (i, target) in [-1i16, 2, i16::MAX, i16::MIN, 0, 100].iter().enumerate() {
        packet[24 + i * 2..26 + i * 2].copy_from_slice(&target.to_le_bytes());
    }
    let prices: [u32; 6] = [
        0x7fc01234, 0x80000000, 0x3f800000, 0x40400000, 0x40800000, 0x40a00000,
    ];
    for (i, bits) in prices.iter().enumerate() {
        packet[36 + i * 4..40 + i * 4].copy_from_slice(&bits.to_le_bytes());
    }
    packet[60..64].copy_from_slice(&23.5f32.to_bits().to_le_bytes());
    let openmt2001::live_commands::Command::Terms(terms) = saved_command(&packet).unwrap() else {
        panic!("command18 did not decode as terms")
    };
    assert_eq!(terms.business_index, 17);
    assert_eq!(
        terms.terms.stock_targets,
        [-1, 2, i16::MAX, i16::MIN, 0, 100]
    );
    assert_eq!(terms.terms.prices.map(f32::to_bits), prices);
    assert_eq!(terms.terms.rent.to_bits(), 23.5f32.to_bits());
    assert!(saved_command(&packet[..63]).is_err());
}
#[test]
#[ignore = "requires original v1.4 inputs and both supplied original saves"]
fn loads_supplied_original_saves_into_sessions() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(loads_supplied_original_saves_into_sessions_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn loads_supplied_original_saves_into_sessions_inner() {
    let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let built = read("OPENMT2001_BUILD_FILE");
    let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = read("OPENMT2001_PARAMETERS");
    let timetable = read("OPENMT2001_TIMETABLE");
    let scripts = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .join("scripts");
    let era_styles = super::super::load_era_styles(
        Path::new(&openmt2001::fixtures::path("PARAMETERS"))
            .parent()
            .unwrap(),
    )
    .unwrap();
    let names = std::array::from_fn(|_| Vec::new());
    for (key, scenario, difficulty, count, businesses) in [
        ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, 340, 15),
        ("OPENMT2001_SAVE_TWO", "sandbox", 0, 500, 0),
    ] {
        let saved = read(key);
        let inputs = || Inputs {
            tree_locators: None,
            image: &image,
            board: &board,
            file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                &image, &index, &archive,
            )
            .unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(
                &image, &index, &archive,
            )
            .unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario,
            difficulty,
            selected_character: 4,
            profile_name: b"Saved session",
            character_names: &names,
        };
        let policy = || Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 0x11c],
        };
        let load = |saved_bytes: &[u8]| Session::load_original(inputs(), policy(), saved_bytes);
        let session = load(&saved).unwrap();
        assert_eq!(session.world.creation.auxiliary.used(), count as usize);
        let mut references = std::collections::BTreeSet::new();
        for node in session
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .filter(|node| node.is_active())
        {
            let reference = node.auxiliary_reference();
            assert!((1..=count).contains(&reference));
            assert!(
                references.insert(reference),
                "loaded residents share auxiliary slot"
            );
        }
        assert_eq!(references.len(), count as usize);
        let mut appearance_categories = session.world.categories;
        let mut appearance_buildings = session.world.buildings.clone();
        let mut appearance_calls = Vec::new();
        restore::post_load_appearance_walk(
            &mut appearance_categories,
            &mut appearance_buildings,
            0x591318,
            |building, alias, _, _| {
                appearance_calls.push((building, alias));
                Ok(())
            },
        )
        .unwrap();
        println!(
            "{key}: {} original 485520 post-load callbacks",
            appearance_calls.len()
        );
        assert_eq!(
            appearance_calls.len(),
            businesses,
            "supplied save post-load appearance callback count"
        );
        if key == "OPENMT2001_SAVE_ONE" {
            let counts: Vec<_> = appearance_calls
                .iter()
                .map(|(building, _)| {
                    session.models.appearance_candidates[building]
                        .renderer_candidates
                        .len()
                })
                .collect();
            // Native 488230 -> 4d4a00 -> 485200, v1.4 SHA pinned by
            // tools/native_save_appearance_counts.py at FPCW 0x037f.
            assert_eq!(counts, [3, 2, 5, 3, 6, 9, 3, 2, 3, 4, 4, 3, 4, 5, 3]);
            let active = appearance_calls
                .iter()
                .flat_map(|(building, _)| {
                    &session.models.appearance_candidates[building].renderer_templates
                })
                .filter(|template| u32::from_le_bytes(template.row[..4].try_into().unwrap()) == 1)
                .count();
            assert_eq!(active, 45, "native 4846f0 active decoration rows");
        }
        // These are retained V2 counts; original scene registration may
        // populate the native +2e4 count differently before 485520.
        println!(
            "{key}: retained 485520 model counts {:?}",
            appearance_calls
                .iter()
                .map(|(building, _)| u32::from_le_bytes(
                    session.world.buildings[*building][0x2e4..0x2e8]
                        .try_into()
                        .unwrap()
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(session.world.ledgers.residents.active_count, count);
        assert_eq!(
            session
                .world
                .ledgers
                .businesses
                .iter()
                .filter(|r| r[0x21c] != 0)
                .count(),
            businesses
        );
        assert_eq!(session.roster.manager.players.len(), 1);
        assert!(session.saved_cameras.is_some());
        let mut expected_demand = openmt2001::ai_demand::State::default();
        let city = &session.world.city;
        expected_demand
            .rebuild(
                &city.ledgers.residents,
                &city.ledgers.businesses,
                &city.buildings,
                &city.catalog.info,
                &city.catalog.products,
                &city.commodities.rows,
                &image,
            )
            .unwrap();
        let demands = |rows: &[[u8; 264]; 33]| {
            let mut values: Vec<_> = rows
                .iter()
                .map(|r| {
                    (
                        u32::from_le_bytes(r[0..4].try_into().unwrap()),
                        u32::from_le_bytes(r[4..8].try_into().unwrap()),
                    )
                })
                .collect();
            values.sort_unstable();
            values
        };
        assert_eq!(
            demands(&city.ai_demand.rows),
            demands(&expected_demand.rows),
            "post-load demand counts must reflect saved residents and businesses"
        );
        assert_eq!(city.ai_demand.ready, 1);
        let mut expected_footprints = [[0; 2]; 52];
        openmt2001::ai_space::refresh_all(&mut expected_footprints, &city.categories, &image)
            .unwrap();
        assert_eq!(
            city.category_footprints, expected_footprints,
            "post-load footprints must reflect reconstructed geometry"
        );
        if key == "OPENMT2001_SAVE_ONE" {
            assert_eq!(city.category_footprints[36], [1, 6]);
        }
        let source = SaveFile::parse(&saved).unwrap();
        if key == "OPENMT2001_SAVE_ONE" {
            let current = session
                .roster
                .manager
                .players
                .iter()
                .position(|p| p.reference == session.roster.status.current_player)
                .unwrap();
            let id = session
                .world
                .ledgers
                .businesses
                .iter()
                .position(|business| business[0x21c] != 0 && business[10] as usize == current + 1)
                .unwrap();
            let mut packet = [0; 28];
            openmt2001::business_sale::Command {
                business: id as u32,
                player: current as u8,
            }
            .write_packet(&mut packet);
            packet[8..12].copy_from_slice(&28u32.to_le_bytes());
            let mut payload = [0; 24].to_vec();
            payload[8..12].copy_from_slice(&session.world.clock.timing[0].to_le_bytes());
            payload.extend_from_slice(&packet);
            let at = source
                .sections
                .iter()
                .find(|s| s.tag == 0x10000)
                .unwrap()
                .offset;
            let mut sale_bytes = saved[..at].to_vec();
            sale_bytes.extend_from_slice(&0x4000u32.to_le_bytes());
            sale_bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            sale_bytes.extend_from_slice(&payload);
            sale_bytes.extend_from_slice(&saved[at..]);
            assert!(
                load(&sale_bytes)
                    .err()
                    .unwrap()
                    .contains("presentation owner")
            );
            let (sold, _) =
                Session::load_original_with_presentation(inputs(), policy(), &sale_bytes).unwrap();
            let building = i16::from_le_bytes(
                session.world.ledgers.businesses[id][8..10]
                    .try_into()
                    .unwrap(),
            ) as usize;
            assert_eq!(sold.world.buildings[building][0x2d], 0);
            assert_eq!(sold.world.ledgers.businesses[id][10], 0);
            assert_eq!(
                sold.models.appearance_candidates[&building],
                session.models.appearance_candidates[&building]
            );
            let mut categories = sold.world.categories;
            let mut buildings = sold.world.buildings.clone();
            let mut callbacks = Vec::new();
            restore::post_load_appearance_walk(
                &mut categories,
                &mut buildings,
                0x591318,
                |slot, alias, _, _| {
                    if slot == building {
                        callbacks.push(alias);
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(callbacks, [restore::AppearanceAlias::Business(id)]);
            for slot in 0..15 {
                let identity = &session.models.appearance_candidates[&slot];
                assert!(
                    checked_positive_sale_appearance(identity, session.world.buildings[slot][8]),
                    "original positive sale identity {slot}"
                );
            }
            let mut unknown_model = session.models.appearance_candidates[&building].clone();
            unknown_model
                .renderer_templates
                .iter_mut()
                .find(|row| u32::from_le_bytes(row.row[..4].try_into().unwrap()) == 1)
                .unwrap()
                .descriptor = 0x0300_0001;
            assert!(!checked_positive_sale_appearance(
                &unknown_model,
                session.world.buildings[building][8]
            ));
            assert!(!checked_positive_sale_appearance(
                &session.models.appearance_candidates[&building],
                session.world.buildings[building][8].wrapping_add(1)
            ));
        }
        let section = |tag| {
            source
                .sections
                .iter()
                .find(|record| record.tag == tag)
                .unwrap()
                .payload
        };
        assert_eq!(session.world.pending_world.to_bytes(), section(0x800));
        assert_eq!(session.world.objective.to_bytes(), section(0x10000));
        assert_eq!(session.history.as_bytes(), section(0x20000));
        assert_eq!(session.saved_cameras.unwrap()[0], section(0x100));
        assert_eq!(session.saved_cameras.unwrap()[1], section(0x100000));
        let saved_players = source.players().unwrap();
        assert_eq!(
            session.roster.manager.players[0].bytes[4..0x1bc],
            *saved_players[0].bytes
        );
        assert_eq!(session.saved_player_metadata[0], *saved_players[0].metadata);
        for (record, bytes) in session
            .world
            .ledgers
            .businesses
            .iter()
            .zip(section(0x20).chunks_exact(0x220))
        {
            assert_eq!(record.as_slice(), bytes);
        }
        for (record, bytes) in session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .zip(section(0x1000).chunks_exact(0x90))
        {
            assert_eq!(record.as_bytes(), bytes);
        }
        for category in 0..52 {
            assert_eq!(
                session.world.categories[category][5],
                source.category_owner(category).unwrap()
            );
        }
        if key == "OPENMT2001_SAVE_ONE" {
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let original_clock = session.world.clock;
            let original_count = session.world.ledgers.residents.active_count;
            assert!(openmt2001::building_pool::can_place(
                session.world.categories[13][0x9c..0xcd].try_into().unwrap(),
                [0, 0, 2, 2],
            ));
            let mut with_pending = saved.clone();
            let building_at = source
                .sections
                .iter()
                .find(|record| record.tag == 4)
                .unwrap()
                .offset
                + 8;
            let mut new_building = [0u8; 64];
            new_building.copy_from_slice(&with_pending[building_at..building_at + 64]);
            new_building[..4].copy_from_slice(&15u32.to_le_bytes());
            new_building[0x14..0x18].copy_from_slice(&2u32.to_le_bytes());
            new_building[0x1c..0x20].fill(0xff);
            with_pending[building_at + 15 * 64..building_at + 16 * 64]
                .copy_from_slice(&new_building);
            // Scenario3's category13 has no saved buildings. Its first
            // slot accepts the copied 0..2 footprint without overlap.
            let empty_category = building_at + 0x9600 + 13 * 140;
            with_pending[empty_category + 8..empty_category + 12]
                .copy_from_slice(&1u32.to_le_bytes());
            with_pending[empty_category + 12..empty_category + 16]
                .copy_from_slice(&15u32.to_le_bytes());
            let pending_at = source
                .sections
                .iter()
                .find(|record| record.tag == 0x800)
                .unwrap()
                .offset
                + 8;
            with_pending[pending_at..pending_at + 4].copy_from_slice(&1u32.to_le_bytes());
            for (offset, value) in [
                (0, 13u32),
                (4, 15),
                (8, 1),
                (12, 1),
                (16, 0),
                (20, 42),
                (24, 1),
            ] {
                with_pending[pending_at + 4 + offset..pending_at + 8 + offset]
                    .copy_from_slice(&value.to_le_bytes());
            }
            let pending_save = SaveFile::parse(&with_pending).unwrap();
            let mut pending_session = load(&with_pending).unwrap();
            assert!(!openmt2001::building_pool::can_place(
                pending_session.world.categories[13][0x9c..0xcd]
                    .try_into()
                    .unwrap(),
                [0, 0, 2, 2],
            ));
            assert_eq!(pending_session.world.pending_world.count, 1);
            assert_eq!(
                pending_session.world.pending_world.to_bytes(),
                pending_save.pending_world().unwrap().to_bytes()
            );
            // Resume the saved construction just before its completion
            // callback. 4b0810 decrements progress through signed -1,
            // then 48ad80 queues command11 for the original 25-tick
            // local admission. This verifies the restored record reaches
            // the production pending path with its original player ID.
            for step in 0..2 {
                pending_session.world.clock.timing[0] = original_clock.timing[0] + step;
                pending_session
                    .advance_pending(
                        &board,
                        &mut openmt2001::pending_world::Controls {
                            network_enabled: false,
                            network_override: false,
                            effect_model_reference: 0,
                        },
                        |event, _| match event {
                            openmt2001::live_commands::PendingCall::World(
                                openmt2001::pending_world::Event::Progress { .. },
                            ) => Ok(0),
                            other => Err(format!("unexpected restored pending effect {other:?}")),
                        },
                    )
                    .unwrap();
            }
            assert_eq!(pending_session.world.pending_world.count, 0);
            assert_eq!(
                u32::from_le_bytes(
                    pending_session.world.buildings[15][0x14..0x18]
                        .try_into()
                        .unwrap()
                ),
                4
            );
            pending_session.world.clock.timing[0] = original_clock.timing[0] + 25;
            pending_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(&pending_session.world.buildings[15][0x1c..0x20], &[0xff; 4]);
            pending_session.world.clock.timing[0] = original_clock.timing[0] + 26;
            pending_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_ne!(&pending_session.world.buildings[15][0x1c..0x20], &[0xff; 4]);
            assert_eq!(
                pending_session
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|row| row[0x21c] != 0)
                    .count(),
                businesses + 1
            );
            let opened = u32::from_le_bytes(
                pending_session.world.buildings[15][0x1c..0x20]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let row = &pending_session.world.ledgers.businesses[opened];
            assert_eq!(u16::from_le_bytes(row[8..10].try_into().unwrap()), 15);
            assert_eq!(row[0xa], 1);
            assert_eq!(u32::from_le_bytes(row[4..8].try_into().unwrap()), 42);
            let append = |tag: u32, payload: &[u8]| {
                let mut bytes = saved[..saved.len() - 8].to_vec();
                bytes.extend_from_slice(&tag.to_le_bytes());
                bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                bytes.extend_from_slice(payload);
                bytes.extend_from_slice(&saved[saved.len() - 8..]);
                bytes
            };
            // A real supplied human-only save admits the native zero-price
            // City release at its source queue position. Category3 has no
            // buildings, so it needs no renderer appearance callbacks.
            assert_ne!(
                u32::from_le_bytes(session.world.categories[3][0x78..0x7c].try_into().unwrap()),
                2,
                "release must change saved category state"
            );
            let mut release_packet = [0u8; 28];
            release_packet[4..8].copy_from_slice(&0x16u32.to_le_bytes());
            release_packet[8..12].copy_from_slice(&28u32.to_le_bytes());
            release_packet[12..16].copy_from_slice(&original_clock.timing[0].to_le_bytes());
            release_packet[24..26].copy_from_slice(&3i16.to_le_bytes());
            let mut release_header = [0u8; 24];
            release_header[8..12].copy_from_slice(&original_clock.timing[0].to_le_bytes());
            let mut release_payload = release_header.to_vec();
            release_payload.extend_from_slice(&release_packet);
            let mut released = load(&append(0x4000, &release_payload)).unwrap();
            assert_eq!(released.world.categories[3][5], 0);
            assert_eq!(released.world.categories[3][0x6c], 0);
            assert_eq!(
                u32::from_le_bytes(released.world.categories[3][0x78..0x7c].try_into().unwrap()),
                2
            );
            assert!(released.live_commands.snapshot().unwrap().is_empty());
            let cash_before: Vec<_> = released
                .world
                .ledgers
                .players
                .iter()
                .map(|p| p.cash)
                .collect();
            released.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                released
                    .world
                    .ledgers
                    .players
                    .iter()
                    .map(|p| p.cash)
                    .collect::<Vec<_>>(),
                cash_before
            );
            // Native 420500 suppresses a current-player buyer notice. A
            // real City-owned property becomes this human's owned property.
            let current_slot = session
                .roster
                .manager
                .players
                .iter()
                .find(|player| player.reference == session.roster.status.current_player)
                .unwrap()
                .bytes[0x1bc];
            let mut purchase_payload = release_payload.clone();
            purchase_payload[50] = current_slot + 1;
            let purchased = load(&append(0x4000, &purchase_payload)).unwrap();
            assert_eq!(purchased.world.categories[3][5], current_slot + 1);
            let purchaser = purchased
                .roster
                .manager
                .players
                .iter()
                .find(|player| player.reference == purchased.roster.status.current_player)
                .unwrap();
            assert_eq!(purchaser.bytes[0x178 + 3], 1);
            assert_eq!(
                purchased
                    .world
                    .ledgers
                    .players
                    .iter()
                    .map(|p| p.cash)
                    .collect::<Vec<_>>(),
                cash_before
            );
            assert!(purchased.live_commands.snapshot().unwrap().is_empty());
            let mut rival_payload = release_payload.clone();
            rival_payload[50] = current_slot + 2;
            assert!(
                load(&append(0x4000, &rival_payload))
                    .err()
                    .unwrap()
                    .contains("City/self-buyer")
            );
            // Price-bearing transfer remains guarded until cash/audio and
            // property-notice callbacks have an actual load-stage owner.
            let mut priced_release = release_payload.clone();
            priced_release[44..48].copy_from_slice(&1i32.to_le_bytes());
            assert!(load(&append(0x4000, &priced_release)).is_err());
            assert!(
                Session::load_original_with_presentation(
                    inputs(),
                    policy(),
                    &append(0x4000, &priced_release)
                )
                .is_err(),
                "priced City release stays gated"
            );
            for amount in [50000i32, -50000] {
                let mut priced = purchase_payload.clone();
                priced[44..48].copy_from_slice(&amount.to_le_bytes());
                let priced_save = append(0x4000, &priced);
                assert!(
                    load(&priced_save).is_err(),
                    "default loader cannot own priced cash cues"
                );
                let mode_at = SaveFile::parse(&priced_save)
                    .unwrap()
                    .sections
                    .iter()
                    .find(|section| section.tag == 0x400)
                    .unwrap()
                    .offset
                    + 8
                    + 5 * 4;
                let mut modal = priced_save.clone();
                modal[mode_at..mode_at + 4].copy_from_slice(&2u32.to_le_bytes());
                assert!(
                    Session::load_original_with_presentation(inputs(), policy(), &modal)
                        .err()
                        .unwrap()
                        .contains("without auction activation")
                );
                let (paid, calls) =
                    Session::load_original_with_presentation(inputs(), policy(), &priced_save)
                        .unwrap();
                let mut expected = load(&saved).unwrap();
                let clock = expected.world.clock;
                let context = openmt2001::live_commands::Context {
                    image: &image,
                    clock: &clock,
                    accounting_enabled: false,
                    notifications: false,
                    tutorial_state: 0,
                    network_enabled: false,
                    network_override: false,
                    utility_factor: 0.,
                };
                let mut messages = openmt2001::player_messages::Context {
                    arena: openmt2001::player_messages::Arena {
                        next: expected.allocation_next,
                    },
                    current_player: expected.roster.status.current_player,
                    network: [false, false],
                    prior: [0; 48],
                };
                let mut expected_calls = Vec::new();
                openmt2001::live_commands::property_transfer(
                    3,
                    current_slot + 1,
                    amount,
                    &mut expected.world.city,
                    &mut expected.roster,
                    &mut messages,
                    &context,
                    |event| {
                        if let openmt2001::live_commands::Event::AccountPosted(posting) = event
                            && posting.player == usize::from(current_slot)
                            && posting.amount != 0.
                        {
                            expected_calls.push(posting);
                        }
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(calls, expected_calls);
                assert_eq!(paid.world.categories, expected.world.categories);
                assert_eq!(paid.world.buildings, expected.world.buildings);
                assert_eq!(
                    paid.world.ledgers.businesses,
                    expected.world.ledgers.businesses
                );
                for (actual, expected) in paid
                    .roster
                    .manager
                    .players
                    .iter()
                    .zip(&expected.roster.manager.players)
                {
                    assert_eq!(actual.bytes, expected.bytes);
                }
                assert!(paid.live_commands.snapshot().unwrap().is_empty());
                let mut host = crate::live_host::Host::new([0.; 3]);
                let (sound, requests) = std::sync::mpsc::channel();
                host.gameplay_sound = Some(sound);
                host.apply_restored_presentation(calls).unwrap();
                assert_eq!(
                    requests
                        .try_iter()
                        .map(|request| request.operation)
                        .collect::<Vec<_>>(),
                    vec![crate::gameplay_sound::Operation::Ledger { sale: amount < 0 }]
                );
                assert!(host.cash_arrow_state().active);
                assert_eq!(
                    host.cash_arrow_state().frame,
                    if amount < 0 { 41 } else { 0 }
                );
            }
            // A controlled saved tag4 record uses the same v1.4
            // residential corner that native 488230 selects as model
            // 1744. Its 485200 renderer count is zero, so the full
            // 45110e -> 463560 -> 4450a0 -> 485520 load callback can
            // finish without a renderer-owned replacement handle.
            let mut zero_model_save = saved.clone();
            let mut zero_model_building = [0u8; 64];
            zero_model_building[..4].copy_from_slice(&15u32.to_le_bytes());
            zero_model_building[4..9].copy_from_slice(&[0, 0, 6, 0, 4]);
            zero_model_building[0x1c..0x20].fill(0xff);
            zero_model_building[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
            zero_model_building[0x2c] = 2;
            zero_model_building[0x3f] = 1;
            zero_model_save[building_at + 15 * 64..building_at + 16 * 64]
                .copy_from_slice(&zero_model_building);
            let category_at = building_at + 0x9600 + 13 * 140;
            zero_model_save[category_at + 8..category_at + 12].copy_from_slice(&1u32.to_le_bytes());
            zero_model_save[category_at + 12..category_at + 16]
                .copy_from_slice(&15u32.to_le_bytes());
            let mut zero_packet = [0u8; 36];
            zero_packet[..4].copy_from_slice(&0x525de4u32.to_le_bytes());
            zero_packet[4..8].copy_from_slice(&0x11u32.to_le_bytes());
            zero_packet[8..12].copy_from_slice(&36u32.to_le_bytes());
            zero_packet[20..24].copy_from_slice(&42u32.to_le_bytes());
            zero_packet[24..28].copy_from_slice(&15u32.to_le_bytes());
            zero_packet[28..32].copy_from_slice(&1u32.to_le_bytes());
            zero_packet[32] = 1;
            let mut zero_payload = [0u8; 60];
            zero_payload[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
            zero_payload[24..].copy_from_slice(&zero_packet);
            let mut zero_open_save = zero_model_save[..zero_model_save.len() - 8].to_vec();
            zero_open_save.extend_from_slice(&0x4000u32.to_le_bytes());
            zero_open_save.extend_from_slice(&(zero_payload.len() as u32).to_le_bytes());
            zero_open_save.extend_from_slice(&zero_payload);
            zero_open_save.extend_from_slice(&zero_model_save[zero_model_save.len() - 8..]);
            let mut zero_open = load(&zero_open_save).unwrap();
            assert_eq!(zero_open.models.appearance_candidates[&15].descriptor, 1744);
            assert!(
                zero_open.models.appearance_candidates[&15]
                    .renderer_templates
                    .is_empty()
            );
            let opened = u32::from_le_bytes(
                zero_open.world.buildings[15][0x1c..0x20]
                    .try_into()
                    .unwrap(),
            ) as usize;
            assert_ne!(opened, u32::MAX as usize);
            let opened_row = &zero_open.world.ledgers.businesses[opened];
            assert_eq!(u32::from_le_bytes(opened_row[4..8].try_into().unwrap()), 42);
            assert_eq!(
                u16::from_le_bytes(opened_row[8..10].try_into().unwrap()),
                15
            );
            assert_eq!(opened_row[0xa], 1);
            // Native zero-profile command11 writes zero to +0x18 and
            // +0x28 at 46369c and 4629e1, under FPCW 0x037f.
            assert_eq!(&opened_row[0x18..0x1a], &[0, 0]);
            assert_eq!(&opened_row[0x28..0x2c], &[0, 0, 0, 0]);
            assert_eq!(
                zero_open
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|row| row[0x21c] != 0)
                    .count(),
                businesses + 1
            );
            // Native businesssettings.lua property setters finish before
            // 4bc290 begins the save stream. For this single-building
            // kind42 case, their complete row matches the later saved
            // BusinessInfo row; no product/commodity entries are read.
            let native_properties = read("OPENMT2001_NATIVE_BUSINESS_PROPERTIES");
            assert_eq!(&native_properties[..8], b"MTBIP001");
            let row42_at = 16 + 42 * 120 + 4;
            assert_eq!(
                &native_properties[row42_at..row42_at + 116],
                source.business_catalog().unwrap()[42].as_slice(),
            );
            let queue_at = source
                .sections
                .iter()
                .find(|section| section.tag == 0x10000)
                .unwrap()
                .offset;
            let mut canonical_open_save = zero_model_save[..queue_at].to_vec();
            canonical_open_save.extend_from_slice(&0x4000u32.to_le_bytes());
            canonical_open_save.extend_from_slice(&(zero_payload.len() as u32).to_le_bytes());
            canonical_open_save.extend_from_slice(&zero_payload);
            canonical_open_save.extend_from_slice(&zero_model_save[queue_at..]);
            let canonical_open = load(&canonical_open_save).unwrap();
            assert_eq!(
                canonical_open.world.ledgers.businesses[opened],
                zero_open.world.ledgers.businesses[opened],
                "original writer queue position should preserve the complete opened row",
            );
            assert_eq!(
                canonical_open.world.buildings[15][..64],
                zero_open.world.buildings[15][..64],
            );
            // Re-export the native-backed opened fixture, then admit a
            // single sale at the original writer's queue position. This
            // source corner has zero capital: ownership/account semantics
            // are tested here; nonzero cash cues use the128 native cases.
            let cameras = zero_open.saved_cameras.unwrap();
            let exported = zero_open.save_v2(&image, cameras, 1).unwrap();
            let exported_save = SaveFile::parse(&exported).unwrap();
            let extension_at = exported_save
                .sections
                .iter()
                .find(|s| s.tag == openmt2001::save::extension::TAG)
                .unwrap()
                .offset;
            let mut native = exported[..extension_at].to_vec();
            native.extend_from_slice(&exported[exported.len() - 8..]);
            let native_save = SaveFile::parse(&native).unwrap();
            let queue_at = native_save
                .sections
                .iter()
                .find(|s| s.tag == 0x10000)
                .unwrap()
                .offset;
            let mut packet = [0; 28];
            openmt2001::business_sale::Command {
                business: opened as u32,
                player: 0,
            }
            .write_packet(&mut packet);
            packet[8..12].copy_from_slice(&28u32.to_le_bytes());
            let mut payload = [0; 24].to_vec();
            payload[8..12].copy_from_slice(&zero_open.world.clock.timing[0].to_le_bytes());
            payload.extend_from_slice(&packet);
            let mut sale = native[..queue_at].to_vec();
            sale.extend_from_slice(&0x4000u32.to_le_bytes());
            sale.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            sale.extend_from_slice(&payload);
            sale.extend_from_slice(&native[queue_at..]);
            assert!(load(&sale).err().unwrap().contains("presentation owner"));
            let (sold, calls) =
                Session::load_original_with_presentation(inputs(), policy(), &sale).unwrap();
            let value = openmt2001::ai_actions::liquidation_score::business_value(
                &zero_open.world.ledgers.businesses[opened],
                &zero_open.world.buildings,
                &image,
            )
            .unwrap()
            .to_f32();
            assert_eq!(value, 0.);
            assert!(calls.is_empty());
            assert_eq!(sold.world.ledgers.businesses[opened][10], 0);
            assert_eq!(sold.world.buildings[15][0x2d], 0);
            assert_eq!(sold.models.appearance_candidates[&15].descriptor, 1744);
            assert!(sold.live_commands.snapshot().unwrap().is_empty());
            assert_eq!(
                sold.world
                    .ledgers
                    .players
                    .iter()
                    .map(|p| p.cash.to_bits())
                    .collect::<Vec<_>>(),
                zero_open
                    .world
                    .ledgers
                    .players
                    .iter()
                    .map(|p| p.cash.to_bits())
                    .collect::<Vec<_>>()
            );
            let sale_save = SaveFile::parse(&sale).unwrap();
            let info_at = sale_save
                .sections
                .iter()
                .find(|s| s.tag == 0x40000)
                .unwrap()
                .offset
                + 8
                + 42 * 116;
            let mut changed = sale.clone();
            changed[info_at] ^= 1;
            assert!(
                Session::load_original_with_presentation(inputs(), policy(), &changed)
                    .err()
                    .unwrap()
                    .contains("pre-queue BusinessInfo")
            );
            println!(
                "zero-detail1744 original due-now sale: City owner, unchanged zero-capital cash, empty queue"
            );
            let mut changed_catalog = canonical_open_save.clone();
            let catalog_at = source
                .sections
                .iter()
                .find(|section| section.tag == 0x40000)
                .unwrap()
                .offset
                + 8
                + (8 + zero_payload.len())
                + 42 * 0x74
                + 0x60;
            changed_catalog[catalog_at] ^= 1;
            assert!(
                load(&changed_catalog)
                    .err()
                    .unwrap()
                    .contains("pre-queue BusinessInfo differs"),
                "a later saved catalog change must not be used at the queue's earlier position",
            );
            // 450a30's future-inner gate is shared by command11. The
            // outer -1 event may be immediate while its open waits on
            // the total clock; opening at load would bind too early.
            let mut deferred_open_save = zero_open_save.clone();
            let deferred_due = original_clock.timing[0] + 2;
            let packet_due_at = deferred_open_save.len() - 8 - 36 + 12;
            deferred_open_save[packet_due_at..packet_due_at + 4]
                .copy_from_slice(&deferred_due.to_le_bytes());
            let mut deferred_open = load(&deferred_open_save).unwrap();
            assert_eq!(
                deferred_open
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|row| row[0x21c] != 0)
                    .count(),
                businesses,
            );
            deferred_open.world.clock.timing[0] = deferred_due - 1;
            deferred_open.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                deferred_open
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|row| row[0x21c] != 0)
                    .count(),
                businesses,
            );
            deferred_open.world.clock.timing[0] = deferred_due;
            deferred_open.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                deferred_open
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|row| row[0x21c] != 0)
                    .count(),
                businesses + 1,
            );
            let mut overdue_open_save = zero_open_save.clone();
            overdue_open_save[packet_due_at..packet_due_at + 4]
                .copy_from_slice(&(original_clock.timing[0] - 1).to_le_bytes());
            let overdue_open = load(&overdue_open_save).unwrap();
            assert_eq!(
                overdue_open.world.ledgers.businesses[opened],
                zero_open.world.ledgers.businesses[opened],
                "overdue command11 should bind the same zero-detail business",
            );
            let mut positive = zero_open_save.clone();
            let mut positive_building = [0u8; 64];
            positive_building.copy_from_slice(&saved[building_at..building_at + 64]);
            positive_building[..4].copy_from_slice(&15u32.to_le_bytes());
            positive_building[0x1c..0x20].fill(0xff);
            positive[building_at + 15 * 64..building_at + 16 * 64]
                .copy_from_slice(&positive_building);
            let positive_error = load(&positive).err().unwrap();
            assert!(
                positive_error.contains("native zero-detail model1744"),
                "{positive_error}"
            );
            let mut pending = [0u8; 24];
            pending[..4].copy_from_slice(&1u32.to_le_bytes());
            pending[4..8].copy_from_slice(&1u32.to_le_bytes());
            let due = original_clock.timing[1].wrapping_add(2);
            pending[8..12].copy_from_slice(&due.to_le_bytes());
            pending[12..16].copy_from_slice(&36u32.to_le_bytes());
            let queued = append(0x4000, &pending);
            let queued = SaveFile::parse(&queued).unwrap();
            assert_eq!(queued.scheduled().unwrap().len(), 1);
            let mut queued_session = load(&append(0x4000, &pending)).unwrap();
            queued_session.world.categories[36][0x78..0x7c].fill(0);
            queued_session.world.clock.timing[1] = due.wrapping_sub(1);
            queued_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(&queued_session.world.categories[36][0x78..0x7c], &[0; 4]);
            queued_session.world.clock.timing[1] = due;
            queued_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                u32::from_le_bytes(
                    queued_session.world.categories[36][0x78..0x7c]
                        .try_into()
                        .unwrap()
                ),
                2
            );
            // Native 4810e0 dispatches kind one synchronously for the
            // -1 sentinel and for a normal-clock deadline already due.
            // The restored category state must be visible before any
            // post-load Session tick.
            for (domain, immediate_due) in [
                (0u32, u32::MAX),
                (0, original_clock.timing[0]),
                (1, u32::MAX),
                (1, original_clock.timing[1]),
                (1, original_clock.timing[1].wrapping_sub(1)),
            ] {
                let mut immediate = pending;
                immediate[4..8].copy_from_slice(&domain.to_le_bytes());
                immediate[8..12].copy_from_slice(&immediate_due.to_le_bytes());
                let ready_session = load(&append(0x4000, &immediate)).unwrap();
                assert_eq!(
                    u32::from_le_bytes(
                        ready_session.world.categories[36][0x78..0x7c]
                            .try_into()
                            .unwrap()
                    ),
                    2,
                    "domain {domain}, due {immediate_due}"
                );
            }

            // Native 4810e0 enters 450a30 command11 immediately for
            // the sentinel/equal outer deadline. The retained narrow
            // owner requires an active, prepared zero-detail building.
            let mut open_packet = [0u8; 0x24];
            open_packet[4..8].copy_from_slice(&0x11u32.to_le_bytes());
            open_packet[8..12].copy_from_slice(&0x24u32.to_le_bytes());
            open_packet[20..24].copy_from_slice(&0u32.to_le_bytes());
            open_packet[24..28].copy_from_slice(&15u32.to_le_bytes());
            open_packet[28..32].copy_from_slice(&1u32.to_le_bytes());
            let mut open_header = [0u8; 24];
            open_header[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
            let mut open_payload = open_header.to_vec();
            open_payload.extend_from_slice(&open_packet);
            assert!(
                load(&append(0x4000, &open_payload))
                    .err()
                    .unwrap()
                    .contains("lacks prepared model identity")
            );
            open_header[8..12]
                .copy_from_slice(&original_clock.timing[0].wrapping_add(2).to_le_bytes());
            open_payload[..24].copy_from_slice(&open_header);
            let _future_open = load(&append(0x4000, &open_payload)).unwrap();

            // Kind-zero saved packets have two clocks: the outer node is
            // eligible on the normal list now, but 450a30 readmits this
            // command to the total list at packet+0xc. Command19 begins
            // the real category wait only at that inner deadline.
            let mut packet = [0u8; 52];
            packet[4..8].copy_from_slice(&0x19u32.to_le_bytes());
            packet[8..12].copy_from_slice(&52u32.to_le_bytes());
            let packet_due = original_clock.timing[0].wrapping_add(2);
            packet[12..16].copy_from_slice(&packet_due.to_le_bytes());
            packet[20..24].copy_from_slice(&36u32.to_le_bytes());
            let mut header = [0u8; 24];
            header[4..8].copy_from_slice(&1u32.to_le_bytes());
            let outer_due = original_clock.timing[1].wrapping_add(1);
            header[8..12].copy_from_slice(&outer_due.to_le_bytes());
            let mut payload = header.to_vec();
            payload.extend_from_slice(&packet);
            // A due inner command19 executes at the source queue position,
            // preserving its order with synchronous category-ready rows.
            let mut immediate_wait = payload.clone();
            immediate_wait[8..12].copy_from_slice(&original_clock.timing[1].to_le_bytes());
            immediate_wait[36..40].copy_from_slice(&original_clock.timing[0].to_le_bytes());
            for ready_after in [false, true] {
                let mut waited_bytes = append(0x4000, &immediate_wait);
                let mut ready = [0u8; 24];
                ready[..4].copy_from_slice(&1u32.to_le_bytes());
                ready[4..8].copy_from_slice(&1u32.to_le_bytes());
                ready[8..12].copy_from_slice(&original_clock.timing[1].to_le_bytes());
                ready[12..16].copy_from_slice(&36u32.to_le_bytes());
                let queue_offset = waited_bytes.len() - 8 - immediate_wait.len() - 8;
                let mut record = 0x4000u32.to_le_bytes().to_vec();
                record.extend_from_slice(&24u32.to_le_bytes());
                record.extend_from_slice(&ready);
                let insert = if ready_after {
                    waited_bytes.len() - 8
                } else {
                    queue_offset
                };
                waited_bytes.splice(insert..insert, record);
                let mut waited = load(&waited_bytes).unwrap();
                let mut expected = session.world.categories[36];
                openmt2001::category_ownership::begin_wait(&mut expected, original_clock.timing[1]);
                if ready_after {
                    openmt2001::category_ownership::end_wait(&mut expected);
                }
                assert_eq!(
                    waited.world.categories[36], expected,
                    "saved wait source order {ready_after}"
                );
                waited.world.clock.timing[1] = original_clock.timing[1] + 14999;
                waited.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(waited.world.categories[36], expected);
                waited.world.clock.timing[1] += 1;
                waited.drain_commands(&assets, |_| Ok(())).unwrap();
                openmt2001::category_ownership::end_wait(&mut expected);
                assert_eq!(waited.world.categories[36], expected);
            }
            let mut immediate_packet = payload.clone();
            immediate_packet[8..12].copy_from_slice(&original_clock.timing[1].to_le_bytes());
            // A due outer normal-clock command19 is readmitted on the
            // total clock at packet+0xc. The category wait must begin
            // only when that inner deadline arrives.
            let mut readmitted_category = load(&append(0x4000, &immediate_packet)).unwrap();
            readmitted_category.world.categories[36][0x78..0x7c].fill(0);
            readmitted_category.world.clock.timing[0] = packet_due - 1;
            readmitted_category
                .drain_commands(&assets, |_| Ok(()))
                .unwrap();
            assert_eq!(
                &readmitted_category.world.categories[36][0x78..0x7c],
                &[0; 4]
            );
            readmitted_category.world.clock.timing[0] = packet_due;
            readmitted_category
                .drain_commands(&assets, |_| Ok(()))
                .unwrap();
            assert_eq!(
                &readmitted_category.world.categories[36][0x78..0x7c],
                &1u32.to_le_bytes(),
            );
            let mut noop = [0u8; 24];
            noop[4..8].copy_from_slice(&0x14u32.to_le_bytes());
            noop[8..12].copy_from_slice(&24u32.to_le_bytes());
            for outer_due in [original_clock.timing[1], original_clock.timing[1] + 1] {
                let mut noop_header = header;
                noop_header[8..12].copy_from_slice(&outer_due.to_le_bytes());
                let mut noop_payload = noop_header.to_vec();
                noop_payload.extend_from_slice(&noop);
                let mut noop_session = load(&append(0x4000, &noop_payload)).unwrap();
                let before = noop_session.world.categories[36];
                noop_session.world.clock.timing[1] = outer_due + 1;
                noop_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(noop_session.world.categories[36], before);
            }
            // 480ec0 removes nonpacket scheduled kinds above one without
            // command or category dispatch. This kind-two record is a
            // valid writer-sized queue node on the normal clock list.
            let mut inert = [0u8; 24];
            inert[..4].copy_from_slice(&2u32.to_le_bytes());
            inert[4..8].copy_from_slice(&1u32.to_le_bytes());
            let inert_due = original_clock.timing[1] + 1;
            inert[8..12].copy_from_slice(&inert_due.to_le_bytes());
            inert[12..16].copy_from_slice(&36u32.to_le_bytes());
            let mut inert_session = load(&append(0x4000, &inert)).unwrap();
            let before = inert_session.world.categories[36];
            inert_session.world.clock.timing[1] = inert_due;
            inert_session
                .drain_commands(&assets, |_| Err("inert saved queue node dispatched".into()))
                .unwrap();
            assert_eq!(inert_session.world.categories[36], before);
            let mut packet_session = load(&append(0x4000, &payload)).unwrap();
            packet_session.world.categories[36][0x78..0x7c].fill(0);
            packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(&packet_session.world.categories[36][0x78..0x7c], &[0; 4]);
            packet_session.world.clock.timing[1] = outer_due;
            packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(&packet_session.world.categories[36][0x78..0x7c], &[0; 4]);
            packet_session.world.clock.timing[0] = packet_due.wrapping_sub(1);
            packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(&packet_session.world.categories[36][0x78..0x7c], &[0; 4]);
            packet_session.world.clock.timing[0] = packet_due;
            packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                u32::from_le_bytes(
                    packet_session.world.categories[36][0x78..0x7c]
                        .try_into()
                        .unwrap()
                ),
                1
            );
            let mut terms_packet = [0u8; 64];
            terms_packet[4..8].copy_from_slice(&0x18u32.to_le_bytes());
            terms_packet[8..12].copy_from_slice(&64u32.to_le_bytes());
            let terms_due = original_clock.timing[0].wrapping_add(3);
            terms_packet[12..16].copy_from_slice(&terms_due.to_le_bytes());
            let business = &session.world.ledgers.businesses[0];
            terms_packet[20..24].copy_from_slice(&business[0x210..0x214]);
            for i in 0..6 {
                terms_packet[24 + i * 2..26 + i * 2]
                    .copy_from_slice(&(10 + i as i16).to_le_bytes());
                terms_packet[36 + i * 4..40 + i * 4]
                    .copy_from_slice(&(4.0 + i as f32).to_le_bytes());
            }
            terms_packet[60..64].copy_from_slice(&20.0f32.to_le_bytes());
            let mut terms_payload = header.to_vec();
            terms_payload.extend_from_slice(&terms_packet);
            let mut terms_session = load(&append(0x4000, &terms_payload)).unwrap();
            terms_session.world.clock.timing[1] = outer_due;
            terms_session.world.clock.timing[0] = terms_due.wrapping_sub(1);
            terms_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_ne!(
                &terms_session.world.ledgers.businesses[0][0x1e0..0x1e2],
                &10i16.to_le_bytes()
            );
            terms_session.world.clock.timing[0] = terms_due;
            terms_session.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                &terms_session.world.ledgers.businesses[0][0x1e0..0x1e2],
                &10i16.to_le_bytes()
            );
            assert_eq!(
                &terms_session.world.ledgers.businesses[0][0x1c4..0x1c8],
                &4.0f32.to_le_bytes()
            );
            // 4810e0 immediately admits the outer -1 event, and
            // 450a30 requeues command18 on total time terms_due.
            let mut readmitted_terms = terms_payload.clone();
            readmitted_terms[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
            let mut readmitted = load(&append(0x4000, &readmitted_terms)).unwrap();
            assert_ne!(
                &readmitted.world.ledgers.businesses[0][0x1e0..0x1e2],
                &10i16.to_le_bytes()
            );
            readmitted.world.clock.timing[0] = terms_due.wrapping_sub(1);
            readmitted.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_ne!(
                &readmitted.world.ledgers.businesses[0][0x1e0..0x1e2],
                &10i16.to_le_bytes()
            );
            readmitted.world.clock.timing[0] = terms_due;
            readmitted.drain_commands(&assets, |_| Ok(())).unwrap();
            assert_eq!(
                &readmitted.world.ledgers.businesses[0][0x1e0..0x1e2],
                &10i16.to_le_bytes()
            );
            assert_eq!(
                &readmitted.world.ledgers.businesses[0][0x1c4..0x1c8],
                &4.0f32.to_le_bytes()
            );
            for inner_due in [original_clock.timing[0], u32::MAX] {
                readmitted_terms[24 + 12..24 + 16].copy_from_slice(&inner_due.to_le_bytes());
                let error = load(&append(0x4000, &readmitted_terms)).err().unwrap();
                assert!(
                    error.contains(if inner_due == u32::MAX {
                        "450a30 synchronous dispatch"
                    } else {
                        "synchronous tenant callbacks"
                    }),
                    "{error}"
                );
            }
            // 463f70 completes synchronously for an unoccupied saved
            // business. Slot 9 has no tenant chain in this supplied save;
            // slot 0 still requires the original tenant callback path.
            let unoccupied = &session.world.ledgers.businesses[9];
            assert_eq!(&unoccupied[0x20c..0x210], &(-1i32).to_le_bytes());
            let mut now = terms_payload.clone();
            now[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
            now[24 + 12..24 + 16].copy_from_slice(&0u32.to_le_bytes());
            now[24 + 20..24 + 24].copy_from_slice(&unoccupied[0x210..0x214]);
            let loaded_now = load(&append(0x4000, &now)).unwrap();
            let changed = &loaded_now.world.ledgers.businesses[9];
            assert_eq!(&changed[0x1e0..0x1e2], &10i16.to_le_bytes());
            assert_eq!(&changed[0x1c4..0x1c8], &4.0f32.to_le_bytes());
            assert_eq!(&changed[0x204..0x208], &20.0f32.to_le_bytes());
            assert_ne!(&business[0x20c..0x210], &(-1i32).to_le_bytes());
            let old_rent = f32::from_le_bytes(business[0x1dc..0x1e0].try_into().unwrap());
            assert!(old_rent.is_finite());
            let lower_rent = old_rent - 1.0;
            let mut occupied_decrease = now.clone();
            occupied_decrease[24 + 20..24 + 24].copy_from_slice(&business[0x210..0x214]);
            occupied_decrease[24 + 60..24 + 64].copy_from_slice(&lower_rent.to_le_bytes());
            let occupied = load(&append(0x4000, &occupied_decrease)).unwrap();
            let occupied_row = &occupied.world.ledgers.businesses[0];
            assert_eq!(&occupied_row[0x20c..0x210], &business[0x20c..0x210]);
            assert_eq!(&occupied_row[0x1e0..0x1e2], &10i16.to_le_bytes());
            assert_eq!(&occupied_row[0x1dc..0x1e0], &lower_rent.to_le_bytes());
            let mut overdue_now = now.clone();
            overdue_now[24 + 12..24 + 16]
                .copy_from_slice(&(original_clock.timing[0] - 1).to_le_bytes());
            let overdue = load(&append(0x4000, &overdue_now)).unwrap();
            assert_eq!(
                overdue.world.ledgers.businesses[9], loaded_now.world.ledgers.businesses[9],
                "overdue command18 should finish the same unoccupied branch",
            );
            let queue_at = source
                .sections
                .iter()
                .find(|section| section.tag == 0x10000)
                .unwrap()
                .offset;
            let mut canonical = saved[..queue_at].to_vec();
            canonical.extend_from_slice(&0x4000u32.to_le_bytes());
            canonical.extend_from_slice(&(now.len() as u32).to_le_bytes());
            canonical.extend_from_slice(&now);
            canonical.extend_from_slice(&saved[queue_at..]);
            let canonical_loaded = load(&canonical).unwrap();
            assert_eq!(
                canonical_loaded.world.ledgers.businesses[9],
                loaded_now.world.ledgers.businesses[9],
                "original writer queue position should retain command18 row effects",
            );
            let business_at = source
                .sections
                .iter()
                .find(|section| section.tag == 0x20)
                .unwrap()
                .offset;
            let mut before_business = saved[..business_at].to_vec();
            before_business.extend_from_slice(&0x4000u32.to_le_bytes());
            before_business.extend_from_slice(&(now.len() as u32).to_le_bytes());
            before_business.extend_from_slice(&now);
            before_business.extend_from_slice(&saved[business_at..]);
            assert!(
                load(&before_business)
                    .err()
                    .unwrap()
                    .contains("business-before-queue order")
            );
            now[24 + 20..24 + 24].copy_from_slice(&business[0x210..0x214]);
            assert!(
                load(&append(0x4000, &now))
                    .err()
                    .unwrap()
                    .contains("synchronous tenant callbacks")
            );
            let mut auction = [0u8; 344];
            auction[0x7d] = 1;
            auction[4..8].copy_from_slice(&36u32.to_le_bytes());
            auction[0x80..0x84].copy_from_slice(&100u32.to_le_bytes());
            let auction_session = load(&append(0x8000, &auction)).unwrap();
            assert_eq!(
                auction_session.world.city.auction.controller.reference,
                0x7f4758
            );
            assert_eq!(
                auction_session.world.city.auction.controller.record[0x7d],
                1
            );
            assert_eq!(auction_session.world.city.categories[36][0x6c], 1);
            assert_eq!(
                auction_session.world.city.auction.presentation.countdown,
                100
            );
            assert!(auction_session.world.city.auction.presentation.visible);
            let mut mode2 = append(0x8000, &auction);
            let mode_record = SaveFile::parse(&mode2)
                .unwrap()
                .sections
                .iter()
                .find(|section| section.tag == 0x400)
                .unwrap()
                .offset
                + 8
                + 5 * 4;
            mode2[mode_record..mode_record + 4].copy_from_slice(&2u32.to_le_bytes());
            let mode2_session = load(&mode2).unwrap();
            assert_eq!(mode2_session.saved_logic_mode, Some(2));
            assert_eq!(
                mode2_session.world.city.auction.controller.reference,
                0x7f4758
            );
            assert_eq!(mode2_session.world.city.auction.controller.record[0x7d], 1);
            assert!(!mode2_session.world.city.auction.presentation.visible);
            // The UI receives the retained Session before Host invokes
            // 464aa0 through its synchronous renderer bridge.

            // Both supplied files have only a human. Insert a second
            // native metadata/player pair immediately after the existing
            // pair to exercise the 48c800 AI constructor and 4bbd20
            // post-load strategy activation on a retained opponent.
            let player_section = source
                .sections
                .iter()
                .find(|section| section.tag == 0x40)
                .unwrap();
            let insert_at = player_section.offset + 8 + player_section.payload.len();
            let mut metadata = *saved_players[0].metadata;
            metadata[..4].copy_from_slice(&5u32.to_le_bytes());
            metadata[4..8].copy_from_slice(&1u32.to_le_bytes());
            metadata[44..48].copy_from_slice(&2u32.to_le_bytes());
            let mut opponent = *saved_players[0].bytes;
            opponent[0x14c..0x150].copy_from_slice(&5u32.to_le_bytes());
            opponent[0x150..0x154].copy_from_slice(&1u32.to_le_bytes());
            opponent[0x1b4..0x1b8].fill(0); // no held event card
            let mut pair = Vec::new();
            for (tag, payload) in [
                (0x80000u32, metadata.as_slice()),
                (0x40, opponent.as_slice()),
            ] {
                pair.extend_from_slice(&tag.to_le_bytes());
                pair.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                pair.extend_from_slice(payload);
            }
            let mut with_opponent = saved[..insert_at].to_vec();
            with_opponent.extend_from_slice(&pair);
            with_opponent.extend_from_slice(&saved[insert_at..]);
            let mut opponent_session = load(&with_opponent).unwrap();
            assert_eq!(opponent_session.roster.manager.players.len(), 2);
            assert_eq!(opponent_session.roster.status.ai_count, 1);
            assert!(opponent_session.roster.status.has_ai);
            assert_eq!(
                opponent_session.roster.manager.players[1].bytes.len(),
                0x1e50
            );
            assert_eq!(
                &opponent_session.roster.manager.players[1].bytes[0x13f4..0x13f8],
                &2u32.to_le_bytes()
            );
            assert!(
                opponent_session.roster.profiles[5]
                    .iter()
                    .any(|byte| *byte != 0),
                "4d7a80 must initialize the restored AI's script profile"
            );
            // One retained AI controller pass consumes the reconstructed
            // player/profile/city rather than merely inspecting bytes.
            {
                let player = &mut opponent_session.roster.manager.players[1];
                player.bytes[0x1403] = 0;
                player.bytes[0x1df8] = 1;
                player.bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
            }
            let tick = opponent_session.world.clock.timing[0];
            opponent_session
                .roster
                .advance_ai(
                    &mut opponent_session.world.city,
                    1,
                    &image,
                    &mut openmt2001::ai_tick::Controls {
                        normal_tick: tick,
                        day_tick: opponent_session.world.clock.timing[3],
                        event_gate: false,
                    },
                    &mut opponent_session.allocation_next,
                    &opponent_session.hooks.utilities,
                    &opponent_session.population.properties,
                    None,
                    |call, _, _| Err(format!("loaded AI escaped guarded controller: {call:?}")),
                )
                .unwrap();
            opponent_session.world.clock.timing[1] =
                opponent_session.world.clock.timing[1].wrapping_add(1);
            opponent_session
                .drain_commands(&assets, |_| Ok(()))
                .unwrap();
            let mut malformed = vec![0u8; 36];
            malformed[32..36].copy_from_slice(&16u32.to_le_bytes());
            let invalid = append(0x4000, &malformed);
            let invalid = SaveFile::parse(&invalid).unwrap();
            assert!(invalid.scheduled().is_err());
            assert_eq!(session.world.clock, original_clock);
            assert_eq!(session.world.ledgers.residents.active_count, original_count);
        }
        println!(
            "{key}: loaded {} residents, {businesses} businesses, hour {}",
            count,
            session.world.clock.hour()
        );
        let mut session = session;
        assert_import_model_policy(&mut session);
    }
}

fn assert_import_model_policy(session: &mut Session<'_>) {
    use crate::compatibility::Mode;
    let nodes = session
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .map(|n| (*n.as_bytes(), n.active_byte(), n.auxiliary_reference()))
        .collect::<Vec<_>>();
    let accounts = session
        .world
        .ledgers
        .residents
        .accounts
        .iter()
        .map(|a| *a.as_bytes())
        .collect::<Vec<_>>();
    let auxiliary = session.world.creation.auxiliary.slots().to_vec();
    let bindings = |s: &Session<'_>| {
        s.population
            .bindings
            .iter()
            .map(|b| {
                (
                    b.resident_reference,
                    b.auxiliary_reference,
                    b.auxiliary_slot,
                    b.model_frame_count,
                )
            })
            .collect::<Vec<_>>()
    };
    let before_bindings = bindings(session);
    let controllers = session.population.animations.records().to_vec();
    let clock = session.world.clock.clone();
    let rng = |s: &Session<'_>| {
        [
            s.world.creation.identity_rng,
            s.world.creation.preference_rng,
            s.world.creation.initialization_rng.constructor,
            s.world.creation.initialization_rng.movement,
            s.world.creation.initialization_rng.objective,
            s.script_random_state,
            s.credit_random_state,
            s.particle_random_state,
            s.policy.seed,
        ]
    };
    let before_rng = rng(session);
    let mut expected = auxiliary.clone();
    let mut expected_bindings = before_bindings.clone();
    let mut changed = Vec::new();
    for node in session
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .filter(|node| node.is_active())
    {
        let slot = (node.auxiliary_reference() - 1) as usize;
        let row = usize::from(node.model_variant());
        let model = &session.resident_models[row];
        if auxiliary[slot][..4] != model.reference.to_le_bytes() {
            expected[slot][..4].copy_from_slice(&model.reference.to_le_bytes());
            expected[slot][0x44..0xb8].copy_from_slice(&model.definition);
            let binding = expected_bindings
                .iter_mut()
                .find(|b| b.1 == node.auxiliary_reference())
                .unwrap();
            assert_eq!(binding.2, slot);
            binding.3 = session.people.geometry_source(row, false).unwrap().frames as u16;
            changed.push(slot);
        }
    }
    assert!(
        !changed.is_empty(),
        "supplied import must exercise a saved/fresh identity mismatch"
    );
    for (mode, retained) in [
        (Mode::StrictV14, false),
        (Mode::StrictV14, true),
        (Mode::Modern, true),
    ] {
        assert_eq!(
            session
                .repair_imported_resident_models(mode, retained)
                .unwrap(),
            0
        );
        assert_eq!(session.world.creation.auxiliary.slots(), auxiliary);
        assert_eq!(bindings(session), before_bindings);
    }
    // A late invalid candidate must not install any earlier valid repair.
    let last = *changed.last().unwrap();
    session.world.creation.auxiliary.get_mut(last).unwrap()[0x2c..0x2e]
        .copy_from_slice(&i16::MAX.to_le_bytes());
    let invalid = session.world.creation.auxiliary.slots().to_vec();
    assert!(
        session
            .repair_imported_resident_models(Mode::Modern, false)
            .is_err()
    );
    assert_eq!(session.world.creation.auxiliary.slots(), invalid);
    assert_eq!(bindings(session), before_bindings);
    session.world.creation.auxiliary.get_mut(last).unwrap()[0x2c..0x2e]
        .copy_from_slice(&auxiliary[last][0x2c..0x2e]);
    assert_eq!(
        session
            .repair_imported_resident_models(Mode::Modern, false)
            .unwrap(),
        changed.len()
    );
    assert_eq!(session.world.creation.auxiliary.slots(), expected);
    assert_eq!(bindings(session), expected_bindings);
    assert_eq!(
        session
            .repair_imported_resident_models(Mode::Modern, false)
            .unwrap(),
        0
    );
    assert_eq!(session.world.creation.auxiliary.slots(), expected);
    assert_eq!(bindings(session), expected_bindings);
    assert_eq!(
        session
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .map(|n| (*n.as_bytes(), n.active_byte(), n.auxiliary_reference()))
            .collect::<Vec<_>>(),
        nodes
    );
    assert_eq!(
        session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .map(|a| *a.as_bytes())
            .collect::<Vec<_>>(),
        accounts
    );
    assert_eq!(session.population.animations.records(), controllers);
    assert_eq!(session.world.clock, clock);
    assert_eq!(rng(session), before_rng);
    println!(
        "Modern import repaired {} bindings; exact byte policy, strict/runtime exclusion, failure atomicity and idempotence passed",
        changed.len()
    );
}

#[test]
#[ignore = "requires supplied saves and original headless scheduler assets"]
fn advances_loaded_saves_to_next_natural_hour() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(advances_loaded_saves_to_next_natural_hour_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn advances_loaded_saves_to_next_natural_hour_inner() {
    let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let built = read("OPENMT2001_BUILD_FILE");
    let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = read("OPENMT2001_PARAMETERS");
    let timetable = read("OPENMT2001_TIMETABLE");
    let scripts = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .join("scripts");
    let era_styles = super::super::load_era_styles(
        Path::new(&openmt2001::fixtures::path("PARAMETERS"))
            .parent()
            .unwrap(),
    )
    .unwrap();
    let names = std::array::from_fn(|_| Vec::new());
    for (key, scenario, difficulty, release, buyer) in [
        ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, false, 0),
        ("OPENMT2001_SAVE_TWO", "sandbox", 0, false, 0),
        ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, true, 0),
        ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, true, 1),
    ] {
        let mut saved = read(key);
        if release {
            let total = SaveFile::parse(&saved).unwrap().clock().unwrap().timing[0];
            let mut event = [0u8; 52];
            event[8..12].copy_from_slice(&total.to_le_bytes());
            event[28..32].copy_from_slice(&0x16u32.to_le_bytes());
            event[32..36].copy_from_slice(&28u32.to_le_bytes());
            event[36..40].copy_from_slice(&total.to_le_bytes());
            event[48..50].copy_from_slice(&3i16.to_le_bytes());
            event[50] = buyer;
            let at = saved.len() - 8;
            let mut record = 0x4000u32.to_le_bytes().to_vec();
            record.extend_from_slice(&52u32.to_le_bytes());
            record.extend_from_slice(&event);
            saved.splice(at..at, record);
        }
        let mut session = Session::load_original(
            Inputs {
                tree_locators: None,
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: era_styles.clone(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario,
                difficulty,
                selected_character: 4,
                profile_name: b"Saved session",
                character_names: &names,
            },
            Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            &saved,
        )
        .unwrap();
        if release {
            assert_eq!(session.world.categories[3][5], buyer);
        }
        let assets = CommandAssets {
            image: &image,
            board: &board,
            file: &file,
        };
        let mut policy = super::super::tick::Policy {
            scheduler: openmt2001::scheduler::Controls {
                iterations: 1,
                abort: false,
                alternate_script_active: false,
                network_enabled: false,
                network_override: false,
                deadline: u32::MAX,
                gate_32: false,
                gate_32_offset: false,
                before_six_sound: 0,
            },
            calendar: openmt2001::clock::Controls {
                years_per_day: 0,
                suppress_rollover_query: false,
                rollover_events_enabled: false,
                autosave_disabled: true,
            },
            hourly: openmt2001::hourly::Controls {
                suppressed: false,
                world_mode: 1,
                before_six_sound: 0,
                network_enabled: false,
                network_override: false,
            },
            camera: [0., 20., 0.],
            distance_disabled: false,
            notifications: false,
            effect_model: 0,
            market_share_prior: [0.; 6],
            player_message_prior: [0; 48],
            ai_event_gate: false,
        };
        let before = session.world.clock.timing[0];
        let before_accounts: Vec<_> = session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .map(|a| *a.as_bytes())
            .collect();
        let before_residents: Vec<_> = session
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .take(session.world.ledgers.residents.active_count as usize)
            .map(|r| *r.as_bytes())
            .collect();
        let before_businesses = session.world.ledgers.businesses.clone();
        let before_owners: Vec<_> = session.world.categories.iter().map(|r| r[5]).collect();
        let mut result = Ok(());
        let mut completed = 0;
        let start_hour = session.world.clock.hour();
        for _ in 0..700 {
            let before_cash = cash(&session);
            let mut postings = Vec::new();
            result = session.advance_session(&assets, &mut policy, |event, _| match event {
                super::super::tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                    0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                )) => Ok(0),
                super::super::tick::Event::Script { commands: [], .. } => Ok(0),
                super::super::tick::Event::ScriptCommands([]) => Ok(0),
                super::super::tick::Event::TransportSound(_) => Ok(0),
                super::super::tick::Event::AccountPosted(posting) => {
                    postings.push(posting);
                    Ok(0)
                }
                _ => Err(format!("unbound post-load event {event:?}")),
            });
            if result.is_err() {
                break;
            }
            assert_account_postings(&before_cash, &cash(&session), &postings);
            completed += 1;
            if completed == 25 {
                assert_eq!(session.world.clock.timing[0], before + 25);
            }
            if completed >= 25 && session.world.clock.hour() != start_hour {
                break;
            }
        }
        let account_changes = session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .zip(&before_accounts)
            .filter(|(a, before)| a.as_bytes() != *before)
            .count();
        let resident_changes = session
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .zip(&before_residents)
            .filter(|(r, before)| r.as_bytes() != *before)
            .count();
        let business_changes = session
            .world
            .ledgers
            .businesses
            .iter()
            .zip(&before_businesses)
            .filter(|(r, before)| *r != *before)
            .count();
        let owner_changes = session
            .world
            .categories
            .iter()
            .zip(&before_owners)
            .filter(|(r, before)| r[5] != **before)
            .count();
        println!(
            "{key}: {completed} normal ticks, {} -> {}, hour {} -> {}, account changes {account_changes}, resident changes {resident_changes}, business changes {business_changes}, owner changes {owner_changes}, result {result:?}",
            before,
            session.world.clock.timing[0],
            start_hour,
            session.world.clock.hour()
        );
        assert!(result.is_ok(), "{key}: {result:?}");
        assert!(completed >= 25 && session.world.clock.hour() != start_hour);
        assert!(
            resident_changes > 0,
            "normal ticks should mutate retained residents"
        );
        if key == "OPENMT2001_SAVE_ONE" {
            assert!(
                account_changes > 0,
                "next hourly boundary should update resident accounts"
            );
            assert!(
                business_changes > 0,
                "next hourly boundary should update businesses"
            );
        }
        assert_eq!(
            owner_changes, 0,
            "unrequested property transfer during loaded hour"
        );
    }
}
#[test]
#[ignore = "requires supplied saves and original headless scheduler assets"]
fn supplied_save_advance_export_reload_continuation() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(supplied_save_advance_export_reload_continuation_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn supplied_save_advance_export_reload_continuation_inner() {
    let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let built = read("OPENMT2001_BUILD_FILE");
    let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = read("OPENMT2001_PARAMETERS");
    let timetable = read("OPENMT2001_TIMETABLE");
    let scripts = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .join("scripts");
    let era_styles = super::super::load_era_styles(
        Path::new(&openmt2001::fixtures::path("PARAMETERS"))
            .parent()
            .unwrap(),
    )
    .unwrap();
    let names = std::array::from_fn(|_| Vec::new());
    let (key, scenario, difficulty) = ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1);
    {
        let saved = read(key);
        let inputs = || Inputs {
            tree_locators: None,
            image: &image,
            board: &board,
            file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                &image, &index, &archive,
            )
            .unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(
                &image, &index, &archive,
            )
            .unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario,
            difficulty,
            selected_character: 4,
            profile_name: b"Saved session",
            character_names: &names,
        };
        let load_policy = || Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 0x11c],
        };
        let mut session = Session::load_original(inputs(), load_policy(), &saved).unwrap();
        let assets = CommandAssets {
            image: &image,
            board: &board,
            file: &file,
        };
        let tick_policy = || super::super::tick::Policy {
            scheduler: openmt2001::scheduler::Controls {
                iterations: 1,
                abort: false,
                alternate_script_active: false,
                network_enabled: false,
                network_override: false,
                deadline: u32::MAX,
                gate_32: false,
                gate_32_offset: false,
                before_six_sound: 0,
            },
            calendar: openmt2001::clock::Controls {
                years_per_day: 0,
                suppress_rollover_query: false,
                rollover_events_enabled: false,
                autosave_disabled: true,
            },
            hourly: openmt2001::hourly::Controls {
                suppressed: false,
                world_mode: 1,
                before_six_sound: 0,
                network_enabled: false,
                network_override: false,
            },
            camera: [0., 20., 0.],
            distance_disabled: false,
            notifications: false,
            effect_model: 0,
            market_share_prior: [0.; 6],
            player_message_prior: [0; 48],
            ai_event_gate: false,
        };
        let mut policy = tick_policy();
        let before = session.world.clock.timing[0];
        let before_accounts: Vec<_> = session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .map(|a| *a.as_bytes())
            .collect();
        let before_residents: Vec<_> = session
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .take(session.world.ledgers.residents.active_count as usize)
            .map(|r| *r.as_bytes())
            .collect();
        let before_businesses = session.world.ledgers.businesses.clone();
        let before_owners: Vec<_> = session.world.categories.iter().map(|r| r[5]).collect();
        let mut result = Ok(());
        let mut completed = 0;
        let start_hour = session.world.clock.hour();
        for _ in 0..700 {
            let before_cash = cash(&session);
            let mut postings = Vec::new();
            result = session.advance_session(&assets, &mut policy, |event, _| match event {
                super::super::tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                    0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                )) => Ok(0),
                super::super::tick::Event::Script { commands: [], .. } => Ok(0),
                super::super::tick::Event::ScriptCommands([]) => Ok(0),
                super::super::tick::Event::TransportSound(_) => Ok(0),
                super::super::tick::Event::AccountPosted(posting) => {
                    postings.push(posting);
                    Ok(0)
                }
                _ => Err(format!("unbound post-load event {event:?}")),
            });
            if result.is_err() {
                break;
            }
            assert_account_postings(&before_cash, &cash(&session), &postings);
            completed += 1;
            if completed == 25 {
                assert_eq!(session.world.clock.timing[0], before + 25);
            }
            if completed >= 25 && session.world.clock.hour() != start_hour {
                break;
            }
        }
        let account_changes = session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .zip(&before_accounts)
            .filter(|(a, before)| a.as_bytes() != *before)
            .count();
        let resident_changes = session
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .zip(&before_residents)
            .filter(|(r, before)| r.as_bytes() != *before)
            .count();
        let business_changes = session
            .world
            .ledgers
            .businesses
            .iter()
            .zip(&before_businesses)
            .filter(|(r, before)| *r != *before)
            .count();
        let owner_changes = session
            .world
            .categories
            .iter()
            .zip(&before_owners)
            .filter(|(r, before)| r[5] != **before)
            .count();
        println!(
            "{key}: {completed} normal ticks, {} -> {}, hour {} -> {}, account changes {account_changes}, resident changes {resident_changes}, business changes {business_changes}, owner changes {owner_changes}, result {result:?}",
            before,
            session.world.clock.timing[0],
            start_hour,
            session.world.clock.hour()
        );
        assert!(result.is_ok(), "{key}: {result:?}");
        assert!(completed >= 25 && session.world.clock.hour() != start_hour);
        assert!(
            resident_changes > 0,
            "normal ticks should mutate retained residents"
        );
        if key == "OPENMT2001_SAVE_ONE" {
            assert!(
                account_changes > 0,
                "next hourly boundary should update resident accounts"
            );
            assert!(
                business_changes > 0,
                "next hourly boundary should update businesses"
            );
        }
        assert_eq!(
            owner_changes, 0,
            "unrequested property transfer during loaded hour"
        );

        // Same explicit bounded host abstractions as the natural-hour test:
        // named scheduler helpers, empty script dispatch, transport audio
        // and account notifications checked against authoritative cash.
        let camera = openmt2001::flyby::CameraSample {
            eye: [100., 120., 100.],
            target: [0., 0., 0.],
            up: [0., 1., 0.],
            vertical_fov: 0.7,
        };
        let cameras = crate::save_menu::cameras(None, camera).unwrap();
        let bytes = session
            .save_v2(&image, cameras, 1)
            .expect("supplied-save modern export");
        let parent = openmt2001::fixtures::scratch("TEST_SCRATCH");
        fs::create_dir_all(&parent).unwrap();
        let root = parent.join(format!(
            "supplied-save-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Exclusive creation establishes ownership before installing cleanup.
        fs::create_dir(&root).unwrap();
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                if let Err(error) = fs::remove_dir_all(&self.0) {
                    eprintln!("scratch cleanup {}: {error}", self.0.display());
                }
            }
        }
        let _scratch = Scratch(root.clone());
        let path = crate::save_menu::write_new(&root, &bytes).unwrap();
        let loaded = fs::read(&path).unwrap();
        let save = SaveFile::parse(&loaded).unwrap();
        let mut resumed = Session::load_original(inputs(), load_policy(), &loaded).unwrap();
        let native_allocation_next = resumed.allocation_next;
        assert!(
            resumed
                .restore_v2_extension_with_image(&save, &image)
                .unwrap()
                .is_some()
        );
        crate::session_save::continuation_test::load_boundary(
            &mut session,
            &resumed,
            &assets,
            &save,
            native_allocation_next,
        );
        assert_eq!(resumed.saved_cameras, Some(cameras));
        let mut resumed_policy = tick_policy();
        for step in 0..=30 {
            supplied_save_state_equal(&mut session, &mut resumed, &image, cameras);
            if step == 30 {
                break;
            }
            for (branch, controls) in [
                (&mut session, &mut policy),
                (&mut resumed, &mut resumed_policy),
            ] {
                let before_cash = cash(branch);
                let mut postings = Vec::new();
                branch
                    .advance_session(&assets, controls, |event, _| match event {
                        super::super::tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                            0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                        )) => Ok(0),
                        super::super::tick::Event::Script { commands: [], .. } => Ok(0),
                        super::super::tick::Event::ScriptCommands([]) => Ok(0),
                        super::super::tick::Event::TransportSound(_) => Ok(0),
                        super::super::tick::Event::AccountPosted(posting) => {
                            postings.push(posting);
                            Ok(0)
                        }
                        _ => Err(format!(
                            "unbound supplied-save continuation event {event:?}"
                        )),
                    })
                    .unwrap();
                assert_account_postings(&before_cash, &cash(branch), &postings);
            }
        }
        println!(
            "supplied SAVE_ONE: export tick {}, final tick {}, 30 identical continuation ticks; artifact {}",
            save.clock().unwrap().timing[0],
            session.world.clock.timing[0],
            path.display()
        );
    }
}

fn supplied_save_state_equal(
    a: &mut Session<'_>,
    b: &mut Session<'_>,
    image: &openmt2001::pe::PeImage<'_>,
    cameras: [[u8; 196]; 2],
) {
    assert_eq!(a.world.clock, b.world.clock);
    assert_eq!(a.world.ledgers.businesses, b.world.ledgers.businesses);
    assert_eq!(a.world.buildings, b.world.buildings);
    assert_eq!(a.world.categories, b.world.categories);
    assert_eq!(
        a.world.pending_world.to_bytes(),
        b.world.pending_world.to_bytes()
    );
    assert_eq!(a.world.objective.to_bytes(), b.world.objective.to_bytes());
    assert_eq!(a.history.as_bytes(), b.history.as_bytes());
    assert_eq!(
        a.world.ledgers.residents.active_count,
        b.world.ledgers.residents.active_count
    );
    assert_eq!(
        a.world.ledgers.residents.nodes.len(),
        b.world.ledgers.residents.nodes.len()
    );
    for (x, y) in a
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .zip(&b.world.ledgers.residents.nodes)
    {
        assert_eq!(x.as_bytes(), y.as_bytes());
    }
    assert_eq!(
        a.world.ledgers.residents.accounts.len(),
        b.world.ledgers.residents.accounts.len()
    );
    for (x, y) in a
        .world
        .ledgers
        .residents
        .accounts
        .iter()
        .zip(&b.world.ledgers.residents.accounts)
    {
        assert_eq!(x.as_bytes(), y.as_bytes());
    }
    for (x, y) in a.world.ledgers.players.iter().zip(&b.world.ledgers.players) {
        let mut left = [0; 0x1bc];
        let mut right = [0; 0x1bc];
        x.write_runtime(&mut left).unwrap();
        y.write_runtime(&mut right).unwrap();
        assert_eq!(left, right);
    }
    assert_eq!(a.roster.profiles, b.roster.profiles);
    let queue = |s: &mut Session<'_>| {
        let bytes = s.save_v2(image, cameras, 1).unwrap();
        let save = SaveFile::parse(&bytes).unwrap();
        openmt2001::save::extension::Extension::parse(&save)
            .unwrap()
            .unwrap()
            .required(9)
            .unwrap()
            .to_vec()
    };
    assert_eq!(
        queue(a),
        queue(b),
        "delayed command domains/deadlines/payloads"
    );
    let rng = |s: &Session<'_>| {
        (
            s.random_registry.process,
            s.random_registry.head,
            s.random_registry.tail,
            s.script_random_state,
            s.credit_random_state,
            s.particle_random_state,
            s.build_random_state,
            s.world.creation.initialization_rng.constructor,
            s.world.creation.initialization_rng.movement,
            s.world.creation.initialization_rng.objective,
            s.world.creation.identity_rng,
            s.world.creation.preference_rng,
        )
    };
    assert_eq!(rng(a), rng(b));
    assert_eq!(
        a.road_traffic.admission.random,
        b.road_traffic.admission.random
    );
    assert_eq!(a.demolition_effects.random, b.demolition_effects.random);
}
