use super::*;

#[test]
#[ignore = "requires supplied original assets; fresh offline Skirmish kernel"]
fn original_net1_skirmish_session() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build = read("OPENMT2001_BUILD_FILE");
    let _sun = read("OPENMT2001_SUNCOLS");
    let parameters = read("OPENMT2001_PARAMETERS");
    let timetable = read("OPENMT2001_TIMETABLE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
    let scripts = default_path.parent().unwrap().parent().unwrap();
    let language = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");

    let names = character_names(&image, &language).unwrap();
    use openmt2001::skirmish::roster::{Kind, Participant};
    let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
    let launch = SkirmishLaunch {
        level_id: 300001,
        directory: "net1".into(),
        map: "empty.lua".into(),
        objective_kind: 6,
        target: 3500.,
        difficulty: 1,
        participants: vec![
            Participant {
                kind: Kind::Human,
                token: Some(4),
                name: b"Fixture Player".to_vec(),
                ready: true,
                difficulty: 2,
            },
            Participant {
                kind: Kind::Ai,
                token: Some(1),
                name: b"Explicit Opponent".to_vec(),
                ready: true,
                difficulty: 0,
            },
        ],
    };
    let session = Session::initialize_skirmish(
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
            road_routes: std::fs::read(openmt2001::fixtures::path("ROAD_ROUTE_TABLE")).unwrap(),
            scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: load_era_styles(
                Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                    .parent()
                    .unwrap(),
            )
            .unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: "NET1",
            difficulty: 1,
            selected_character: 4,
            profile_name: b"Fixture Player",
            character_names: &names,
        },
        Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 0x11c],
        },
        catalog,
        launch,
    )
    .unwrap();
    let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(session.catalog.selected_level().unwrap().id(), 300001);
    assert!(!session.catalog.selected_level().unwrap().single_player());
    assert_eq!(
        session.catalog.selected_level().unwrap().objective_target(),
        3500.
    );
    assert_eq!(session.world.objective.kind, 6);
    assert_eq!(session.world.objective.target, 3500.);
    assert_eq!(session.difficulty, 1);
    assert_eq!(session.roster.manager.players.len(), 2);
    assert_eq!(session.roster.status.ai_count, 1);
    assert_eq!(session.world.player_money, 6500.);
    assert_eq!(session.world.ledgers.residents.active_count, 251);
    for (slot, (token, kind, difficulty, peer, name)) in [
        (4, 0, 2, 0, b"Fixture Player".as_slice()),
        (1, 1, 0, u32::MAX, b"Explicit Opponent".as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        let row = &session.saved_player_metadata[slot];
        assert_eq!(word(row, 0), token);
        assert_eq!(word(row, 4), kind);
        assert_eq!(word(row, 40), peer);
        assert_eq!(word(row, 44), difficulty);
        assert_eq!(&row[8..8 + name.len()], name);
        assert_eq!(
            word(&session.roster.manager.players[slot].bytes, 0x150),
            token
        );
        assert_eq!(
            word(&session.roster.manager.players[slot].bytes, 0x154),
            kind
        );
        if kind == 1 {
            assert_eq!(
                word(&session.roster.manager.players[slot].bytes, 0x13f4),
                difficulty
            );
        }
        assert_eq!(session.world.ledgers.players[slot].cash, 6500.);
    }
    assert!(session.campaign_profile.is_none());
    assert!(session.network_command_timer.is_none());
}
#[test]
#[ignore = "requires supplied originals; bounded net1 cold reload Host continuation"]
fn original_net1_skirmish_cold_reload_continuation() {
    skirmish_map_cold_reload_continuation("empty.lua");
}

#[test]
#[ignore = "requires supplied originals; selected-map v6 NET1 cold reload Host continuation"]
fn original_net1_monopoly_map_cold_reload_continuation() {
    skirmish_map_cold_reload_continuation("monopolymap.lua");
}

// Shared source-backed record/model checks; no Host or renderer execution.
fn check_authored_records(
    state: &Session<'_>,
    specs: &[openmt2001::scene::BuildingSpec],
    board: &openmt2001::board::Board,
    file: &openmt2001::build_file::BuildFile<'_>,
) -> Vec<(usize, usize)> {
    let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let mut identities = Vec::new();
    for spec in specs {
        let scene = openmt2001::scene::SceneBuilding::resolve(spec.clone(), board, file).unwrap();
        let category_reference = openmt2001::building_geometry::V14_CATEGORY_BASE
            + scene.block_id as u32 * openmt2001::building_geometry::CATEGORY_STRIDE;
        let [x, y, w, d] = spec.lot;
        let endpoints = [x, y, x + w - 1, y + d - 1];
        let matches: Vec<_> = state
            .world
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                row[0x3f] != 0 && word(*row, 0x40) == category_reference && row[4..8] == endpoints
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "authored lot missing or duplicated: {:?}",
            spec.lot
        );
        let (slot, row) = matches[0];
        assert_eq!(word(row, 0), slot as u32);
        assert_eq!(row[8], spec.height);
        assert_eq!(word(row, 0x20), spec.variation);
        assert_eq!(word(row, 0x24), u32::from(spec.kind));
        assert_eq!(row[0x2c], spec.quality);
        assert_eq!(
            [
                f32::from_bits(word(row, 0xc)),
                f32::from_bits(word(row, 0x10))
            ],
            scene.center
        );
        assert_eq!(row[0x2d], state.world.categories[scene.block_id][5]);
        let business_slot = word(row, 0x1c) as usize;
        let business = &state.world.ledgers.businesses[business_slot];
        assert_ne!(business[0x21c], 0);
        assert_eq!(word(business, 0x210), business_slot as u32);
        assert_eq!(
            u16::from_le_bytes(business[8..10].try_into().unwrap()) as usize,
            slot
        );
        assert_eq!(spec.businesses.len(), 1);
        let request = openmt2001::map_script::Business::from_arguments(
            &[
                openmt2001::lua4::Value::Number(slot as f64),
                openmt2001::lua4::Value::Bytes(spec.businesses[0].as_bytes().to_vec()),
            ],
            &state.world.catalog.info,
        )
        .unwrap()
        .unwrap();
        assert_eq!(word(business, 4), request.kind);
        assert_eq!(
            word(business, 0),
            word(
                state.world.catalog.info.row(request.kind as usize).unwrap(),
                0xc
            )
        );
        let binding = state.models.bindings.get(&slot).unwrap();
        assert_eq!(word(row, 0x170), binding.handle);
        assert_ne!(binding.handle, 0);
        let model = &state.models.models[binding.handle as usize - 1];
        let prepared = openmt2001::geometry::prepare(&scene.layers);
        let positions: Vec<_> = openmt2001::geometry::facade_positions(&prepared)
            .into_iter()
            .chain(openmt2001::roof::mesh(&prepared).positions)
            .map(|p| scene.local_position(p))
            .collect();
        assert!(!positions.is_empty());
        assert_eq!(model.positions, positions);
        let bytes: Vec<_> = positions
            .iter()
            .flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        assert_eq!(
            model.definition,
            openmt2001::mesh::model_definition(&bytes).unwrap()
        );
        assert_eq!(
            binding.translation,
            std::array::from_fn(|i| f32::from_bits(word(row, 0x58 + i * 4)))
        );
        identities.push((slot, business_slot));
    }
    let buildings: std::collections::BTreeSet<_> = identities.iter().map(|pair| pair.0).collect();
    let businesses: std::collections::BTreeSet<_> = identities.iter().map(|pair| pair.1).collect();
    assert_eq!(
        buildings.len(),
        specs.len(),
        "authored building identities must be unique"
    );
    assert_eq!(
        businesses.len(),
        specs.len(),
        "authored business identities must be unique"
    );
    identities
}

#[test]
#[ignore = "requires supplied originals; bounded sequential25-map NET1 startup only"]
fn original_25_skirmish_maps_net1_startup_matrix() {
    let started = std::time::Instant::now();
    std::thread::Builder::new().stack_size(64*1024*1024).spawn(move || {
            let budget = std::time::Duration::from_secs(120);
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let road_routes = read("OPENMT2001_ROAD_ROUTE_TABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let scripts = default_path.parent().unwrap().parent().unwrap();
            let language = openmt2001::fixtures::path("EXE")
                .parent().unwrap().parent().unwrap().join("US/language/usa");
            let names = character_names(&image, &language).unwrap();
            assert_eq!(names[0].as_slice(), b"Racecar");
            assert_eq!(names[1].as_slice(), b"Wheelbarrow");
            let era_styles = load_era_styles(
                Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap();
            let maps = crate::skirmish_maps::Catalog::discover(scripts).unwrap();
            assert_eq!(maps.names().len(), 25, "requires actual supplied25-map catalog");
            let mut seen = std::collections::BTreeSet::new();
            let mut completed = 0;
            use crate::skirmish_maps::MapIdentity;
            use openmt2001::skirmish::roster::{Kind, Participant};
            for (case, basename) in maps.names().iter().enumerate() {
                assert!(started.elapsed() < budget, "120s total startup matrix budget before {basename}");
                assert!(seen.insert(basename.to_ascii_lowercase()), "duplicate supplied basename: {basename}");
                let case_started = std::time::Instant::now();
                println!("NET1_MATRIX BEGIN case={}/25 map={basename} total_ms={}", case+1, started.elapsed().as_millis());
                let resolved = maps.resolve(scripts, basename).unwrap();
                let specs = openmt2001::scene::parse_static_map(std::str::from_utf8(&resolved.bytes).unwrap()).unwrap();
                let source_len = u32::try_from(resolved.bytes.len()).unwrap();
                let checksum = resolved.bytes.iter().fold(0u32, |sum,&byte| sum.wrapping_add(u32::from(byte)));
                let expected_identity = if basename == "empty.lua" && resolved.bytes.is_empty() {
                    MapIdentity::LegacyEmpty
                } else {
                    MapIdentity::Named { basename: basename.clone(), checksum, source_len }
                };
                assert_eq!(resolved.identity, expected_identity);
                let expected_buildings = specs.len();
                let expected_businesses: usize = specs.iter().map(|spec| spec.businesses.len()).sum();
                let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
                let launch = SkirmishLaunch {
                    level_id: 300001, directory: "net1".into(), map: basename.clone(),
                    objective_kind: 6, target: 2000., difficulty: 0,
                    participants: vec![
                        Participant { kind: Kind::Human, token: Some(0), name: names[0].clone(), ready: true, difficulty: 0 },
                        Participant { kind: Kind::Ai, token: Some(1), name: names[1].clone(), ready: true, difficulty: 0 },
                    ],
                };
                // Exactly one live Session per case; drop explicitly before the next map.
                let session = Session::initialize_skirmish(Inputs {
                    tree_locators: None, image: &image, board: &board, file: &file,
                    people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                    vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                    transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                    road_routes: road_routes.clone(), scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(), era_styles: era_styles.clone(),
                    timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "NET1",
                    difficulty: 0, selected_character: 0, profile_name: &names[0], character_names: &names,
                }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0;0x11c] }, catalog, launch).unwrap();
                assert_eq!(session.skirmish_map.as_ref(), Some(&expected_identity), "{basename}");
                let selected = session.catalog.selected_level().unwrap();
                assert_eq!(selected.id(), 300001);
                assert!(!selected.single_player());
                assert!(selected.directory().eq_ignore_ascii_case(b"net1"));
                assert_eq!(selected.objective_target(), 2000.);
                assert_eq!(session.world.objective.kind, 6);
                assert_eq!(session.world.objective.target, 2000.);
                assert_eq!(session.difficulty, 0);
                assert_eq!(session.world.player_money, 8000.);
                assert_eq!(session.roster.manager.players.len(), 2);
                assert_eq!(session.roster.status.ai_count, 1);
                let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
                for (slot,token,kind) in [(0,0,0),(1,1,1)] {
                    let saved = &session.saved_player_metadata[slot];
                    assert_eq!(word(saved,0), token);
                    assert_eq!(word(saved,4), kind);
                    assert_eq!(word(saved,44), 0);
                    assert_eq!(&saved[8..8+names[slot].len()], names[slot].as_slice());
                    let player = &session.roster.manager.players[slot].bytes;
                    assert_eq!(word(player,0x150), token);
                    assert_eq!(word(player,0x154), kind);
                }
                assert!(session.campaign_profile.is_none());
                assert!(session.network_command_timer.is_none());
                let buildings = session.world.buildings.iter().filter(|row| row[0x3f] != 0).count();
                let businesses = session.world.ledgers.businesses.iter().filter(|row| row[0x21c] != 0).count();
                assert_eq!(buildings, expected_buildings, "{basename}: active building count");
                assert_eq!(businesses, expected_businesses, "{basename}: active business count");
                let identities = check_authored_records(&session, &specs, &board, &file);
                assert_eq!(identities.len(), expected_buildings);
                assert_eq!(session.models.bindings.len(), expected_buildings, "{basename}: exact model binding count");
                for &(building,business) in &identities {
                    assert_eq!(session.world.buildings[building][0x2d], 0);
                    assert_eq!(session.world.ledgers.businesses[business][0xa], 0);
                }
                drop(session);
                assert!(started.elapsed() < budget, "120s total startup matrix budget after {basename}");
                completed += 1;
                println!("NET1_MATRIX PASS case={}/25 map={basename} bytes={source_len} sum={checksum} specs={expected_buildings} buildings={buildings} businesses={businesses} bindings={} case_ms={} total_ms={}",
                    case+1, identities.len(), case_started.elapsed().as_millis(), started.elapsed().as_millis());
            }
            assert_eq!(seen.len(), 25);
            assert_eq!(completed, 25);
            assert!(started.elapsed() < budget, "120s total startup matrix budget at completion");
            println!("NET1_MATRIX COMPLETE cases=25 level=300001 difficulty=0 objective=6 target=2000 seed=12345678 host_frames=0 total_ms={}", started.elapsed().as_millis());
        }).unwrap().join().unwrap();
}

fn skirmish_map_cold_reload_continuation(map: &'static str) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let build = read("OPENMT2001_BUILD_FILE");
            let _sun = read("OPENMT2001_SUNCOLS");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index =
                openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let scripts = default_path.parent().unwrap().parent().unwrap();
            let language = openmt2001::fixtures::path("EXE")
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa");

            use crate::skirmish_maps::{MapIdentity, Selection};
            // Read the actual supplied launch file, never manufacture or patch it.
            let authored = if map == "monopolymap.lua" {
                let path = crate::campaign_store::original_path(scripts, "MAPS/monopolymap.lua")
                    .unwrap()
                    .unwrap();
                let bytes = fs::read(path).unwrap();
                assert_eq!(bytes.len(), 222);
                let checksum = bytes
                    .iter()
                    .fold(0u32, |sum, &byte| sum.wrapping_add(u32::from(byte)));
                assert_eq!(checksum, 15_786);
                let specs =
                    openmt2001::scene::parse_static_map(std::str::from_utf8(&bytes).unwrap())
                        .unwrap();
                assert_eq!(specs.len(), 2);
                Some((
                    MapIdentity::Named {
                        basename: map.into(),
                        checksum,
                        source_len: 222,
                    },
                    specs,
                ))
            } else {
                assert_eq!(map, "empty.lua");
                None
            };
            let names = character_names(&image, &language).unwrap();
            use openmt2001::skirmish::roster::{Kind, Participant};
            let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
            let launch = SkirmishLaunch {
                level_id: 300001,
                directory: "net1".into(),
                map: map.into(),
                objective_kind: 6,
                target: 3500.,
                difficulty: 1,
                participants: vec![
                    Participant {
                        kind: Kind::Human,
                        token: Some(4),
                        name: b"Fixture Player".to_vec(),
                        ready: true,
                        difficulty: 2,
                    },
                    Participant {
                        kind: Kind::Ai,
                        token: Some(1),
                        name: b"Explicit Opponent".to_vec(),
                        ready: true,
                        difficulty: 0,
                    },
                ],
            };
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
                road_routes: std::fs::read(openmt2001::fixtures::path("ROAD_ROUTE_TABLE")).unwrap(),
                scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(
                    Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                        .parent()
                        .unwrap(),
                )
                .unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: "NET1",
                difficulty: 1,
                selected_character: 4,
                profile_name: b"Fixture Player",
                character_names: &names,
            };
            let policy = || Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 0x11c],
            };
            let mut session =
                Session::initialize_skirmish(inputs(), policy(), catalog, launch).unwrap();
            let word =
                |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            // Preserve the cold fixture's two-pair distinctness assertions.
            let check_authored =
                |state: &Session<'_>, specs: &[openmt2001::scene::BuildingSpec]| {
                    let identities = check_authored_records(state, specs, &board, &file);
                    assert_ne!(identities[0].0, identities[1].0);
                    assert_ne!(identities[0].1, identities[1].1);
                    identities
                };
            let authored_identities = authored.as_ref().map(|(identity, specs)| {
                assert_eq!(session.skirmish_map.as_ref(), Some(identity));
                assert_eq!(
                    session
                        .world
                        .buildings
                        .iter()
                        .filter(|row| row[0x3f] != 0)
                        .count(),
                    2
                );
                assert_eq!(
                    session
                        .world
                        .ledgers
                        .businesses
                        .iter()
                        .filter(|row| row[0x21c] != 0)
                        .count(),
                    2
                );
                let identities = check_authored(&session, specs);
                for &(building, business) in &identities {
                    // Original AddBuilding/AddBusiness pass neutral owner0.
                    assert_eq!(session.world.buildings[building][0x2d], 0);
                    assert_eq!(session.world.ledgers.businesses[business][0xa], 0);
                }
                identities
            });
            let check_selection = |save: &openmt2001::save::SaveFile<'_>| {
                let extension = openmt2001::save::extension::Extension::parse(save)
                    .unwrap()
                    .unwrap();
                let (version, identity) = match &authored {
                    Some((identity, _)) => (6, identity.clone()),
                    None => (5, MapIdentity::LegacyEmpty),
                };
                assert_eq!(extension.version, version);
                let payload = extension.required(18).unwrap();
                if version == 6 {
                    assert_eq!(
                        payload[..28]
                            .chunks_exact(4)
                            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                            .collect::<Vec<_>>(),
                        [
                            300001,
                            3500f32.to_bits(),
                            1,
                            1,
                            222,
                            15_786,
                            map.len() as u32
                        ]
                    );
                    assert_eq!(&payload[28..], b"monopolymap.lua");
                }
                assert_eq!(
                    payload.len(),
                    if version == 6 { 28 + map.len() } else { 16 }
                );
                let selection = Selection::decode(extension.version, payload).unwrap();
                assert_eq!(selection.level_id, 300001);
                assert_eq!(selection.target.to_bits(), 3500f32.to_bits());
                assert_eq!(selection.map, identity);
                assert_eq!(selection.encode(version).unwrap(), payload);
            };
            assert_eq!(session.catalog.selected_level().unwrap().id(), 300001);
            assert!(!session.catalog.selected_level().unwrap().single_player());
            assert_eq!(
                session.catalog.selected_level().unwrap().objective_target(),
                3500.
            );
            assert_eq!(session.world.objective.kind, 6);
            assert_eq!(session.world.objective.target, 3500.);
            assert_eq!(session.difficulty, 1);
            assert_eq!(session.roster.manager.players.len(), 2);
            assert_eq!(session.roster.status.ai_count, 1);
            assert_eq!(session.world.player_money, 6500.);
            assert_eq!(session.world.ledgers.residents.active_count, 251);
            for (slot, (token, kind, difficulty, peer, name)) in [
                (4, 0, 2, 0, b"Fixture Player".as_slice()),
                (1, 1, 0, u32::MAX, b"Explicit Opponent".as_slice()),
            ]
            .into_iter()
            .enumerate()
            {
                let row = &session.saved_player_metadata[slot];
                assert_eq!(word(row, 0), token);
                assert_eq!(word(row, 4), kind);
                assert_eq!(word(row, 40), peer);
                assert_eq!(word(row, 44), difficulty);
                assert_eq!(&row[8..8 + name.len()], name);
                assert_eq!(
                    word(&session.roster.manager.players[slot].bytes, 0x150),
                    token
                );
                assert_eq!(
                    word(&session.roster.manager.players[slot].bytes, 0x154),
                    kind
                );
                if kind == 1 {
                    assert_eq!(
                        word(&session.roster.manager.players[slot].bytes, 0x13f4),
                        difficulty
                    );
                }
                assert_eq!(session.world.ledgers.players[slot].cash, 6500.);
            }
            assert!(session.campaign_profile.is_none());
            assert!(session.network_command_timer.is_none());
            // A deliberately distinct active objective must not replace selected+870.
            session.world.objective.target = 3499.;
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let camera = openmt2001::flyby::CameraSample {
                eye: [100., 120., 100.],
                target: [0., 0., 0.],
                up: [0., 1., 0.],
                vertical_fov: 0.7,
            };
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            let mut host = crate::live_host::Host::new(camera.eye);
            host.running = true;
            let initial = session.world.clock.timing[0];
            for _ in 0..30 {
                host.tick(&mut session, &assets);
                assert!(host.failure.is_none(), "{:?}", host.failure);
            }
            assert!(session.world.clock.timing[0] > initial);
            struct Scratch(std::path::PathBuf);
            impl Drop for Scratch {
                fn drop(&mut self) {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
            let path = std::path::PathBuf::from("work/save").join(format!(
                "net1-cold-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::create_dir(&path).unwrap();
            let scratch = Scratch(path);
            let bytes = session.save_v2(&image, cameras, 1).unwrap();
            let saved_path = crate::save_menu::write_new(&scratch.0, &bytes).unwrap();
            let bytes = fs::read(saved_path).unwrap();
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            check_selection(&save);
            assert_eq!(save.level_metadata().unwrap()[0], 300001);
            assert_eq!(save.session_option_word().unwrap(), 1);
            let mut resumed = Session::load_original(inputs(), policy(), &bytes).unwrap();
            resumed
                .restore_v2_extension_with_image(&save, &image)
                .unwrap();
            assert_eq!(
                resumed.catalog.selected_level().unwrap().objective_target(),
                3500.
            );
            assert_eq!(resumed.world.objective.target, 3499.);
            assert_eq!(resumed.world.objective.kind, 6);
            assert_eq!(resumed.difficulty, 1);
            assert_eq!(
                &resumed.saved_player_metadata[..2],
                &session.saved_player_metadata[..2]
            );
            assert!(resumed.campaign_profile.is_none());
            assert!(resumed.network_command_timer.is_none());
            assert_eq!(resumed.skirmish_map, session.skirmish_map);
            if let Some((identity, specs)) = &authored {
                assert_eq!(resumed.skirmish_map.as_ref(), Some(identity));
                assert_eq!(
                    check_authored(&session, specs),
                    *authored_identities.as_ref().unwrap()
                );
                assert_eq!(
                    check_authored(&resumed, specs),
                    *authored_identities.as_ref().unwrap()
                );
                assert_eq!(
                    resumed
                        .world
                        .buildings
                        .iter()
                        .filter(|r| r[0x3f] != 0)
                        .count(),
                    session
                        .world
                        .buildings
                        .iter()
                        .filter(|r| r[0x3f] != 0)
                        .count()
                );
                assert_eq!(
                    resumed
                        .world
                        .ledgers
                        .businesses
                        .iter()
                        .filter(|r| r[0x21c] != 0)
                        .count(),
                    session
                        .world
                        .ledgers
                        .businesses
                        .iter()
                        .filter(|r| r[0x21c] != 0)
                        .count()
                );
                for &(building, business) in authored_identities.as_ref().unwrap() {
                    assert_eq!(
                        resumed.world.buildings[building][0x2d],
                        session.world.buildings[building][0x2d]
                    );
                    assert_eq!(
                        resumed.world.ledgers.businesses[business],
                        session.world.ledgers.businesses[business]
                    );
                }
            }
            let reexport = resumed.save_v2(&image, cameras, 1).unwrap();
            let reexport_save = openmt2001::save::SaveFile::parse(&reexport).unwrap();
            check_selection(&reexport_save);
            let mut other_host = crate::live_host::Host::new(camera.eye);
            other_host.running = true;
            // Both hosts restart at the same wall-frame boundary, as visible load does.
            host = crate::live_host::Host::new(camera.eye);
            host.running = true;
            let before = resumed.world.clock.timing[0];
            for _ in 0..60 {
                host.tick(&mut session, &assets);
                other_host.tick(&mut resumed, &assets);
                assert!(host.failure.is_none(), "{:?}", host.failure);
                assert!(other_host.failure.is_none(), "{:?}", other_host.failure);
                assert_eq!(session.world.clock, resumed.world.clock);
                assert_eq!(
                    session.world.ledgers.businesses,
                    resumed.world.ledgers.businesses
                );
                assert_eq!(
                    session.world.pending_world.to_bytes(),
                    resumed.world.pending_world.to_bytes()
                );
                assert_eq!(session.roster.profiles, resumed.roster.profiles);
                assert_eq!(session.script_random_state, resumed.script_random_state);
                for (left, right) in session
                    .roster
                    .manager
                    .players
                    .iter()
                    .zip(&resumed.roster.manager.players)
                {
                    assert_eq!(left.bytes, right.bytes);
                }
            }
            assert!(resumed.world.clock.timing[0] > before);
            // Complete native+modern serialization compares the entire portable world,
            // retained scripts, participant metadata and scheduler after continuation.
            assert_eq!(
                session.save_v2(&image, cameras, 1).unwrap(),
                resumed.save_v2(&image, cameras, 1).unwrap()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
