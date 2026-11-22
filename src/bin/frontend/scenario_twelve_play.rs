//! Original Scenario12 wealth objective on the naturally unlocked campaign.
//! Reuse the accepted parent's legal builders, controls and result sequence.
use super::{
    CommandAssets, Inputs, Path, PathBuf, Policy, Session, Terms, apply_terms, character_names,
    current, export_live_price_scores, finish_original_win, fs, load_era_styles,
    submit_business_shape, validate_natural_owned_shoppers, word,
};

pub(super) fn wealth(session: &Session<'_>, image: &openmt2001::pe::PeImage<'_>) -> openmt2001::x87::Sum {
    let financial = crate::player_info::State::from_runtime(
        &session.roster.manager,
        &session.world.ledgers.players,
        session.roster.status.current_player,
    )
    .unwrap();
    let value = openmt2001::player_metrics::runtime_metric(
        &session.world,
        &financial.records[0],
        0,
        0x47fbb0,
        image,
        0x591318,
    )
    .unwrap();
    let data =
        crate::comparison::Data::from_runtime(&session.world, &financial.records, image, 0x591318)
            .unwrap();
    let controls = crate::comparison::State::new(5);
    assert_eq!((controls.mode, controls.label()), (4, 1534));
    assert_eq!(
        data.values(4)
            .into_iter()
            .find(|(slot, _)| *slot == 0)
            .unwrap()
            .1,
        value.to_f32(),
        "wealth UI rounded a different original objective value"
    );
    value
}

pub(super) fn export(session: &Session<'_>, image: &openmt2001::pe::PeImage<'_>, name: &str) {
    let financial = crate::player_info::State::from_runtime(
        &session.roster.manager,
        &session.world.ledgers.players,
        session.roster.status.current_player,
    )
    .unwrap();
    let snapshot =
        crate::comparison::Data::verify_runtime(&session.world, &financial.records, image, name, 0);
    println!("{name} original metric snapshot{:?}", snapshot.rows);
}

/// Hotel is original planning's special residential choice, despite its
/// script catalog class. Use the actual landlord/color-group gate and Form.
fn submit_hotel(session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) -> Option<i32> {
    for category in 0..52 {
        let context = crate::build_panel::Context::from_session(session, category).unwrap();
        if !matches!(context.entry(), crate::build_panel::Entry::Open) {
            continue;
        }
        let members = crate::build_panel::special_residential_group(
            assets.image,
            word(&context.category_record, 0x8c),
        )
        .unwrap();
        if !crate::build_panel::special_residential_group_owned(
            &members,
            context.category_record[5],
            &context.category_owners,
        )
        .unwrap()
        {
            continue;
        }
        for x in 0..=5 {
            for y in 0..=4 {
                let mut form = crate::build_panel::Form {
                    category: category as i32,
                    rectangle: [x, y, x + 1, y + 2],
                    height: 5,
                    quality: 0,
                    choices: [0; 3],
                    variants: vec![0],
                    kind: 1,
                    business: 43,
                    displayed_quote: 0.,
                };
                let request = form.request(false, true, true).unwrap().unwrap();
                let quote = session.inspect_build(&request, assets).unwrap();
                if !quote.allowed() || session.world.ledgers.players[0].cash - quote.total < 1500. {
                    continue;
                }
                form.displayed_quote = quote.total;
                let request = form
                    .request(
                        quote.category_locked,
                        quote.affordable,
                        quote.preview.is_some(),
                    )
                    .unwrap()
                    .unwrap();
                assert!(
                    session
                        .submit_build(&request, assets, |_| Err(
                            "Hotel applied before original deadline".into()
                        ))
                        .unwrap()
                        .submitted
                );
                println!(
                    "legal human Residential Hotel43 category{category} size2x3 height5 quote{} group{members:?} landlord{}",
                    quote.total, context.category_record[5]
                );
                return Some(category as i32);
            }
        }
    }
    None
}

fn hotel_sales(session: &Session<'_>) -> i32 {
    session
        .world
        .ledgers
        .businesses
        .iter()
        .filter(|b| b[0x21c] != 0 && b[10] == 1 && word(*b, 4) == 43)
        .map(|b| {
            (0..4)
                .map(|p| {
                    i32::from(i16::from_le_bytes(
                        b[0x82 + p * 2..0x84 + p * 2].try_into().unwrap(),
                    ))
                })
                .sum::<i32>()
        })
        .sum()
}

#[test]
#[ignore = "requires original assets and the real Scenario2 Win profile"]
fn legal_scenario_twelve_wealth_win_and_profile_reload() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
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
        let exe = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts = exe.parent().unwrap().join("scripts");
        let names = character_names(&image, &exe.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        let profile = PathBuf::from(fs::read_to_string("work/economy/scenario3-natural-win-profile-path.txt").unwrap());
        let before_profile = fs::read(&profile).unwrap();
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
        let level = &catalog.levels[catalog.find(200012).unwrap()];
        assert!(level.unlocked()); assert_eq!(level.attempts(), 0);
        assert!(!catalog.levels[catalog.find(200001).unwrap()].unlocked());
        for id in [200003, 200004, 200005, 200002] {
            let previous = &catalog.levels[catalog.find(id).unwrap()];
            assert_eq!(previous.attempts(), 1); assert!(previous.completed()[0]);
        }
        let mut session = Session::initialize_campaign(Inputs {
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO12", difficulty: 0,
            selected_character: 4, profile_name: b"Scenario3 Legal Sales", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }, catalog, profile.clone()).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        assert_eq!((session.world.objective.kind, session.world.objective.target), (5, 20000.));
        assert_eq!(session.world.ledgers.players[0].cash, 8000.);
        assert_eq!(session.world.ledgers.residents.active_count, 560);
        assert_eq!(session.roster.manager.players.len(), 3);
        let hotel = session.world.catalog.info.row(43).unwrap();
        assert_eq!(word(hotel, 0xc), 0, "authored Hotel must retain its retail class");
        assert_eq!(hotel[0x4c], 1);
        assert_eq!(session.world.catalog.products[43][0][0], 31);
        assert_eq!(word(hotel, 0x70), 0x80000000);
        assert_eq!(wealth(&session, &image).to_f32(), 8000.);
        let mut host = crate::live_host::Host::new([0., 20., 0.]);
        host.set_running(true);
        let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
        host.auction_renderer = Some(sender);
        let presenter = std::thread::spawn(move || {
            let exe = fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
            let dir = fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
            let bin = fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view = crate::auction_bridge::View::default(); let mut counts = [0; 3];
            while let Ok(request) = receiver.recv() {
                let index = match &request.operation {
                    crate::auction_bridge::Operation::Prepare { .. } => 0,
                    crate::auction_bridge::Operation::Activate => 1,
                    crate::auction_bridge::Operation::Restore => 2,
                };
                let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                if result.is_ok() { counts[index] += 1; }
                request.reply.send(result).unwrap();
            }
            (counts, view.scene.is_some())
        });
        let plan = [(43, Some((6, 5))), (11, Some((9, 4))), (13, None), (20, None), (21, None)];
        let mut submitted = 0; let mut checks: Vec<Terms> = Vec::new();
        let mut controlled = std::collections::BTreeSet::new();
        let mut owned_shopper = false; let mut previous_hour = u32::MAX;
        let mut lease_submitted = false; let mut lease_seen = false; let mut lease_due = None;
        let mut offer_due = None; let mut leases_owned = std::collections::BTreeSet::new();
        let mut result_dialog = None;
        let day_limit = std::env::var("OPENMT2001_SCENARIO12_MAX_DAYS").ok()
            .map(|v| v.parse::<u32>().unwrap()).unwrap_or(4);
        for frame in 0..110000 {
            if frame % 625 == 0 && submitted < plan.len() {
                let (business, shape) = plan[submitted];
                let built = if business == 43 { submit_hotel(&mut session, &assets) }
                    else { submit_business_shape(&mut session, &assets, business, &[], shape) };
                if built.is_some() {
                    submitted += 1;
                }
            }
            host.frame(&mut session, &assets, 40);
            assert!(host.failure.is_none(), "legal Scenario12 stopped: {:?}", host.failure);
            if !owned_shopper { owned_shopper = validate_natural_owned_shoppers(&session, &image); }
            let status = host.status(&session);
            for business in 0..600 {
                let row = &session.world.ledgers.businesses[business];
                if row[0x21c] != 0 && row[10] == 1 && word(row, 0) == 0 && !controlled.contains(&business) {
                    let target = crate::business_edit::Target { business,
                        building: usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap())) };
                    // Original default price minus one tick, Stock-1 and Apply.
                    // No unverified Hotel pricing or class/stock field injection.
                    apply_terms(&mut session, &assets, target, &mut checks);
                    controlled.insert(business);
                }
            }
            for check in &mut checks {
                if check.checked { continue; }
                let tick = session.world.clock.timing[0];
                if tick < check.due { assert_eq!(current(&session, check.target), check.before); }
                else { assert_eq!(tick, check.due); assert_eq!(current(&session, check.target), check.after); check.checked = true; }
            }
            let lease_target = if leases_owned.contains(&0u32) { 16 } else { 0 };
            // Auction closure queues ownership before releasing the controller.
            // Observe that retained command before deciding an offer was denied.
            for (_, _, due, _, command) in session.live_commands.snapshot().unwrap() {
                if let openmt2001::live_commands::Command::Auction { descriptor } = command
                    && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                    && u32::from(u16::from_le_bytes(descriptor[24..26].try_into().unwrap())) == lease_target
                {
                    if lease_due.is_none() { assert_eq!(due, session.world.clock.timing[0] + 25); lease_due = Some(due); }
                    assert_ne!(session.world.categories[lease_target as usize][5], 1);
                }
            }
            // Original465330 rejects even an invisible pending auction. A UI
            // submission proves queue admission only; its actual consumer may
            // encounter an intervening AI auction and clear this category lock.
            if lease_submitted && lease_due.is_none()
                && offer_due.is_some_and(|due| session.world.clock.timing[0] > due)
                && (!lease_seen || session.world.auction.controller.active_count == 0)
                && session.world.categories[lease_target as usize][0x6c] == 0
            {
                println!("Scenario12 block{lease_target} unlocked without queued human ownership; legal offer may retry");
                lease_submitted = false; lease_seen = false; offer_due = None;
            }
            if !leases_owned.contains(&lease_target) && !lease_submitted
                && submitted == plan.len() && session.world.clock.hour() >= 20
                && session.world.auction.controller.active_count == 0
                && !status.auction.as_ref().is_some_and(|a| a.visible)
            {
                let panel = host.inspect_property_offer(&mut session, &assets, lease_target,
                    openmt2001::property_offer::Panel::default(), true).unwrap().panel;
                if panel.action_visible != 0 && session.world.ledgers.players[0].cash >= panel.amount + 1500. {
                    let result = host.property_offer(&mut session, &assets, lease_target, panel).unwrap();
                    assert!(result.submitted); assert_eq!(session.world.categories[lease_target as usize][5], 0);
                    lease_submitted = true;
                    offer_due = Some(session.world.clock.timing[0] + 25);
                    println!("Scenario12 original block{lease_target} offer{} at tick{}", result.panel.amount, session.world.clock.timing[0]);
                }
            }
            lease_seen |= status.auction.as_ref().is_some_and(|a| a.visible)
                && word(&session.world.auction.controller.record, 4) == lease_target;
            if status.auction.is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                let category = word(&session.world.auction.controller.record, 4);
                let next = word(&session.world.auction.controller.record, 0x6c).saturating_add(50);
                let bid = category == lease_target && lease_submitted
                    && session.world.ledgers.players[0].cash >= next as f32 + 200.;
                if bid { println!("Scenario12 actual block{category} Bid{next} cash{}", session.world.ledgers.players[0].cash); }
                host.auction_button(&mut session, &assets, if bid { 0 } else { 3 }).unwrap();
            }
            if !leases_owned.contains(&lease_target) && session.world.categories[lease_target as usize][5] == 1 {
                assert!(lease_seen); assert_eq!(Some(session.world.clock.timing[0]), lease_due);
                leases_owned.insert(lease_target);
                println!("Scenario12 natural block{lease_target} ownership tick{}", session.world.clock.timing[0]);
                lease_submitted = false; lease_seen = false; lease_due = None; offer_due = None;
            }
            for prompt in status.prompts {
                if let Some(dialog) = prompt.dialog {
                    if dialog.callback == 0x48b490 { result_dialog = Some(dialog); break; }
                    let button = u32::from(dialog.button_count == 2);
                    host.human_dialog_reply(&mut session, &assets,
                        crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context, button }).unwrap();
                } else if prompt.dismissal_id != 0 {
                    session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32, &assets, false).unwrap();
                }
            }
            if previous_hour != session.world.clock.hour() {
                previous_hour = session.world.clock.hour();
                crate::session_validation::assert_live_links(&session);
                let value = wealth(&session, &image);
                let hotel_sales = hotel_sales(&session);
                if previous_hour == 6 && !leases_owned.is_empty() {
                    let chains = openmt2001::building_pool::category_chains(
                        &session.world.categories, &session.world.buildings, 0x591318).unwrap();
                    let mut expected = 0f32;
                    for &category in &leases_owned {
                        for &building in &chains[category as usize] {
                            let Some(business) = session.world.ledgers.businesses.get(word(&session.world.buildings[building], 0x1c) as usize) else { continue; };
                            if business[0x21c] == 0 { continue; }
                            if business[10] == 1 { assert_eq!(f32::from_bits(word(business, 0x28)), 0.); }
                            else { expected += f32::from(u16::from_le_bytes(business[0x18..0x1a].try_into().unwrap())); }
                        }
                    }
                    assert_eq!(session.world.ledgers.players[0].ledger(0x22).unwrap(), expected);
                    println!("Scenario12 original natural landlord income{expected}, retained own-shop LandRent0");
                }
                println!("SCENARIO12 day{} hour{previous_hour} wealth{} cash{} builds{submitted} Hotel rooms{hotel_sales} leases{leases_owned:?}",
                    session.world.clock.calendar[4], value.to_f32(), session.world.ledgers.players[0].cash);
                if previous_hour == 23 {
                    export(&session, &image, &format!("SCENARIO12-DAY{}", session.world.clock.calendar[4]));
                    export_live_price_scores(&session);
                    if session.world.clock.calendar[4] >= day_limit { break; }
                }
            }
            if result_dialog.is_some() { break; }
        }
        export(&session, &image, "SCENARIO12");
        let dialog = result_dialog.expect("bounded legal wealth strategy did not reach an original result");
        assert_eq!(dialog.context, 0, "original Scenario12 won by an opponent");
        assert_eq!(session.world.objective.reached[0], 1,
            "Scenario12 survival result did not prove the wealth objective");
        assert!(wealth(&session, &image).compare(openmt2001::x87::Sum::from_f32(20000.).unwrap()).is_ge());
        assert!(owned_shopper); assert!(checks.iter().all(|c| c.checked));
        assert!(hotel_sales(&session) > 0, "legal Hotel must have natural room purchases");
        assert_eq!((session.world.objective.kind, session.world.objective.target), (5, 20000.));
        assert_eq!(fs::read(&profile).unwrap(), before_profile);
        host.human_dialog_reply(&mut session, &assets,
            crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context, button: 0 }).unwrap();
        finish_original_win(&mut session, &assets, &mut host, &profile, &before_profile);
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
        for id in [200003, 200004, 200005, 200002, 200012] {
            let level = &catalog.levels[catalog.find(id).unwrap()];
            assert_eq!(level.attempts(), 1); assert!(level.completed()[0]);
        }
        assert!(catalog.levels[catalog.find(200001).unwrap()].unlocked());
        drop(host); let (counts, active) = presenter.join().unwrap();
        assert!(!active); assert_eq!(counts[0], counts[1]); assert_eq!(counts[1], counts[2]);
        println!("Natural Scenario12 wealth Win once; Scenario1 unlocked, real auction scenes{counts:?}");
    }).unwrap().join().unwrap();
}
