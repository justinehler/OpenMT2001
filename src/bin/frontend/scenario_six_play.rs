//! Original Scenario6 mayor elections on the naturally unlocked campaign.
use super::scenario_one_play::build_ranked;
use super::scenario_twelve_play::{export, wealth};
use super::{
    CommandAssets, Inputs, Path, PathBuf, Policy, Session, character_names, check_measured_result,
    finish_original_result, fs, load_era_styles, result_sequence, validate_natural_owned_shoppers,
    word,
};
use std::cell::RefCell;

struct Capture {
    day: u32,
    bytes: Vec<u8>,
}
thread_local! { static SHARE: RefCell<Option<Capture>> = const { RefCell::new(None) }; }

fn build_supported_apartment(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    target_category: i32,
    min_cash_after: f32,
    submit: bool,
) -> Option<(i32, f32)> {
    let info = session.world.catalog.info.row(42).unwrap();
    let kind = word(info, 0xc);
    let height = word(info, 0x44) as i32;
    for category in [target_category] {
        if !matches!(
            crate::build_panel::Context::from_session(session, category as usize)
                .unwrap()
                .entry(),
            crate::build_panel::Entry::Open
        ) {
            continue;
        }
        for (width, depth) in [(4, 4), (4, 3), (3, 4)] {
            for x in 0..=7 - width {
                for y in 0..=7 - depth {
                    let proposal = crate::build_panel::Request {
                        category,
                        origin: [x, y],
                        size: [width, depth],
                        height,
                        packed_model: 0,
                        kind,
                        quality: 0,
                        displayed_quote: 0.,
                        business: 42,
                    };
                    let inspected = session.inspect_build(&proposal, assets).unwrap();
                    if !inspected.allowed()
                        || session.world.ledgers.players[0].cash - inspected.total < min_cash_after
                    {
                        continue;
                    }
                    if !submit {
                        return Some((category, inspected.total));
                    }
                    let form = crate::build_panel::Form {
                        category,
                        rectangle: [x, y, x + width - 1, y + depth - 1],
                        height,
                        quality: 0,
                        choices: [0; 3],
                        variants: vec![0],
                        kind,
                        business: 42,
                        displayed_quote: inspected.total,
                    };
                    let request = form
                        .request(
                            inspected.category_locked,
                            inspected.affordable,
                            inspected.preview.is_some(),
                        )
                        .unwrap()
                        .unwrap();
                    assert!(
                        session
                            .submit_build(&request, assets, |_| Err(
                                "apartment applied before native deadline".into()
                            ))
                            .unwrap()
                            .submitted
                    );
                    println!(
                        "legal human supported apartment: category{category} size{width}x{depth} height{height} quote{}",
                        inspected.total
                    );
                    return Some((category, inspected.total));
                }
            }
        }
    }
    None
}

/// Observe the exact imminent canonical publication without touching the game.
fn observe_before(
    manager: &openmt2001::player_manager::Manager,
    city: &openmt2001::level_startup::City,
    current: u32,
    day: u32,
    prior: [f32; 6],
    network: [bool; 2],
) {
    if std::env::var_os("OPENMT2001_SCENARIO6_SHARE_ORACLE").is_none() {
        return;
    }
    if city.objective.kind != 1 {
        return;
    }
    let mut bytes = b"MTSHLIV1".to_vec();
    let current = manager
        .players
        .iter()
        .position(|p| p.reference == current)
        .unwrap();
    for value in [
        manager.players.len() as u32,
        day,
        current as u32,
        u32::from(network[0]),
        u32::from(network[1]),
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in prior {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&city.objective.to_bytes());
    for (player, account) in manager.players.iter().zip(&city.ledgers.players) {
        let mut raw = player.bytes[..548].to_vec();
        account.write_runtime(&mut raw).unwrap();
        bytes.extend_from_slice(&player.reference.to_le_bytes());
        bytes.extend_from_slice(&raw);
    }
    SHARE.with(|p| assert!(p.replace(Some(Capture { day, bytes })).is_none()));
}

fn observe_after(
    manager: &openmt2001::player_manager::Manager,
    city: &openmt2001::level_startup::City,
) {
    let Some(prefix) = std::env::var_os("OPENMT2001_SCENARIO6_SHARE_ORACLE") else {
        return;
    };
    let Some(mut capture) = SHARE.with(|p| p.replace(None)) else {
        return;
    };
    capture.bytes.extend_from_slice(&city.objective.to_bytes());
    for player in &manager.players {
        capture
            .bytes
            .extend_from_slice(&player.reference.to_le_bytes());
        capture.bytes.extend_from_slice(&player.bytes[..548]);
    }
    fs::write(
        format!("{}-DAY{}.bin", PathBuf::from(prefix).display(), capture.day),
        capture.bytes,
    )
    .unwrap();
    println!(
        "Scenario6 exact18:00 native share input/output day{} aux{:?} markers{:?}",
        capture.day,
        manager
            .players
            .iter()
            .map(|p| word(&p.bytes, 0x14c))
            .collect::<Vec<_>>(),
        city.objective.reached
    );
}

#[test]
#[ignore = "requires original assets and the real Scenario1 Win profile; measured Scenario6 result"]
fn legal_scenario_six_mayor_win_and_profile_reload() {
    assert!(std::env::var_os("OPENMT2001_SCENARIO6_SHARE_ORACLE").is_some());
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        openmt2001::player_share::observer::set(Some(openmt2001::player_share::observer::Hooks {
            before: observe_before,
            after: observe_after,
        }));
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
        let profile = crate::campaign_store::checkpoint_profile(&image, &scripts, 200006);
        let before_profile = fs::read(&profile).unwrap();
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
        let level = &catalog.levels[catalog.find(200006).unwrap()];
        assert!(level.unlocked()); assert_eq!(level.attempts(), 0);
        assert!(!catalog.levels[catalog.find(200011).unwrap()].unlocked());
        for id in [200003, 200004, 200005, 200002, 200012, 200001] {
            let previous = &catalog.levels[catalog.find(id).unwrap()];
            assert_eq!(previous.attempts(), 1); assert!(previous.completed()[0]);
        }
        let mut session = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO6", difficulty: 0,
            selected_character: 4, profile_name: b"Scenario3 Legal Sales", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }, catalog, profile.clone()).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        // This original seed exposes category0 as an open legal apartment
        // site. Price it before the auction so every bid still funds the
        // actual 4x4x9 construction quote and a positive cash reserve.
        let (apartment_category, apartment_quote) =
            build_supported_apartment(&mut session, &assets, 0, 0., false).unwrap();
        assert_eq!(apartment_category, 0);
        assert_eq!((session.world.objective.kind, session.world.objective.target), (1, 3.));
        assert_eq!(session.world.ledgers.players[0].cash, 8000.);
        assert_eq!(session.roster.manager.players.len(), 4);
        assert_eq!(wealth(&session, &image).to_f32(), 8000.);
        let mut host = crate::live_host::Host::new([0., 20., 0.]);
        host.set_running(true);
        let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
        host.auction_renderer = Some(sender);
        let presenter = std::thread::spawn(move || {
            let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
            let dir = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
            let bin = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
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
        // Original 48a580 includes the persistent residential counter in the
        // election share. Retail-only construction left that counter at zero.
        let plan = [42, 20, 11];
        let mut built = Vec::new(); let mut owned_shopper = false;
        let mut rent_terms = Vec::new();
        let mut apartment_controlled = false;
        let mut lease_requested_at = None;
        let mut lease_seen = false;
        let mut lease_deadline = None;
        let mut lease_acquired = false;
        let mut lease_attempts = 0;
        let mut rights_due = None;
        let mut rights_ready = false;
        let mut previous_hour = u32::MAX; let mut result_dialog = None;
        let start_hour = session.world.clock.calendar[4] * 24 + session.world.clock.hour();
        for frame in 0..360000 {
            // The block's measured natural land rent is about $1884 per day.
            // Lease this original open lot before construction. Every bid
            // reserves the inspected apartment quote; build only after the
            // auction's native delayed ownership transfer.
            if frame % 625 == 0 && rights_ready && built.len() < plan.len()
                && let Some(category) = if built.is_empty() {
                    build_supported_apartment(&mut session, &assets, apartment_category, 200., true)
                        .map(|(category, quote)| {
                            assert_eq!(category, apartment_category);
                            assert_eq!(quote.to_bits(), apartment_quote.to_bits());
                            category
                        })
                } else {
                    build_ranked(&mut session, &assets, plan[built.len()], &built)
                } {
                built.push(category);
            }
            if !owned_shopper && frame % 125 == 0 {
                owned_shopper = validate_natural_owned_shoppers(&session, &image);
            }
            host.frame(&mut session, &assets, 40);
            assert!(host.failure.is_none(), "actual Scenario6 Host boundary: {:?}", host.failure);
            if !apartment_controlled
                && let Some((business, row)) = session.world.ledgers.businesses.iter().enumerate()
                    .find(|(_, row)| row[0x21c] != 0 && row[10] == 1 && word(*row, 4) == 42)
            {
                let target = crate::business_edit::Target {
                    business,
                    building: usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap())),
                };
                super::apply_terms(&mut session, &assets, target, &mut rent_terms);
                apartment_controlled = true;
            }
            for check in &mut rent_terms {
                if !check.checked && session.world.clock.timing[0] >= check.due {
                    assert_eq!(session.world.clock.timing[0], check.due);
                    assert_eq!(super::current(&session, check.target), check.after);
                    check.checked = true;
                }
            }
            let status = host.status(&session);
            let lease_target = apartment_category as usize;
            // The native daily account charges land rent to the owner of the
            // block. Acquire the apartment's block through the ordinary
            // property offer, GPU auction, and delayed transfer sequence.
            // Original465330 can reject an invisible pending auction after a
            // legal UI submission. Retry only when that producer has passed
            // its 25-tick window, the block is unlocked, and no transfer was
            // queued. Scenario12 exercises the same admission edge.
            if lease_requested_at.is_some_and(|tick| session.world.clock.timing[0] > tick + 25)
                && lease_deadline.is_none() && !lease_acquired && !lease_seen
                && session.world.categories[lease_target][0x6c] == 0
                && session.world.auction.controller.active_count == 0
            {
                println!("Scenario6 category{lease_target} offer{} had no ownership command; retry after native queue admission",
                    lease_attempts);
                lease_requested_at = None;
                lease_seen = false;
            }
            if lease_requested_at.is_none() && !lease_acquired
                && session.world.clock.timing[0] >= 1
                && session.world.categories[lease_target][5] == 0
                && session.world.auction.controller.active_count == 0
                && !status.auction.as_ref().is_some_and(|auction| auction.visible)
            {
                let inspected = host.inspect_property_offer(&mut session, &assets, lease_target as u32,
                    openmt2001::property_offer::Panel::default(), true).unwrap();
                if inspected.panel.action_visible != 0
                && session.world.ledgers.players[0].cash >= inspected.panel.amount + apartment_quote + 200.
                {
                    let tick = session.world.clock.timing[0];
                    let offer = host.property_offer(&mut session, &assets, lease_target as u32, inspected.panel).unwrap();
                    assert!(offer.submitted);
                    assert_eq!(session.world.categories[lease_target][5], 0,
                        "property offer changed ownership before its auction");
                    lease_requested_at = Some(tick);
                    lease_attempts += 1;
                    println!("Scenario6 legal apartment-block category{lease_target} property offer attempt{lease_attempts} opening{} at tick{tick}",offer.panel.amount);
                }
            }
            for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                if let openmt2001::live_commands::Command::Auction { descriptor } = command
                    && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                    && usize::from(u16::from_le_bytes(
                        descriptor[24..26].try_into().unwrap())) == lease_target
                {
                    assert!(matches!(domain,openmt2001::scheduler::ClockDomain::Total));
                    assert_eq!(outer,inner);
                    assert!(!direct);
                    if lease_deadline.is_none() {
                        assert_eq!(inner,session.world.clock.timing[0]+25);
                        lease_deadline=Some(inner);
                        println!("Scenario6 category{} human ownership command queued at tick{} due{inner}",
                            lease_target,session.world.clock.timing[0]);
                    }
                    assert_eq!(session.world.categories[lease_target][5],0,
                        "apartment block transferred before its original queue deadline");
                }
            }
            let target_auction_visible = status.auction.as_ref().is_some_and(|auction| auction.visible)
                && word(&session.world.auction.controller.record,4) == lease_target as u32;
            if target_auction_visible && !lease_seen {
                println!("Scenario6 apartment-block category{} visible auction at tick{} price{}",
                    lease_target,session.world.clock.timing[0],
                    word(&session.world.auction.controller.record,0x6c));
            }
            lease_seen |= target_auction_visible;
            if !lease_acquired && session.world.categories[lease_target][5] == 1 {
                assert!(lease_seen,"apartment block changed owner without a visible auction");
                assert!(session.world.clock.timing[0] > lease_requested_at.unwrap());
                assert_eq!(Some(session.world.clock.timing[0]),lease_deadline,
                    "apartment block did not transfer at the native queued deadline");
                lease_acquired=true;
                println!("Scenario6 apartment block category{} lease acquired tick{} cash{}",
                    lease_target,session.world.clock.timing[0],session.world.ledgers.players[0].cash);
            }
            if lease_acquired && rights_due.is_none() {
                let before = host.property_rights(&mut session, &assets,
                    lease_target as u32, 0, false).unwrap();
                assert_eq!(before.message,Some(1202));
                assert!(before.activate_visible);
                let due = session.world.clock.timing[0] + 25;
                let pending = host.property_rights(&mut session, &assets,
                    lease_target as u32, 0, true).unwrap();
                assert_eq!(pending.message,Some(1406));
                assert_eq!(session.world.categories[lease_target][0x6d],1);
                rights_due=Some(due);
                println!("Scenario6 legal Activate Building Rights category{lease_target} tick{} due{due}",
                    session.world.clock.timing[0]);
            }
            if let Some(due) = rights_due
                && !rights_ready && session.world.clock.timing[0] >= due
            {
                assert_eq!(session.world.clock.timing[0],due);
                assert_eq!((session.world.categories[lease_target][0x6d],
                    word(&session.world.categories[lease_target],0x78)),(0,1));
                rights_ready=true;
                println!("Scenario6 category{lease_target} original building rights active at tick{due}");
            }
            if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                let category = word(&session.world.auction.controller.record,4);
                let next = word(&session.world.auction.controller.record,0x6c).saturating_add(50);
                let cash = session.world.ledgers.players[0].cash;
                let bid = category == lease_target as u32
                    && lease_requested_at.is_some()
                    && cash >= next as f32 + apartment_quote + 200.;
                host.auction_button(&mut session, &assets, if bid {0} else {3}).unwrap();
                println!("Scenario6 auction category{category} next{next} cash{} button{} at tick{}",
                    session.world.ledgers.players[0].cash,if bid {0} else {3},session.world.clock.timing[0]);
            }
            for prompt in status.prompts {
                if let Some(dialog) = prompt.dialog {
                    if dialog.callback == 0x48b490 { result_dialog = Some(dialog); break; }
                    host.human_dialog_reply(&mut session, &assets,
                        crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context,
                            button: u32::from(dialog.button_count == 2) }).unwrap();
                } else if prompt.dismissal_id != 0 {
                    session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                        &assets, false).unwrap();
                }
            }
            if previous_hour != session.world.clock.hour() {
                previous_hour = session.world.clock.hour();
                crate::session_validation::assert_live_links(&session);
                println!("SCENARIO6 day{} hour{previous_hour} wealth{} cash{} buildings{} power{} housing{} aux{:?} reached{:?}",
                    session.world.clock.calendar[4], wealth(&session,&image).to_f32(),
                    session.world.ledgers.players[0].cash, built.len(),
                    word(&session.roster.manager.players[0].bytes,0x1ac),
                    word(&session.roster.manager.players[0].bytes,0x1b0),
                    session.roster.manager.players.iter().map(|p| word(&p.bytes,0x14c)).collect::<Vec<_>>(),
                    session.world.objective.reached);
                if previous_hour == 6 {
                    let account = &session.world.ledgers.players[0];
                    println!("Scenario6 daily account land-rent{} housing-rent{} landlord-income{} category-owner{}",
                        account.ledger(14).unwrap(),account.ledger(33).unwrap(),
                        account.ledger(34).unwrap(),session.world.categories[lease_target][5]);
                }
                if matches!(previous_hour,18|23) {
                    export(&session,&image,&format!("SCENARIO6-DAY{}-H{previous_hour}",session.world.clock.calendar[4]));
                }
            }
            if result_dialog.is_some() { break; }
            if session.world.clock.calendar[4]*24+session.world.clock.hour() >= start_hour+144 { break; }
        }
        let result_tick=session.world.clock.timing[0];
        export(&session,&image,"SCENARIO6");
        let dialog = result_dialog.expect("bounded legal mayor strategy did not reach an original result");
        let winner=dialog.context;let won=winner==0;
        println!("Scenario6 original result winner slot{winner} tick{result_tick} mayor-score{:?}",
            session.roster.manager.players.iter().map(|p|i32::from_le_bytes(p.bytes[0x1b4..0x1b8].try_into().unwrap())).collect::<Vec<_>>());
        assert_eq!(word(&session.roster.manager.players[winner as usize].bytes,0x14c),3,"survival alone does not prove three elections");
        assert_eq!(session.world.objective.reached[winner as usize],1);
        assert_eq!((session.world.objective.kind,session.world.objective.target),(1,3.));
        assert_eq!(built.len(),plan.len()); assert!(owned_shopper);
        assert!(apartment_controlled && rent_terms.iter().all(|check|check.checked));
        assert!(lease_acquired,"mayor strategy did not acquire the apartment block");
        assert_eq!(fs::read(&profile).unwrap(),before_profile);
        host.human_dialog_reply(&mut session,&assets,
            crate::human_dialog::Reply{callback:dialog.callback,context:dialog.context,button:0}).unwrap();
        finish_original_result(&mut session,&assets,&mut host,&profile,&before_profile,result_sequence(winner));
        let mut catalog=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog,&profile).unwrap();
        for id in [200003,200004,200005,200002,200012,200001] {
            let level=&catalog.levels[catalog.find(id).unwrap()];
            assert_eq!(level.attempts(),1); assert!(level.completed()[0]);
        }
        let mayor=&catalog.levels[catalog.find(200006).unwrap()];
        assert_eq!(mayor.attempts(),1); assert_eq!(mayor.completed()[0],won);
        assert_eq!(catalog.levels[catalog.find(200011).unwrap()].unlocked(),won);
        drop(host); let(counts,active)=presenter.join().unwrap();
        assert!(!active); assert_eq!(counts[0],counts[1]); assert_eq!(counts[1],counts[2]);
        println!("Natural Scenario6 three-election mayor {:?} once; 11 unlocked{won}, auctions{counts:?}",result_sequence(winner));
        check_measured_result("SCENARIO6",0,winner,result_tick);
    }).unwrap().join().unwrap();
}
