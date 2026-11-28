//! Post-load AI reconstruction and market-driven resident decisions.
use super::*;

#[test]
#[ignore = "requires original v1.4 game assets and audited scripts"]
fn engine_save_keeps_native_ai_bootstrap_and_invalidates_market() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use crate::fresh_city::{CommandAssets, Inputs, Policy};
            let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            let exe = read("OPENMT2001_EXE");
            let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let build = read("OPENMT2001_BUILD_FILE");
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let index =
                openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let scripts = default.parent().unwrap().parent().unwrap();
            let styles = crate::fresh_city::load_era_styles(
                std::path::Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                    .parent()
                    .unwrap(),
            )
            .unwrap();
            let names = std::array::from_fn(|_| Vec::new());
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
                scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: styles.clone(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: "SCENARIO1",
                difficulty: 0,
                selected_character: 4,
                profile_name: b"AI save fixture",
                character_names: &names,
            };
            let policy = || Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 284],
            };
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let camera = openmt2001::flyby::CameraSample {
                eye: [100., 120., 100.],
                target: [0.; 3],
                up: [0., 1., 0.],
                vertical_fov: 0.7,
            };
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            let mut session = Session::initialize(inputs(), policy()).unwrap();
            assert_eq!(
                session.roster.manager.players.len(),
                4,
                "original Scenario1 must include three live opponents"
            );
            let mut host = crate::live_host::Host::new(camera.eye);
            host.running = true;
            while session.world.clock.timing[0] < 400 {
                host.tick(&mut session, &assets);
                assert!(host.failure.is_none(), "{:?}", host.failure);
                for prompt in host.take_prompts(&session) {
                    session
                        .dispatch_session_event(
                            "InterfacePromptDismiss",
                            prompt.dismissal_id as i32,
                            &assets,
                            true,
                        )
                        .unwrap();
                }
            }

            // Poison only extension-owned transient data after the live save cut.
            for profile in &mut session.roster.profiles {
                profile[0x500..0x504].fill(0x77);
            }
            session.world.ai_demand.rows.as_flattened_mut().fill(0x55);
            session.world.ai_demand.ready = 255;
            session.roster.category_counts.fill(1234);
            session
                .roster
                .ai_ownership
                .lists
                .insert(0x33333000, vec![[0x55; 28]]);
            session.world.ai_neighbors.owned_properties.fill(1234);
            session.population.business_counts.values.fill(1234);
            for player in &mut session.roster.manager.players {
                if word(&player.bytes, 0x154) == 1 {
                    put(&mut player.bytes, 0x13e0, 1234);
                }
            }
            let buildings = session
                .world
                .buildings
                .iter()
                .map(|b| b[..64].try_into().unwrap())
                .collect::<Vec<[u8; 64]>>();
            let inputs_market = openmt2001::market::Inputs {
                businesses: &session.world.ledgers.businesses,
                buildings: &buildings,
                catalogs: &session.population.settings.catalogs,
                commodities: &session.population.settings.commodities,
            };
            let mut fresh = openmt2001::market::Cache {
                dirty: true,
                prices: [0.; 32],
                rent: 0.,
            };
            let average = fresh.residential_rent(&inputs_market).unwrap();
            assert!(average.is_finite() && average > 0.);
            session.population.market = openmt2001::market::Cache {
                dirty: false,
                prices: [-999.; 32],
                rent: average / 2.,
            };
            assert_eq!(
                housing_preference(&mut session.population.market, &inputs_market, average),
                0
            );
            let bytes = session.save_v2(&image, cameras, 1).unwrap();
            let save = SaveFile::parse(&bytes).unwrap();
            let mut resumed = Session::load_original(inputs(), policy(), &bytes).unwrap();
            let profiles = resumed.roster.profiles.clone();
            let counts = resumed.roster.category_counts;
            let demand = resumed.world.ai_demand.rows;
            let ready = resumed.world.ai_demand.ready;
            let neighbors = resumed.world.ai_neighbors.rows;
            let owned = resumed.world.ai_neighbors.owned_properties;
            let groups = resumed.world.ai_neighbors.completed_groups;
            let business_counts = resumed.population.business_counts.values;
            let scores = resumed.roster.scripts.scores.bytes;
            let players = resumed
                .roster
                .manager
                .players
                .iter()
                .map(|p| p.bytes.clone())
                .collect::<Vec<_>>();
            let ownership = resumed.roster.ai_ownership.lists.clone();
            assert_ne!(profiles, session.roster.profiles);
            assert_ne!(demand, session.world.ai_demand.rows);
            assert_ne!(ownership, session.roster.ai_ownership.lists);
            resumed
                .restore_v2_extension_with_image(&save, &image)
                .unwrap();
            assert_eq!(resumed.roster.profiles, profiles);
            assert_eq!(resumed.roster.category_counts, counts);
            assert_eq!(resumed.world.ai_demand.rows, demand);
            assert_eq!(resumed.world.ai_demand.ready, ready);
            assert_eq!(resumed.world.ai_neighbors.rows, neighbors);
            assert_eq!(resumed.world.ai_neighbors.owned_properties, owned);
            assert_eq!(resumed.world.ai_neighbors.completed_groups, groups);
            assert_eq!(resumed.population.business_counts.values, business_counts);
            assert_eq!(resumed.population.business_counts.age, 100);
            assert_eq!(resumed.world.ownership_refresh_counter, 100);
            assert_eq!(resumed.roster.scripts.scores.bytes, scores);
            assert_eq!(resumed.roster.ai_ownership.lists, ownership);
            for ((loaded, native), saved) in resumed
                .roster
                .manager
                .players
                .iter()
                .zip(&players)
                .zip(&session.roster.manager.players)
            {
                assert_eq!(&loaded.bytes[4..0x1bc], &native[4..0x1bc]);
                assert_eq!(&loaded.bytes[4..8], &saved.bytes[4..8], "saved cash");
                if word(&loaded.bytes, 0x154) == 1 {
                    assert_eq!(
                        &loaded.bytes[0x238..],
                        &native[0x238..],
                        "native strategy/ownership bootstrap"
                    );
                }
            }
            assert_eq!(
                resumed.world.ledgers.businesses,
                session.world.ledgers.businesses
            );
            for (saved, loaded) in session
                .world
                .categories
                .iter()
                .zip(&resumed.world.categories)
            {
                assert_eq!(saved[5], loaded[5], "saved property ownership");
            }
            assert!(resumed.population.market.dirty);
            let buildings = resumed
                .world
                .buildings
                .iter()
                .map(|b| b[..64].try_into().unwrap())
                .collect::<Vec<[u8; 64]>>();
            let inputs_market = openmt2001::market::Inputs {
                businesses: &resumed.world.ledgers.businesses,
                buildings: &buildings,
                catalogs: &resumed.population.settings.catalogs,
                commodities: &resumed.population.settings.commodities,
            };
            assert_eq!(
                housing_preference(&mut resumed.population.market, &inputs_market, average),
                1,
                "first qualifying resident decision uses restored businesses, not stale clean cache"
            );
            assert!(!resumed.population.market.dirty);
            assert_eq!(resumed.population.market.rent, average);
            assert_eq!(resumed.population.market.prices, fresh.prices);
            let mut resumed_host = crate::live_host::Host::new(camera.eye);
            resumed_host.running = true;
            let mut auction_admission = None;
            while resumed.world.clock.timing[0] < 2000 {
                resumed_host.tick(&mut resumed, &assets);
                assert!(resumed_host.failure.is_none(), "{:?}", resumed_host.failure);
                for prompt in resumed_host.take_prompts(&resumed) {
                    resumed
                        .dispatch_session_event(
                            "InterfacePromptDismiss",
                            prompt.dismissal_id as i32,
                            &assets,
                            true,
                        )
                        .unwrap();
                }
                if resumed.world.auction.controller.reference != 0 {
                    auction_admission = Some(resumed.world.clock.timing[0]);
                    break;
                }
            }
            assert!(
                auction_admission.is_some(),
                "rebuilt opponents must make an auction decision"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

// Exercise the resident scoring path with two affordable offers. Only the
// citywide rent callback varies; it reads the real restored business pools.
fn housing_preference(
    cache: &mut openmt2001::market::Cache,
    market: &openmt2001::market::Inputs<'_>,
    average: f32,
) -> usize {
    let scores = [0., 1.5]
        .into_iter()
        .enumerate()
        .map(|(i, district_value)| {
            openmt2001::resident::score_housing_category_cached(
                &[openmt2001::resident::HousingCandidate {
                    residential: true,
                    capacity: 1,
                    occupied: 0,
                    attribute_2c: 0,
                    rent: average * if i == 0 { 0.5 } else { 1. },
                }],
                None,
                openmt2001::resident::HousingScoreInputs {
                    affordability_factor: 1.,
                    welfare: average * 5.,
                    rent_weight: 1.,
                    district_weight: 1.,
                    district_value,
                    average_rent: 0.,
                    current_category: false,
                },
                || cache.residential_rent(market),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    usize::from(scores[1] > scores[0])
}

#[test]
fn player_runtime_keeps_native_strategy_and_saved_account_rng() {
    let mut player = openmt2001::player_manager::Player {
        reference: 0x20000000,
        bytes: vec![0; 0x1e50],
        auxiliaries: vec![],
    };
    put(&mut player.bytes, 0x154, 1);
    put(&mut player.bytes, 4, 1234_f32.to_bits());
    put(&mut player.bytes, 0x13e0, 42);
    let mut saved = player.bytes.clone();
    put(&mut saved, 0x13e0, 999);
    put(&mut saved, 0x220, 1);
    put(&mut saved, 0x224, 7);
    put(&mut saved, 0x228, 0x343fd);
    put(&mut saved, 0x22c, 0x269ec3);
    put(&mut saved, 0x21c, 0x123000);
    restore_player_runtime(&mut player, &saved, false).unwrap();
    assert_eq!(word(&player.bytes, 0x13e0), 42);
    assert_eq!(word(&player.bytes, 0x220), 0);
    assert_eq!(word(&player.bytes, 4), 1234_f32.to_bits());
    assert_eq!(word(&player.bytes, 0x21c), 0x123000);
    let mut profiles = [[0; 0x650]; 10];
    put(&mut profiles[0], 0x614, 1000);
    assert!(player.initial_count(&profiles, 400).unwrap() > 0);
    assert_ne!(
        word(&player.bytes, 0x224),
        7,
        "saved embedded RNG remains usable"
    );
    assert!(restore_player_runtime(&mut player, &saved[..100], false).is_err());
}

#[test]
fn clean_saved_market_recomputes_before_resident_choice() {
    let mut businesses = vec![[0; 544]; 600];
    for business in &mut businesses {
        business[8..10].fill(255);
    }
    businesses[0][8..10].fill(0);
    put(&mut businesses[0], 0, 1);
    businesses[0][0xe..0x10].copy_from_slice(&1_u16.to_le_bytes());
    put(&mut businesses[0], 0x1dc, 100_f32.to_bits());
    put(&mut businesses[0], 0x1c4, 30_f32.to_bits());
    let mut buildings = vec![[0; 64]; 600];
    buildings[0][0x3f] = 1;
    let mut catalogs = openmt2001::shopping::Catalogs {
        commodities: std::array::from_fn(|_| Vec::new()),
        fractions: std::array::from_fn(|_| Vec::new()),
    };
    catalogs.commodities[0].push(0);
    let commodities = [openmt2001::resident::Commodity {
        active: false,
        value: 10.,
        markup: 2.,
    }; 32];
    let inputs = openmt2001::market::Inputs {
        businesses: &businesses,
        buildings: &buildings,
        catalogs: &catalogs,
        commodities: &commodities,
    };
    let mut market = openmt2001::market::Cache {
        dirty: false,
        prices: [-999.; 32],
        rent: 50.,
    };
    assert_eq!(housing_preference(&mut market, &inputs, 100.), 0);
    let saved = words(
        [0].into_iter()
            .chain(market.prices.map(f32::to_bits))
            .chain([market.rent.to_bits()]),
    );
    let mut input = Input::new(&saved);
    restore_market_runtime(&mut input, &mut market, false).unwrap();
    input.end().unwrap();
    assert!(market.dirty);
    assert_eq!(housing_preference(&mut market, &inputs, 100.), 1);
    assert!(!market.dirty);
    assert_eq!(market.rent, 100.);
    assert_eq!(market.price(0, &inputs).unwrap(), 25.);
    let mut invalid = saved.clone();
    put(&mut invalid, 0, 2);
    assert!(restore_market_runtime(&mut Input::new(&invalid), &mut market, false).is_err());
}
