//! Legal affordable housing, queued rent eviction and natural rehousing.
//! Scenario3 setup uses original inputs with a controlled seed; tenants,
//! construction, rent controls and clock progression follow production paths.
//! Full native terms captures isolate the command primitive on cloned real
//! pools; the Host checks its actual admission deadline and later daily cycle.
use crate::fresh_city::{CommandAssets, Inputs, Policy, Session, character_names, load_era_styles};
use openmt2001::{
    ai_demand,
    economy::{BusinessTerms, PlayerAccount, ResidentAccounts},
    housing,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn occupied(session: &Session<'_>, target: crate::business_edit::Target) -> u16 {
    u16::from_le_bytes(
        session.world.ledgers.businesses[target.business][0x10..0x12]
            .try_into()
            .unwrap(),
    )
}
fn assert_apartment_members(session: &Session<'_>, target: crate::business_edit::Target) {
    let mut members = std::collections::BTreeSet::new();
    let mut next = word(&session.world.ledgers.businesses[target.business], 0x20c) as i32;
    while next != -1 {
        let id = usize::try_from(next).expect("invalid tenant link");
        assert!(members.insert(id), "cyclic apartment member chain");
        let resident = &session.world.ledgers.residents.nodes[id];
        assert_eq!(resident.housing_building(), target.building as i16);
        assert_eq!(
            resident.housing_category(),
            session.population.housing.buildings[target.building].category
        );
        next = resident.next_in_housing();
    }
    let housed = session
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, r)| r.housing_building() == target.building as i16)
        .map(|(id, _)| id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        members, housed,
        "apartment list differs from resident home references"
    );
    assert_eq!(members.len(), usize::from(occupied(session, target)));
}
fn frame(
    session: &mut Session<'_>,
    host: &mut crate::live_host::Host,
    assets: &CommandAssets<'_, '_>,
) {
    host.frame(session, assets, 40);
    assert!(
        host.failure.is_none(),
        "rental city stopped: {:?}",
        host.failure
    );
    for prompt in host.take_prompts(session) {
        if let Some(dialog) = prompt.dialog {
            assert_ne!(
                dialog.callback, 0x48b490,
                "unexpected rental-city terminal result"
            );
            host.human_dialog_reply(
                session,
                assets,
                crate::human_dialog::Reply {
                    callback: dialog.callback,
                    context: dialog.context,
                    button: if dialog.button_count == 2 { 1 } else { 0 },
                },
            )
            .unwrap();
        } else if prompt.dismissal_id != 0 {
            session
                .dispatch_session_event(
                    "InterfacePromptDismiss",
                    prompt.dismissal_id as i32,
                    assets,
                    false,
                )
                .unwrap();
        }
    }
}
struct Pools {
    businesses: Vec<[u8; 544]>,
    residents: ResidentAccounts,
    players: Vec<PlayerAccount>,
    available: Vec<usize>,
    demand: ai_demand::State,
    rng: u32,
}
impl Pools {
    fn from_session(session: &Session<'_>) -> Self {
        Self {
            businesses: session.world.ledgers.businesses.clone(),
            residents: ResidentAccounts {
                active_count: session.world.ledgers.residents.active_count,
                nodes: session.world.ledgers.residents.nodes.clone(),
                accounts: session.world.ledgers.residents.accounts.clone(),
            },
            players: session.world.ledgers.players.clone(),
            available: session.world.available.clone(),
            demand: ai_demand::State {
                rows: session.world.ai_demand.rows,
                ready: session.world.ai_demand.ready,
            },
            rng: session.world.creation.initialization_rng.objective,
        }
    }
    fn encode(&self, session: &Session<'_>, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.available.len() as u32).to_le_bytes());
        for &id in &self.available {
            out.extend_from_slice(&(id as u32).to_le_bytes());
        }
        for row in &self.businesses {
            out.extend_from_slice(row);
        }
        for resident in &self.residents.nodes {
            out.extend_from_slice(resident.as_bytes());
            out.push(resident.active_byte());
        }
        for account in &self.residents.accounts {
            out.extend_from_slice(account.as_bytes());
        }
        for (account, player) in self.players.iter().zip(&session.roster.manager.players) {
            let mut bytes = player.bytes.clone();
            account.write_runtime(&mut bytes).unwrap();
            assert_eq!(bytes.len(), 0x224);
            out.extend_from_slice(&bytes);
        }
        for row in &self.demand.rows {
            out.extend_from_slice(row);
        }
        out.push(self.demand.ready);
    }
}
fn shadow_terms(
    session: &Session<'_>,
    target: crate::business_edit::Target,
    terms: &BusinessTerms,
    out: &mut Vec<u8>,
) -> Pools {
    let before = Pools::from_session(session);
    let mut after = Pools::from_session(session);
    let mut no_average =
        |_: &[[u8; 544]]| Err("no affordable alternative should request market average".into());
    let mut world = housing::World {
        demand: Some(&mut after.demand),
        buildings: &session.population.housing.buildings,
        categories: &session.population.housing.categories,
        category_values: &session.population.shopping.category_factors,
        businesses: &mut after.businesses,
        available: &mut after.available,
        players: &mut after.players,
        welfare: &session.world.welfare,
        settings: &session.population.housing_settings,
        average_rent: &mut no_average,
    };
    housing::apply_business_terms(
        &mut after.residents,
        &mut world,
        target.business,
        terms,
        &session.world.configuration.objectives,
        &mut after.rng,
    )
    .unwrap();
    for n in [
        target.business as u32,
        before.residents.active_count,
        before.players.len() as u32,
        before.rng,
        after.rng,
    ] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    out.extend_from_slice(
        &session
            .population
            .housing_settings
            .affordability_factor
            .to_le_bytes(),
    );
    for row in session.world.welfare.amounts {
        for value in row {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    for row in session.population.housing_settings.weights {
        for value in row {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    for n in terms.stock_targets {
        out.extend_from_slice(&n.to_le_bytes());
    }
    for value in terms.prices {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&terms.rent.to_le_bytes());
    for row in &session.world.categories {
        out.extend_from_slice(row);
    }
    for row in &session.world.buildings {
        out.extend_from_slice(row);
    }
    before.encode(session, out);
    after.encode(session, out);
    after
}
fn pending_terms(session: &Session<'_>, target: crate::business_edit::Target) -> BusinessTerms {
    let row = &session.world.ledgers.businesses[target.business];
    BusinessTerms {
        stock_targets: std::array::from_fn(|i| {
            i16::from_le_bytes(row[0x1e0 + i * 2..0x1e2 + i * 2].try_into().unwrap())
        }),
        prices: std::array::from_fn(|i| f32::from_bits(word(row, 0x1ec + i * 4))),
        rent: f32::from_bits(word(row, 0x204)),
    }
}
fn delayed_terms(
    session: &mut Session<'_>,
    host: &mut crate::live_host::Host,
    assets: &CommandAssets<'_, '_>,
    target: crate::business_edit::Target,
    increases: bool,
    steps: usize,
    export: &mut Vec<u8>,
) {
    let old_rent = word(&session.world.ledgers.businesses[target.business], 0x1dc);
    let old_tenants = occupied(session, target);
    for _ in 0..steps {
        crate::business_edit::edit(
            session,
            assets,
            target,
            crate::business_edit::Action::Rent {
                increase: increases,
            },
        )
        .unwrap();
    }
    let terms = pending_terms(session, target);
    let due = session.world.clock.timing[0] + 25;
    crate::business_edit::edit(session, assets, target, crate::business_edit::Action::Apply)
        .unwrap();
    for _ in 0..24 {
        frame(session, host, assets);
        assert_eq!(
            word(&session.world.ledgers.businesses[target.business], 0x1dc),
            old_rent,
            "rent changed before native25-tick deadline"
        );
        assert_eq!(occupied(session, target), old_tenants);
    }
    assert_eq!(session.world.clock.timing[0], due - 1);
    let expected = shadow_terms(session, target, &terms, export);
    let accounts = session
        .world
        .ledgers
        .residents
        .accounts
        .iter()
        .map(|r| *r.as_bytes())
        .collect::<Vec<_>>();
    frame(session, host, assets);
    assert_eq!(session.world.clock.timing[0], due);
    assert_eq!(
        word(&session.world.ledgers.businesses[target.business], 0x1dc),
        terms.rent.to_bits()
    );
    assert_eq!(
        occupied(session, target),
        u16::from_le_bytes(
            expected.businesses[target.business][0x10..0x12]
                .try_into()
                .unwrap()
        )
    );
    assert_eq!(
        session.world.available, expected.available,
        "native deadline did not publish housing availability"
    );
    assert_eq!(
        session.world.ledgers.players[0].housing_counter,
        expected.players[0].housing_counter
    );
    assert_eq!(
        session.world.ai_demand.rows, expected.demand.rows,
        "native deadline lost demand refresh"
    );
    assert_eq!(
        session
            .world
            .ledgers
            .residents
            .accounts
            .iter()
            .map(|r| *r.as_bytes())
            .collect::<Vec<_>>(),
        accounts,
        "terms admission unexpectedly changed resident spending accounts"
    );
    for (actual, want) in session
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .zip(&expected.residents.nodes)
    {
        assert_eq!(
            &actual.as_bytes()[0xc..0x1c],
            &want.as_bytes()[0xc..0x1c],
            "home state diverged at terms deadline"
        );
        assert_eq!(
            &actual.as_bytes()[0xec..0xf0],
            &want.as_bytes()[0xec..0xf0],
            "tenant chain diverged at terms deadline"
        );
    }
    println!(
        "rent {} -> {} applied at exact tick{due}, tenants{} -> {} available{} demand-ready{}",
        f32::from_bits(old_rent),
        terms.rent,
        old_tenants,
        occupied(session, target),
        session.world.available.len(),
        session.world.ai_demand.ready
    );
}
#[test]
#[ignore = "requires original assets; legal rental turnover and full native terms replay"]
fn legal_rental_eviction_rehousing_and_next_income() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
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
            let mut session = Session::initialize(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO3", difficulty: 0,
                selected_character: 4, profile_name: b"Rental turnover validation", character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        submit_legal_business(&mut session,&assets,42,None);
        let mut host = crate::live_host::Host::new([0.,20.,0.]); host.set_running(true);
        let mut target = None;
        for _ in 0..16_000 {
            frame(&mut session,&mut host,&assets);
            if target.is_none() && let Some(business) = session.world.ledgers.businesses.iter().position(|b|b[0x21c]!=0 && b[10]==1 && word(b,4)==42) {
                let t = crate::business_edit::Target { business, building: usize::from(u16::from_le_bytes(session.world.ledgers.businesses[business][8..10].try_into().unwrap())) };
                for _ in 0..501 { crate::business_edit::edit(&mut session,&assets,t,crate::business_edit::Action::Rent { increase:false }).unwrap(); }
                crate::business_edit::edit(&mut session,&assets,t,crate::business_edit::Action::Apply).unwrap(); target=Some(t);
            }
            if session.world.clock.calendar[4]>=1 && session.world.clock.hour()>=7 { break; }
        }
        let target=target.unwrap(); assert_apartment_members(&session,target); assert_eq!(occupied(&session,target),68,"initial legal apartment did not naturally fill");
        // Reuse this legally reached state for the separate save/terms gate.
        // Host camera stays at its initial eye throughout this headless flow.
        let camera=openmt2001::flyby::CameraSample {
            eye:[0.,20.,0.],target:[0.,0.,1.],up:[0.,1.,0.],vertical_fov:0.7,
        };
        let cameras=crate::save_menu::cameras(None,camera).unwrap();
        let bytes=session.save_v2(&image,cameras,1).unwrap();
        let path=crate::save_menu::write_new(Path::new("work/save/legal-rental-profile"),&bytes).unwrap();
        fs::write("work/save/legal-rental68-v3-path.txt",path.to_str().unwrap()).unwrap();
        let initial_income=session.world.ledgers.players[0].ledger(0x21).unwrap(); assert!(initial_income>0.);
        let tenants=session.world.ledgers.residents.nodes.iter().enumerate().filter(|(_,r)|r.housing_building()==target.building as i16).map(|(i,_)|i).collect::<Vec<_>>(); assert_eq!(tenants.len(),68);
        let mut export=b"MTRTURN1".to_vec(); for n in [0x37fu32,2] { export.extend_from_slice(&n.to_le_bytes()); }
        delayed_terms(&mut session,&mut host,&assets,target,true,600,&mut export);
        assert_apartment_members(&session,target); assert_eq!(occupied(&session,target),0); assert_eq!(word(&session.world.ledgers.businesses[target.business],0x20c),u32::MAX);
        for &id in &tenants { let r=&session.world.ledgers.residents.nodes[id]; assert_eq!(r.housing_building(),-1); assert_eq!(r.next_in_housing(),-1); assert_eq!(r.housing_category(),-1); }
        assert!(session.world.available.contains(&target.business));
        delayed_terms(&mut session,&mut host,&assets,target,false,600,&mut export);
        assert_eq!(occupied(&session,target),0,"lowering rent must wait for natural housing actions");
        for _ in 0..16_000 { frame(&mut session,&mut host,&assets); if session.world.clock.calendar[4]>=2 && session.world.clock.hour()>=5 { break; } }
        assert_eq!(occupied(&session,target),68,"natural timetable did not refill before rent collection");
        let before_rent=Pools::from_session(&session);
        let mut after_rent=Pools::from_session(&session);
        let rent_total=after_rent.residents.collect_housing_income(&mut after_rent.businesses[target.business],Some(&mut after_rent.players[0])).unwrap();
        export.extend_from_slice(b"MTRENT01");
        for n in [target.business as u32,before_rent.players.len() as u32] { export.extend_from_slice(&n.to_le_bytes()); }
        for row in &session.world.categories { export.extend_from_slice(row); }
        for row in &session.world.buildings { export.extend_from_slice(row); }
        before_rent.encode(&session,&mut export);after_rent.encode(&session,&mut export);
        for _ in 0..2_000 { frame(&mut session,&mut host,&assets); if session.world.clock.calendar[4]>=2 && session.world.clock.hour()>=7 { break; } }
        assert!(session.world.clock.calendar[4]>=2 && session.world.clock.hour()>=7);
        assert_eq!(occupied(&session,target),68,"next original timetable did not naturally refill apartment");
        let next_income=session.world.ledgers.players[0].ledger(0x21).unwrap(); assert!(next_income>0.); assert_eq!(next_income.to_bits(),rent_total.to_bits(),"natural next cycle differs from native rent-chain total");
        assert_eq!(word(&session.world.ledgers.businesses[target.business],0x50),next_income.to_bits(),"next real rental cycle lost business/player posting equality");
        assert!(!session.world.available.contains(&target.business),"full apartment remained available");
        assert_apartment_members(&session,target); crate::session_validation::assert_live_links(&session);
        let polls=crate::consumer_polls::Poll::from_runtime(&session.world).unwrap(); assert_eq!(polls.iter().map(|p|p.residents).sum::<u16>(),68);
        let financial=crate::player_info::State::from_runtime(&session.roster.manager,&session.world.ledgers.players,session.roster.status.current_player).unwrap();
        assert_eq!(crate::player_info::report(&financial.records[0],true).unwrap().iter().find(|r|r.label==0x5bb).unwrap().amount,next_income.to_bits());
        fs::write("work/economy/natural-rental-turnover.bin",export).unwrap();
        println!("legal rental turnover:68 naturally housed ->68 evicted ->68 naturally rehoused, day{} hour{}, rental income{} -> {}, full native term snapshots2",session.world.clock.calendar[4],session.world.clock.hour(),initial_income,next_income);
    }).unwrap().join().unwrap();
}

fn submit_legal_business(
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    business: u32,
    excluded: Option<i32>,
) -> i32 {
    let info = session
        .world
        .catalog
        .info
        .row(business as usize)
        .unwrap()
        .to_vec();
    let word = |at| i32::from_le_bytes(info[at..at + 4].try_into().unwrap());
    let kind = word(0xc) as u32;
    let height = word(if business == 42 { 0x44 } else { 0x48 });
    let dimensions = if business == 42 {
        (1..=6).rev().collect::<Vec<_>>()
    } else {
        (1..=6).collect()
    };
    for category in 0..52 {
        if excluded == Some(category)
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
                if width * depth < word(0x38)
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
                        if !inspected.allowed() {
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
                        return category;
                    }
                }
            }
        }
    }
    panic!("no legal business{business} build available");
}
