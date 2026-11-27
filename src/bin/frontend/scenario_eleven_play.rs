//! Natural Scenario11 sales objective through legal Host construction and trade.
use super::scenario_twelve_play::wealth;
use super::{
    CommandAssets, Inputs, Path, PathBuf, Policy, Session, character_names, finish_original_win,
    fs, load_era_styles, submit_business_shape, word,
};

fn build_capacity_shop(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    business: u32,
    prior: &[i32],
    required_category: Option<usize>,
) -> Option<i32> {
    let info = session.world.catalog.info.row(business as usize).unwrap();
    let signed = |at| i32::from_le_bytes(info[at..at + 4].try_into().unwrap());
    // These five original-input shapes were admitted in the earlier 72-hour
    // capacity frontier and kept cash positive through its third day.
    let (wanted_area, wanted_height) = match business {
        20 => (10, 4),
        21 => (8, 4),
        11 => (4, 2),
        13 => (4, 4),
        26 => (3, 3),
        _ => return None,
    };
    let area = wanted_area.max(signed(0x38)).min(signed(0x3c));
    let height = wanted_height.max(signed(0x48)).min(signed(0x44));
    let mut categories = (0..52usize).collect::<Vec<_>>();
    categories.sort_by_key(|&category| {
        std::cmp::Reverse(word(&session.world.categories[category], 0x88))
    });
    for category in categories {
        if prior.contains(&(category as i32))
            || required_category.is_some_and(|required| category != required)
        {
            continue;
        }
        let excluded = (0..52i32)
            .filter(|&candidate| candidate != category as i32)
            .collect::<Vec<_>>();
        if let Some(category) =
            submit_business_shape(session, assets, business, &excluded, Some((area, height)))
        {
            println!(
                "Scenario11 legal capacity shop kind{business} category{category} area-at-least{area} height{height}"
            );
            return Some(category);
        }
    }
    None
}

/// Inspect the original planner on the already-owned lot before spending the
/// renewal and trading reserve. Return the quoted legal shape when cash must
/// accrue; an absent shape is a genuine placement frontier for this layout.
fn build_reserved_owned_shop(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    business: u32,
    category: usize,
    reserve: f32,
) -> Option<(f32, bool)> {
    let info = session
        .world
        .catalog
        .info
        .row(business as usize)
        .unwrap()
        .to_vec();
    let signed = |at| i32::from_le_bytes(info[at..at + 4].try_into().unwrap());
    assert_ne!(
        info[0x30], 0,
        "original business unavailable at planner attempt"
    );
    assert!(matches!(
        crate::build_panel::Context::from_session(session, category)
            .unwrap()
            .entry(),
        crate::build_panel::Entry::Open
    ));
    let minimum_area = signed(0x38);
    let height = signed(0x48);
    let kind = signed(0x0c) as u32;
    for width in 1..=6 {
        for depth in 1..=6 {
            if width * depth < minimum_area
                || width * depth > signed(0x3c)
                || width.max(depth) > signed(0x40)
            {
                continue;
            }
            for x in 0..=7 - width {
                for y in 0..=7 - depth {
                    let probe = crate::build_panel::Request {
                        category: category as i32,
                        origin: [x, y],
                        size: [width, depth],
                        height,
                        packed_model: 0,
                        kind,
                        quality: 0,
                        displayed_quote: 0.,
                        business,
                    };
                    let inspected = session.inspect_build(&probe, assets).unwrap();
                    if inspected.category_locked
                        || !inspected.placement_allowed
                        || inspected.preview.is_none()
                    {
                        continue;
                    }
                    let cash = session.world.ledgers.players[0].cash;
                    println!(
                        "Scenario11 original kind{business} planner category{category} size{width}x{depth} height{height} origin{x},{y} quote{} cash{cash} reserve{reserve} affordable{} day{} hour{}",
                        inspected.total,
                        inspected.affordable,
                        session.world.clock.calendar[4],
                        session.world.clock.hour(),
                    );
                    if !inspected.affordable || cash < inspected.total + reserve {
                        return Some((inspected.total, false));
                    }
                    let form = crate::build_panel::Form {
                        category: category as i32,
                        rectangle: [x, y, x + width - 1, y + depth - 1],
                        height,
                        quality: 0,
                        choices: [0; 3],
                        variants: vec![0],
                        kind,
                        business,
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
                            .submit_build(&request, assets, |_| {
                                Err("reserved shop applied before native deadline".into())
                            })
                            .unwrap()
                            .submitted
                    );
                    return Some((inspected.total, true));
                }
            }
        }
    }
    None
}

fn sold(session: &Session<'_>) -> i32 {
    let count = session.world.ledgers.players[0].retail_counters[1];
    assert_eq!(
        word(&session.roster.manager.players[0].bytes, 0x1b4) as i32,
        count
    );
    count
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_sales_win_and_profile_reload() {
    run_scenario_eleven(0);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_two_shops_on_owned_category_probe() {
    run_scenario_eleven(2);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_five_shops_on_owned_category_probe() {
    run_scenario_eleven(5);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_owned_and_distinct_sites_probe() {
    run_scenario_eleven(3);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_staged_distinct_shop_probe() {
    run_scenario_eleven(6);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_staged_sales_win_and_profile_reload() {
    run_scenario_eleven(7);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_rival_sales_diagnostic() {
    run_scenario_eleven(8);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_owned_kind_six_sales_win_and_profile_reload() {
    run_scenario_eleven(9);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_owned_staple_cluster_frontier() {
    run_scenario_eleven(10);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_category_twelve_ownership_frontier() {
    run_scenario_eleven(11);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn legal_scenario_eleven_category_twelve_deli_frontier() {
    run_scenario_eleven(12);
}

#[test]
#[ignore = "requires original assets and genuine Scenario1+6 Win profile"]
fn natural_scenario_eleven_two_shop_category_twenty_six_auction_frontier() {
    run_scenario_eleven(13);
}

fn run_scenario_eleven(probe_shops: usize) {
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
            let index = openmt2001::archive::ArchiveIndex::parse(
                &directory,
                archive.len() as u64,
            )
            .unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let scripts = exe.parent().unwrap().join("scripts");
            let names = character_names(
                &image,
                &exe.parent().unwrap().parent().unwrap().join("US/language/usa"),
            )
            .unwrap();
            let source_profile = PathBuf::from(
                fs::read_to_string(openmt2001::fixtures::path("SCENARIO3_NATURAL_WIN_PROFILE_PATH"))
                    .unwrap(),
            );
            let source_bytes = fs::read(&source_profile).unwrap();
            let profile = PathBuf::from(format!(
                "work/economy/scenario11-legal-{}-profile.lua",
                std::process::id()
            ));
            fs::write(&profile, &source_bytes).unwrap();
            let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
            crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
            for id in [200001, 200006] {
                let prior = &catalog.levels[catalog.find(id).unwrap()];
                assert_eq!(prior.attempts(), 1);
                assert!(prior.completed()[0]);
            }
            let level = &catalog.levels[catalog.find(200011).unwrap()];
            assert!(level.unlocked());
            assert_eq!(level.attempts(), 0);
            let mut session = Session::initialize_campaign(
                Inputs {
                    tree_locators: None,
                    image: &image,
                    board: &board,
                    file: &file,
                    people: openmt2001::people::PeopleModels::load(&image, &index, &archive)
                        .unwrap(),
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
                    era_styles: load_era_styles(
                        Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                            .parent()
                            .unwrap(),
                    )
                    .unwrap(),
                    timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario: "SCENARIO11",
                    difficulty: 0,
                    selected_character: 4,
                    profile_name: b"Scenario11 Legal Sales",
                    character_names: &names,
                },
                Policy {
                    seed: 0x12345678,
                    auxiliary_prior_bytes: [0; 0x11c],
                },
                catalog,
                profile.clone(),
            )
            .unwrap();
            assert_eq!((session.world.objective.kind, session.world.objective.target), (6, 3000.));
            assert_eq!(session.roster.manager.players.len(), 5);
            assert_eq!(session.world.ledgers.residents.active_count, 440);
            assert_eq!(session.world.ledgers.players[0].cash, 8000.);
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.begin_presentation(&session).unwrap();
            host.set_running(true);
            let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            host.auction_renderer = Some(sender);
            let presenter = std::thread::spawn(move || {
                let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
                let dir = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
                let bin = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor))
                    .unwrap();
                let mut view = crate::auction_bridge::View::default();
                let mut counts = [0; 3];
                while let Ok(request) = receiver.recv() {
                    let stage = match &request.operation {
                        crate::auction_bridge::Operation::Prepare { .. } => 0,
                        crate::auction_bridge::Operation::Activate => 1,
                        crate::auction_bridge::Operation::Restore => 2,
                    };
                    let result = view.present(request.operation, &renderer, (&exe, &dir, &bin), 0);
                    if result.is_ok() {
                        counts[stage] += 1;
                    }
                    request.reply.send(result).unwrap();
                }
                (counts, view.scene.is_some())
            });
            // Category0 ownership saves the measured rent on collocated shops.
            // The separate sites below admitted the earlier 1330-unit natural
            // frontier; this probe tests their marginal demand and daily rent.
            let owned_lot_probe = probe_shops != 0;
            let distinct_sites_probe = probe_shops == 3;
            let staged_shop_probe = matches!(probe_shops, 6..=8 | 11 | 12);
            let category_twelve_probe = matches!(probe_shops, 11 | 12);
            let deli_probe = probe_shops == 12;
            let geographic_probe = probe_shops == 13;
            let kind_six_probe = probe_shops == 9;
            let staple_probe = probe_shops == 10;
            // Original planner admitted these three sequential edge footprints;
            // the next minimum kind4 had no legal perimeter position.
            let staple_kinds = [0_u32, 1, 3];
            let plan: &[(u32, usize)] = match probe_shops {
                2 => &[(20, 0), (11, 0)],
                3 => &[(20, 0), (11, 0), (21, 12), (13, 13), (26, 0)],
                6 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                7 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                8 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                9 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                10 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                11 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                12 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                13 => &[(20, 0), (11, 0)],
                5 => &[(20, 0), (11, 0), (21, 0), (13, 0), (26, 0)],
                _ => &[(21, 12), (11, 36), (13, 13), (26, 30), (20, 0)],
            };
            let mut built = Vec::new();
            let lease_target = 0usize;
            let kind11_category = if owned_lot_probe { 0 } else { 36 };
            let mut lease_requested_at = None;
            let mut lease_seen = false;
            let mut lease_deadline = None;
            let mut lease_acquired = false;
            let mut lease_attempts = 0;
            let mut lease_cycles = 0;
            let mut rights_due = None;
            let mut rights_ready = false;
            let mut rights_activations = 0;
            let mut kind11_slot = None;
            let mut kind11_state = None;
            let mut kind11_category_owner = session.world.categories[kind11_category][5];
            let mut probe_rent_checked = false;
            let mut initial_owned_tenants_checked = false;
            let mut probe_shop_slots = [None; 5];
            let mut probe_shop_states = [None; 5];
            let mut staged_shop_built = false;
            let mut staged_shop_slot = None;
            let mut staged_shop_rent_seen = false;
            let mut category_twelve_offer_at = None;
            let mut category_twelve_auction_due = None;
            let mut category_twelve_auction_seen = false;
            let mut category_twelve_owned = false;
            let mut category_twelve_waiver_seen = false;
            let mut category_twelve_offer_rejected = false;
            let mut category_twelve_offer_last_day = None;
            let mut category_twelve_lost = false;
            let mut deli_rights_due = None;
            let mut deli_rights_ready = false;
            let mut deli_available_at = None;
            let mut deli_attempt_hour = None;
            let mut deli_quote = None;
            let mut deli_built = false;
            let mut deli_slot = None;
            let mut geographic_auction_seen = false;
            let mut kind_six_quote = None;
            let mut kind_six_built = false;
            let mut kind_six_slot = None;
            let mut kind_six_attempt_day = None;
            let mut kind_six_available_at = None;
            let mut staple_index = 0;
            let mut staple_slots = [None; 3];
            let mut staple_attempt_day = None;
            let mut previous_hour = u32::MAX;
            let mut result_dialog = None;
            let mut pending_buyout = None;
            let start_hour = session.world.clock.calendar[4] * 24 + session.world.clock.hour();
            for frame in 0..2_000_000 {
                let deli_available = session.world.catalog.info.row(2).unwrap()[0x30] != 0;
                if deli_probe && deli_available && deli_available_at.is_none() {
                    deli_available_at = Some(session.world.clock.timing[0]);
                    println!("Scenario11 original DELI availability became live day{} hour{} era{} tick{} cash{}",
                        session.world.clock.calendar[4], session.world.clock.hour(),
                        session.world.clock.calendar[2], session.world.clock.timing[0],
                        session.world.ledgers.players[0].cash);
                }
                let kind_six_available = session.world.catalog.info.row(6).unwrap()[0x30] != 0;
                if kind_six_probe && kind_six_available && kind_six_available_at.is_none() {
                    kind_six_available_at = Some(session.world.clock.timing[0]);
                    println!("Scenario11 original GENERAL_STORE availability became live day{} hour{} era{} tick{} cash{}",
                        session.world.clock.calendar[4], session.world.clock.hour(),
                        session.world.clock.calendar[2], session.world.clock.timing[0],
                        session.world.ledgers.players[0].cash);
                }
                if frame % 625 == 0 && built.len() < plan.len()
                    && (if owned_lot_probe { rights_ready } else { built.len() < 4 || rights_ready })
                    && let Some(category) = build_capacity_shop(
                        &mut session,
                        &assets,
                        plan[built.len()].0,
                        if owned_lot_probe { &[] } else { &built },
                        Some(plan[built.len()].1),
                    )
                {
                    assert_eq!(category, plan[built.len()].1 as i32);
                    built.push(category);
                }
                if staged_shop_probe && !staged_shop_built && built.len() == plan.len()
                    && session.world.clock.calendar[4] >= 3
                    && frame % 625 == 0
                    // Original category12 kind21 quote was $2114. Reserve the
                    // measured $1900 renewal auction plus $2000 for trading.
                    && session.world.ledgers.players[0].cash >= 2114. + 1900. + 2000.
                    && let Some(category) = build_capacity_shop(
                        &mut session, &assets, 21, &[], Some(12))
                {
                    assert_eq!(category, 12);
                    staged_shop_built = true;
                    println!("Scenario11 staged distinct kind21 category12 day{} hour{} tick{} cash{}",
                        session.world.clock.calendar[4], session.world.clock.hour(),
                        session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                }
                if kind_six_probe && !kind_six_built && built.len() == plan.len()
                    && rights_ready && session.world.categories[0][5] == 1
                    && session.world.clock.calendar[4] >= 3 && kind_six_available
                    && frame % 625 == 0
                    && kind_six_attempt_day != Some(session.world.clock.calendar[4])
                {
                    kind_six_attempt_day = Some(session.world.clock.calendar[4]);
                    let (quote, built_now) = build_reserved_owned_shop(&mut session, &assets, 6, 0, 3900.)
                        .expect("original planner found no legal minimum kind6 shape on owned category0");
                    kind_six_quote = Some(quote);
                    kind_six_built = built_now;
                    if built_now {
                        println!("Scenario11 kind6 category0 submitted day{} hour{} tick{} quote{quote} cash{}",
                            session.world.clock.calendar[4], session.world.clock.hour(),
                            session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                    }
                }
                if staple_probe && staple_index < staple_kinds.len()
                    && built.len() == plan.len() && rights_ready
                    && session.world.categories[0][5] == 1
                    && (staple_index == 0 || staple_slots[staple_index - 1].is_some())
                    && session.world.clock.calendar[4] >= 3
                    && frame % 625 == 0
                    && staple_attempt_day != Some((session.world.clock.calendar[4], staple_index))
                {
                    let business = staple_kinds[staple_index];
                    assert_ne!(session.world.catalog.info.row(business as usize).unwrap()[0x30], 0,
                        "original staple business became unavailable");
                    staple_attempt_day = Some((session.world.clock.calendar[4], staple_index));
                    let (quote, built_now) = build_reserved_owned_shop(&mut session, &assets, business, 0, 3900.)
                        .unwrap_or_else(|| panic!("original category0 geometry rejects minimum kind{business} after {staple_index} staples"));
                    if built_now {
                        println!("Scenario11 staple kind{business} category0 submitted day{} hour{} tick{} quote{quote} cash{}",
                            session.world.clock.calendar[4], session.world.clock.hour(),
                            session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                        staple_index += 1;
                    }
                }
                if deli_probe && category_twelve_owned && deli_rights_ready
                    && deli_available && !deli_built && frame % 625 == 0
                    && matches!(session.world.clock.hour(), 0 | 23)
                    && deli_attempt_hour != Some((session.world.clock.calendar[4], session.world.clock.hour()))
                {
                    deli_attempt_hour = Some((session.world.clock.calendar[4], session.world.clock.hour()));
                    // Measured category0 renewal reached $2950, category12 first
                    // auction $1100. Keep both renewal and $2000 trading cash.
                    let reserve = if lease_cycles < 2 { 2950. + 1100. + 2000. }
                        else { 1100. + 2000. };
                    let inspected = build_reserved_owned_shop(&mut session, &assets, 2, 12, reserve)
                        .expect("original planner found no legal minimum DELI shape on owned category12");
                    deli_quote = Some(inspected.0);
                    deli_built = inspected.1;
                    println!("Scenario11 category12 DELI legal quote{} submitted{} reserve{reserve} cash{} day{} hour{}",
                        inspected.0, inspected.1, session.world.ledgers.players[0].cash,
                        session.world.clock.calendar[4], session.world.clock.hour());
                }
                let due = session.world.clock.timing[0] + 25;
                for (_, _, inner, _, command) in session.live_commands.snapshot().unwrap() {
                    if inner != due { continue; }
                    let producer = match command {
                        openmt2001::live_commands::Command::EventCard(card) => {
                            let row = openmt2001::event_cards::lifecycle::card_row(&image, card.card).unwrap();
                            (word(row, 12) == 6 && word(row, 8) == 8)
                                .then_some((card.target, format!("targeted-card{} actor{}", card.card, card.player)))
                        }
                        openmt2001::live_commands::Command::Takeover(takeover) =>
                            Some((takeover.building as i32, "property-owner takeover".to_string())),
                        _ => None,
                    };
                    if let Some((building, name)) = producer {
                        let owned = session.world.buildings.get(building as usize)
                            .and_then(|row| session.world.ledgers.businesses.get(word(row, 0x1c) as usize))
                            .is_some_and(|row| row[0x21c] != 0 && row[10] == 1);
                        if owned {
                            println!("Scenario11 pending human-business transfer producer{name} building{building} due{due}");
                            pending_buyout = Some((due, building, name));
                        }
                    }
                }
                host.frame(&mut session, &assets, 40);
                assert!(host.failure.is_none(), "natural Scenario11 Host: {:?}", host.failure);
                let status = host.status(&session);
                if staple_probe {
                    for (i, &kind) in staple_kinds.iter().enumerate().take(staple_index) {
                        if staple_slots[i].is_some() { continue; }
                        staple_slots[i] = session.world.ledgers.businesses.iter().position(|row| {
                            if row[0x21c] == 0 || row[10] != 1 || word(row, 4) != kind { return false; }
                            let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                            session.world.buildings.get(building)
                                .is_some_and(|record| word(record, 0x40) == 0x617628)
                        });
                        if let Some(slot) = staple_slots[i] {
                            println!("Scenario11 staple kind{kind} linked slot{slot} tick{}",
                                session.world.clock.timing[0]);
                        }
                    }
                }
                if kind_six_built && kind_six_slot.is_none() {
                    kind_six_slot = session.world.ledgers.businesses.iter().position(|row| {
                        if row[0x21c] == 0 || row[10] != 1 || word(row, 4) != 6 { return false; }
                        let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                        session.world.buildings.get(building)
                            .is_some_and(|record| word(record, 0x40) == 0x617628)
                    });
                    if let Some(slot) = kind_six_slot {
                        println!("Scenario11 kind6 linked slot{slot} tick{}",
                            session.world.clock.timing[0]);
                    }
                }
                if staged_shop_built && staged_shop_slot.is_none() {
                    staged_shop_slot = session.world.ledgers.businesses.iter().position(|row| {
                        if row[0x21c] == 0 || row[10] != 1 || word(row, 4) != 21 { return false; }
                        let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                        session.world.buildings.get(building)
                            .is_some_and(|record| word(record, 0x40) == 0x617628 + 12 * 380)
                    });
                    if let Some(slot) = staged_shop_slot {
                        println!("Scenario11 staged shop linked slot{slot} tick{}",
                            session.world.clock.timing[0]);
                    }
                }
                if category_twelve_probe && staged_shop_slot.is_some()
                    && category_twelve_offer_at.is_none() && !category_twelve_offer_rejected
                    && session.world.categories[12][5] == 0
                    && session.world.categories[0][5] == 1
                    && session.world.clock.calendar[4] >= 3
                    && category_twelve_offer_last_day != Some(session.world.clock.calendar[4])
                    && session.world.auction.controller.active_count == 0
                    && !status.auction.as_ref().is_some_and(|auction| auction.visible)
                {
                    category_twelve_offer_last_day = Some(session.world.clock.calendar[4]);
                    let inspected = host.inspect_property_offer(
                        &mut session, &assets, 12,
                        openmt2001::property_offer::Panel::default(), true,
                    ).unwrap();
                    let cash = session.world.ledgers.players[0].cash;
                    println!("Scenario11 category12 live property offer opening{} action-visible{} cash{cash} reserve3900 day{} hour{} tick{}",
                        inspected.panel.amount, inspected.panel.action_visible,
                        session.world.clock.calendar[4], session.world.clock.hour(),
                        session.world.clock.timing[0]);
                    if inspected.panel.action_visible == 0 {
                        category_twelve_offer_rejected = true;
                    } else if cash >= inspected.panel.amount + 1900. + 2000. {
                        let tick = session.world.clock.timing[0];
                        let offered = host.property_offer(
                            &mut session, &assets, 12, inspected.panel,
                        ).unwrap();
                        assert!(offered.submitted);
                        assert_eq!(session.world.categories[12][5], 0);
                        category_twelve_offer_at = Some(tick);
                        println!("Scenario11 legal category12 property offer submitted tick{tick}");
                    }
                }
                if owned_lot_probe {
                    for (i, &(kind, category)) in plan.iter().enumerate() {
                        if built.len() <= i {
                            continue;
                        }
                        if probe_shop_slots[i].is_none() {
                            probe_shop_slots[i] = session.world.ledgers.businesses.iter().position(|row|
                                row[0x21c] != 0 && row[10] == 1 && word(row, 4) == kind);
                        }
                        if let Some(slot) = probe_shop_slots[i] {
                            let row = &session.world.ledgers.businesses[slot];
                            let state = (row[0x21c], row[10], word(row, 0), word(row, 4));
                            if probe_shop_states[i] != Some(state) {
                                println!("Scenario11 planned kind{kind} category{category} slot{slot} raw-state{state:?} tick{} day{} hour{} category0-owner{} producer{:?}",
                                    session.world.clock.timing[0], session.world.clock.calendar[4],
                                    session.world.clock.hour(), session.world.categories[lease_target][5],
                                    pending_buyout.as_ref().filter(|(due, building, _)|
                                        *due == session.world.clock.timing[0]
                                            && *building == i32::from(i16::from_le_bytes(row[8..10].try_into().unwrap()))));
                                probe_shop_states[i] = Some(state);
                            }
                        }
                    }
                }
                if session.world.categories[kind11_category][5] != kind11_category_owner {
                    println!("Scenario11 kind11 category{kind11_category} owner{}->{} tick{} day{} hour{} lease-hours{}",
                        kind11_category_owner, session.world.categories[kind11_category][5],
                        session.world.clock.timing[0], session.world.clock.calendar[4],
                        session.world.clock.hour(), word(&session.world.categories[kind11_category], 0x7c));
                    kind11_category_owner = session.world.categories[kind11_category][5];
                }
                if built.len() >= 2 && kind11_slot.is_none() {
                    kind11_slot = session.world.ledgers.businesses.iter().position(|row|
                        row[0x21c] != 0 && row[10] == 1 && word(row, 4) == 11);
                    if let Some(slot) = kind11_slot {
                        println!("Scenario11 original kind11 business allocated slot{slot} tick{}", session.world.clock.timing[0]);
                    }
                }
                if let Some(slot) = kind11_slot {
                    let row = &session.world.ledgers.businesses[slot];
                    let state = (row[0x21c], row[10], word(row, 0), word(row, 4),
                        u16::from_le_bytes(row[8..10].try_into().unwrap()));
                    if kind11_state != Some(state) {
                        println!("Scenario11 kind11 slot{slot} raw-state{state:?} tick{} day{} hour{}",
                            session.world.clock.timing[0], session.world.clock.calendar[4],
                            session.world.clock.hour());
                        let pending = session.live_commands.snapshot().unwrap()
                            .into_iter().map(|(_, outer, inner, _, command)| {
                                let name = match command {
                                    openmt2001::live_commands::Command::SellBusiness(sale) =>
                                        format!("sell-business{}", sale.business),
                                    openmt2001::live_commands::Command::Takeover(takeover) =>
                                        format!("takeover-building{}", takeover.building),
                                    openmt2001::live_commands::Command::Demolish(demolition) =>
                                        format!("demolish-building{}", demolition.building),
                                    openmt2001::live_commands::Command::Auction { descriptor } =>
                                        format!("auction-command{}-category{}", word(descriptor, 4),
                                            u16::from_le_bytes(descriptor[24..26].try_into().unwrap())),
                                    _ => "other".to_string(),
                                };
                                (outer, inner, name)
                            }).collect::<Vec<_>>();
                        println!("Scenario11 kind11 transition pending-native-commands{pending:?}");
                        kind11_state = Some(state);
                    }
                }
                if lease_acquired && session.world.categories[lease_target][5] != 1 {
                    let new_owner = session.world.categories[lease_target][5];
                    let remaining = word(&session.world.categories[lease_target], 0x7c);
                    let alive = session.roster.manager.players[0].bytes[0x10] != 0;
                    println!("Scenario11 category0 ownership changed to{new_owner} at tick{} day{} hour{} remaining-hours{remaining} human-active{alive}",
                        session.world.clock.timing[0], session.world.clock.calendar[4],
                        session.world.clock.hour());
                    lease_acquired = false;
                    lease_requested_at = None;
                    lease_seen = false;
                    lease_deadline = None;
                    rights_due = None;
                    rights_ready = false;
                }
                if lease_requested_at.is_some_and(|tick| session.world.clock.timing[0] > tick + 25)
                    && lease_deadline.is_none() && !lease_acquired && !lease_seen
                    && session.world.categories[lease_target][0x6c] == 0
                    && session.world.auction.controller.active_count == 0
                {
                    println!("Scenario11 category0 offer{lease_attempts} had no queued ownership command; retry after native admission window");
                    lease_requested_at = None;
                    lease_seen = false;
                }
                if lease_requested_at.is_none() && !lease_acquired
                    && session.roster.manager.players[0].bytes[0x10] != 0
                    && session.world.clock.timing[0] >= 1
                    && session.world.categories[lease_target][5] == 0
                    && session.world.auction.controller.active_count == 0
                    && !status.auction.as_ref().is_some_and(|auction| auction.visible)
                {
                    let inspected = host.inspect_property_offer(
                        &mut session, &assets, lease_target as u32,
                        openmt2001::property_offer::Panel::default(), true,
                    ).unwrap();
                    let remaining_build_reserve = if built.len() < plan.len() { 1675. + 200. } else { 0. };
                    if inspected.panel.action_visible != 0
                        && session.world.ledgers.players[0].cash >= inspected.panel.amount + remaining_build_reserve
                    {
                        let tick = session.world.clock.timing[0];
                        let offer = host.property_offer(
                            &mut session, &assets, lease_target as u32, inspected.panel,
                        ).unwrap();
                        assert!(offer.submitted);
                        assert_eq!(session.world.categories[lease_target][5], 0);
                        lease_requested_at = Some(tick);
                        lease_attempts += 1;
                        println!("Scenario11 legal category0 property offer attempt{lease_attempts} opening{} at tick{tick}", offer.panel.amount);
                    }
                }
                for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                    if let openmt2001::live_commands::Command::Auction { descriptor } = command
                        && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                    {
                        let category = usize::from(u16::from_le_bytes(descriptor[24..26].try_into().unwrap()));
                        if category != lease_target && !(category_twelve_probe && category == 12) {
                            continue;
                        }
                        assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                        assert_eq!(outer, inner);
                        assert!(!direct);
                        if category == lease_target {
                            if lease_deadline.is_none() {
                                assert_eq!(inner, session.world.clock.timing[0] + 25);
                                lease_deadline = Some(inner);
                                println!("Scenario11 category0 ownership queued at tick{} due{inner}", session.world.clock.timing[0]);
                            }
                        } else if category_twelve_probe && category_twelve_auction_due.is_none() {
                            assert_eq!(inner, session.world.clock.timing[0] + 25);
                            category_twelve_auction_due = Some(inner);
                            println!("Scenario11 category12 ownership queued at tick{} due{inner}", session.world.clock.timing[0]);
                        }
                        assert_eq!(session.world.categories[category][5], 0);
                    }
                }
                if category_twelve_probe && category_twelve_offer_at.is_some()
                    && category_twelve_auction_due.is_none()
                    && session.world.clock.timing[0] > category_twelve_offer_at.unwrap() + 25
                    && session.world.auction.controller.active_count == 0
                {
                    category_twelve_offer_rejected = true;
                    println!("Scenario11 category12 offer produced no queued native ownership command");
                }
                let target_auction_visible = status.auction.as_ref().is_some_and(|auction| auction.visible)
                    && word(&session.world.auction.controller.record, 4) == lease_target as u32;
                if target_auction_visible && !lease_seen {
                    println!("Scenario11 category0 visible auction at tick{} price{}",
                        session.world.clock.timing[0], word(&session.world.auction.controller.record, 0x6c));
                }
                lease_seen |= target_auction_visible;
                let category_twelve_visible = category_twelve_probe
                    && status.auction.as_ref().is_some_and(|auction| auction.visible)
                    && word(&session.world.auction.controller.record, 4) == 12;
                if category_twelve_visible && !category_twelve_auction_seen {
                    println!("Scenario11 category12 visible auction tick{} price{}",
                        session.world.clock.timing[0],
                        word(&session.world.auction.controller.record, 0x6c));
                }
                category_twelve_auction_seen |= category_twelve_visible;
                let geographic_visible = geographic_probe
                    && status.auction.as_ref().is_some_and(|auction| auction.visible)
                    && word(&session.world.auction.controller.record, 4) == 26;
                geographic_auction_seen |= geographic_visible;
                if category_twelve_probe && !category_twelve_owned
                    && session.world.categories[12][5] == 1 {
                    assert!(category_twelve_auction_seen);
                    assert!(session.world.clock.timing[0] > category_twelve_offer_at.unwrap());
                    assert_eq!(Some(session.world.clock.timing[0]), category_twelve_auction_due);
                    category_twelve_owned = true;
                    println!("Scenario11 category12 ownership transferred tick{} cash{} lease-hours{}",
                        session.world.clock.timing[0], session.world.ledgers.players[0].cash,
                        word(&session.world.categories[12], 0x7c));
                }
                if deli_probe && category_twelve_owned && deli_rights_due.is_none() {
                    let before = host.property_rights(&mut session, &assets, 12, 0, false).unwrap();
                    assert_eq!(before.message, Some(1202));
                    assert!(before.activate_visible);
                    let due = session.world.clock.timing[0] + 25;
                    let pending = host.property_rights(&mut session, &assets, 12, 0, true).unwrap();
                    assert_eq!(pending.message, Some(1406));
                    assert_eq!(session.world.categories[12][0x6d], 1);
                    deli_rights_due = Some(due);
                    println!("Scenario11 legal Activate Building Rights category12 tick{} due{due}",
                        session.world.clock.timing[0]);
                }
                if let Some(due) = deli_rights_due
                    && !deli_rights_ready && session.world.clock.timing[0] >= due
                {
                    assert_eq!(session.world.clock.timing[0], due);
                    assert_eq!((session.world.categories[12][0x6d],
                        word(&session.world.categories[12], 0x78)), (0, 1));
                    deli_rights_ready = true;
                    println!("Scenario11 category12 original building rights active at tick{due}");
                }
                if deli_probe && deli_built && deli_slot.is_none() {
                    deli_slot = session.world.ledgers.businesses.iter().position(|row| {
                        if row[0x21c] == 0 || row[10] != 1 || word(row, 4) != 2 { return false; }
                        let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                        session.world.buildings.get(building)
                            .is_some_and(|record| word(record, 0x40) == 0x617628 + 12 * 380)
                    });
                    if let Some(slot) = deli_slot {
                        println!("Scenario11 category12 DELI linked slot{slot} tick{}",
                            session.world.clock.timing[0]);
                    }
                }
                if category_twelve_auction_seen && !category_twelve_owned
                    && !category_twelve_visible && session.world.auction.controller.active_count == 0
                    && !category_twelve_lost && session.world.categories[12][5] != 0 {
                    category_twelve_lost = true;
                    println!("Scenario11 category12 auction awarded to owner{} tick{}",
                        session.world.categories[12][5], session.world.clock.timing[0]);
                }
                if !lease_acquired && session.world.categories[lease_target][5] == 1 {
                    assert!(lease_seen, "category0 changed owner without visible auction");
                    assert!(session.world.clock.timing[0] > lease_requested_at.unwrap());
                    assert_eq!(Some(session.world.clock.timing[0]), lease_deadline);
                    lease_acquired = true;
                    lease_cycles += 1;
                    assert_eq!(word(&session.world.categories[lease_target], 0x7c),
                        session.world.settings.lease_length() as u32);
                    println!("Scenario11 category0 lease acquired tick{} cash{}",
                        session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                }
                if lease_acquired && rights_due.is_none() {
                    let before = host.property_rights(&mut session, &assets, lease_target as u32, 0, false).unwrap();
                    assert_eq!(before.message, Some(1202));
                    assert!(before.activate_visible);
                    let due = session.world.clock.timing[0] + 25;
                    let pending = host.property_rights(&mut session, &assets, lease_target as u32, 0, true).unwrap();
                    assert_eq!(pending.message, Some(1406));
                    assert_eq!(session.world.categories[lease_target][0x6d], 1);
                    rights_due = Some(due);
                    rights_activations += 1;
                    println!("Scenario11 legal Activate Building Rights category0 tick{} due{due}", session.world.clock.timing[0]);
                }
                if let Some(due) = rights_due
                    && !rights_ready && session.world.clock.timing[0] >= due
                {
                    assert_eq!(session.world.clock.timing[0], due);
                    assert_eq!((session.world.categories[lease_target][0x6d],
                        word(&session.world.categories[lease_target], 0x78)), (0, 1));
                    rights_ready = true;
                    println!("Scenario11 category0 original building rights active at tick{due}");
                }
                if status.auction.as_ref().is_some_and(|auction| {
                    auction.visible && auction.controls.buttons_enabled & 8 != 0
                }) {
                    let category = word(&session.world.auction.controller.record, 4);
                    let next = word(&session.world.auction.controller.record, 0x6c).saturating_add(50);
                    let cash = session.world.ledgers.players[0].cash;
                    let reserve = if category_twelve_probe && category == 12 { 1900. + 2000. }
                        else if category_twelve_probe && category == lease_target as u32 {
                            if deli_probe { 3100. } else { 2000. }
                        }
                        else if owned_lot_probe { 3000. }
                        else if built.len() == plan.len() { 200. } else { 1675. + 200. };
                    let target = (category == lease_target as u32 && lease_requested_at.is_some())
                        || (category_twelve_probe && category == 12 && category_twelve_offer_at.is_some());
                    let bid = target
                        && cash >= next as f32 + reserve;
                    host.auction_button(&mut session, &assets, if bid { 0 } else { 3 }).unwrap();
                    println!("Scenario11 auction category{category} next{next} cash{cash} button{} at tick{}",
                        if bid { 0 } else { 3 }, session.world.clock.timing[0]);
                }
                for prompt in status.prompts {
                    if let Some(dialog) = prompt.dialog {
                        println!("Scenario11 human dialog callback{:x} context{} buttons{} tick{}",
                            dialog.callback, dialog.context, dialog.button_count,
                            session.world.clock.timing[0]);
                        if dialog.callback == 0x48b490 {
                            result_dialog = Some(dialog);
                            break;
                        }
                        host.human_dialog_reply(
                            &mut session,
                            &assets,
                            crate::human_dialog::Reply {
                                callback: dialog.callback,
                                context: dialog.context,
                                button: u32::from(dialog.button_count == 2),
                            },
                        )
                        .unwrap();
                    } else if prompt.dismissal_id != 0 {
                        session
                            .dispatch_session_event(
                                "InterfacePromptDismiss",
                                prompt.dismissal_id as i32,
                                &assets,
                                false,
                            )
                            .unwrap();
                    }
                }
                if previous_hour != session.world.clock.hour() {
                    previous_hour = session.world.clock.hour();
                    crate::session_validation::assert_live_links(&session);
                    if matches!(previous_hour, 0 | 6 | 14 | 23) {
                        println!(
                            "Scenario11 natural day{} hour{previous_hour} tick{} built{} sales{} cash{} wealth{} opponent-sales{:?} reached{:?}",
                            session.world.clock.calendar[4],
                            session.world.clock.timing[0],
                            built.len(),
                            sold(&session),
                            session.world.ledgers.players[0].cash,
                            wealth(&session, &image).to_f32(),
                            session.roster.manager.players.iter().skip(1)
                                .map(|player| word(&player.bytes, 0x1b4))
                                .collect::<Vec<_>>(),
                            session.world.objective.reached,
                        );
                    }
                    if previous_hour == 6 {
                        let account = &session.world.ledgers.players[0];
                        let utilities = [4, 5, 6, 7]
                            .into_iter()
                            .map(|event| account.ledger(event).unwrap())
                            .sum::<f32>();
                        println!(
                            "Scenario11 day{} account land-rent{} housing-rent{} landlord-income{} utilities{} cash{} category0-owner{} lease-hours{}",
                            session.world.clock.calendar[4],
                            account.ledger(14).unwrap(),
                            account.ledger(33).unwrap(),
                            account.ledger(34).unwrap(),
                            utilities,
                            account.cash,
                            session.world.categories[lease_target][5],
                            word(&session.world.categories[lease_target], 0x7c),
                        );
                        if owned_lot_probe && built.len() == plan.len()
                            && session.world.categories[lease_target][5] == 1
                            && session.world.clock.calendar[4] >= 1
                        {
                            if !initial_owned_tenants_checked {
                                for &(kind, category) in plan {
                                    let row = session.world.ledgers.businesses.iter().find(|row|
                                        row[0x21c] != 0 && row[10] == 1 && word(*row, 4) == kind)
                                        .expect("original owned-lot retail tenant missing at first daily settlement");
                                    let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                                    assert_eq!(word(&session.world.buildings[building], 0x40),
                                        0x617628 + category as u32 * 380);
                                    assert!(u16::from_le_bytes(row[0x18..0x1a].try_into().unwrap()) > 0);
                                }
                                initial_owned_tenants_checked = true;
                            }
                            for &slot in probe_shop_slots.iter().flatten() {
                                let row = &session.world.ledgers.businesses[slot];
                                if row[0x21c] == 0 || row[10] != 1 { continue; }
                                let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                                let category = plan[probe_shop_slots.iter().position(|candidate|
                                    *candidate == Some(slot)).unwrap()].1;
                                assert_eq!(word(&session.world.buildings[building], 0x40),
                                    0x617628 + category as u32 * 380);
                            }
                            if distinct_sites_probe {
                                assert!(account.ledger(14).unwrap() < 1503.,
                                    "hybrid site rent did not improve on measured distributed frontier");
                            } else if staged_shop_probe && staged_shop_built {
                                if let Some(slot) = staged_shop_slot {
                                    let row = &session.world.ledgers.businesses[slot];
                                    if row[0x21c] != 0 && row[10] == 1 {
                                        if session.world.categories[12][5] != 1 {
                                            let rent = f32::from(u16::from_le_bytes(row[0x18..0x1a].try_into().unwrap()));
                                            assert_eq!(account.ledger(14).unwrap(), -rent);
                                            staged_shop_rent_seen = true;
                                        } else if category_twelve_probe {
                                            assert_eq!(account.ledger(14).unwrap(), 0.);
                                            category_twelve_waiver_seen = true;
                                            println!("Scenario11 category12 owned shop land-rent waived day{} cash{}",
                                                session.world.clock.calendar[4], account.cash);
                                        }
                                    }
                                }
                            } else {
                                assert_eq!(account.ledger(14).unwrap(), 0.,
                                    "actual category0 retail tenants must owe no land rent to themselves");
                            }
                            probe_rent_checked = true;
                        }
                    }
                    if previous_hour == 23 && session.world.clock.calendar[4] <= 5 {
                        for (slot, business) in session.world.ledgers.businesses.iter().enumerate()
                            .filter(|(_, row)| row[0x21c] != 0 && row[10] == 1 && word(*row, 0) == 0)
                        {
                            let kind = word(business, 4) as usize;
                            let rent = u16::from_le_bytes(business[0x18..0x1a].try_into().unwrap());
                            let building = usize::from(u16::from_le_bytes(business[8..10].try_into().unwrap()));
                            let reference = word(&session.world.buildings[building], 0x40);
                            let offset = reference.checked_sub(0x617628)
                                .expect("retail building has no linked category");
                            assert_eq!(offset % 380, 0);
                            let category = usize::try_from(offset / 380).unwrap();
                            assert!(category < 52);
                            let landlord = session.world.categories[category][5];
                            let count = usize::from(session.world.catalog.info.row(kind).unwrap()[0x4c]);
                            let prior = (usize::from(business[0x209]) + 3) % 4;
                            let products = (0..count)
                                .map(|product| {
                                    let offset = product * 0x34;
                                    let stock = u16::from_le_bytes(business[0x80 + offset..0x82 + offset].try_into().unwrap());
                                    let target = i16::from_le_bytes(business[0x1b8 + product * 2..0x1ba + product * 2].try_into().unwrap());
                                    let turnover = u16::from_le_bytes(business[0x8a + offset + prior * 2..0x8c + offset + prior * 2].try_into().unwrap());
                                    (stock, target, turnover)
                                })
                                .collect::<Vec<_>>();
                            println!("Scenario11 day{} shop-slot{slot} kind{kind} building{building} category{category} rent{rent} landlord{landlord} products(stock,target,turnover){products:?}",
                                session.world.clock.calendar[4]);
                        }
                        if let Some(slot) = staged_shop_slot {
                            let row = &session.world.ledgers.businesses[slot];
                            let stock = u16::from_le_bytes(row[0x80..0x82].try_into().unwrap());
                            let target = i16::from_le_bytes(row[0x1b8..0x1ba].try_into().unwrap());
                            println!("Scenario11 staged kind21 slot{slot} day{} owner{} stock{stock} target{target} cash{} sales{}",
                                session.world.clock.calendar[4], row[10],
                                session.world.ledgers.players[0].cash, sold(&session));
                        }
                    }
                    if kind_six_probe && previous_hour == 23 && let Some(slot) = kind_six_slot {
                            let row = &session.world.ledgers.businesses[slot];
                            let count = usize::from(session.world.catalog.info.row(6).unwrap()[0x4c]);
                            let prior = (usize::from(row[0x209]) + 3) % 4;
                            let products = (0..count).map(|product| {
                                let offset = product * 0x34;
                                let stock = u16::from_le_bytes(row[0x80 + offset..0x82 + offset].try_into().unwrap());
                                let target = i16::from_le_bytes(row[0x1b8 + product * 2..0x1ba + product * 2].try_into().unwrap());
                                let turnover = u16::from_le_bytes(row[0x8a + offset + prior * 2..0x8c + offset + prior * 2].try_into().unwrap());
                                let price = f32::from_bits(word(row, 0x1c4 + product * 4));
                                (stock, target, turnover, price)
                            }).collect::<Vec<_>>();
                            println!("Scenario11 kind6 day{} shop{slot} owner{} products(stock,target,turnover,price){products:?} human-sales{} rival3-sales{} land-rent{} cash{}",
                                session.world.clock.calendar[4], row[10], sold(&session),
                                word(&session.roster.manager.players[3].bytes, 0x1b4),
                                session.world.ledgers.players[0].ledger(14).unwrap(),
                                session.world.ledgers.players[0].cash);
                    }
                    if deli_probe && previous_hour == 23
                        && (3..=7).contains(&session.world.clock.calendar[4]) {
                        if let Some(slot) = deli_slot {
                            let row = &session.world.ledgers.businesses[slot];
                            let count = usize::from(session.world.catalog.info.row(2).unwrap()[0x4c]);
                            let prior = (usize::from(row[0x209]) + 3) % 4;
                            let products = (0..count).map(|product| {
                                let offset = product * 0x34;
                                let stock = u16::from_le_bytes(row[0x80 + offset..0x82 + offset].try_into().unwrap());
                                let target = i16::from_le_bytes(row[0x1b8 + product * 2..0x1ba + product * 2].try_into().unwrap());
                                let turnover = u16::from_le_bytes(row[0x8a + offset + prior * 2..0x8c + offset + prior * 2].try_into().unwrap());
                                let price = f32::from_bits(word(row, 0x1c4 + product * 4));
                                (stock, target, turnover, price)
                            }).collect::<Vec<_>>();
                            println!("Scenario11 category12 DELI day{} slot{slot} owner{} products(stock,target,turnover,price){products:?} human-sales{} rival3-sales{} land-rent{} cash{}",
                                session.world.clock.calendar[4], row[10], sold(&session),
                                word(&session.roster.manager.players[3].bytes, 0x1b4),
                                session.world.ledgers.players[0].ledger(14).unwrap(),
                                session.world.ledgers.players[0].cash);
                        } else {
                            println!("Scenario11 category12 DELI frontier day{} available{} rights{} quote{:?} cash{} human-sales{} rival3-sales{}",
                                session.world.clock.calendar[4], deli_available, deli_rights_ready,
                                deli_quote, session.world.ledgers.players[0].cash, sold(&session),
                                word(&session.roster.manager.players[3].bytes, 0x1b4));
                        }
                    }
                    if geographic_probe && previous_hour == 23 {
                        println!("Scenario11 two-shop seed day{} category26-owner{} visible-auction-seen{} human-sales{} rival3-sales{} cash{} category0-lease-hours{}",
                            session.world.clock.calendar[4], session.world.categories[26][5],
                            geographic_auction_seen, sold(&session),
                            word(&session.roster.manager.players[3].bytes, 0x1b4),
                            session.world.ledgers.players[0].cash,
                            word(&session.world.categories[0], 0x7c));
                    }
                    if staple_probe && previous_hour == 23
                        && (3..=7).contains(&session.world.clock.calendar[4]) {
                        let products = staple_slots.iter().enumerate().filter_map(|(i, slot)| {
                            let slot = (*slot)?;
                            let row = &session.world.ledgers.businesses[slot];
                            let history = (usize::from(row[0x209]) + 3) % 4;
                            let turnover = u16::from_le_bytes(row[0x8a + history * 2..0x8c + history * 2].try_into().unwrap());
                            let stock = u16::from_le_bytes(row[0x80..0x82].try_into().unwrap());
                            let target = i16::from_le_bytes(row[0x1b8..0x1ba].try_into().unwrap());
                            Some((staple_kinds[i], slot, row[10], stock, target, turnover,
                                f32::from_bits(word(row, 0x1c4))))
                        }).collect::<Vec<_>>();
                        println!("Scenario11 staple day{} kind-slot-owner-stock-target-turnover-price{products:?} human-sales{} rival3-sales{} cash{} land-rent{} lease-hours{}",
                            session.world.clock.calendar[4], sold(&session),
                            word(&session.roster.manager.players[3].bytes, 0x1b4),
                            session.world.ledgers.players[0].cash,
                            session.world.ledgers.players[0].ledger(14).unwrap(),
                            word(&session.world.categories[0], 0x7c));
                    }
                    if probe_shops == 8 && previous_hour == 23
                        && (14..=19).contains(&session.world.clock.calendar[4]) {
                        let rival = &session.roster.manager.players[3];
                        println!("Scenario11 rival3 day{} sales{} cash{} active{} human-sales{}",
                            session.world.clock.calendar[4], word(&rival.bytes, 0x1b4),
                            session.world.ledgers.players[3].cash, rival.bytes[0x10], sold(&session));
                        for (slot, row) in session.world.ledgers.businesses.iter().enumerate()
                            .filter(|(_, row)| row[0x21c] != 0 && matches!(row[10], 1 | 4)) {
                            let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                            let pointer = word(&session.world.buildings[building], 0x40);
                            let category = pointer.checked_sub(0x617628)
                                .filter(|offset| offset % 380 == 0).map(|offset| offset / 380);
                            let kind = word(row, 4) as usize;
                            let count = usize::from(session.world.catalog.info.row(kind).unwrap()[0x4c]);
                            let prior = (usize::from(row[0x209]) + 3) % 4;
                            let products = (0..count).map(|product| {
                                let offset = product * 0x34;
                                let stock = u16::from_le_bytes(row[0x80 + offset..0x82 + offset].try_into().unwrap());
                                let target = i16::from_le_bytes(row[0x1b8 + product * 2..0x1ba + product * 2].try_into().unwrap());
                                let turnover = u16::from_le_bytes(row[0x8a + offset + prior * 2..0x8c + offset + prior * 2].try_into().unwrap());
                                let price = f32::from_bits(word(row, 0x1c4 + product * 4));
                                (stock, target, turnover, price)
                            }).collect::<Vec<_>>();
                            println!("Scenario11 demand day{} owner{} shop{slot} kind{kind} building{building} category{category:?} products(stock,target,turnover,price){products:?}",
                                session.world.clock.calendar[4], row[10]);
                        }
                    }
                }
                if result_dialog.is_some() {
                    break;
                }
                if session.world.clock.calendar[4] * 24 + session.world.clock.hour()
                    >= start_hour + match probe_shops { 2 => 48, 3 | 6 => 24 * 14, 5 | 10 | 11 | 12 | 13 => 24 * 7, _ => 24 * 30 }
                {
                    break;
                }
            }
            println!("Scenario11 bounded terminal day{} hour{} sales{} cash{} result-context{:?} reached{:?}",
                session.world.clock.calendar[4],session.world.clock.hour(),sold(&session),
                session.world.ledgers.players[0].cash,result_dialog.as_ref().map(|dialog| dialog.context),
                session.world.objective.reached);
            if owned_lot_probe && !matches!(probe_shops, 7 | 9) {
                assert_eq!(built, plan.iter().map(|(_, category)| *category as i32).collect::<Vec<_>>(),
                    "original planner did not admit the planned legal shops");
                assert!(lease_cycles >= 1 && rights_activations >= 1);
                assert!(probe_rent_checked);
                assert!(initial_owned_tenants_checked);
                if staged_shop_probe {
                    assert!(staged_shop_built && staged_shop_slot.is_some(),
                        "cash never admitted measured staged distinct-shop quote and renewal reserve");
                    assert!(staged_shop_rent_seen || category_twelve_waiver_seen,
                        "staged external shop never reached original daily land-rent settlement");
                    if category_twelve_probe {
                        assert!(category_twelve_offer_at.is_some() || category_twelve_offer_rejected,
                            "category12 offer never met the measured renewal and operations reserve");
                        if category_twelve_owned {
                            assert!(category_twelve_auction_due.is_some()
                                && category_twelve_auction_seen && category_twelve_waiver_seen,
                                "owned category12 never reached queued, visible, transferred, rent-waived state");
                        } else {
                            assert!(!category_twelve_waiver_seen);
                            println!("Scenario11 category12 legal frontier: offer-rejected{category_twelve_offer_rejected} visible{category_twelve_auction_seen} rival-won{category_twelve_lost} owner{}",
                                session.world.categories[12][5]);
                        }
                    }
                    if deli_probe {
                        assert!(category_twelve_owned && category_twelve_waiver_seen,
                            "legal category12 acquisition never transferred and waived daily land rent");
                        assert!(deli_rights_ready && deli_available_at.is_some(),
                            "original DELI availability or category12 rights never became live");
                        assert!(deli_quote.is_some() && deli_built && deli_slot.is_some(),
                            "legal category12 DELI quote did not fit the measured renewal and operating reserve");
                        assert!(result_dialog.is_none(),
                            "category12 DELI probe ended in an original result before day7");
                        assert_eq!(session.world.objective.reached[0], 0);
                    }
                    if let Some(dialog) = &result_dialog {
                        if dialog.context == 0 {
                            assert!(sold(&session) >= 3000);
                            assert_eq!(session.world.objective.reached[0], 1);
                        } else {
                            assert_eq!(session.world.objective.reached[0], 0);
                        }
                    }
                    if probe_shops == 8 {
                        let dialog = result_dialog.as_ref().expect("same-seed rival frontier did not terminate");
                        assert_eq!(dialog.context, 3);
                        assert_eq!(session.world.objective.reached[3], 1);
                        assert!(sold(&session) < 3000);
                    }
                }
                assert!(sold(&session) > 0);
                if staple_probe {
                    assert_eq!(staple_index, staple_kinds.len(),
                        "observed quotes did not admit three staples above the renewal and operating reserve");
                    assert!(staple_slots.iter().all(Option::is_some),
                        "submitted staple did not reach a linked original business tenant");
                    assert!(result_dialog.is_none(), "staple probe ended in an original result before day7");
                }
                if geographic_probe {
                    assert!(!geographic_auction_seen,
                        "two-shop seed unexpectedly produced a visible category26 auction");
                    assert_eq!(session.world.categories[26][5], 0,
                        "two-shop seed unexpectedly changed category26 ownership");
                    assert!(result_dialog.is_none(),
                        "two-shop seed ended in original result before day7");
                    assert_eq!(session.world.objective.reached[0], 0);
                }
                if probe_shops == 2 {
                    assert!(result_dialog.is_none(), "early owned-lot probe ended before 48 hours");
                    for kind in [20, 11] {
                        let row = session.world.ledgers.businesses.iter().find(|row|
                            row[0x21c] != 0 && row[10] == 1 && word(*row, 4) == kind)
                            .expect("owned-lot shop changed tenant or closed during early probe");
                        let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                        assert_eq!(word(&session.world.buildings[building], 0x40), 0x617628);
                        assert!(u16::from_le_bytes(row[0x18..0x1a].try_into().unwrap()) > 0);
                    }
                } else if session.roster.manager.players[0].bytes[0x10] != 0
                    && session.world.clock.calendar[4] >= 6
                {
                    assert!(lease_cycles >= 2,
                        "living human failed the measured category0 120-hour renewal path");
                }
                assert_eq!(fs::read(&profile).unwrap(), source_bytes);
                assert_eq!(fs::read(&source_profile).unwrap(), source_bytes);
                drop(host);
                let (counts, active) = presenter.join().unwrap();
                assert!(!active);
                assert_eq!(counts[0], counts[1]);
                assert_eq!(counts[1], counts[2]);
                println!("Scenario11 owned category0 probe shops{probe_shops} sales{} day{} hour{} cash{} lease-cycles{lease_cycles} objective-reached{} result-context{:?} GPU auctions{counts:?}",
                    sold(&session), session.world.clock.calendar[4], session.world.clock.hour(),
                    session.world.ledgers.players[0].cash, session.world.objective.reached[0],
                    result_dialog.as_ref().map(|dialog| dialog.context));
                return;
            }
            let dialog = result_dialog.expect("30-day legal Scenario11 sales strategy reached no original result");
            assert_eq!(dialog.context, 0, "non-human or non-objective result ended legal sales strategy");
            assert!(sold(&session) >= 3000);
            assert_eq!(session.world.objective.reached[0], 1);
            assert_eq!((session.world.objective.kind, session.world.objective.target), (6, 3000.));
            assert_eq!(built.len(), plan.len());
            assert!(lease_cycles >= 1 && rights_activations >= 1);
            if staged_shop_probe {
                assert!(staged_shop_built && staged_shop_slot.is_some() && staged_shop_rent_seen);
            }
            if kind_six_probe {
                assert!(kind_six_built && kind_six_slot.is_some() && kind_six_quote.is_some(),
                    "original kind6 was not built under the measured renewal and operating reserve");
            }
            assert_eq!(fs::read(&profile).unwrap(), source_bytes);
            host.human_dialog_reply(
                &mut session,
                &assets,
                crate::human_dialog::Reply {
                    callback: dialog.callback,
                    context: dialog.context,
                    button: 0,
                },
            )
            .unwrap();
            finish_original_win(&mut session, &assets, &mut host, &profile, &source_bytes);
            let mut reloaded = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
            crate::campaign_store::load_progress(&mut reloaded, &profile).unwrap();
            for id in [200001, 200006, 200011] {
                let level = &reloaded.levels[reloaded.find(id).unwrap()];
                assert_eq!(level.attempts(), 1);
                assert!(level.completed()[0]);
            }
            assert_eq!(fs::read(&source_profile).unwrap(), source_bytes);
            drop(host);
            let (counts, active) = presenter.join().unwrap();
            assert!(!active);
            assert_eq!(counts[0], counts[1]);
            assert_eq!(counts[1], counts[2]);
            println!(
                "Natural Scenario11 3000-sales Win once; human sales{}, buildings{}, GPU auctions{counts:?}",
                sold(&session),
                built.len(),
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
