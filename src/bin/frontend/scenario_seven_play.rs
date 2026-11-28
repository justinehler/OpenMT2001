//! Original Scenario7 weakest-link admission through legal city play.
// Keep successful measurements in nightly logs despite libtest output capture.
use std::io::Write as _;

use super::*;

fn apartment(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    submit: bool,
) -> Option<f32> {
    let info = session.world.catalog.info.row(42).unwrap();
    let kind = word(info, 0xc);
    let height = word(info, 0x44) as i32;
    for (width, depth) in [(4, 4), (4, 3), (3, 4)] {
        for x in 0..=7 - width {
            for y in 0..=7 - depth {
                let proposal = crate::build_panel::Request {
                    category: 0,
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
                if !inspected.allowed() {
                    continue;
                }
                if submit {
                    let form = crate::build_panel::Form {
                        category: 0,
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
                                "apartment applied before the native deadline".into()
                            ))
                            .unwrap()
                            .submitted
                    );
                }
                println!(
                    "Scenario7 legal apartment category0 size{width}x{depth} height{height} quote{} submit{submit}",
                    inspected.total
                );
                return Some(inspected.total);
            }
        }
    }
    None
}

fn small_apartment(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    origin: [i32; 2],
    submit: bool,
) -> Option<f32> {
    let info = session.world.catalog.info.row(42).unwrap();
    let kind = word(info, 0xc);
    let height = word(info, 0x44) as i32;
    let proposal = crate::build_panel::Request {
        category: 0,
        origin,
        size: [3, 3],
        height,
        packed_model: 0,
        kind,
        quality: 0,
        displayed_quote: 0.,
        business: 42,
    };
    let inspected = session.inspect_build(&proposal, assets).unwrap();
    if !inspected.allowed() {
        return None;
    }
    if submit {
        let form = crate::build_panel::Form {
            category: 0,
            rectangle: [origin[0], origin[1], origin[0] + 2, origin[1] + 2],
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
                    "second apartment applied before the native deadline".into()
                ))
                .unwrap()
                .submitted
        );
    }
    Some(inspected.total)
}

#[test]
#[ignore = "frontier measurement; requires original v1.4 Scenario7 scripts, map, and assets"]
fn legal_scenario_seven_first_weakest_link_frontier() {
    run_scenario_seven_weakest_link(true, false, false);
}

/// Continue the same original legal apartment strategy until the natural
/// survivor result, a human elimination, or the bounded tenth-day frontier.
#[test]
#[ignore = "frontier measurement; requires original v1.4 Scenario7 scripts, map, and assets"]
fn legal_scenario_seven_survivor_result_frontier() {
    run_scenario_seven_weakest_link(false, false, false);
}

#[test]
#[ignore = "frontier measurement; requires original v1.4 Scenario7 scripts, map, and assets"]
fn legal_scenario_seven_virginia_and_apartment_frontier() {
    run_scenario_seven_weakest_link(false, true, false);
}

#[test]
#[ignore = "frontier measurement; requires original v1.4 Scenario7 scripts, map, and assets"]
fn legal_scenario_seven_virginia_three_apartments_frontier() {
    run_scenario_seven_weakest_link(false, true, true);
}

#[test]
#[ignore = "requires the original v1.4 catalog"]
fn scenario_seven_requires_natural_scenario_eleven_campaign_unlock() {
    let executable = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let scripts = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .join("scripts");
    let profile = crate::campaign_store::checkpoint_profile(&image, &scripts, 200011);
    let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
    crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
    for id in [200003, 200004, 200005, 200002, 200012, 200001, 200006] {
        let level = &catalog.levels[catalog.find(id).unwrap()];
        assert_eq!(
            level.attempts(),
            1,
            "predecessor {id} was not completed exactly once"
        );
        assert!(
            level.completed()[0],
            "predecessor {id} lacks its real Easy Win"
        );
    }
    let eleven = &catalog.levels[catalog.find(200011).unwrap()];
    assert!(eleven.unlocked());
    assert_eq!(eleven.attempts(), 0);
    for id in [200007, 200008] {
        let level = &catalog.levels[catalog.find(id).unwrap()];
        assert!(
            !level.unlocked(),
            "scenario {id} was admitted without its predecessor Win"
        );
        assert_eq!(level.attempts(), 0);
    }
}

fn run_scenario_seven_weakest_link(stop_after_first: bool, virginia: bool, third_apartment: bool) {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(move || {
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
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO7", difficulty: 0,
            selected_character: 4, profile_name: b"Scenario7 Weakest Link", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        assert_eq!((session.world.objective.kind, session.world.objective.target), (2, 2.));
        assert_eq!(session.roster.manager.players.len(), 5);
        assert_eq!(session.world.ledgers.players[0].cash, 8000.);
        assert_eq!(session.world.categories[0][5], 0);
        let quote = apartment(&mut session, &assets, false)
            .expect("original goodspread category0 has no legal apartment footprint");

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

        let mut offer_tick = None;
        let mut offer_attempts = 0;
        let mut auction_seen = false;
        let mut transfer_due = None;
        let mut owned = false;
        let mut virginia_offer_tick: Option<u32> = None;
        let mut virginia_auction_seen = false;
        let mut virginia_transfer_due: Option<u32> = None;
        let mut virginia_owned = false;
        let mut rights_due = None;
        let mut built = false;
        let mut apartment_target: Option<crate::business_edit::Target> = None;
        let mut managed_apartments = std::collections::BTreeSet::new();
        let mut second_apartment_submitted = false;
        let mut second_apartment_quote = None;
        let mut third_apartment_attempted = false;
        let mut third_apartment_submitted = false;
        let mut rent_terms = Vec::new();
        let mut day_one_notice = 0;
        let mut day_one_active = None;
        let mut day_two_active = None;
        let mut day_two_housing = None;
        let mut removed = std::collections::BTreeSet::new();
        let mut survivor_notice_due = None;
        let mut terminal = None;
        let mut previous_clock = (session.world.clock.calendar[4], session.world.clock.timing[0]);
        let mut previous_hour = session.world.clock.hour();
        let mut previous_category_owner = session.world.categories[0][5];
        let mut human_removal_notice_seen = false;
        let mut second_apartment_quotes_seen = false;
        let frame_limit = if stop_after_first { 100_000 } else { 360_000 };
        for frame in 0..frame_limit {
            let before_share = if !stop_after_first
                && session.world.clock.calendar[4] >= 3
                && (17..=19).contains(&session.world.clock.hour())
            {
                let players: Vec<_> = session.roster.manager.players.iter()
                    .zip(&session.world.ledgers.players)
                    .map(|(runtime, account)| {
                        let mut bytes = runtime.bytes.clone();
                        account.write_runtime(&mut bytes).unwrap();
                        Some(openmt2001::player_manager::Player {
                            reference: runtime.reference,
                            bytes,
                            auxiliaries: Vec::new(),
                        })
                    })
                    .collect();
                let rows: Vec<_> = players.iter().map(|player| {
                    let bytes = &player.as_ref().unwrap().bytes;
                    let retail = i32::from_le_bytes(bytes[0x1ac..0x1b0].try_into().unwrap());
                    let housing = i32::from_le_bytes(bytes[0x1b0..0x1b4].try_into().unwrap());
                    (bytes[0x10] != 0, retail, housing, retail.wrapping_add(housing).max(0))
                }).collect();
                let share = openmt2001::player_share::compute_with_prior(
                    &players, [0.; 6]).unwrap();
                let queued: Vec<_> = session.live_commands.snapshot().unwrap().into_iter()
                    .filter_map(|(_, _, due, _, command)| match command {
                        openmt2001::live_commands::Command::OutcomeNotice { player, reason } =>
                            Some((*player, *reason, due)),
                        _ => None,
                    }).collect();
                Some((session.world.clock.calendar[4], session.world.clock.hour(),
                    session.world.clock.timing[0], rows, share, queued))
            } else { None };
            host.frame(&mut session, &assets, 40);
            let clock = (session.world.clock.calendar[4], session.world.clock.timing[0]);
            assert!(clock.0 >= previous_clock.0 && clock.1 >= previous_clock.1,
                "Scenario7 clock moved backwards");
            previous_clock = clock;
            assert!(host.failure.is_none(), "Scenario7 Host boundary: {:?}", host.failure);
            if !human_removal_notice_seen
                && let Some((_, _, due, _, _)) = session.live_commands.snapshot().unwrap()
                    .into_iter().find(|(_, _, _, _, command)| matches!(command,
                        openmt2001::live_commands::Command::OutcomeNotice { player: 0, reason: 1 }))
            {
                human_removal_notice_seen = true;
                let after: Vec<_> = session.roster.manager.players.iter()
                    .zip(&session.world.ledgers.players)
                    .map(|(player, account)| (player.bytes[0x10] != 0,
                        account.retail_counters[0], account.housing_counter)).collect();
                let ai_categories: Vec<_> = session.world.categories.iter().enumerate()
                    .filter_map(|(category, row)| (row[5] == 2).then_some(category)).collect();
                let mut selected_categories = [0u32; 52];
                for resident in session.world.ledgers.residents.nodes.iter()
                    .take(session.world.ledgers.residents.active_count as usize)
                {
                    let category = usize::from(resident.as_bytes()[0x44]);
                    if resident.active_byte() != 0 && category < 52 {
                        selected_categories[category] += 1;
                    }
                }
                let mut selected: Vec<_> = selected_categories.iter().enumerate()
                    .filter(|(_, count)| **count != 0)
                    .map(|(category, count)| (category, *count, session.world.categories[category][5]))
                    .collect();
                selected.sort_by_key(|(_, count, _)| std::cmp::Reverse(*count));
                println!("Scenario7 first human reason1 notice due{due} after-tick{} day{} hour{} pre-frame{before_share:?} after-counters{after:?} mode{}",
                    session.world.clock.timing[0], session.world.clock.calendar[4],
                    session.world.clock.hour(), word(&session.roster.manager.bytes, 0xc));
                println!("Scenario7 AI1 owned categories{ai_categories:?} resident last-selected categories{selected:?}");
            }
            let category_owner = session.world.categories[0][5];
            if category_owner != previous_category_owner {
                let business_state = apartment_target.map(|target| {
                    let row = &session.world.ledgers.businesses[target.business];
                    (row[10], u16::from_le_bytes(row[0x10..0x12].try_into().unwrap()))
                });
                println!("Scenario7 category0 owner{}→{} tick{} day{} hour{} lease{} business{business_state:?} human-housing{} cash{}",
                    previous_category_owner, category_owner, session.world.clock.timing[0],
                    session.world.clock.calendar[4], session.world.clock.hour(),
                    word(&session.world.categories[0], 0x7c),
                    word(&session.roster.manager.players[0].bytes, 0x1b0),
                    session.world.ledgers.players[0].cash);
                previous_category_owner = category_owner;
            }
            let new_apartments: Vec<_> = session.world.ledgers.businesses.iter().enumerate()
                .filter(|(business, row)| row[0x21c] != 0 && row[10] == 1
                    && word(*row, 4) == 42 && !managed_apartments.contains(business))
                .map(|(business, row)| (business,
                    usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap())),
                    u16::from_le_bytes(row[0xe..0x10].try_into().unwrap()),
                    u16::from_le_bytes(row[0x10..0x12].try_into().unwrap()),
                    word(row, 0x218), f32::from_bits(word(row, 0x1dc))))
                .collect();
            for (business, building, capacity, occupied, stock, rent) in new_apartments {
                let target = crate::business_edit::Target {
                    business, building,
                };
                println!("Scenario7 apartment business{business} capacity{capacity} occupied{occupied} stock{stock} rent{rent}");
                super::apply_terms(&mut session, &assets, target, &mut rent_terms);
                apartment_target.get_or_insert(target);
                managed_apartments.insert(business);
            }
            for check in &mut rent_terms {
                if !check.checked && session.world.clock.timing[0] >= check.due {
                    assert_eq!(session.world.clock.timing[0], check.due);
                    assert_eq!(super::current(&session, check.target), check.after);
                    check.checked = true;
                    println!("Scenario7 apartment legal rent terms applied at tick{}",
                        session.world.clock.timing[0]);
                }
            }
            if !stop_after_first && built && !second_apartment_quotes_seen
                && session.world.clock.calendar[4] >= 3
            {
                second_apartment_quotes_seen = true;
                let info = session.world.catalog.info.row(42).unwrap();
                let kind = word(info, 0xc);
                let height = word(info, 0x44) as i32;
                let mut quotes = Vec::new();
                for quality in 0..=2 {
                    for (width, depth) in [(4, 3), (3, 4), (3, 3)] {
                        for x in 0..=7 - width {
                            for y in 0..=7 - depth {
                                let proposal = crate::build_panel::Request {
                                    category: 0, origin: [x, y], size: [width, depth],
                                    height, packed_model: 0, kind, quality,
                                    displayed_quote: 0., business: 42,
                                };
                                if let Ok(inspected) = session.inspect_build(&proposal, &assets)
                                    && inspected.placement_allowed && inspected.preview.is_some()
                                {
                                    quotes.push((quality, width, depth, x, y,
                                        inspected.total, inspected.affordable));
                                }
                            }
                        }
                    }
                }
                quotes.sort_by(|a, b| a.5.total_cmp(&b.5));
                println!("Scenario7 remaining legal apartment quotes day{} hour{} cash{} {quotes:?}",
                    session.world.clock.calendar[4], session.world.clock.hour(),
                    session.world.ledgers.players[0].cash);
            }
            if virginia && built && second_apartment_quote.is_none()
                && session.world.clock.calendar[4] >= 3
            {
                let second_quote = small_apartment(&mut session, &assets, [0, 4], false)
                    .expect("Scenario7 measured 3x3 apartment was no longer legal");
                second_apartment_quote = Some(second_quote);
                println!("Scenario7 second legal apartment quote{second_quote} reserved against Virginia bid at tick{} cash{}",
                    session.world.clock.timing[0], session.world.ledgers.players[0].cash);
            }
            if virginia && virginia_owned && !second_apartment_submitted
                && let Some(quote) = second_apartment_quote
                && session.world.ledgers.players[0].cash >= quote + 100.
            {
                let submitted_quote = small_apartment(&mut session, &assets, [0, 4], true)
                    .expect("Scenario7 reserved second apartment lost legal placement");
                assert_eq!(submitted_quote.to_bits(), quote.to_bits());
                second_apartment_submitted = true;
                println!("Scenario7 second legal apartment submitted quote{quote} after Virginia transfer tick{} cash{}",
                    session.world.clock.timing[0], session.world.ledgers.players[0].cash);
            }
            if third_apartment && second_apartment_submitted && !third_apartment_attempted
                && session.world.clock.calendar[4] == 4
                && session.world.clock.hour() >= 6
                && session.world.clock.hour() < 18
            {
                third_apartment_attempted = true;
                let cash = session.world.ledgers.players[0].cash;
                let quoted = small_apartment(&mut session, &assets, [4, 0], false);
                println!("Scenario7 third apartment category0 origin[4,0] quote{quoted:?} cash{cash} tick{}",
                    session.world.clock.timing[0]);
                if let Some(quote) = quoted
                    && cash >= quote + 100.
                {
                    let submitted = small_apartment(&mut session, &assets, [4, 0], true)
                        .expect("Scenario7 third apartment lost its legal quote before submission");
                    assert_eq!(submitted.to_bits(), quote.to_bits());
                    third_apartment_submitted = true;
                    println!("Scenario7 third legal apartment submitted quote{quote} tick{} cash{}",
                        session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                }
            }
            let status = host.status(&session);
            for prompt in status.prompts {
                if let Some(dialog) = prompt.dialog {
                    if matches!(dialog.body, 0x6c0 | 0x6c1) {
                        println!("Scenario7 original lease modal body{} day{} hour{} lease{} owner{}",
                            dialog.body, session.world.clock.calendar[4], session.world.clock.hour(),
                            word(&session.world.categories[0], 0x7c), session.world.categories[0][5]);
                    }
                    if dialog.callback == 0x48b490 { terminal = Some(dialog); break; }
                    if session.world.clock.calendar[4] == 1 && dialog.callback == 0 {
                        day_one_notice += 1;
                    }
                    host.human_dialog_reply(&mut session, &assets,
                        crate::human_dialog::Reply { callback: dialog.callback,
                            context: dialog.context, button: 0 }).unwrap();
                } else if prompt.dismissal_id != 0 {
                    session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32,
                        &assets, false).unwrap();
                }
            }
            if terminal.is_some() {
                println!("Scenario7 pre-terminal share state {before_share:?}");
                break;
            }
            // Native 465330 can decline an invisible offer after the UI
            // accepted it. Retry only once that original queue window passed.
            if offer_tick.is_some_and(|tick| session.world.clock.timing[0] > tick + 25)
                && !auction_seen && transfer_due.is_none() && !owned
                && session.world.categories[0][0x6c] == 0
                && session.world.auction.controller.active_count == 0
            {
                offer_tick = None;
                println!("Scenario7 category0 offer had no native auction admission; retrying");
            }
            if virginia && virginia_offer_tick.is_some_and(|tick|
                session.world.clock.timing[0] > tick + 25)
                && !virginia_auction_seen && virginia_transfer_due.is_none()
                && !virginia_owned && session.world.categories[26][5] == 0
                && session.world.categories[26][0x6c] == 0
                && session.world.auction.controller.active_count == 0
            {
                virginia_offer_tick = None;
                println!("Scenario7 category26 offer had no native auction admission; retrying");
            }
            for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                match command {
                    openmt2001::live_commands::Command::Auction { descriptor }
                        if word(descriptor, 4) == 0x16 && descriptor[26] == 1
                            && u16::from_le_bytes(descriptor[24..26].try_into().unwrap()) == 0 => {
                        assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                        assert_eq!(outer, inner);
                        assert!(!direct);
                        if transfer_due.is_none() { transfer_due = Some(inner); }
                    }
                    openmt2001::live_commands::Command::Auction { descriptor }
                        if word(descriptor, 4) == 0x16 && descriptor[26] == 1
                            && u16::from_le_bytes(descriptor[24..26].try_into().unwrap()) == 26 => {
                        assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                        assert_eq!(outer, inner);
                        assert!(!direct);
                        virginia_transfer_due.get_or_insert(inner);
                    }
                    openmt2001::live_commands::Command::OutcomeNotice { player, reason: 4 } => {
                        assert!((*player as usize) < session.roster.manager.players.len(),
                            "Scenario7 survivor notice is outside the roster");
                        assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                        assert_eq!(outer, inner);
                        assert!(!direct);
                        survivor_notice_due = Some(inner);
                    }
                    _ => {}
                }
            }
            if !owned && session.world.categories[0][5] == 1 {
                assert!(auction_seen, "Scenario7 category0 changed owner without visible auction");
                assert_eq!(Some(session.world.clock.timing[0]), transfer_due);
                owned = true;
                println!("Scenario7 acquired category0 tick{} cash{}", session.world.clock.timing[0],
                    session.world.ledgers.players[0].cash);
            }
            if !owned && session.world.categories[0][5] != 0 {
                println!("Scenario7 category0 auction lost to owner{}", session.world.categories[0][5]);
                break;
            }
            if virginia && !virginia_owned && session.world.categories[26][5] == 1 {
                assert!(virginia_auction_seen,
                    "Scenario7 Virginia ownership changed without a visible auction");
                assert_eq!(Some(session.world.clock.timing[0]), virginia_transfer_due,
                    "Scenario7 Virginia ownership did not use its queued native transfer");
                virginia_owned = true;
                println!("Scenario7 acquired Virginia category26 tick{} day{} hour{} cash{}",
                    session.world.clock.timing[0], session.world.clock.calendar[4],
                    session.world.clock.hour(), session.world.ledgers.players[0].cash);
            }
            if virginia && session.world.categories[26][5] > 1 {
                println!("Scenario7 Virginia auction lost to owner{} tick{} cash{}",
                    session.world.categories[26][5], session.world.clock.timing[0],
                    session.world.ledgers.players[0].cash);
                break;
            }
            if owned && rights_due.is_none() {
                let before = host.property_rights(&mut session, &assets, 0, 0, false).unwrap();
                assert_eq!(before.message, Some(1202));
                let pending = host.property_rights(&mut session, &assets, 0, 0, true).unwrap();
                assert_eq!(pending.message, Some(1406));
                rights_due = Some(session.world.clock.timing[0] + 25);
            }
            if let Some(due) = rights_due
                && !built && session.world.clock.timing[0] >= due
            {
                assert_eq!(session.world.clock.timing[0], due);
                assert_eq!(word(&session.world.categories[0], 0x78), 1);
                assert_eq!(apartment(&mut session, &assets, true).unwrap().to_bits(), quote.to_bits());
                built = true;
            }
            if status.auction.as_ref().is_some_and(|a| a.visible) {
                let category = word(&session.world.auction.controller.record, 4);
                if category == 0 { auction_seen = true; }
                if category == 26 { virginia_auction_seen = true; }
            }
            if status.auction.as_ref().is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                let category = word(&session.world.auction.controller.record, 4);
                let next = word(&session.world.auction.controller.record, 0x6c).saturating_add(50);
                let cash = session.world.ledgers.players[0].cash;
                let bid = (category == 0 && cash >= next as f32 + quote + 200.)
                    || (virginia && category == 26 && second_apartment_quote
                        .is_some_and(|reserve| cash >= next as f32 + reserve + 100.));
                let button = if bid { 0 } else { 3 };
                host.auction_button(&mut session, &assets, button).unwrap();
                println!("Scenario7 auction category{category} next{next} button{button} cash{}",
                    session.world.ledgers.players[0].cash);
            }
            if !owned && offer_tick.is_none() && !status.auction.as_ref().is_some_and(|a| a.visible)
                && session.world.clock.timing[0] >= 1
            {
                let inspected = host.inspect_property_offer(&mut session, &assets, 0,
                    openmt2001::property_offer::Panel::default(), true).unwrap();
                assert_ne!(inspected.panel.action_visible, 0);
                if session.world.ledgers.players[0].cash >= inspected.panel.amount + quote + 200. {
                    let offer = host.property_offer(&mut session, &assets, 0, inspected.panel).unwrap();
                    assert!(offer.submitted);
                    offer_tick = Some(session.world.clock.timing[0]);
                    offer_attempts += 1;
                    println!("Scenario7 offered category0 attempt{offer_attempts} opening{} tick{}", offer.panel.amount,
                        session.world.clock.timing[0]);
                }
            }
            if virginia && second_apartment_quote.is_some() && virginia_offer_tick.is_none()
                && !virginia_owned && session.world.categories[26][5] == 0
                && session.world.auction.controller.active_count == 0
                && !status.auction.as_ref().is_some_and(|a| a.visible)
            {
                let inspected = host.inspect_property_offer(&mut session, &assets, 26,
                    openmt2001::property_offer::Panel::default(), true).unwrap();
                println!("Scenario7 Virginia quote{} visible{} cash{} tick{}",
                    inspected.panel.amount, inspected.panel.action_visible,
                    session.world.ledgers.players[0].cash, session.world.clock.timing[0]);
                if inspected.panel.action_visible != 0
                    && second_apartment_quote.is_some_and(|reserve|
                        session.world.ledgers.players[0].cash >= inspected.panel.amount + reserve + 100.)
                {
                    let offer = host.property_offer(&mut session, &assets, 26,
                        inspected.panel).unwrap();
                    assert!(offer.submitted);
                    virginia_offer_tick = Some(session.world.clock.timing[0]);
                    println!("Scenario7 offered Virginia category26 opening{} tick{}",
                        offer.panel.amount, session.world.clock.timing[0]);
                }
            }
            if session.world.clock.hour() != previous_hour {
                let preceding_hour = previous_hour;
                previous_hour = session.world.clock.hour();
                let day = session.world.clock.calendar[4];
                if previous_hour == 6 || previous_hour == 19 {
                    writeln!(std::io::stdout(),"Scenario7 checkpoint day{day} hour{previous_hour} accounts(cash,sales,income){:?}",
                        session.world.ledgers.players.iter().map(|account|
                            (account.cash, account.retail_counters[1],
                            (22..37).map(|event| account.ledger(event).unwrap()).sum::<f32>()))
                            .collect::<Vec<_>>()).unwrap();
                }
                if virginia && day >= 4 && preceding_hour == 17 && previous_hour == 18 {
                    println!("Scenario7 Virginia day{day} 18h producer preframe {before_share:?} owner26{} apartments{} cash{}",
                        session.world.categories[26][5], managed_apartments.len(),
                        session.world.ledgers.players[0].cash);
                }
                if !stop_after_first && day >= 4 {
                    println!("Scenario7 day{day} hour{previous_hour} lease{} owner{} human-housing{} cash{}",
                        word(&session.world.categories[0], 0x7c), session.world.categories[0][5],
                        word(&session.roster.manager.players[0].bytes, 0x1b0),
                        session.world.ledgers.players[0].cash);
                }
                if previous_hour == 19 && day >= 1 {
                    let active: Vec<_> = session.roster.manager.players.iter()
                        .map(|player| player.bytes[0x10] != 0).collect();
                    // 4899e0 resets +0x1ac after the share pass; these are
                    // retained post-pass counters, not the sampled shares.
                    let post_share_sources: Vec<_> = session.roster.manager.players.iter()
                        .map(|player| word(&player.bytes, 0x1ac).wrapping_add(word(&player.bytes, 0x1b0)))
                        .collect();
                    let apartment_state = apartment_target.map(|target| {
                        let row = &session.world.ledgers.businesses[target.business];
                        (u16::from_le_bytes(row[0xe..0x10].try_into().unwrap()),
                            u16::from_le_bytes(row[0x10..0x12].try_into().unwrap()),
                            word(row, 0x218), f32::from_bits(word(row, 0x1dc)))
                    });
                    println!("Scenario7 day{day} 19h active{active:?} post-share{post_share_sources:?} apartment{apartment_state:?} housing{} mode{} notice{day_one_notice}",
                        word(&session.roster.manager.players[0].bytes, 0x1b0),
                        word(&session.roster.manager.bytes, 0xc));
                    if day == 1 {
                        day_one_active = Some(active);
                    } else {
                        for (slot, &is_active) in active.iter().enumerate().skip(1) {
                            if !is_active { removed.insert(slot); }
                        }
                        if day == 2 {
                            day_two_housing = Some(word(&session.roster.manager.players[0].bytes, 0x1b0));
                            day_two_active = Some(active.clone());
                            if stop_after_first || active.iter().filter(|&&value| value).count() != 4 {
                                break;
                            }
                        }
                        if !active[0] || day >= 10 { break; }
                    }
                }
            }
            if frame + 1 == frame_limit { println!("Scenario7 bounded frame limit at day{} hour{}", session.world.clock.calendar[4], session.world.clock.hour()); }
        }
        for (slot, player) in session.roster.manager.players.iter().enumerate().skip(1) {
            if player.bytes[0x10] == 0 { removed.insert(slot); }
        }
        writeln!(std::io::stdout(),"Scenario7 measurement stop-after-first{stop_after_first} virginia{virginia} third-apartment{third_apartment} frontier day{} hour{} owner{} lease{} human-housing{} offer{offer_tick:?} auction{auction_seen} due{transfer_due:?} owned{owned} rights{rights_due:?} built{built} apartment{apartment_target:?} apartments{} secondQuote{second_apartment_quote:?} secondSubmitted{second_apartment_submitted} thirdAttempted{third_apartment_attempted} thirdSubmitted{third_apartment_submitted} virginiaOffer{virginia_offer_tick:?} virginiaAuction{virginia_auction_seen} virginiaDue{virginia_transfer_due:?} virginiaOwned{virginia_owned} rentApplied{} day1{day_one_active:?} day2{day_two_active:?} removed{removed:?} notice{day_one_notice} survivorDue{survivor_notice_due:?} terminal{:?}",
            session.world.clock.calendar[4], session.world.clock.hour(),
            session.world.categories[0][5], word(&session.world.categories[0], 0x7c),
            word(&session.roster.manager.players[0].bytes, 0x1b0),
            managed_apartments.len(),
            rent_terms.iter().all(|check| check.checked),
            terminal.as_ref().map(|dialog| dialog.context)).unwrap();
        // Strategy admission is measured above; only validate gates actually reached.
        assert!(session.world.clock.timing[0] > 0, "Scenario7 clock never advanced");
        if let Some(active) = day_one_active {
            assert_eq!(active, vec![true; 5], "Scenario7 removed a player on its notice day");
        }
        if let Some(active) = day_two_active {
            assert_eq!(active.iter().filter(|&&value| value).count(), 4,
                "Scenario7 first elimination did not remove exactly one player");
        }
        writeln!(std::io::stdout(),"Scenario7 terminal accounts(cash,sales,income,active){:?} day2-housing{day_two_housing:?}",
            session.world.ledgers.players.iter().zip(&session.roster.manager.players)
                .map(|(account, player)| (account.cash, account.retail_counters[1],
                    (22..37).map(|event| account.ledger(event).unwrap()).sum::<f32>(), player.bytes[0x10] != 0)).collect::<Vec<_>>()).unwrap();
        drop(host.auction_renderer.take());
        let prepared = presenter.join().unwrap();
        if auction_seen || virginia_auction_seen {
            assert!(prepared >= 1, "Scenario7 visible auction lacked a prepared scene");
        }
        if let Some(dialog) = terminal {
            // Context10 means human elimination, with no declared survivor yet.
            assert!(dialog.context == 10 || (dialog.context as usize) < session.roster.manager.players.len(),
                "Scenario7 result identity is outside the roster");
            host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply {
                callback: dialog.callback, context: dialog.context, button: 0,
            }).unwrap();
            let expected = if dialog.context == 0 { openmt2001::outcome_notice::Sequence::Win }
                else { openmt2001::outcome_notice::Sequence::Lose };
            assert_eq!(host.result_sequence(), Some(expected));
        }
    }).unwrap().join().unwrap();
}
