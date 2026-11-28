//! Original Scenario3 objective through legal construction, trading and Win.
//! Setup has a controlled seed; game ledgers, population, objective and clock
//! are never injected. Result callbacks follow original camera/controller edges.
use crate::fresh_city::{CommandAssets, Inputs, Policy, Session, character_names, load_era_styles};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[path = "scenario_eight_play.rs"]
mod scenario_eight_play;
#[path = "scenario_eleven_play.rs"]
mod scenario_eleven_play;
#[path = "scenario_one_play.rs"]
mod scenario_one_play;
#[path = "scenario_seven_play.rs"]
mod scenario_seven_play;
#[path = "scenario_six_play.rs"]
pub(crate) mod scenario_six_play;
#[path = "scenario_twelve_play.rs"]
mod scenario_twelve_play;
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
struct Terms {
    target: crate::business_edit::Target,
    due: u32,
    before: [u32; 3],
    after: [u32; 3],
    checked: bool,
}
struct CitySale {
    business: usize,
    building: usize,
    due: u32,
    checked: bool,
}
fn current(session: &Session<'_>, target: crate::business_edit::Target) -> [u32; 3] {
    let b = &session.world.ledgers.businesses[target.business];
    [
        word(b, 0x1c4),
        u16::from_le_bytes(b[0x1b8..0x1ba].try_into().unwrap()) as u32,
        word(b, 0x1dc),
    ]
}
fn apply_terms(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    target: crate::business_edit::Target,
    checks: &mut Vec<Terms>,
) {
    let before = current(session, target);
    let apartment = word(&session.world.ledgers.businesses[target.business], 4) == 42;
    if apartment {
        for _ in 0..501 {
            crate::business_edit::edit(
                session,
                assets,
                target,
                crate::business_edit::Action::Rent { increase: false },
            )
            .unwrap();
        }
    } else {
        crate::business_edit::edit(
            session,
            assets,
            target,
            crate::business_edit::Action::Price {
                slot: 0,
                increase: false,
            },
        )
        .unwrap();
        crate::business_edit::edit(
            session,
            assets,
            target,
            crate::business_edit::Action::Stock {
                slot: 0,
                increase: false,
            },
        )
        .unwrap();
    }
    let draft = &session.world.ledgers.businesses[target.business];
    let after = [
        word(draft, 0x1ec),
        u16::from_le_bytes(draft[0x1e0..0x1e2].try_into().unwrap()) as u32,
        word(draft, 0x204),
    ];
    assert_ne!(before, after);
    let due = session.world.clock.timing[0] + 25;
    crate::business_edit::edit(session, assets, target, crate::business_edit::Action::Apply)
        .unwrap();
    checks.push(Terms {
        target,
        due,
        before,
        after,
        checked: false,
    });
}
fn check_sales(session: &Session<'_>, image: &openmt2001::pe::PeImage<'_>) -> i32 {
    let sales = session.world.ledgers.players[0].retail_counters[1];
    assert_eq!(
        word(&session.roster.manager.players[0].bytes, 0x1b4) as i32,
        sales
    );
    let history = session
        .world
        .ledgers
        .businesses
        .iter()
        .filter(|b| b[0x21c] != 0 && b[10] == 1 && word(*b, 0) == 0)
        .map(|b| {
            (0..6)
                .flat_map(|slot| {
                    (0..4).map(move |period| {
                        i32::from(i16::from_le_bytes(
                            b[0x82 + slot * 0x34 + period * 2..0x84 + slot * 0x34 + period * 2]
                                .try_into()
                                .unwrap(),
                        ))
                    })
                })
                .sum::<i32>()
        })
        .sum::<i32>();
    assert_eq!(
        sales, history,
        "natural sales counter differs from complete business history rings"
    );
    let financial = crate::player_info::State::from_runtime(
        &session.roster.manager,
        &session.world.ledgers.players,
        session.roster.status.current_player,
    )
    .unwrap();
    let data =
        crate::comparison::Data::from_runtime(&session.world, &financial.records, image, 0x591318)
            .unwrap();
    let mut controls = crate::comparison::State::new(data.objective);
    assert_eq!(controls.mode, 8);
    assert_eq!(controls.label(), 1386);
    controls.cycle(true);
    assert_ne!(controls.mode, 8);
    for _ in 0..9 {
        if controls.mode == 8 {
            break;
        }
        controls.cycle(true);
    }
    assert_eq!(controls.mode, 8);
    assert_eq!(
        data.values(controls.mode)
            .into_iter()
            .find(|(slot, _)| *slot == 0)
            .unwrap()
            .1,
        sales as f32
    );
    sales
}
#[test]
#[ignore = "requires original assets; natural100-sales Scenario3 Win and profile reload"]
fn legal_scenario_three_sales_win_and_profile_reload() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(||{
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
            let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let root=PathBuf::from("work/validation").join(format!("scenario3-human-play-{}-{nonce}",std::process::id()));
            let profile=crate::campaign_store::profile_path(&root,b"Scenario3 Legal Sales");
            assert!(!profile.exists());
            let mut catalog=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
            crate::campaign_store::load_progress(&mut catalog,&profile).unwrap();
            assert!(catalog.levels[catalog.find(200003).unwrap()].unlocked(),"original first campaign entry is locked");
            assert!(!catalog.levels[catalog.find(200004).unwrap()].unlocked());
            assert!(!catalog.levels[catalog.find(200002).unwrap()].unlocked());
            let mut session = Session::initialize_campaign(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO3", difficulty: 0,
                selected_character: 4, profile_name: b"Scenario3 Legal Sales", character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] },catalog,profile.clone()).unwrap();
        let assets = CommandAssets {image:&image,board:&board,file:&file};
        assert_eq!((session.world.objective.kind,session.world.objective.target),(6,100.));
        assert_eq!(session.world.ledgers.residents.active_count,423);
        assert_eq!(session.world.ledgers.players[0].cash,15000.);
        assert_eq!(session.roster.manager.players.len(),1);
        let mut host=crate::live_host::Host::new([0.,20.,0.]);host.set_running(true);
        let plan=[42u32,0,20,21,11,13,26];let mut submitted=0;let mut categories=Vec::new();let mut checks=Vec::new();
        let mut controlled=std::collections::BTreeSet::new();let mut winning_dialog=None;let mut previous_hour=session.world.clock.hour();
        for _ in 0..45_000 {
            if submitted<plan.len()&&session.world.clock.timing[0].is_multiple_of(625)
                &&let Some(category)=submit_legal_business(&mut session,&assets,plan[submitted],&categories){
                submitted+=1;categories.push(category);
            }
            host.frame(&mut session,&assets,40);
            assert!(host.failure.is_none(),"legal Scenario3 stopped: {:?}",host.failure);
            for business in 0..session.world.ledgers.businesses.len(){
                let b=&session.world.ledgers.businesses[business];
                if b[0x21c]!=0&&b[10]==1&&!controlled.contains(&business){
                    let target=crate::business_edit::Target{business,building:usize::from(u16::from_le_bytes(b[8..10].try_into().unwrap()))};
                    apply_terms(&mut session,&assets,target,&mut checks);controlled.insert(business);
                }
            }
            for check in &mut checks {
                if check.checked {continue;}let now=session.world.clock.timing[0];
                if now<check.due {assert_eq!(current(&session,check.target),check.before,"business controls applied early");}
                else {assert_eq!(now,check.due);assert_eq!(current(&session,check.target),check.after);check.checked=true;println!("human business{} controls applied at native25-tick deadline{}",check.target.business,check.due);}
            }
            for prompt in host.take_prompts(&session){
                if let Some(dialog)=prompt.dialog {
                    if dialog.callback==0x48b490 {winning_dialog=Some(dialog);break;}
                    host.human_dialog_reply(&mut session,&assets,crate::human_dialog::Reply{callback:dialog.callback,context:dialog.context,button:if dialog.button_count==2{1}else{0}}).unwrap();
                }else if prompt.dismissal_id!=0 {session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,false).unwrap();}
            }
            if session.world.clock.hour()!=previous_hour {
                previous_hour=session.world.clock.hour();crate::session_validation::assert_live_links(&session);
                let sales=check_sales(&session,&image);
                println!("Scenario3 natural progress tick{} day{} hour{} builds{} sales{sales}/100 cash{} tenants{}",session.world.clock.timing[0],session.world.clock.calendar[4],previous_hour,submitted,session.world.ledgers.players[0].cash,crate::consumer_polls::Poll::from_runtime(&session.world).unwrap().iter().map(|p|u32::from(p.residents)).sum::<u32>());
            }
            if winning_dialog.is_some(){break;}
        }
        let dialog=winning_dialog.expect("bounded legal mixed-shop strategy did not reach original100-sales result");
        assert_eq!(dialog.context,0,"natural Scenario3 did not produce human winner");
        assert_eq!(session.world.objective.reached[0],1,"Scenario3 survival result did not prove the sales objective");
        assert!(check_sales(&session,&image)>=100);assert!(checks.iter().all(|c|c.checked));
        assert_eq!((session.world.objective.kind,session.world.objective.target),(6,100.));
        let financial=crate::player_info::State::from_runtime(&session.roster.manager,&session.world.ledgers.players,session.roster.status.current_player).unwrap();
        crate::comparison::Data::verify_runtime(&session.world,&financial.records,&image,"SCENARIO3",0);
        assert_eq!(session.catalog.selected_level().unwrap().attempts(),0);assert!(!profile.exists());
        host.human_dialog_reply(&mut session,&assets,crate::human_dialog::Reply{callback:dialog.callback,context:dialog.context,button:0}).unwrap();
        assert_eq!(host.result_sequence(),Some(openmt2001::outcome_notice::Sequence::Win));
        assert!(host.complete_result_sequence(&mut session).is_err());assert!(!profile.exists());
        let state=session.result_presentation().unwrap().clone();
        let installer=Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap().parent().unwrap().to_path_buf();
        let flyby=state.flyby.as_ref().unwrap();let name=flyby.rsplit('/').next().unwrap();
        let camera_bytes=fs::read(installer.join("Standard/Cut_scenes").join(name)).unwrap();
        let camera=openmt2001::flyby::Flyby::parse(&camera_bytes).unwrap();
        let mut controller=crate::result_controller::Controller::new(state.bypass_enabled,state.credits,state.credit_rows);
        controller.set_identifier(state.cut_scene_identifier);
        for frame in 1..camera.frame_count(){camera.sample(frame).unwrap();assert!(!controller.tick(false));}
        session.dispatch_result_event("FlybyEnded",&assets).unwrap();
        let state=session.result_presentation().unwrap();assert_eq!(state.cut_scene_identifier,Some(1));
        controller.set_identifier(state.cut_scene_identifier);controller.set_bypass(state.bypass_enabled);
        assert!(host.complete_result_sequence(&mut session).is_err());assert!(!profile.exists());
        let mut ended=false;
        for tick in 0..200 {if controller.tick(tick==0){ended=true;break;}}
        assert!(ended,"original result controller did not emit its bypass completion edge");
        session.dispatch_result_event("CutSceneEnded",&assets).unwrap();
        assert_eq!(session.result_presentation().unwrap().end_requested,Some(1));assert!(!profile.exists());
        assert_eq!(host.complete_result_sequence(&mut session).unwrap(),openmt2001::outcome_notice::Sequence::Win);
        let bytes=fs::read(&profile).unwrap();assert!(host.complete_result_sequence(&mut session).is_err());assert_eq!(fs::read(&profile).unwrap(),bytes);
        let mut reloaded=crate::campaign_store::load_catalog(&image,&scripts).unwrap();crate::campaign_store::load_progress(&mut reloaded,&profile).unwrap();
        let level=&reloaded.levels[reloaded.find(200003).unwrap()];assert_eq!(level.attempts(),1);assert!(level.completed()[0]);
        assert!(reloaded.levels[reloaded.find(200004).unwrap()].unlocked());assert!(!reloaded.levels[reloaded.find(200002).unwrap()].unlocked());
        println!("Natural Scenario3 Win exactly once: builds{submitted}, sales{}, profile{}, nextScenario4 unlocked; originalScenario2 remains locked behind4/5",session.world.ledgers.players[0].retail_counters[1],profile.display());
    }).unwrap().join().unwrap();
}
fn submit_legal_business(
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    business: u32,
    excluded: &[i32],
) -> Option<i32> {
    submit_business_shape(session, assets, business, excluded, None)
}
fn submit_business_shape(
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    business: u32,
    excluded: &[i32],
    shape: Option<(i32, i32)>,
) -> Option<i32> {
    let info = session
        .world
        .catalog
        .info
        .row(business as usize)
        .unwrap()
        .to_vec();
    let word = |at| i32::from_le_bytes(info[at..at + 4].try_into().unwrap());
    let kind = word(0xc) as u32;
    if info[0x30] == 0 {
        return None;
    }
    let default_height = if business == 42 {
        word(0x44)
    } else {
        (word(0x48)
            + match business {
                13 | 26 => 2,
                20 => 1,
                _ => 0,
            })
        .min(word(0x44))
    };
    let height = shape.map_or(default_height, |(_, height)| height);
    let minimum_area = shape.map_or(word(0x38), |(area, _)| area.max(word(0x38)));
    let dimensions = if business == 42 {
        (1..=6).rev().collect::<Vec<_>>()
    } else {
        (1..=6).collect()
    };
    for category in 0..52 {
        if excluded.contains(&category)
            || !matches!(
                crate::build_panel::Context::from_session(session, category as usize)
                    .unwrap()
                    .entry(),
                crate::build_panel::Entry::Open
            )
        {
            continue;
        }
        for &width in &dimensions {
            for &depth in &dimensions {
                if width * depth < minimum_area
                    || width * depth > word(0x3c)
                    || width.max(depth) > word(0x40)
                {
                    continue;
                }
                for x in 0..=7 - width {
                    for y in 0..=7 - depth {
                        let request = crate::build_panel::Request {
                            category,
                            origin: [x, y],
                            size: [width, depth],
                            height,
                            packed_model: 0,
                            kind,
                            quality: 0,
                            displayed_quote: 0.,
                            business,
                        };
                        let inspected = session.inspect_build(&request, assets).unwrap();
                        if !inspected.allowed()
                            || session.world.ledgers.players[0].cash - inspected.total < 1500.
                        {
                            continue;
                        }
                        let form = crate::build_panel::Form {
                            category,
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
                                .submit_build(&request, assets, |_| Err(
                                    "build applied before native deadline".into()
                                ))
                                .unwrap()
                                .submitted
                        );
                        println!(
                            "legal human business{business} build: category{category} size{width}x{depth} height{height} quote{}",
                            inspected.total
                        );
                        return Some(category);
                    }
                }
            }
        }
    }
    None
}

// Each continuation reads the profile written by the preceding real Win.
// Run these focused gates in campaign order; failure cannot grant an entitlement.
#[test]
#[ignore = "requires original assets and naturally completed Scenario3 profile"]
fn legal_scenario_four_sales_win_and_profile_reload() {
    campaign_continuation("SCENARIO4", 200004, 6, 100., 200005, 15000., 423, 1);
}
#[test]
#[ignore = "requires original assets and naturally completed Scenario4 profile"]
fn legal_scenario_five_sales_win_and_profile_reload() {
    campaign_continuation("SCENARIO5", 200005, 6, 200., 200002, 15000., 423, 2);
}
#[test]
#[ignore = "requires original assets and naturally completed Scenario5 profile"]
fn legal_scenario_two_profit_win_and_profile_reload() {
    campaign_continuation("SCENARIO2", 200002, 3, 3000., 200012, 8000., 483, 3);
}
#[allow(clippy::too_many_arguments)]
fn campaign_continuation(
    scenario: &'static str,
    level_id: u32,
    kind: i32,
    target: f32,
    next_id: u32,
    starting_cash: f32,
    population: u32,
    roster: usize,
) {
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
        let profile = crate::campaign_store::checkpoint_profile(&image, &scripts, level_id);
        let profile_before = fs::read(&profile).unwrap();
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut catalog, &profile).unwrap();
        let level = &catalog.levels[catalog.find(level_id).unwrap()];
        assert!(level.unlocked(), "{scenario} lacks a natural campaign entitlement");
        assert_eq!(level.attempts(), 0, "use the preceding checkpoint profile, not an already completed level");
        assert!(!catalog.levels[catalog.find(next_id).unwrap()].unlocked());
        let mut session = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario, difficulty: 0,
            selected_character: 4, profile_name: b"Scenario3 Legal Sales", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }, catalog, profile.clone()).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        assert_eq!((session.world.objective.kind, session.world.objective.target), (kind, target));
        assert_eq!(session.world.ledgers.residents.active_count, population);
        assert_eq!(session.world.ledgers.players[0].cash, starting_cash);
        assert_eq!(session.roster.manager.players.len(), roster);
        let mut host = crate::live_host::Host::new([0., 20., 0.]); host.set_running(true);
        let presenter = if kind == 3 {
            let (sender, receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            host.auction_renderer = Some(sender);
            Some(std::thread::spawn(move || {
                let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
                let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
                let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
                let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view = crate::auction_bridge::View::default(); let mut counts = [0; 3];
                while let Ok(request) = receiver.recv() {
                    let operation = match &request.operation {
                        crate::auction_bridge::Operation::Prepare { .. } => 0,
                        crate::auction_bridge::Operation::Activate => 1,
                        crate::auction_bridge::Operation::Restore => 2,
                    };
                    let result = view.present(request.operation, &renderer, (&exe, &directory, &archive), 0);
                    if result.is_ok() { counts[operation] += 1; }
                    request.reply.send(result).unwrap();
                }
                (counts, view.scene.is_some())
            }))
        } else { None };
        let plan: &[u32] = if kind == 3 { &[11, 26, 13, 20, 21, 42] } else { &[42, 0, 20, 21, 11, 13, 26] };
        let mut submitted = 0; let mut categories = Vec::new(); let mut checks = Vec::new();
        let mut controlled = std::collections::BTreeSet::new(); let mut winning_dialog = None;
        let mut previous_hour = session.world.clock.hour();
        let mut validated_owned_shopper = false;
        let mut lease_requested = std::collections::BTreeSet::new();
        let mut leases_acquired = std::collections::BTreeSet::new();
        let mut lease_deadlines = std::collections::BTreeMap::new();
        let mut pending_lease: Option<(u32, u32, bool)> = None;
        let mut sales_to_city = Vec::new();
        let mut sale_ledger: Option<(u32, f32)> = None;
        let mut sale_ledger_checked = false;
        for _ in 0..45_000 {
            if submitted < plan.len() && session.world.clock.timing[0].is_multiple_of(625) {
                let built = if kind == 3 {
                    let business = plan[submitted];
                    // Original tips recommend separate clusters for day/night.
                    // Earlier actual stockouts and native budget scores support
                    // Clothes capacity81 and Jewellery27 at modest higher prices.
                    let excluded = match business {
                        11 | 26 => Vec::new(),
                        13 | 20 | 21 => categories[..submitted.min(2)].to_vec(),
                        _ => categories.clone(),
                    };
                    let shape = match business { 11 => Some((9, 3)), 26 => Some((6, 3)), _ => None };
                    submit_business_shape(&mut session, &assets, business, &excluded, shape)
                } else { submit_legal_business(&mut session, &assets, plan[submitted], &categories) };
                if let Some(category) = built { submitted += 1; categories.push(category); }
            }
            host.frame(&mut session, &assets, 40);
            assert!(host.failure.is_none(), "legal {scenario} stopped: {:?}", host.failure);
            if !validated_owned_shopper {
                validated_owned_shopper = validate_natural_owned_shoppers(&session, &image);
            }
            let status = host.status(&session);
            if kind == 3 && sale_ledger.is_none() && session.world.clock.calendar[4] == 1
                && session.world.clock.hour() == 18 && leases_acquired.len() == 2
                && !status.auction.as_ref().is_some_and(|a| a.visible)
            {
                // Actual evening stock reports show these oversized daytime
                // stores sell very few units. Original Trade sells to the City
                // and releases their original4643e0 capital value; no artificial
                // cash or ownership is needed to fund the apartment auction.
                let mut expected = session.world.ledgers.players[0].ledger(0x18).unwrap();
                let due = session.world.clock.timing[0] + 25;
                for business in (0..600).filter(|&i| session.world.ledgers.businesses[i][0x21c] != 0
                    && session.world.ledgers.businesses[i][10] == 1
                    && matches!(word(&session.world.ledgers.businesses[i], 4), 11 | 26)).collect::<Vec<_>>()
                {
                    let row = &session.world.ledgers.businesses[business];
                    let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                    let value = openmt2001::ai_actions::liquidation_score::business_value(row,
                        &session.world.buildings, &image).unwrap().to_f32();
                    assert!(value > 0.);
                    assert!(host.business_operation(&mut session, &assets, building,
                        openmt2001::business_controls::Action::Trade).unwrap());
                    assert_eq!(session.world.ledgers.businesses[business][10], 1,
                        "Trade changed ownership before original25-tick sale consumer");
                    expected = (f64::from(expected) + f64::from(value)) as f32;
                    sales_to_city.push(CitySale { business, building, due, checked: false });
                    println!("Scenario2 legal Trade business{business} native sale value{value} due{due}");
                }
                assert_eq!(sales_to_city.len(), 2);
                sale_ledger = Some((due, expected));
            }
            for sale in &mut sales_to_city {
                if sale.checked { continue; }
                let row = &session.world.ledgers.businesses[sale.business];
                if session.world.clock.timing[0] < sale.due {
                    assert_eq!(row[10], 1);
                } else {
                    assert_eq!(session.world.clock.timing[0], sale.due);
                    assert_eq!(row[10], 0);
                    assert_ne!(row[0x21c], 0, "Trade removed rather than retained the City business");
                    assert_eq!(usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap())), sale.building);
                    assert_eq!(word(&session.world.buildings[sale.building], 0x1c) as usize, sale.business);
                    assert_eq!(session.world.buildings[sale.building][0x2d], 0);
                    sale.checked = true;
                    println!("Scenario2 actual City sale business{} reached owner0 at exact deadline{}", sale.business, sale.due);
                }
            }
            if let Some((due, expected)) = sale_ledger
                && !sale_ledger_checked && session.world.clock.timing[0] >= due
            {
                assert_eq!(session.world.ledgers.players[0].ledger(0x18).unwrap(), expected,
                    "actual original Trade proceeds differ from quoted sale ledger");
                sale_ledger_checked = true;
            }
            if kind == 3 {
                for (domain, outer, inner, direct, command) in session.live_commands.snapshot().unwrap() {
                    if let openmt2001::live_commands::Command::Auction { descriptor } = command
                        && word(descriptor, 4) == 0x16 && descriptor[26] == 1
                    {
                        let category = u32::from(u16::from_le_bytes(descriptor[24..26].try_into().unwrap()));
                        assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                        assert_eq!(outer, inner);
                        assert!(!direct);
                        if let std::collections::btree_map::Entry::Vacant(entry) = lease_deadlines.entry(category) {
                            assert_eq!(inner, session.world.clock.timing[0] + 25);
                            entry.insert(inner);
                            println!("Scenario2 original lease transfer category{category} queued at tick{} due{inner} amount{}", session.world.clock.timing[0], word(descriptor, 20));
                        }
                        assert_ne!(session.world.categories[category as usize][5], 1,
                            "lease ownership changed before original transfer deadline");
                    }
                }
            }
            if let Some((category, submitted_tick, seen_visible)) = &mut pending_lease {
                *seen_visible |= status.auction.as_ref().is_some_and(|a| a.visible)
                    && word(&session.world.auction.controller.record, 4) == *category;
                if session.world.categories[*category as usize][5] == 1 {
                    assert!(*seen_visible, "lease ownership bypassed the actual auction");
                    assert!(session.world.clock.timing[0] > *submitted_tick);
                    assert_eq!(session.world.clock.timing[0], *lease_deadlines.get(category).expect("lease has no original queued transfer"));
                    leases_acquired.insert(*category);
                    println!("Scenario2 legal lease acquired category{category} at tick{} cash{}", session.world.clock.timing[0], session.world.ledgers.players[0].cash);
                    pending_lease = None;
                } else if *seen_visible && !status.auction.as_ref().is_some_and(|a| a.visible)
                    && session.world.categories[*category as usize][5] != 0 {
                    println!("Scenario2 legal lease category{category} lost to owner{}", session.world.categories[*category as usize][5]);
                    pending_lease = None;
                }
            }
            // The original Scenario2 advice explicitly recommends leases to
            // avoid Land Rent. Buy the built shop blocks and an authored City
            // tenant block through the original property panel. The actual
            // category16 City tenants pay1312 per natural rent cycle after
            // its four parks transfer to the landlord as original48a8f0 does;
            // original4477e0 quotes1000 and463e1c credits City-tenant rent.
            if kind == 3 && submitted == plan.len()
                && (session.world.clock.calendar[4] > 0 || session.world.clock.hour() >= 20)
                && pending_lease.is_none() && !status.auction.as_ref().is_some_and(|a| a.visible)
            {
                let mut clusters = categories[..categories.len() - 1].iter().copied()
                    .collect::<std::collections::BTreeSet<_>>();
                clusters.insert(16);
                if let Some(category) = clusters.into_iter().find(|category| !lease_requested.contains(category)
                    && session.world.categories[*category as usize][5] == 0)
                {
                    let inspected = host.inspect_property_offer(&mut session, &assets, category as u32,
                        openmt2001::property_offer::Panel::default(), true).unwrap();
                    if inspected.panel.action_visible != 0
                        && session.world.ledgers.players[0].cash >= inspected.panel.amount + 1500.
                    {
                        let before_owner = session.world.categories[category as usize][5];
                        let tick = session.world.clock.timing[0];
                        let result = host.property_offer(&mut session, &assets, category as u32, inspected.panel).unwrap();
                        assert!(result.submitted);
                        assert_eq!(session.world.categories[category as usize][5], before_owner,
                            "property submission changed ownership before auction completion");
                        lease_requested.insert(category);
                        pending_lease = Some((category as u32, tick, false));
                        println!("Scenario2 legal property Bid category{category} opening{} at tick{tick}", result.panel.amount);
                    }
                }
            }
            if status.auction.is_some_and(|a| a.visible && a.controls.buttons_enabled & 8 != 0) {
                let category = word(&session.world.auction.controller.record, 4);
                let price = word(&session.world.auction.controller.record, 0x6c);
                let next = price.saturating_add(50);
                let wanted = pending_lease.is_some_and(|(selected, _, _)| selected == category);
                // The authored City block's actual rents support a larger
                // capital offer, funded by original Trade proceeds.
                // Preserve positive cash alongside original affordability.
                let income_block = category == 16;
                let (ceiling, reserve) = if income_block {
                    ((session.world.ledgers.players[0].cash - 200.).max(0.) as u32, 200.)
                } else { (1800, 800.) };
                let bid = wanted && next <= ceiling
                    && session.world.ledgers.players[0].cash >= next as f32 + reserve;
                let button = if bid { 0 } else { 3 };
                host.auction_button(&mut session, &assets, button).unwrap();
                println!("{scenario} legal human auction button{button} category{category} price{price} at tick{}", session.world.clock.timing[0]);
            }
            for business in 0..session.world.ledgers.businesses.len() {
                let b = &session.world.ledgers.businesses[business];
                if b[0x21c] != 0 && b[10] == 1 && !controlled.contains(&business)
                    && (word(b, 0) == 0 || word(b, 4) == 42)
                {
                    let edit_target = crate::business_edit::Target { business, building: usize::from(u16::from_le_bytes(b[8..10].try_into().unwrap())) };
                    if kind == 3 { apply_profit_terms(&mut session, &assets, edit_target, &mut checks); }
                    else { apply_terms(&mut session, &assets, edit_target, &mut checks); }
                    controlled.insert(business);
                }
            }
            for check in &mut checks {
                if check.checked { continue; }
                let now = session.world.clock.timing[0];
                if now < check.due { assert_eq!(current(&session, check.target), check.before, "business controls applied early"); }
                else { assert_eq!(now, check.due); assert_eq!(current(&session, check.target), check.after); check.checked = true; }
            }
            for prompt in status.prompts {
                if let Some(dialog) = prompt.dialog {
                    if dialog.callback == 0x48b490 { winning_dialog = Some(dialog); break; }
                    host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context, button: if dialog.button_count == 2 { 1 } else { 0 } }).unwrap();
                } else if prompt.dismissal_id != 0 {
                    session.dispatch_session_event("InterfacePromptDismiss", prompt.dismissal_id as i32, &assets, false).unwrap();
                }
            }
            if session.world.clock.hour() != previous_hour {
                previous_hour = session.world.clock.hour(); crate::session_validation::assert_live_links(&session);
                let metric = campaign_metric(&session, &image, kind);
                if kind == 3 && previous_hour == 6 && session.world.clock.calendar[4] > 0 {
                    let chains = openmt2001::building_pool::category_chains(
                        &session.world.categories, &session.world.buildings, 0x591318).unwrap();
                    for b in session.world.ledgers.businesses.iter().filter(|b| b[0x21c] != 0 && b[10] == 1) {
                        let building = usize::from(u16::from_le_bytes(b[8..10].try_into().unwrap()));
                        let category = chains.iter().position(|chain| chain.contains(&building)).unwrap();
                        if leases_acquired.contains(&(category as u32)) {
                            assert_eq!(f32::from_bits(word(b, 0x28)), 0., "owned cluster charged its human tenant Land Rent");
                        }
                    }
                    let mut landlord_income = 0f32;
                    for &category in &leases_acquired {
                        for &building in &chains[category as usize] {
                            let business = word(&session.world.buildings[building], 0x1c) as usize;
                            let Some(row) = session.world.ledgers.businesses.get(business) else { continue; };
                            if row[0x21c] != 0 && row[10] != 1 {
                                landlord_income += f32::from(u16::from_le_bytes(row[0x18..0x1a].try_into().unwrap()));
                            }
                        }
                    }
                    assert_eq!(session.world.ledgers.players[0].ledger(0x22).unwrap(), landlord_income,
                        "natural landlord income differs from actual linked tenant rents");
                    println!("Scenario2 natural rent cycle with legal leases{leases_acquired:?}, daily profit{metric}");
                }
                println!("{scenario} natural tick{} day{} hour{} builds{submitted} objective{metric}/{target} cash{} sales{}", session.world.clock.timing[0], session.world.clock.calendar[4], previous_hour, session.world.ledgers.players[0].cash, session.world.ledgers.players[0].retail_counters[1]);
                if kind == 3 && previous_hour == 23 {
                    let financial = crate::player_info::State::from_runtime(&session.roster.manager, &session.world.ledgers.players, session.roster.status.current_player).unwrap();
                    crate::comparison::Data::verify_runtime(&session.world, &financial.records, &image, &format!("{scenario}-DAY{}", session.world.clock.calendar[4]), 0);
                    export_live_price_scores(&session);
                    for (slot, b) in session.world.ledgers.businesses.iter().enumerate().filter(|(_, b)| b[0x21c] != 0 && b[10] == 1 && word(*b, 0) == 0) {
                        let business = word(b, 4) as usize;
                        let products = (0..usize::from(session.world.catalog.info.row(business).unwrap()[0x4c])).map(|product| {
                            let at = product * 0x34;
                            let period = usize::from(b[0x209]);
                            (product, u16::from_le_bytes(b[at + 0x80..at + 0x82].try_into().unwrap()), i16::from_le_bytes(b[at + 0x82 + period * 2..at + 0x84 + period * 2].try_into().unwrap()), f32::from_bits(word(b, 0x1c4 + product * 4)))
                        }).collect::<Vec<_>>();
                        println!("{scenario} live business{slot} kind{business} product(slot,remainingstock,sales,currentprice){products:?}");
                    }
                    // A first-evening offer enters its original delayed auction
                    // overnight. Include the next rent cycle after acquisitions.
                    if session.world.clock.calendar[4] >= 2 { break; }
                }
            }
            if winning_dialog.is_some() { break; }
        }
        let financial = crate::player_info::State::from_runtime(&session.roster.manager, &session.world.ledgers.players, session.roster.status.current_player).unwrap();
        let snapshot = crate::comparison::Data::verify_runtime(&session.world, &financial.records, &image, scenario, 0);
        println!("{scenario} bounded final metrics{:?}", snapshot.rows);
        let dialog = winning_dialog.expect("bounded legal campaign strategy did not reach an original result");
        assert!(validated_owned_shopper, "campaign did not exercise visible shoppers at owned businesses");
        println!("{scenario} original outcome context{} metrics{:?}", dialog.context, snapshot.rows);
        assert_eq!(dialog.context, 0, "natural campaign result was won by an opponent");
        assert_eq!(session.world.objective.reached[0], 1,
            "natural campaign survival result did not prove its objective");
        assert!(campaign_metric(&session, &image, kind) >= target);
        assert!(checks.iter().all(|c| c.checked));
        assert_eq!((session.world.objective.kind, session.world.objective.target), (kind, target));
        assert_eq!(fs::read(&profile).unwrap(), profile_before);
        host.human_dialog_reply(&mut session, &assets, crate::human_dialog::Reply { callback: dialog.callback, context: dialog.context, button: 0 }).unwrap();
        finish_original_win(&mut session, &assets, &mut host, &profile, &profile_before);
        let mut reloaded = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut reloaded, &profile).unwrap();
        let level = &reloaded.levels[reloaded.find(level_id).unwrap()];
        assert_eq!(level.attempts(), 1); assert!(level.completed()[0]);
        for completed in [200003, 200004, 200005, 200002].into_iter().take(match level_id { 200004 => 2, 200005 => 3, _ => 4 }) {
            let previous = &reloaded.levels[reloaded.find(completed).unwrap()];
            assert_eq!(previous.attempts(), 1); assert!(previous.completed()[0]);
        }
        assert!(reloaded.levels[reloaded.find(next_id).unwrap()].unlocked());
        println!("Natural {scenario} Win exactly once; next{next_id} unlocked on same profile{}", profile.display());
        drop(host);
        if let Some(presenter) = presenter {
            let (counts, active_scene) = presenter.join().unwrap();
            assert!(!active_scene); assert_eq!(counts[0], counts[1]); assert_eq!(counts[1], counts[2]);
            println!("{scenario} real original auction scenes prepared/activated/restored{counts:?}");
        }
    }).unwrap().join().unwrap();
}
// Raw live inputs plus host query expectations for a separate original-code
// replay. Candidate prices and an open store are controlled query inputs;
// these cloned records never enter the live campaign or its ledgers.
fn export_live_price_scores(session: &Session<'_>) {
    let Ok(prefix) = std::env::var("OPENMT2001_FRESH_COMPARISON_ORACLE") else {
        return;
    };
    let mut bytes = b"MTSCPRC1".to_vec();
    let count = session.world.ledgers.residents.active_count as usize;
    bytes.extend((count as u32).to_le_bytes());
    for node in &session.world.ledgers.residents.nodes[..count] {
        bytes.extend(node.as_bytes());
    }
    assert_eq!(session.world.ledgers.residents.accounts.len(), 2000);
    for account in &session.world.ledgers.residents.accounts {
        bytes.extend(account.as_bytes());
    }
    for commodity in &session.world.commodities.rows {
        bytes.extend(commodity);
    }
    for kind in 0..45 {
        bytes.extend(session.world.catalog.info.row(kind).unwrap());
    }
    for catalog in &session.world.catalog.products {
        for product in catalog {
            for value in product {
                bytes.extend(value.to_le_bytes());
            }
        }
    }
    for class in session.world.configuration.class_weights.rows {
        for weight in class {
            bytes.extend(weight.to_le_bytes());
        }
    }
    let mut categories = [None; 600];
    let chains = openmt2001::building_pool::category_chains(
        &session.world.categories,
        &session.world.buildings,
        0x591318,
    )
    .unwrap();
    for (category, chain) in chains.iter().enumerate() {
        for &building in chain {
            categories[building] = Some(category);
        }
    }
    let commodities = session.world.commodities.resident_inputs();
    let mut cases = Vec::new();
    let mut queries = 0u32;
    for (business, row) in session
        .world
        .ledgers
        .businesses
        .iter()
        .enumerate()
        .filter(|(_, b)| b[0x21c] != 0 && b[10] == 1 && word(*b, 0) == 0)
    {
        let kind = word(row, 4) as usize;
        let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
        let category = categories[building].unwrap();
        let factor = f32::from_bits(word(&session.world.categories[category], 0x84));
        let catalog: Vec<_> = session.world.catalog.products[kind]
            [..usize::from(session.world.catalog.info.row(kind).unwrap()[0x4c])]
            .iter()
            .map(|p| p[0])
            .collect();
        for (slot, &commodity) in catalog.iter().enumerate() {
            let item = commodities[commodity as usize];
            for fraction in [0.9f32, 1., 1.05, 1.1, 1.2, 1.5, 2.] {
                let price =
                    ((f64::from(item.value) * f64::from(item.markup)) * f64::from(fraction)) as f32;
                let mut candidate = *row;
                candidate[0x6c..0x70].fill(0); // Explicit open-store query seam.
                candidate[0x1c4 + slot * 4..0x1c8 + slot * 4].copy_from_slice(&price.to_le_bytes());
                for (resident, node) in session.world.ledgers.residents.nodes[..count]
                    .iter()
                    .enumerate()
                {
                    let class = word(node.as_bytes(), 0x3c) as usize;
                    let weights = session.world.configuration.class_weights.rows[class];
                    let score = node
                        .score_purchase_candidate(
                            &candidate,
                            &session.world.ledgers.residents.accounts,
                            openmt2001::resident::PurchaseCandidateInputs {
                                commodity: commodity as usize,
                                value: item.value,
                                markup: item.markup,
                                building_attribute: session.world.buildings[building][0x2c] as i8,
                                weights: [weights[0], weights[1], weights[2]],
                            },
                            |b, id| {
                                Ok(f64::from(
                                    openmt2001::resident::purchase_price(b, &catalog, id as u32)
                                        .unwrap(),
                                ))
                            },
                            |_| Ok(factor),
                        )
                        .unwrap();
                    for value in [
                        resident as u32,
                        business as u32,
                        commodity,
                        price.to_bits(),
                        score.ratio.is_some() as u32,
                        score.ratio.unwrap_or(0.).to_bits(),
                        score.category_factor.is_some() as u32,
                        score.category_factor.unwrap_or(0.).to_bits(),
                    ] {
                        cases.extend(value.to_le_bytes());
                    }
                    cases.extend(score.value.to_le_bytes());
                    let integer = if score.value.is_finite() {
                        openmt2001::x87::Sum::from_f64(score.value)
                            .unwrap()
                            .ftol_low_word()
                    } else {
                        0
                    };
                    cases.extend(integer.to_le_bytes());
                    queries += 1;
                }
            }
        }
    }
    for category in categories {
        bytes.extend(category.map_or(-1, |c| c as i16).to_le_bytes());
    }
    bytes.extend(queries.to_le_bytes());
    bytes.extend(cases);
    fs::write(
        format!("{prefix}-PRICE-DAY{}.bin", session.world.clock.calendar[4]),
        bytes,
    )
    .unwrap();
    println!(
        "Scenario2 exports{queries} controlled candidate-price queries over{count} actual natural residents"
    );
}
fn validate_natural_owned_shoppers(
    session: &Session<'_>,
    image: &openmt2001::pe::PeImage<'_>,
) -> bool {
    let shopping_owner = |node: &[u8; 0x14a]| -> Option<usize> {
        if node[0x54] == 0xff {
            return None;
        }
        let building =
            usize::try_from(i16::from_le_bytes(node[0x56..0x58].try_into().unwrap())).ok()?;
        let business = word(session.world.buildings.get(building)?, 0x1c) as usize;
        let owner = session.world.ledgers.businesses.get(business)?[10] as i8;
        usize::try_from(i16::from(owner) - 1).ok()
    };
    let nodes = &session.world.ledgers.residents.nodes
        [..session.world.ledgers.residents.active_count as usize];
    if !nodes
        .iter()
        .zip(session.world.creation.auxiliary.slots())
        .any(|(node, auxiliary)| {
            auxiliary[0xb8] & 1 != 0 && shopping_owner(node.as_bytes()).is_some()
        })
    {
        return false;
    }
    let report =
        crate::city_people::Report::from_world(&session.world, image, &session.roster.manager)
            .unwrap();
    assert!(report.initialized);
    assert_eq!(report.rows[0].value, nodes.len() as i32);
    let mut count = 0;
    for (node, marker) in nodes.iter().zip(&report.markers) {
        if marker.visible
            && let Some(owner) = shopping_owner(node.as_bytes())
        {
            let character = word(&session.roster.manager.players[owner].bytes, 0x150);
            assert_eq!(
                marker.colors[0],
                word(image.at(0x541af0 + character * 40, 4).unwrap(), 0)
            );
            count += 1;
        }
    }
    assert!(count > 0);
    println!(
        "NaturalCityPeople report validates{count} visible owned shoppers at tick{}",
        session.world.clock.timing[0]
    );
    true
}
// Original native price/budget scores permit about20% higher prices for enough
// real residents to fill these capacities; doubling eliminated all purchases.
// This is a bounded human strategy using actual arrows, not a price-field edit.
fn apply_profit_terms(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    target: crate::business_edit::Target,
    checks: &mut Vec<Terms>,
) {
    let business = word(&session.world.ledgers.businesses[target.business], 4);
    if matches!(business, 0 | 42) {
        apply_terms(session, assets, target, checks);
        return;
    }
    let before = current(session, target);
    let price = f32::from_bits(before[0]);
    let commodity = session.world.catalog.products[business as usize][0][0] as usize;
    let item = session.world.commodities.resident_inputs()[commodity];
    let desired_tenths = (f64::from(item.value) * f64::from(item.markup) * 12.).floor() as usize;
    let clicks = desired_tenths.saturating_sub((price * 10.).round() as usize);
    for _ in 0..clicks {
        crate::business_edit::edit(
            session,
            assets,
            target,
            crate::business_edit::Action::Price {
                slot: 0,
                increase: true,
            },
        )
        .unwrap();
    }
    crate::business_edit::edit(
        session,
        assets,
        target,
        crate::business_edit::Action::Stock {
            slot: 0,
            increase: false,
        },
    )
    .unwrap();
    let draft = &session.world.ledgers.businesses[target.business];
    let after = [
        word(draft, 0x1ec),
        u16::from_le_bytes(draft[0x1e0..0x1e2].try_into().unwrap()) as u32,
        word(draft, 0x204),
    ];
    assert!(f32::from_bits(after[0]) > price);
    let due = session.world.clock.timing[0] + 25;
    crate::business_edit::edit(session, assets, target, crate::business_edit::Action::Apply)
        .unwrap();
    checks.push(Terms {
        target,
        due,
        before,
        after,
        checked: false,
    });
    println!(
        "Scenario2 legal business{} price{price}→{} through{clicks} original+arrows; Applydue{due}",
        target.business,
        f32::from_bits(after[0])
    );
}
fn campaign_metric(session: &Session<'_>, image: &openmt2001::pe::PeImage<'_>, kind: i32) -> f32 {
    if kind == 6 {
        return check_sales(session, image) as f32;
    }
    let financial = crate::player_info::State::from_runtime(
        &session.roster.manager,
        &session.world.ledgers.players,
        session.roster.status.current_player,
    )
    .unwrap();
    let data =
        crate::comparison::Data::from_runtime(&session.world, &financial.records, image, 0x591318)
            .unwrap();
    let controls = crate::comparison::State::new(kind);
    assert_eq!(controls.mode, 3);
    assert_eq!(controls.label(), 1533);
    data.values(controls.mode)
        .into_iter()
        .find(|(slot, _)| *slot == 0)
        .unwrap()
        .1
}
fn finish_original_win(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    host: &mut crate::live_host::Host,
    profile: &Path,
    before: &[u8],
) {
    assert_eq!(
        host.result_sequence(),
        Some(openmt2001::outcome_notice::Sequence::Win)
    );
    assert!(host.complete_result_sequence(session).is_err());
    assert_eq!(fs::read(profile).unwrap(), before);
    let state = session.result_presentation().unwrap().clone();
    let installer = Path::new(&openmt2001::fixtures::path("PARAMETERS"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let flyby = state.flyby.as_ref().unwrap();
    let name = flyby.rsplit('/').next().unwrap();
    let camera_bytes = fs::read(installer.join("Standard/Cut_scenes").join(name)).unwrap();
    let camera = openmt2001::flyby::Flyby::parse(&camera_bytes).unwrap();
    let mut controller = crate::result_controller::Controller::new(
        state.bypass_enabled,
        state.credits,
        state.credit_rows,
    );
    controller.set_identifier(state.cut_scene_identifier);
    for frame in 1..camera.frame_count() {
        camera.sample(frame).unwrap();
        assert!(!controller.tick(false));
    }
    session.dispatch_result_event("FlybyEnded", assets).unwrap();
    let state = session.result_presentation().unwrap();
    assert_eq!(state.cut_scene_identifier, Some(1));
    controller.set_identifier(state.cut_scene_identifier);
    controller.set_bypass(state.bypass_enabled);
    assert!(host.complete_result_sequence(session).is_err());
    assert_eq!(fs::read(profile).unwrap(), before);
    let mut ended = false;
    for tick in 0..200 {
        if controller.tick(tick == 0) {
            ended = true;
            break;
        }
    }
    assert!(
        ended,
        "original result controller did not emit its bypass completion edge"
    );
    host.dispatch_result_event(session, assets, "CutSceneEnded")
        .unwrap();
    assert_eq!(
        session.result_presentation().unwrap().end_requested,
        Some(1)
    );
    assert_eq!(fs::read(profile).unwrap(), before);
    let stopped = host.status(session);
    assert!(!stopped.running);
    assert!(stopped.failure.is_none());
    let pause = session.world.pause.bytes;
    let expected_stats = crate::stats_panel::State::show(
        session.world.objective.kind,
        false,
        session.roster.manager.players.len(),
    )
    .unwrap();
    host.stats_action(session, crate::live_host::StatsAction::Open)
        .unwrap();
    assert_eq!(session.world.pause.bytes, pause);
    let opened = host.status(session);
    assert!(!opened.running);
    assert!(opened.failure.is_none());
    assert_eq!(opened.stats.unwrap().state, expected_stats);
    host.stats_action(session, crate::live_host::StatsAction::Close)
        .unwrap();
    assert_eq!(session.world.pause.bytes, pause);
    let closed = host.status(session);
    assert!(!closed.running);
    assert!(closed.failure.is_none());
    assert!(closed.stats.is_none());
    assert_eq!(
        host.complete_result_sequence(session).unwrap(),
        openmt2001::outcome_notice::Sequence::Win
    );
    let bytes = fs::read(profile).unwrap();
    assert_ne!(bytes, before);
    assert!(host.complete_result_sequence(session).is_err());
    assert_eq!(fs::read(profile).unwrap(), bytes);
}
