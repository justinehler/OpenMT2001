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
use std::{fs, path::Path};
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
        .filter(|(_, r)| r.is_active() && r.housing_building() == target.building as i16)
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
fn shadow_average_rent<'a>(
    retained: &openmt2001::market::Cache,
    buildings: &'a [[u8; 64]],
    catalogs: &'a openmt2001::shopping::Catalogs,
    commodities: &'a [openmt2001::resident::Commodity; 32],
) -> impl FnMut(&[[u8; 544]]) -> Result<f32, String> + 'a {
    // Rent edits retain the live cache, including its pending refresh state.
    let mut market = openmt2001::market::Cache {
        dirty: retained.dirty,
        prices: retained.prices,
        rent: retained.rent,
    };
    move |businesses| {
        market.residential_rent(&openmt2001::market::Inputs {
            businesses,
            buildings,
            catalogs,
            commodities,
        })
    }
}

#[test]
fn rental_turnover_shadow_retains_and_lazily_refreshes_market() {
    let catalogs = openmt2001::shopping::Catalogs {
        commodities: std::array::from_fn(|_| Vec::new()),
        fractions: std::array::from_fn(|_| Vec::new()),
    };
    let commodities = [openmt2001::resident::Commodity {
        active: false,
        value: 10.,
        markup: 1.,
    }; 32];
    let mut buildings = vec![[0; 64]; 600];
    buildings[0][0x3f] = 1;
    let mut businesses = vec![[0; 544]; 600];
    businesses[0][..4].copy_from_slice(&1u32.to_le_bytes());
    businesses[0][0xe..0x10].copy_from_slice(&3u16.to_le_bytes());
    businesses[0][0x1dc..0x1e0].copy_from_slice(&27f32.to_le_bytes());
    let retained = openmt2001::market::Cache {
        dirty: false,
        prices: [7.; 32],
        rent: 18.,
    };
    let mut warm = shadow_average_rent(&retained, &buildings, &catalogs, &commodities);
    assert_eq!(warm(&businesses).unwrap(), 18.);

    let dirty = openmt2001::market::Cache {
        dirty: true,
        ..retained
    };
    let mut cold = shadow_average_rent(&dirty, &buildings, &catalogs, &commodities);
    assert_eq!(cold(&businesses).unwrap(), 27.);
    businesses[0][0x1dc..0x1e0].copy_from_slice(&54f32.to_le_bytes());
    assert_eq!(cold(&businesses).unwrap(), 27.);
    assert_eq!(warm(&businesses).unwrap(), 18.);
    assert!(
        dirty.dirty,
        "shadow refresh mutated the retained live cache"
    );
    assert_eq!(dirty.rent, 18.);
    assert_eq!(dirty.prices, [7.; 32]);
}

fn shadow_terms(
    session: &Session<'_>,
    target: crate::business_edit::Target,
    terms: &BusinessTerms,
    out: &mut Vec<u8>,
) -> Pools {
    let before = Pools::from_session(session);
    let mut after = Pools::from_session(session);
    let buildings = session
        .world
        .buildings
        .iter()
        .map(|building| building[..64].try_into().unwrap())
        .collect::<Vec<_>>();
    let mut average_rent = shadow_average_rent(
        &session.population.market,
        &buildings,
        &session.population.settings.catalogs,
        &session.population.settings.commodities,
    );
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
        average_rent: &mut average_rent,
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
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| with_legal_rental_save(check_rental_turnover))
        .unwrap()
        .join()
        .unwrap();
}

fn check_rental_turnover(
    mut session: Session<'_>,
    mut host: crate::live_host::Host,
    assets: CommandAssets<'_, '_>,
    target: crate::business_edit::Target,
    path: std::path::PathBuf,
) {
    let initial_income = session.world.ledgers.players[0].ledger(0x21).unwrap();
    assert!(initial_income > 0.);
    let tenants = session
        .world
        .ledgers
        .residents
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, resident)| resident.housing_building() == target.building as i16)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    assert_eq!(tenants.len(), 68);
    let tenant_count = u16::try_from(tenants.len()).unwrap();

    // The city already offers other apartments. Eviction should use
    // their real vacancies rather than leave these tenants homeless.
    let alternatives = session
        .population
        .housing
        .buildings
        .iter()
        .enumerate()
        .filter_map(|(building, housing)| {
            let business = housing.business?;
            let row = &session.world.ledgers.businesses[business];
            let capacity = u16::from_le_bytes(row[0xe..0x10].try_into().unwrap());
            let occupied = u16::from_le_bytes(row[0x10..0x12].try_into().unwrap());
            if business == target.business
                || housing.category < 0
                || word(row, 0) != 1
                || capacity <= occupied
            {
                return None;
            }
            Some((
                crate::business_edit::Target { business, building },
                capacity - occupied,
                occupied,
            ))
        })
        .collect::<Vec<_>>();
    let affordable_alternatives = tenants
        .iter()
        .map(|&id| {
            let resident = &session.world.ledgers.residents.nodes[id];
            let budget = f64::from(session.population.housing_settings.affordability_factor)
                * f64::from(
                    session.world.welfare.amounts[resident.class() as usize]
                        [resident.age() as usize],
                );
            let affordable = alternatives
                .iter()
                .enumerate()
                .filter(|(_, (home, _, _))| {
                    f64::from(f32::from_bits(word(
                        &session.world.ledgers.businesses[home.business],
                        0x1dc,
                    ))) <= budget
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let affordable_units = affordable
                .iter()
                .map(|&index| usize::from(alternatives[index].1))
                .sum::<usize>();
            assert!(
                affordable_units >= tenants.len(),
                "setup lacks affordable relocation capacity for resident {id}"
            );
            affordable
        })
        .collect::<Vec<_>>();

    let mut export = b"MTRTURN1".to_vec();
    for n in [0x37fu32, 2] {
        export.extend_from_slice(&n.to_le_bytes());
    }
    delayed_terms(
        &mut session,
        &mut host,
        &assets,
        target,
        true,
        600,
        &mut export,
    );
    assert_apartment_members(&session, target);
    assert_eq!(occupied(&session, target), 0);
    assert_eq!(
        word(&session.world.ledgers.businesses[target.business], 0x20c),
        u32::MAX
    );
    let mut relocated = vec![0u16; alternatives.len()];
    for (&id, affordable) in tenants.iter().zip(&affordable_alternatives) {
        let resident = &session.world.ledgers.residents.nodes[id];
        let (index, (home, _, _)) = alternatives
            .iter()
            .enumerate()
            .find(|(_, (home, _, _))| resident.housing_building() == home.building as i16)
            .unwrap_or_else(|| {
                panic!("evicted resident {id} did not move into available city housing")
            });
        assert!(
            affordable.contains(&index),
            "resident {id} moved into unaffordable housing"
        );
        assert_eq!(
            resident.housing_category(),
            session.population.housing.buildings[home.building].category
        );
        relocated[index] += 1;
    }
    for ((home, vacancies, prior_occupied), moved) in alternatives.iter().zip(relocated) {
        assert!(
            moved <= *vacancies,
            "relocation exceeded available city units"
        );
        assert_eq!(occupied(&session, *home), prior_occupied + moved);
        assert_apartment_members(&session, *home);
    }
    assert!(session.world.available.contains(&target.business));
    delayed_terms(
        &mut session,
        &mut host,
        &assets,
        target,
        false,
        600,
        &mut export,
    );
    assert_eq!(
        occupied(&session, target),
        0,
        "lowering rent must wait for natural housing actions"
    );
    // City apartments can retain tenants, so the next day's apartment need
    // not be full. Observe the real chain just before the hour-six collection.
    for _ in 0..16_000 {
        frame(&mut session, &mut host, &assets);
        if session.world.clock.calendar[4] == 2
            && session.world.clock.hour() == 5
            && session.world.clock.timing[2] == 624
        {
            break;
        }
    }
    assert_eq!(session.world.clock.calendar[4], 2);
    assert_eq!(session.world.clock.hour(), 5);
    assert_eq!(session.world.clock.timing[2], 624);
    assert_apartment_members(&session, target);
    let rehoused = occupied(&session, target);
    assert!(
        rehoused > 0,
        "natural timetable did not rehouse any tenants before rent collection"
    );
    let capacity = u16::from_le_bytes(
        session.world.ledgers.businesses[target.business][0xe..0x10]
            .try_into()
            .unwrap(),
    );
    assert!(rehoused <= capacity);
    let rent = f32::from_bits(word(
        &session.world.ledgers.businesses[target.business],
        0x1dc,
    ));
    let expected_income = (0..rehoused).fold(0f32, |total, _| total + rent);
    let before_rent = Pools::from_session(&session);
    let mut after_rent = Pools::from_session(&session);
    let rent_total = after_rent
        .residents
        .collect_housing_income(
            &mut after_rent.businesses[target.business],
            Some(&mut after_rent.players[0]),
        )
        .unwrap();
    assert_eq!(
        rent_total.to_bits(),
        expected_income.to_bits(),
        "rent-chain total differs from occupied units at the actual rent"
    );
    export.extend_from_slice(b"MTRENT01");
    for n in [target.business as u32, before_rent.players.len() as u32] {
        export.extend_from_slice(&n.to_le_bytes());
    }
    for row in &session.world.categories {
        export.extend_from_slice(row);
    }
    for row in &session.world.buildings {
        export.extend_from_slice(row);
    }
    before_rent.encode(&session, &mut export);
    after_rent.encode(&session, &mut export);
    frame(&mut session, &mut host, &assets);
    assert_eq!(session.world.clock.hour(), 6);
    assert_eq!(
        session.world.ledgers.players[0]
            .ledger(0x21)
            .unwrap()
            .to_bits(),
        expected_income.to_bits(),
        "next hour-six update did not post the occupied apartment's rent"
    );
    assert_eq!(
        u16::from_le_bytes(
            session.world.ledgers.businesses[target.business][0x12..0x14]
                .try_into()
                .unwrap()
        ),
        rehoused,
        "rent collection did not record the actual tenant count"
    );
    for _ in 0..2_000 {
        frame(&mut session, &mut host, &assets);
        if session.world.clock.calendar[4] >= 2 && session.world.clock.hour() >= 7 {
            break;
        }
    }
    assert!(session.world.clock.calendar[4] >= 2 && session.world.clock.hour() >= 7);
    let current_occupied = occupied(&session, target);
    assert!(current_occupied > 0 && current_occupied <= capacity);
    let next_income = session.world.ledgers.players[0].ledger(0x21).unwrap();
    assert!(next_income > 0.);
    assert_eq!(
        next_income.to_bits(),
        rent_total.to_bits(),
        "natural next cycle differs from native rent-chain total"
    );
    assert_eq!(
        word(&session.world.ledgers.businesses[target.business], 0x50),
        next_income.to_bits(),
        "next real rental cycle lost business/player posting equality"
    );
    assert_eq!(
        session.world.available.contains(&target.business),
        current_occupied < capacity,
        "housing availability differs from the remaining apartment capacity"
    );
    assert_apartment_members(&session, target);
    crate::session_validation::assert_live_links(&session);
    let polls = crate::consumer_polls::Poll::from_runtime(&session.world).unwrap();
    // Polls cover every city apartment, including the alternative housing.
    let expected_polls: [u16; 52] = std::array::from_fn(|category| {
        session.population.housing.categories[category]
            .buildings
            .iter()
            .filter_map(|&building| session.population.housing.buildings[building].business)
            .map(|business| &session.world.ledgers.businesses[business])
            .filter(|&row| word(row, 4) == 42)
            .map(|row| u16::from_le_bytes(row[0x10..0x12].try_into().unwrap()))
            .sum()
    });
    assert_eq!(polls.map(|poll| poll.residents), expected_polls);
    let financial = crate::player_info::State::from_runtime(
        &session.roster.manager,
        &session.world.ledgers.players,
        session.roster.status.current_player,
    )
    .unwrap();
    assert_eq!(
        crate::player_info::report(&financial.records[0], true)
            .unwrap()
            .iter()
            .find(|row| row.label == 0x5bb)
            .unwrap()
            .amount,
        next_income.to_bits()
    );
    fs::write("work/economy/natural-rental-turnover.bin", export).unwrap();
    println!(
        "legal rental turnover: {tenant_count} evicted, {rehoused} paying rent next cycle, day{} hour{}, rental income{} -> {}",
        session.world.clock.calendar[4],
        session.world.clock.hour(),
        initial_income,
        next_income
    );
    fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
}

/// Reach a naturally occupied apartment and publish an isolated input save.
fn with_legal_rental_save<T>(
    check: impl FnOnce(
        Session<'_>,
        crate::live_host::Host,
        CommandAssets<'_, '_>,
        crate::business_edit::Target,
        std::path::PathBuf,
    ) -> T,
) -> T {
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
    let names = character_names(
        &image,
        &exe.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa"),
    )
    .unwrap();
    let mut session = Session::initialize(
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
            era_styles: load_era_styles(
                Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                    .parent()
                    .unwrap(),
            )
            .unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: "SCENARIO3",
            difficulty: 0,
            selected_character: 4,
            profile_name: b"Rental turnover validation",
            character_names: &names,
        },
        Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 0x11c],
        },
    )
    .unwrap();
    let assets = CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    submit_legal_business(&mut session, &assets, 42, None);
    let mut host = crate::live_host::Host::new([0., 20., 0.]);
    host.set_running(true);
    let mut target = None;
    for _ in 0..16_000 {
        frame(&mut session, &mut host, &assets);
        if target.is_none()
            && let Some(business) = session
                .world
                .ledgers
                .businesses
                .iter()
                .position(|b| b[0x21c] != 0 && b[10] == 1 && word(b, 4) == 42)
        {
            let t = crate::business_edit::Target {
                business,
                building: usize::from(u16::from_le_bytes(
                    session.world.ledgers.businesses[business][8..10]
                        .try_into()
                        .unwrap(),
                )),
            };
            for _ in 0..501 {
                crate::business_edit::edit(
                    &mut session,
                    &assets,
                    t,
                    crate::business_edit::Action::Rent { increase: false },
                )
                .unwrap();
            }
            crate::business_edit::edit(
                &mut session,
                &assets,
                t,
                crate::business_edit::Action::Apply,
            )
            .unwrap();
            target = Some(t);
        }
        if session.world.clock.calendar[4] >= 1 && session.world.clock.hour() >= 7 {
            break;
        }
    }
    let target = target.unwrap();
    assert_apartment_members(&session, target);
    assert_eq!(
        occupied(&session, target),
        68,
        "initial legal apartment did not naturally fill"
    );
    // Save the legally reached state for independent continuation tests.
    // Host camera stays at its initial eye throughout this headless flow.
    let camera = openmt2001::flyby::CameraSample {
        eye: [0., 20., 0.],
        target: [0., 0., 1.],
        up: [0., 1., 0.],
        vertical_fov: 0.7,
    };
    let cameras = crate::save_menu::cameras(None, camera).unwrap();
    let bytes = session.save_v2(&image, cameras, 1).unwrap();
    let root = openmt2001::fixtures::scratch("RENTAL_SAVES").join(format!(
        "rental-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = crate::save_menu::write_new(&root, &bytes).unwrap();
    check(session, host, assets, target, path)
}

pub(crate) fn legal_rental_save() -> std::path::PathBuf {
    with_legal_rental_save(|_, _, _, _, path| path)
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
