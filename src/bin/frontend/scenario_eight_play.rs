//! Original Scenario8 profit frontier from its sparse-map startup and legal city play.
use super::{
    CommandAssets, Inputs, Path, Policy, Session, character_names, fs, load_era_styles, word,
};

#[test]
#[ignore = "requires original v1.4 Scenario8 scripts, map, and assets"]
fn legal_scenario_eight_occupied_property_profit_frontier() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let build = read("OPENMT2001_BUILD_FILE");
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe = openmt2001::fixtures::path("EXE");
        let scripts = exe.parent().unwrap().join("scripts");
        let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        let mut session = Session::initialize(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO8", difficulty: 0,
            selected_character: 4, profile_name: b"Scenario8 Legal Profit", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        assert_eq!((session.world.objective.kind, session.world.objective.target), (3, 6000.));
        assert_eq!(session.world.ledgers.players[0].cash, 8000.);
        assert_eq!(session.roster.manager.players.len(), 6);
        let initial_population = session.world.ledgers.residents.active_count;
        let initial_hour = session.world.clock.hour();
        let mut host = crate::live_host::Host::new([0., 20., 0.]);
        host.begin_presentation(&session).unwrap();
        host.set_running(true);
        let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
        host.auction_renderer = Some(sender);
        let presenter = std::thread::spawn(move || {
            let exe = read("OPENMT2001_EXE");
            let dir = read("OPENMT2001_ARCHIVE_DIR");
            let bin = read("OPENMT2001_ARCHIVE_BIN");
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view = crate::auction_bridge::View::default();
            let mut counts = [0; 3];
            while let Ok(request) = receiver.recv() {
                let slot = match &request.operation {
                    crate::auction_bridge::Operation::Prepare { .. } => 0,
                    crate::auction_bridge::Operation::Activate => 1,
                    crate::auction_bridge::Operation::Restore => 2,
                };
                let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                if result.is_ok() { counts[slot] += 1; }
                request.reply.send(result).unwrap();
            }
            counts
        });
        // Sparsemap has occupied City buildings on each of these six blocks.
        // The live linked City tenant rows below supply the rent actually
        // posted by 463e1c: 2361 per cycle across all six at startup.
        // 4477e0 minimum property values total 6100 before rival bids.
        let targets = [26usize, 30, 23, 16, 1, 9];
        let chains = openmt2001::building_pool::category_chains(
            &session.world.categories, &session.world.buildings, 0x591318).unwrap();
        let mut tenant_rent = std::collections::BTreeMap::new();
        let mut quote = std::collections::BTreeMap::new();
        for category in targets {
            assert_eq!(session.world.categories[category][5], 0,
                "Scenario8 authored occupied block is not City owned");
            let tenants = chains[category].iter().filter_map(|&building| {
                let business = word(&session.world.buildings[building], 0x1c) as usize;
                let row = session.world.ledgers.businesses.get(business)?;
                (row[0x21c] != 0 && row[10] == 0).then_some(
                    u16::from_le_bytes(row[0x18..0x1a].try_into().unwrap()) as u32)
            }).collect::<Vec<_>>();
            assert!(!tenants.is_empty(), "Scenario8 property{category} lacks an occupied City tenant");
            tenant_rent.insert(category, tenants.iter().sum::<u32>());
            let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                openmt2001::property_offer::Panel::default(), true).unwrap();
            assert_ne!(inspected.panel.action_visible, 0,
                "Scenario8 property{category} lacks a legal offer action");
            quote.insert(category, inspected.panel.amount);
        }
        assert!(quote.values().sum::<f32>() <= 8000.,
            "Scenario8 six original minimum offers exceed the starting cash before bidding");
        println!("Scenario8 occupied City property rent{tenant_rent:?} quote{quote:?}");
        let mut requested = std::collections::BTreeSet::new();
        let mut acquired = std::collections::BTreeSet::new();
        let mut lost = std::collections::BTreeSet::new();
        let mut transfer_due = std::collections::BTreeMap::new();
        let mut pending: Option<(usize, u32, bool)> = None;
        let mut outcome = None;
        let mut previous_hour = initial_hour;
        let mut hourly_advances: usize = 0;
        let mut positive_metric = false;
        let mut posted_rent = false;
        for _ in 0..45_000 {
            host.frame(&mut session, &assets, 40);
            assert!(host.failure.is_none(), "Scenario8 Host boundary: {:?}", host.failure);
            let status = host.status(&session);
            for prompt in status.prompts.iter().cloned() {
                if let Some(dialog) = prompt.dialog {
                    if dialog.callback == 0x48b490 { outcome = Some(dialog); break; }
                    host.human_dialog_reply(&mut session, &assets,
                        crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context,
                            button: u32::from(dialog.button_count == 2) }).unwrap();
                } else if prompt.dismissal_id != 0 {
                    session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                        &assets, false).unwrap();
                }
            }
            if outcome.is_some() { break; }
            for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                if let openmt2001::live_commands::Command::Auction { descriptor } = command
                    && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                {
                    let category = usize::from(u16::from_le_bytes(descriptor[24..26].try_into().unwrap()));
                    if requested.contains(&category) && !transfer_due.contains_key(&category) {
                        assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                        assert_eq!(outer, inner);
                        assert!(!direct);
                        assert_eq!(inner, session.world.clock.timing[0] + 25);
                        assert_eq!(session.world.categories[category][5], 0,
                            "Scenario8 acquired property before native transfer deadline");
                        transfer_due.insert(category, inner);
                    }
                }
            }
            if let Some((category, submitted, seen_visible)) = pending {
                let seen_visible = seen_visible || status.auction.as_ref().is_some_and(|a| a.visible)
                    && word(&session.world.auction.controller.record, 4) == category as u32;
                let owner = session.world.categories[category][5];
                if owner != 0 {
                    assert!(seen_visible, "Scenario8 property transferred without visible native auction");
                    assert!(session.world.clock.timing[0] > submitted);
                    if owner == 1 {
                        assert_eq!(session.world.clock.timing[0], *transfer_due.get(&category)
                            .expect("Scenario8 human transfer lacks original queued command"));
                        acquired.insert(category);
                    } else {
                        lost.insert(category);
                    }
                    println!("Scenario8 property{category} transfer owner{owner} tick{} cash{}",
                        session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                    pending = None;
                } else {
                    pending = Some((category, submitted, seen_visible));
                }
            }
            if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                let category = word(&session.world.auction.controller.record, 4) as usize;
                let price = word(&session.world.auction.controller.record, 0x6c);
                let next = price.saturating_add(50);
                let wanted = pending.is_some_and(|(selected, _, _)| selected == category);
                let ceiling = quote.get(&category).copied().unwrap_or(0.) + 300.;
                let affordable = session.world.ledgers.players[0].cash >= next as f32 + 200.;
                let button = if wanted && affordable && (next as f32) <= ceiling { 0 } else { 3 };
                host.auction_button(&mut session, &assets, button).unwrap();
                println!("Scenario8 auction category{category} price{price} button{button} cash{}",
                    session.world.ledgers.players[0].cash);
            }
            if pending.is_none() && !status.auction.as_ref().is_some_and(|a| a.visible)
                && let Some(category) = targets.into_iter().find(|category|
                    !requested.contains(category) && session.world.categories[*category][5] == 0)
            {
                let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                    openmt2001::property_offer::Panel::default(), true).unwrap();
                if inspected.panel.action_visible != 0
                    && session.world.ledgers.players[0].cash >= inspected.panel.amount + 200.
                {
                    let submitted = session.world.clock.timing[0];
                    let answer = host.property_offer(&mut session, &assets, category as u32,
                        inspected.panel).unwrap();
                    assert!(answer.submitted, "Scenario8 legal occupied-property offer was denied");
                    assert_eq!(session.world.categories[category][5], 0,
                        "Scenario8 property offer changed ownership synchronously");
                    requested.insert(category);
                    pending = Some((category, submitted, false));
                    println!("Scenario8 property{category} offered{} at tick{submitted}", answer.panel.amount);
                }
            }
            if session.world.clock.hour() != previous_hour {
                previous_hour = session.world.clock.hour();
                hourly_advances += 1;
                let metric = super::campaign_metric(&session, &image, 3);
                positive_metric |= metric > 0.;
                posted_rent |= session.world.ledgers.players[0].ledger(0x22).unwrap() > 0.;
                if hourly_advances <= 4 || hourly_advances.is_multiple_of(24) {
                    let account = &session.world.ledgers.players[0];
                    let nonzero = (0..38).filter_map(|event| account.ledger(event)
                        .filter(|amount| *amount != 0.).map(|amount| (event, amount)))
                        .collect::<Vec<_>>();
                    println!("Scenario8 hour{hourly_advances} population{} requested{} acquired{:?} lost{:?} cash{} profit{metric} reached{} ledger{:?}",
                        session.world.ledgers.residents.active_count, requested.len(),
                        acquired, lost, account.cash, session.world.objective.reached[0], nonzero);
                }
            }
        }
        assert!(!requested.is_empty(), "Scenario8 never admitted an occupied-property offer");
        assert!(hourly_advances > 0, "Scenario8 never advanced its native hourly scheduler");
        let metric = super::campaign_metric(&session, &image, 3);
        println!("Scenario8 property frontier: hour{hourly_advances} population{initial_population}→{} requested{requested:?} acquired{acquired:?} lost{lost:?} pending{pending:?} cash{} rent{} profit{metric} positive={positive_metric} reached{} outcome{:?}",
            session.world.ledgers.residents.active_count, session.world.ledgers.players[0].cash,
            session.world.ledgers.players[0].ledger(0x22).unwrap(), session.world.objective.reached[0],
            outcome.map(|dialog| dialog.context));
        assert!(!acquired.is_empty(), "Scenario8 strategy won no occupied property through native auctions");
        for &category in &acquired {
            assert_eq!(session.world.categories[category][5], 1);
            assert!(tenant_rent[&category] > 0);
        }
        assert!(posted_rent, "Scenario8 acquired City tenants never posted native landlord rent");
        let dialog = outcome.expect("Scenario8 occupied-property portfolio did not reach native profit outcome");
        assert_eq!(session.world.objective.reached[0], 1,
            "Scenario8 outcome arrived without the human profit objective");
        assert!(metric >= 6000., "Scenario8 human profit result lacked threshold");
        host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply {
            callback: dialog.callback, context: dialog.context, button: 0,
        }).unwrap();
        assert_eq!(host.result_sequence(), Some(openmt2001::outcome_notice::Sequence::Win));
        drop(host.auction_renderer.take());
        let counts = presenter.join().unwrap();
        assert!(counts[2] <= counts[0], "Scenario8 auction restore lacked a prepared scene");
        assert!(positive_metric, "Scenario8 result never recorded positive human income");
    }).unwrap().join().unwrap();
}

/// Measure original Scenario8 special-property admission and the first
/// unowned income cycle before selecting a legal acquisition sequence.
#[test]
#[ignore = "requires original v1.4 Scenario8 scripts, map, and assets"]
fn scenario_eight_special_property_income_measurement() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let scripts = exe.parent().unwrap().join("scripts");
            let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
            let mut session = Session::initialize(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO8", difficulty: 0,
                selected_character: 4, profile_name: b"Scenario8 Income Measure", character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
            let assets = CommandAssets { image: &image, board: &board, file: &file };
            assert_eq!((session.world.objective.kind, session.world.objective.target), (3, 6000.));
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.begin_presentation(&session).unwrap();
            host.set_running(true);
            let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            host.auction_renderer = Some(sender);
            let presenter = std::thread::spawn(move || {
                let exe = read("OPENMT2001_EXE");
                let dir = read("OPENMT2001_ARCHIVE_DIR");
                let bin = read("OPENMT2001_ARCHIVE_BIN");
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
                let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view = crate::auction_bridge::View::default();
                while let Ok(request) = receiver.recv() {
                    let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                    request.reply.send(result).unwrap();
                }
            });
            let chains = openmt2001::building_pool::category_chains(
                &session.world.categories, &session.world.buildings, 0x591318).unwrap();
            let mut city_rent = std::collections::BTreeMap::new();
            for category in [0, 1, 9, 13, 16, 23, 26, 30, 36] {
                let rent = chains[category].iter().filter_map(|&building| {
                    let business = word(&session.world.buildings[building], 0x1c) as usize;
                    let row = session.world.ledgers.businesses.get(business)?;
                    (row[0x21c] != 0 && row[10] == 0).then_some(
                        u16::from_le_bytes(row[0x18..0x1a].try_into().unwrap()) as u32)
                }).sum::<u32>();
                assert!(rent > 0, "Scenario8 authored City property{category} lacks tenant rent");
                city_rent.insert(category, rent);
            }
            println!("Scenario8 all occupied City rent{city_rent:?} total{}", city_rent.values().sum::<u32>());
            let mut special_quotes = std::collections::BTreeMap::new();
            for category in [2, 4, 17, 35, 39, 42, 43, 44] {
                let owner = session.world.categories[category][5];
                let ownable = session.world.categories[category][0x74];
                let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                    openmt2001::property_offer::Panel::default(), true).unwrap();
                println!("Scenario8 special{category} owner{owner} ownable{ownable} quote{} offer{}",
                    inspected.panel.amount, inspected.panel.action_visible);
                special_quotes.insert(category, inspected.panel.amount);
                assert_eq!(owner, 0, "Scenario8 special property is already owned");
                assert_eq!(ownable, 1, "Scenario8 special property is not legally ownable");
                assert_ne!(inspected.panel.action_visible, 0,
                    "Scenario8 special property has no legal offer action");
            }
            let railroad_quotes = [4, 17, 43, 44].map(|category| special_quotes[&category]).iter().sum::<f32>();
            let utility_quotes = [2, 35, 39, 42].map(|category| special_quotes[&category]).iter().sum::<f32>();
            println!("Scenario8 special minimum rail quotes{railroad_quotes} utility quotes{utility_quotes} initial cash{}",
                session.world.ledgers.players[0].cash);
            assert_eq!(session.population.properties.totals, [800.; 4],
                "original railroad baseline differs from expected first-cycle freight");
            let mut max_utility = [0f32; 4];
            let mut max_railroad = [0f32; 4];
            let mut reached_settlement = false;
            for _ in 0..28_000 {
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "Scenario8 measurement Host boundary: {:?}", host.failure);
                let status = host.status(&session);
                for prompt in status.prompts {
                    if let Some(dialog) = prompt.dialog {
                        host.human_dialog_reply(&mut session, &assets,
                            crate::human_dialog::Reply { callback: dialog.callback,
                                context: dialog.context, button: u32::from(dialog.button_count == 2) }).unwrap();
                    } else if prompt.dismissal_id != 0 {
                        session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                            &assets, false).unwrap();
                    }
                }
                if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                    host.auction_button(&mut session, &assets, 3).unwrap();
                }
                for (maximum, current) in max_utility.iter_mut().zip(session.hooks.utilities.totals) {
                    *maximum = maximum.max(current);
                }
                for (maximum, current) in max_railroad.iter_mut().zip(session.population.properties.totals) {
                    *maximum = maximum.max(current);
                }
                if session.world.clock.calendar[4] >= 1 && session.world.clock.hour() >= 6 {
                    reached_settlement = true;
                    break;
                }
            }
            assert!(reached_settlement, "Scenario8 did not reach its first daily income settlement");
            println!("Scenario8 first daily raw utility maxima{max_utility:?} total{} railroad maxima{max_railroad:?} total{} owners utilities{:?} railroads{:?}",
                max_utility.iter().sum::<f32>(), max_railroad.iter().sum::<f32>(),
                session.hooks.utilities.owners, session.population.properties.owners);
            drop(host.auction_renderer.take());
            presenter.join().unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "requires original v1.4 Scenario8 scripts, map, and assets"]
fn legal_scenario_eight_railroad_profit_frontier() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let scripts = exe.parent().unwrap().join("scripts");
            let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
            let mut session = Session::initialize(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO8", difficulty: 0,
                selected_character: 4, profile_name: b"Scenario8 Railroad Profit", character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
            let assets = CommandAssets { image: &image, board: &board, file: &file };
            assert_eq!((session.world.objective.kind, session.world.objective.target), (3, 6000.));
            assert_eq!(session.world.ledgers.players[0].cash, 8000.);
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.begin_presentation(&session).unwrap();
            host.set_running(true);
            let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            host.auction_renderer = Some(sender);
            let presenter = std::thread::spawn(move || {
                let exe = read("OPENMT2001_EXE");
                let dir = read("OPENMT2001_ARCHIVE_DIR");
                let bin = read("OPENMT2001_ARCHIVE_BIN");
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
                let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view = crate::auction_bridge::View::default();
                let mut prepared = 0;
                while let Ok(request) = receiver.recv() {
                    if matches!(request.operation, crate::auction_bridge::Operation::Prepare { .. }) {
                        prepared += 1;
                    }
                    let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                    request.reply.send(result).unwrap();
                }
                prepared
            });
            // Each quote and bid below is a live original-panel action. The
            // first four offers pursue the original freight group, then a
            // City-tenant block can supply the remaining profit if affordable.
            let targets = [17usize, 44, 43, 4, 26, 30, 23, 16, 1];
            let railroad = [4usize, 17, 43, 44];
            let mut quotes = std::collections::BTreeMap::new();
            for category in targets {
                let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                    openmt2001::property_offer::Panel::default(), true).unwrap();
                assert_ne!(inspected.panel.action_visible, 0,
                    "Scenario8 target{category} lacks original offer action");
                assert_eq!(session.world.categories[category][5], 0);
                quotes.insert(category, inspected.panel.amount);
            }
            assert_eq!(railroad.map(|category| quotes[&category]).iter().sum::<f32>(), 2200.);
            let mut requested = std::collections::BTreeSet::new();
            let mut acquired = std::collections::BTreeSet::new();
            let mut lost = std::collections::BTreeSet::new();
            let mut transfer_due = std::collections::BTreeMap::new();
            let mut pending: Option<(usize, u32, bool)> = None;
            let mut outcome = None;
            let mut previous_hour = session.world.clock.hour();
            let mut hourly_advances = 0usize;
            for _ in 0..75_000 {
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "Scenario8 rail-first Host boundary: {:?}", host.failure);
                let status = host.status(&session);
                for prompt in status.prompts.iter().cloned() {
                    if let Some(dialog) = prompt.dialog {
                        if dialog.callback == 0x48b490 { outcome = Some(dialog); break; }
                        host.human_dialog_reply(&mut session, &assets,
                            crate::human_dialog::Reply { callback: dialog.callback,
                                context: dialog.context, button: u32::from(dialog.button_count == 2) }).unwrap();
                    } else if prompt.dismissal_id != 0 {
                        session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                            &assets, false).unwrap();
                    }
                }
                if outcome.is_some() { break; }
                for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                    if let openmt2001::live_commands::Command::Auction { descriptor } = command
                        && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                    {
                        let category = usize::from(u16::from_le_bytes(descriptor[24..26].try_into().unwrap()));
                        if requested.contains(&category) && !transfer_due.contains_key(&category) {
                            assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                            assert_eq!(outer, inner);
                            assert!(!direct);
                            assert_eq!(inner, session.world.clock.timing[0] + 25);
                            transfer_due.insert(category, inner);
                        }
                    }
                }
                if let Some((category, submitted, seen_visible)) = pending {
                    let visible = status.auction.as_ref().is_some_and(|a| a.visible);
                    let seen_visible = seen_visible || visible
                        && word(&session.world.auction.controller.record, 4) == category as u32;
                    let owner = session.world.categories[category][5];
                    if owner == 1 {
                        assert!(seen_visible, "Scenario8 rail/category transferred without a visible auction");
                        assert!(session.world.clock.timing[0] > submitted);
                        assert_eq!(session.world.clock.timing[0], *transfer_due.get(&category)
                            .expect("Scenario8 human acquisition lacks original queued transfer"));
                        acquired.insert(category);
                        pending = None;
                        println!("Scenario8 rail-first acquired{category} tick{} cash{}",
                            session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                    } else if seen_visible && !visible && owner != 0 {
                        lost.insert(category);
                        pending = None;
                        println!("Scenario8 rail-first lost{category} owner{owner} tick{}",
                            session.world.clock.timing[0]);
                    } else {
                        pending = Some((category, submitted, seen_visible));
                    }
                }
                if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                    let category = word(&session.world.auction.controller.record, 4) as usize;
                    let price = word(&session.world.auction.controller.record, 0x6c);
                    let next = price.saturating_add(50);
                    let wanted = pending.is_some_and(|(selected, _, _)| selected == category);
                    // The prior native run exposed rival bids of 2600 on
                    // Reading, 1800 on Pennsylvania, and 1900 on Virginia.
                    // Keep each response within cash and reserve the original
                    // 550 opening quote for every railroad still to offer.
                    let ceiling = match category {
                        17 => 1750.,
                        44 => 3000.,
                        43 => 2500.,
                        4 => 1600.,
                        26 => 2300.,
                        _ => 1800.,
                    };
                    let remaining_railroads = railroad.iter().filter(|&&target|
                        target != category && !requested.contains(&target)).count();
                    let reserve = remaining_railroads as f32 * 550. + 200.;
                    let affordable = session.world.ledgers.players[0].cash >= next as f32 + reserve;
                    let button = if wanted && affordable && (next as f32) <= ceiling { 0 } else { 3 };
                    host.auction_button(&mut session, &assets, button).unwrap();
                    println!("Scenario8 rail-first auction{category} price{price} button{button} cash{}",
                        session.world.ledgers.players[0].cash);
                }
                if pending.is_none() && !status.auction.as_ref().is_some_and(|a| a.visible)
                    && let Some(category) = targets.into_iter().find(|category| !requested.contains(category))
                {
                    let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                        openmt2001::property_offer::Panel::default(), true).unwrap();
                    if inspected.panel.action_visible != 0
                        && session.world.ledgers.players[0].cash >= inspected.panel.amount + 200.
                    {
                        let submitted = session.world.clock.timing[0];
                        let answer = host.property_offer(&mut session, &assets, category as u32,
                            inspected.panel).unwrap();
                        assert!(answer.submitted, "Scenario8 rail-first original offer was denied");
                        assert_ne!(session.world.categories[category][5], 1,
                            "Scenario8 rail-first offer changed ownership synchronously");
                        requested.insert(category);
                        pending = Some((category, submitted, false));
                        println!("Scenario8 rail-first offered{category} amount{} tick{submitted}", answer.panel.amount);
                    }
                }
                if session.world.clock.hour() != previous_hour {
                    previous_hour = session.world.clock.hour();
                    hourly_advances += 1;
                    if hourly_advances <= 4 || previous_hour == 6 {
                        let player = &session.world.ledgers.players[0];
                        let income = (25..=34).filter_map(|event| player.ledger(event)
                            .filter(|value| *value != 0.).map(|value| (event, value)))
                            .collect::<Vec<_>>();
                        println!("Scenario8 rail-first day{} hour{} owners{:?} cash{} profit{} reached{} income{:?}",
                            session.world.clock.calendar[4], previous_hour,
                            targets.map(|category| (category, session.world.categories[category][5])),
                            player.cash, super::campaign_metric(&session, &image, 3),
                            session.world.objective.reached[0], income);
                    }
                }
            }
            let metric = super::campaign_metric(&session, &image, 3);
            println!("Scenario8 rail-first frontier hour{hourly_advances} requested{requested:?} acquired{acquired:?} lost{lost:?} pending{pending:?} cash{} profit{metric} reached{} outcome{:?}",
                session.world.ledgers.players[0].cash, session.world.objective.reached[0],
                outcome.map(|dialog| dialog.context));
            assert!(railroad.iter().all(|category| acquired.contains(category)),
                "Scenario8 rail-first strategy did not acquire all four railroads");
            assert!(acquired.iter().any(|category| !railroad.contains(category)),
                "Scenario8 rail-first strategy acquired no occupied City property");
            assert_eq!(session.world.objective.reached[0], 1,
                "Scenario8 original profit objective not reached");
            assert!(metric >= 6000., "Scenario8 objective was reached below its original threshold");
            let dialog = outcome.expect("Scenario8 original profit result was not admitted");
            assert_eq!(dialog.context, 0, "Scenario8 result belongs to another player");
            host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply {
                callback: dialog.callback, context: dialog.context, button: 0,
            }).unwrap();
            assert_eq!(host.result_sequence(), Some(openmt2001::outcome_notice::Sequence::Win));
            drop(host.auction_renderer.take());
            assert!(presenter.join().unwrap() >= 1, "Scenario8 never presented its native auctions");
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Utility charges and payouts share one native daily callback, so unowned
/// aggregate totals are not observable at a frame boundary. Buy utility
/// leases through the original Host auction path and read posted income.
#[test]
#[ignore = "requires original v1.4 Scenario8 scripts, map, and assets"]
fn scenario_eight_utility_income_auction_measurement() {
    run_scenario_eight_utility_income_auctions([2400; 4], false, 2);
}

/// The first legal auction run measured the rivals at $2500 and $2750 for
/// gas and water. Contest those bids while preserving a cash reserve for the
/// remaining original utility offer and observe the actual owner-income path.
#[test]
#[ignore = "requires original v1.4 Scenario8 scripts, map, and assets"]
fn scenario_eight_four_utility_income_frontier() {
    run_scenario_eight_utility_income_auctions([1050, 2700, 2900, 1600], true, 2);
}

/// Follow the same legally auctioned utility portfolio through subsequent
/// original daily settlements to test whether passive income can meet the
/// campaign objective without inventing a profit shortcut.
#[test]
#[ignore = "requires original v1.4 Scenario8 scripts, map, and assets"]
fn scenario_eight_two_utility_profit_horizon() {
    run_scenario_eight_utility_income_auctions([2400; 4], false, 12);
}

fn run_scenario_eight_utility_income_auctions(caps: [u32; 4], require_all: bool, horizon_day: u32) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let scripts = exe.parent().unwrap().join("scripts");
            let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
            let mut session = Session::initialize(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO8", difficulty: 0,
                selected_character: 4, profile_name: b"Scenario8 Utility Measure", character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
            let assets = CommandAssets { image: &image, board: &board, file: &file };
            let targets = [2usize, 35, 39, 42];
            assert_eq!(session.world.ledgers.players[0].cash, 8000.);
            assert_eq!((session.world.objective.kind, session.world.objective.target), (3, 6000.));
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.begin_presentation(&session).unwrap();
            host.set_running(true);
            let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            host.auction_renderer = Some(sender);
            let presenter = std::thread::spawn(move || {
                let exe = read("OPENMT2001_EXE");
                let dir = read("OPENMT2001_ARCHIVE_DIR");
                let bin = read("OPENMT2001_ARCHIVE_BIN");
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
                let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view = crate::auction_bridge::View::default();
                let mut prepared = 0;
                while let Ok(request) = receiver.recv() {
                    if matches!(request.operation, crate::auction_bridge::Operation::Prepare { .. }) {
                        prepared += 1;
                    }
                    let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                    request.reply.send(result).unwrap();
                }
                prepared
            });
            let mut requested = std::collections::BTreeSet::new();
            let mut acquired = std::collections::BTreeSet::new();
            let mut lost = std::collections::BTreeSet::new();
            let mut transfer_due = std::collections::BTreeMap::new();
            let mut pending: Option<(usize, u32, bool)> = None;
            let mut result = None;
            let mut settlement_rows = Vec::new();
            let mut previous_hour = session.world.clock.hour();
            let frame_limit = if horizon_day > 2 { 400_000 } else { 65_000 };
            for _ in 0..frame_limit {
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "Scenario8 utility Host boundary: {:?}", host.failure);
                let status = host.status(&session);
                for prompt in status.prompts.iter().cloned() {
                    if let Some(dialog) = prompt.dialog {
                        if dialog.callback == 0x48b490 { result = Some(dialog); break; }
                        host.human_dialog_reply(&mut session, &assets,
                            crate::human_dialog::Reply { callback: dialog.callback,
                                context: dialog.context, button: u32::from(dialog.button_count == 2) }).unwrap();
                    } else if prompt.dismissal_id != 0 {
                        session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                            &assets, false).unwrap();
                    }
                }
                if result.is_some() { break; }
                for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                    if let openmt2001::live_commands::Command::Auction { descriptor } = command
                        && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                    {
                        let category = usize::from(u16::from_le_bytes(descriptor[24..26].try_into().unwrap()));
                        if requested.contains(&category) && !transfer_due.contains_key(&category) {
                            assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                            assert_eq!(outer, inner);
                            assert!(!direct);
                            assert_eq!(inner, session.world.clock.timing[0] + 25);
                            transfer_due.insert(category, inner);
                        }
                    }
                }
                if let Some((category, submitted, seen_visible)) = pending {
                    let visible = status.auction.as_ref().is_some_and(|a| a.visible);
                    let seen_visible = seen_visible || visible
                        && word(&session.world.auction.controller.record, 4) == category as u32;
                    let owner = session.world.categories[category][5];
                    if owner == 1 {
                        assert!(seen_visible, "Scenario8 utility ownership bypassed visible auction");
                        assert!(session.world.clock.timing[0] > submitted);
                        assert_eq!(session.world.clock.timing[0], *transfer_due.get(&category)
                            .expect("Scenario8 utility has no original queued human transfer"));
                        acquired.insert(category);
                        pending = None;
                        println!("Scenario8 utility acquired{category} tick{} cash{}",
                            session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                    } else if seen_visible && !visible && owner != 0 {
                        lost.insert(category);
                        pending = None;
                        println!("Scenario8 utility lost{category} owner{owner} tick{}",
                            session.world.clock.timing[0]);
                    } else {
                        pending = Some((category, submitted, seen_visible));
                    }
                }
                if require_all && !lost.is_empty() { break; }
                if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                    let category = word(&session.world.auction.controller.record, 4) as usize;
                    let price = word(&session.world.auction.controller.record, 0x6c);
                    let next = price.saturating_add(50);
                    let wanted = pending.is_some_and(|(selected, _, _)| selected == category);
                    let remaining = targets.iter().filter(|&&target|
                        target != category && !requested.contains(&target)).count();
                    let reserve = remaining as f32 * 550. + 200.;
                    let affordable = session.world.ledgers.players[0].cash >= next as f32 + reserve;
                    let cap = targets.iter().position(|&target| target == category)
                        .map(|index| caps[index]).unwrap_or(0);
                    let button = if wanted && affordable && next <= cap { 0 } else { 3 };
                    host.auction_button(&mut session, &assets, button).unwrap();
                    println!("Scenario8 utility auction{category} price{price} button{button} cash{}",
                        session.world.ledgers.players[0].cash);
                }
                if pending.is_none() && !status.auction.as_ref().is_some_and(|a| a.visible)
                    && let Some(category) = targets.into_iter().find(|category| !requested.contains(category))
                {
                    let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                        openmt2001::property_offer::Panel::default(), true).unwrap();
                    assert_eq!(session.world.categories[category][0x74], 1);
                    assert_ne!(inspected.panel.action_visible, 0,
                        "Scenario8 utility lacks original property offer action");
                    assert_eq!(inspected.panel.amount, 550.);
                    if session.world.ledgers.players[0].cash >= inspected.panel.amount + 200. {
                        let submitted = session.world.clock.timing[0];
                        let answer = host.property_offer(&mut session, &assets, category as u32,
                            inspected.panel).unwrap();
                        assert!(answer.submitted, "Scenario8 utility offer was denied");
                        requested.insert(category);
                        pending = Some((category, submitted, false));
                        println!("Scenario8 utility offered{category} amount{} tick{submitted}", answer.panel.amount);
                    }
                }
                if session.world.clock.hour() != previous_hour {
                    previous_hour = session.world.clock.hour();
                    if previous_hour == 6 && session.world.clock.calendar[4] > 0 {
                        let income = [28, 27, 25, 26].map(|event|
                            session.world.ledgers.players[0].ledger(event).unwrap());
                        let owners = targets.map(|category| session.world.categories[category][5]);
                        println!("Scenario8 utility day{} owners{owners:?} income{income:?} cash{} profit{} reached{}",
                            session.world.clock.calendar[4], session.world.ledgers.players[0].cash,
                            super::campaign_metric(&session, &image, 3), session.world.objective.reached[0]);
                        settlement_rows.push((owners, income));
                        for (owner, amount) in owners.into_iter().zip(income) {
                            if owner == 1 { assert!(amount > 0., "owned original utility posted no income"); }
                            else { assert_eq!(amount, 0., "unowned original utility paid human income"); }
                        }
                    }
                }
                if requested.len() == targets.len() && pending.is_none()
                    && session.world.clock.calendar[4] >= horizon_day && session.world.clock.hour() >= 6
                {
                    break;
                }
            }
            println!("Scenario8 utility frontier caps{caps:?} requested{requested:?} acquired{acquired:?} lost{lost:?} cash{} profit{} reached{} result{:?} settlements{settlement_rows:?}",
                session.world.ledgers.players[0].cash, super::campaign_metric(&session, &image, 3),
                session.world.objective.reached[0], result.map(|dialog| dialog.context));
            if horizon_day > 2 && result.is_none() {
                assert!(session.world.clock.calendar[4] >= horizon_day,
                    "Scenario8 long-horizon utility measurement ended before its requested day");
            }
            if require_all {
                assert_eq!(acquired.len(), targets.len(),
                    "Scenario8 four-utility budget or rival bids defeated the legal plan");
            }
            assert_eq!(requested.len(), targets.len(), "Scenario8 did not offer all four utilities");
            assert!(!settlement_rows.is_empty(), "Scenario8 did not measure a daily utility payout");
            assert!(!acquired.is_empty(), "Scenario8 legal utility auctions yielded no human lease to measure");
            if let Some(dialog) = result {
                assert_eq!(dialog.context, 0);
                assert_eq!(session.world.objective.reached[0], 1);
                host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply {
                    callback: dialog.callback, context: dialog.context, button: 0,
                }).unwrap();
                assert_eq!(host.result_sequence(), Some(openmt2001::outcome_notice::Sequence::Win));
            }
            drop(host.auction_renderer.take());
            assert!(presenter.join().unwrap() >= 1,
                "Scenario8 utility auctions were never presented");
        })
        .unwrap()
        .join()
        .unwrap();
}
