use super::*;
use openmt2001::session::{CommandAssets, Inputs, Policy, Session, tick::Event};
use openmt2001::{category_ownership::Seam, economy::AccountPosting, live_commands};

fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn put(row: &mut [u8], at: usize, value: u32) {
    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn float(row: &[u8], at: usize) -> f32 {
    f32::from_bits(word(row, at))
}

fn with_session(test: impl FnOnce(&mut Session<'_>, &CommandAssets<'_, '_>)) {
    let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build = read("BUILD_FILE");
    let parameters = read("PARAMETERS");
    let timetable = read("TIMETABLE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
    let scripts = default.parent().unwrap().parent().unwrap();
    let exe = openmt2001::fixtures::path("EXE");
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let names = openmt2001::session::character_names(&image, &language).unwrap();
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
            road_routes: read("ROAD_ROUTE_TABLE"),
            scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: openmt2001::session::load_era_styles(
                openmt2001::fixtures::path("PARAMETERS").parent().unwrap(),
            )
            .unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: "SCENARIO1",
            difficulty: 0,
            selected_character: 4,
            profile_name: b"Account feedback test",
            character_names: &names,
        },
        Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 0x11c],
        },
    )
    .unwrap();
    session.roster.status.current_player = session.roster.manager.players[0].reference;
    session.world.pause.bytes = [0; 9];
    // Completed retail buildings with real geometry and ordinary opening commands.
    // The campaign starts with housing, so shops are explicit test inputs.
    let assets = CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    session.world.clock.calendar[0] = 9;
    let kind = session
        .world
        .catalog
        .info
        .rows()
        .iter()
        .position(|info| word(info, 0xc) == 0 && info[0x4c] > 0)
        .unwrap() as u32;
    for _ in 0..5 {
        let mut selected = None;
        'blocks: for block in &board.blocks {
            if [2, 35, 39, 42, 4, 17, 43, 44].contains(&block.id) {
                continue;
            }
            let category = &session.world.categories[block.id];
            if category[0x6c] == 1 {
                continue;
            }
            for [x, y] in [[0, 0], [0, 5], [5, 0], [5, 5]] {
                let rectangle = [x, y, x + 1, y + 1];
                if !openmt2001::building_pool::can_place(
                    category[0x9c..0xcd].try_into().unwrap(),
                    rectangle,
                ) {
                    continue;
                }
                let spec = openmt2001::scene::BuildingSpec {
                    block: block.name.clone(),
                    lot: [x as u8, y as u8, 2, 2],
                    height: 1,
                    kind: 0,
                    variation: 0,
                    quality: 0,
                    businesses: vec![],
                };
                if openmt2001::scene::SceneBuilding::resolve(spec, &board, &file).is_ok() {
                    selected = Some((block.id, rectangle));
                    break 'blocks;
                }
            }
        }
        let (category, rectangle) = selected.expect("five available retail footprints");
        let slot = openmt2001::building_pool::first_free(&session.world.buildings)
            .unwrap()
            .unwrap();
        let center = openmt2001::building_pool::center(
            &session.world.categories[category],
            rectangle,
            &image,
        )
        .unwrap();
        let city = &mut session.world.city;
        openmt2001::building_pool::initialize_with_parameters(
            &mut city.buildings[slot],
            &openmt2001::building_pool::Initialization {
                rectangle,
                height: 1,
                center,
                variation: 0,
                kind: 0,
                quality: 0,
                total_cost: 0,
                date: [0, 0],
            },
            &city.building_parameters,
        )
        .unwrap();
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            &board,
            &file,
            std::mem::take(&mut session.models),
        );
        for index in 0..geometry.registrar().models.len() {
            let definition = geometry.registrar().models[index].definition;
            geometry
                .retain_model(
                    openmt2001::building_geometry::RegisteredModel::from_definition(
                        index as u32 + 1,
                        &definition,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        openmt2001::building_pool::bind_category(
            &mut city.categories[category],
            &mut city.buildings,
            slot,
            0x591318,
            0x617628 + category as u32 * 380,
            &mut geometry,
        )
        .unwrap();
        session.models = std::mem::take(geometry.registrar_mut());
        session
            .submit_command(
                live_commands::Command::Open(openmt2001::business_open::Command {
                    kind,
                    building: slot as u32,
                    owner: 1,
                    stock_argument: 1,
                }),
                &assets,
                |_| Ok(()),
            )
            .unwrap();
        session.world.clock.timing[0] += 25;
        session.drain_commands(&assets, |_| Ok(())).unwrap();
    }
    test(
        &mut session,
        &CommandAssets {
            image: &image,
            board: &board,
            file: &file,
        },
    );
}

#[derive(Default)]
struct Feedback {
    postings: Vec<AccountPosting>,
    calls: Vec<Seam>,
    sounds: Vec<crate::gameplay_sound::Operation>,
    arrow: crate::cash_arrow::State,
}
impl Feedback {
    fn receive(&mut self, event: &Event<'_>) -> Result<u32, String> {
        let posting = match event {
            Event::AccountPosted(posting)
            | Event::Command(live_commands::Event::AccountPosted(posting)) => Some(*posting),
            _ => None,
        };
        if let Some(posting) = posting {
            self.postings.push(posting);
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let sound = Some(sender);
        let result = account_event_ui_post(event, Some(0), |amount| {
            ledger_ui_post_with(true, amount, 0., |call| {
                self.calls.push(call);
                ledger_ui_call(&mut self.arrow, &sound, call)
            })
        })
        .unwrap_or(Ok(0));
        self.sounds
            .extend(receiver.try_iter().map(|request| request.operation));
        result
    }
    fn assert_calls(&self, credits: &[bool]) {
        let mut expected = vec![];
        for &credit in credits {
            let pulse = Seam::NativeUiCall {
                address: if credit { 0x4c6290 } else { 0x4c62d0 },
                args: [0; 6],
                argument_count: 0,
            };
            let sound = Seam::NativeUiCall {
                address: 0x4ad760,
                args: [if credit { 0x541cc8 } else { 0x541cb4 }, 0, 0, 0, 1, 60],
                argument_count: 6,
            };
            expected.extend(if credit {
                [sound, pulse]
            } else {
                [pulse, sound]
            });
        }
        assert_eq!(self.calls, expected);
        assert_eq!(
            self.sounds,
            credits
                .iter()
                .map(|&sale| crate::gameplay_sound::Operation::Ledger { sale })
                .collect::<Vec<_>>()
        );
    }
}

#[test]
#[ignore = "requires original scripts and assets"]
fn retail_postings_reach_live_feedback_separately() {
    with_session(|session, assets| {
        let (business, building, kind) = session
            .world
            .ledgers
            .businesses
            .iter()
            .enumerate()
            .find_map(|(i, row)| {
                (row[0x21c] != 0 && word(row, 0) == 0).then(|| {
                    (
                        i,
                        u16::from_le_bytes(row[8..10].try_into().unwrap()) as usize,
                        word(row, 4) as usize,
                    )
                })
            })
            .unwrap();
        let commodity = session.world.catalog.products[kind][0][0] as usize;
        let preference = (0..=127)
            .find(|&row| {
                session
                    .population
                    .settings
                    .preferences
                    .get(row)
                    .is_some_and(|list| list.first().is_some_and(|p| p.factor >= 2.))
            })
            .unwrap();
        // Two local sales, one other owner, an unowned shop and a zero sale.
        let owners = [1, 1, 2, 0, 1];
        let mut expected = session.world.ledgers.players.clone();
        let mut sale_buildings = vec![];
        for (i, owner) in owners.into_iter().enumerate() {
            let target = building + i;
            let slot = business + i;
            let mut row = session.world.ledgers.businesses[business];
            row[0xa] = owner;
            row[8..10].copy_from_slice(&(target as u16).to_le_bytes());
            put(&mut row, 0x6c, 0);
            put(&mut row, 0x1c, 0);
            put(&mut row, 0x24, 16_777_216f32.to_bits());
            put(&mut row, 0x1c4, (0.1f32 * (i + 1) as f32).to_bits());
            row[0x80..0x82].copy_from_slice(&10u16.to_le_bytes());
            session.world.ledgers.businesses[slot] = row;
            session.population.shopping.building_businesses[target] = Some(slot);
            let quantity = if i == 4 { 0 } else { 3 };
            let amount = (f64::from(float(&row, 0x1c4)) * f64::from(quantity)) as f32;
            if owner != 0 {
                expected[owner as usize - 1].post(amount, 0x23).unwrap();
            }
            sale_buildings.push((slot, amount));
            let mut resident = [0; 0x14a];
            resident[0x46..0x48].copy_from_slice(&(target as u16).to_le_bytes());
            resident[0x68..0x6a].copy_from_slice(&(i as u16).to_le_bytes());
            resident[0x6a] = preference as u8;
            put(&mut resident, 0x64, commodity as u32);
            put(&mut resident, 0xb8, 1);
            put(&mut resident, 0xc0, 100);
            put(&mut resident, 0xcc, 6);
            put(&mut resident, 0xd0, 5);
            put(&mut resident, 0xd4, 5);
            let reference = session.population.bindings[i].auxiliary_reference;
            session.world.ledgers.residents.nodes[i] =
                openmt2001::resident::Resident::from_runtime_bytes(resident, 1, reference);
            let mut spending = [0; 0x90];
            spending[0] = 1;
            // A little extra budget avoids integer truncation at the exact float boundary.
            put(
                &mut spending,
                4 + commodity * 4,
                ((quantity as f32 + 0.25) * float(&row, 0x1c4)).to_bits(),
            );
            session.world.ledgers.residents.accounts[i] =
                openmt2001::resident::SpendingAccount::from_bytes(spending);
        }
        session.world.ledgers.residents.active_count = 5;
        session.world.clock.calendar[0] = 9;
        session.world.clock.timing[5] = 1;
        session.world.clock.timing[1] = 100;
        let mut feedback = Feedback::default();
        feedback.arrow.start(true);
        feedback.arrow.advance();
        let mut policy = simulation_test_policy();
        session
            .advance_session(assets, &mut policy, |event, _| feedback.receive(&event))
            .unwrap();
        assert_eq!(
            feedback.postings,
            vec![
                AccountPosting {
                    player: 0,
                    amount: sale_buildings[0].1
                },
                AccountPosting {
                    player: 0,
                    amount: sale_buildings[1].1
                },
                AccountPosting {
                    player: 1,
                    amount: sale_buildings[2].1
                },
                AccountPosting {
                    player: 0,
                    amount: 0.
                },
            ]
        );
        feedback.assert_calls(&[true, true]);
        assert_eq!(
            feedback.arrow.frame, 42,
            "second credit must not restart an active arrow"
        );
        for (actual, expected) in session.world.ledgers.players.iter().zip(expected) {
            assert_eq!(actual.cash, expected.cash);
            assert_eq!(actual.ledger(0x23), expected.ledger(0x23));
        }
        for (i, (slot, amount)) in sale_buildings.into_iter().enumerate() {
            let row = &session.world.ledgers.businesses[slot];
            assert_eq!(float(row, 0x1c), amount);
            assert_eq!(
                float(row, 0x24),
                (16_777_216f64
                    + if i == 4 {
                        0.
                    } else {
                        3. * f64::from(float(row, 0x1c4))
                    }) as f32
            );
            assert_eq!(
                u16::from_le_bytes(row[0x80..0x82].try_into().unwrap()),
                if i == 4 { 10 } else { 7 }
            );
        }
    });
}

#[test]
#[ignore = "requires original scripts and assets"]
fn morning_owner_postings_reach_live_feedback_in_order() {
    with_session(|session, assets| {
        session.world.ledgers.residents.active_count = 0;
        for row in &mut session.world.ledgers.businesses {
            row[0x21c] = 0;
        }
        for category in [2, 35, 39, 42, 4, 17, 43, 44] {
            session.world.categories[category][5] = 0;
        }
        for (category, owner) in [(2, 1), (35, 2), (39, 1), (4, 1), (17, 2), (44, 1)] {
            session.world.categories[category][5] = owner;
        }
        session.hooks.utilities.totals = [400., 800., 0., 1200.];
        session.hooks.utilities.enabled = [false; 4];
        session.population.properties.totals = [800., 1000., 1200., 1600.];
        let before = session.world.ledgers.players.clone();
        session.world.clock.calendar[0] = 5;
        session.world.clock.timing[1] = 624;
        session.world.clock.timing[2] = 624;
        session.world.clock.timing[5] = 1;
        let mut feedback = Feedback::default();
        session
            .advance_session(assets, &mut simulation_test_policy(), |event, _| {
                feedback.receive(&event)
            })
            .unwrap();
        assert_eq!(session.world.clock.hour(), 6);
        assert_eq!(
            feedback.postings,
            [
                (0, 200.),
                (1, 200.),
                (0, 0.),
                (0, 800.),
                (1, 250.),
                (0, 400.)
            ]
            .map(|(player, amount)| AccountPosting { player, amount })
        );
        feedback.assert_calls(&[true, true, true]);
        assert_eq!(
            session.world.ledgers.players[0].cash,
            before[0].cash + 1400.
        );
        assert_eq!(session.world.ledgers.players[1].cash, before[1].cash + 450.);
        for (player, category, amount) in [
            (0, 28, 200.),
            (1, 27, 200.),
            (0, 25, 0.),
            (0, 32, 800.),
            (1, 30, 250.),
            (0, 29, 400.),
        ] {
            assert_eq!(
                session.world.ledgers.players[player]
                    .ledger(category)
                    .unwrap(),
                before[player].ledger(category).unwrap() + amount
            );
        }
        assert_eq!(session.hooks.utilities.totals, [0.; 4]);
        assert_eq!(session.hooks.utilities.enabled, [true; 4]);
        assert_eq!(session.population.properties.totals, [800.; 4]);
    });
}

#[test]
#[ignore = "requires original scripts and assets"]
fn attachment_postings_reach_live_feedback_at_their_command_positions() {
    for (hour, landlord, proprietor, conversion, free_conversion) in [
        (9, 1, 2, false, false),
        (5, 1, 2, false, false),
        (9, 2, 2, false, false),
        (9, 0, 2, false, false),
        (9, 2, 1, true, false),
        (9, 1, 2, true, false),
        (5, 2, 1, true, false),
        (5, 1, 2, true, false),
        (9, 2, 1, true, true),
        (5, 2, 1, true, true),
    ] {
        with_session(|session, assets| {
            session.world.ledgers.residents.active_count = 0;
            let (building, old_kind) = session
                .world
                .ledgers
                .businesses
                .iter()
                .find_map(|row| {
                    (row[0x21c] != 0 && word(row, 0) == 0).then(|| {
                        (
                            u16::from_le_bytes(row[8..10].try_into().unwrap()) as usize,
                            word(row, 4),
                        )
                    })
                })
                .unwrap();
            let kind = if conversion {
                (0..42)
                    .find(|&kind| {
                        kind != old_kind
                            && session.world.catalog.info.row(kind as usize).unwrap()[0x4c] > 0
                            && openmt2001::business_conversion::fits(
                                &session.world.buildings[building],
                                session.world.catalog.info.row(kind as usize).unwrap(),
                            )
                            .unwrap()
                    })
                    .unwrap()
            } else {
                old_kind
            };
            let category =
                ((word(&session.world.buildings[building], 0x40) - 0x617628) / 380) as usize;
            session.world.categories[category][5] = landlord;
            for category in [2, 35, 39, 42] {
                session.world.categories[category][5] = 0;
            }
            session.world.categories[2][5] = 1;
            session.world.categories[35][5] = 2;
            session.hooks.utilities.totals = [400., 800., 0., 0.];
            session.world.clock.calendar[0] = hour;
            let command = if conversion {
                live_commands::Command::Convert(openmt2001::business_conversion::Command {
                    kind,
                    building: building as u32,
                    owner: proprietor as u32,
                })
            } else {
                live_commands::Command::Open(openmt2001::business_open::Command {
                    kind,
                    building: building as u32,
                    owner: proprietor as u32,
                    stock_argument: 1,
                })
            };
            session
                .submit_command(command, assets, |_| {
                    panic!("command executed before its delay")
                })
                .unwrap();
            if conversion {
                // Execution must use the live refurbishment cost after the delay.
                let info = session.world.catalog.info.row_mut(kind as usize).unwrap();
                let cost = word(info, 0x54);
                assert_ne!(cost, 0);
                put(info, 0x54, if free_conversion { 0 } else { cost + 7 });
            }
            let b = &session.world.buildings[building];
            let rent = openmt2001::building_binding::initial_rent(
                b,
                float(
                    &session.world.categories
                        [word(&session.world.categories[category], 0) as usize],
                    0x84,
                ),
                assets.image.f32_at(0x53be78).unwrap(),
                assets.image,
            )
            .unwrap() as f32;
            let volume = (i32::from(b[6]) - i32::from(b[4]) + 1)
                * (i32::from(b[7]) - i32::from(b[5]) + 1)
                * i32::from(b[8]);
            let info = session.world.catalog.info.row(kind as usize).unwrap();
            let count = info[0x4c] as usize;
            let products: Vec<_> = session.world.catalog.products[kind as usize][..count]
                .iter()
                .map(|entry| openmt2001::building_binding::StockProduct {
                    value: float(&session.world.commodities.rows[entry[0] as usize], 0x14),
                    fraction: f32::from_bits(entry[1]),
                })
                .collect();
            let stock_cost = openmt2001::building_binding::initial_stock_cost(
                volume,
                float(info, 0x58),
                &products,
            )
            .unwrap()
            .to_f32();
            let mut utilities = openmt2001::economy::Utilities::new([1, 2, 0, 0]);
            utilities.totals = session.hooks.utilities.totals;
            let amounts = utilities.assess(
                volume,
                [0x60, 0x64, 0x68, 0x6c].map(|at| float(info, at)),
                proprietor,
            );
            let mut expected = session.world.ledgers.players.clone();
            let mut expected_postings = vec![];
            if conversion {
                let quote = openmt2001::business_conversion::quote(
                    building as u32,
                    kind,
                    &session.world.categories,
                    &session.world.buildings,
                    &session.world.catalog.info,
                    assets.image,
                )
                .unwrap();
                let amount = -quote.to_f32();
                assert_eq!(amount == 0., free_conversion);
                let player = proprietor as usize - 1;
                expected[player].post(amount, 0x14).unwrap();
                expected_postings.push(AccountPosting { player, amount });
            }
            if hour >= 6 {
                if landlord != 0 && landlord != proprietor {
                    let player = landlord as usize - 1;
                    expected[player].post(rent, 0x22).unwrap();
                    expected_postings.push(AccountPosting {
                        player,
                        amount: rent,
                    });
                }
                for (player, ledger) in [(0, 28), (1, 27)] {
                    let amount = utilities.totals[player] * 0.25;
                    expected[player].post(amount, ledger).unwrap();
                    expected_postings.push(AccountPosting { player, amount });
                }
                if conversion {
                    expected[proprietor as usize - 1]
                        .post(-stock_cost, 0x11)
                        .unwrap();
                    expected_postings.push(AccountPosting {
                        player: proprietor as usize - 1,
                        amount: -stock_cost,
                    });
                }
            }
            session.world.clock.timing[0] += 25;
            let mut feedback = Feedback::default();
            let mut events = vec![];
            session
                .drain_commands(assets, |event| {
                    events.push(event);
                    feedback.receive(&Event::Command(event)).map(|_| ())
                })
                .unwrap();
            assert_eq!(
                feedback.postings, expected_postings,
                "hour={hour}, landlord={landlord}, proprietor={proprietor}, conversion={conversion}"
            );
            let credits: Vec<_> = expected_postings
                .iter()
                .filter(|p| p.player == 0 && p.amount != 0.)
                .map(|p| p.amount > 0.)
                .collect();
            feedback.assert_calls(&credits);
            if conversion {
                assert_eq!(
                    events.first(),
                    Some(&live_commands::Event::AccountPosted(expected_postings[0]))
                );
            }
            let refreshed = events
                .iter()
                .position(|e| {
                    matches!(
                        e,
                        live_commands::Event::Business(
                            openmt2001::business_binding::Call::RefreshBuilding { .. }
                        )
                    )
                })
                .unwrap();
            assert!(
                events
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| matches!(e, live_commands::Event::AccountPosted(_)))
                    .all(|(i, _)| i < refreshed)
            );
            for (actual, expected) in session.world.ledgers.players.iter().zip(expected) {
                assert_eq!(actual.cash, expected.cash);
                for category in [0x0e, 0x11, 0x14, 0x22, 25, 26, 27, 28] {
                    assert_eq!(actual.ledger(category), expected.ledger(category));
                }
            }
            let row = &session.world.ledgers.businesses
                [word(&session.world.buildings[building], 0x1c) as usize];
            assert_eq!(
                float(row, 0x28),
                if hour >= 6 && landlord != proprietor {
                    -rent
                } else {
                    0.
                }
            );
            assert_eq!(float(row, 0x30), if hour >= 6 { -stock_cost } else { 0. });
            assert_eq!(
                float(row, 0x1c),
                if hour >= 6 && conversion {
                    -stock_cost
                } else {
                    0.
                }
            );
            for (at, amount) in [0x44, 0x38, 0x40, 0x3c].into_iter().zip(amounts) {
                assert_eq!(float(row, at), if hour >= 6 { -amount } else { 0. });
            }
            for slot in 0..count {
                let target =
                    u16::from_le_bytes(row[0x1b8 + slot * 2..0x1ba + slot * 2].try_into().unwrap());
                let stock = u16::from_le_bytes(
                    row[0x80 + slot * 0x34..0x82 + slot * 0x34]
                        .try_into()
                        .unwrap(),
                );
                assert_eq!(stock, if hour >= 6 { target } else { 0 });
            }
        });
    }
}

#[test]
#[ignore = "requires original scripts and assets"]
fn conversion_feedback_follows_debit_before_business_replacement() {
    with_session(|session, assets| {
        let (building, kind) = session
            .world
            .ledgers
            .businesses
            .iter()
            .find_map(|row| {
                (row[0x21c] != 0 && word(row, 0) == 0).then(|| {
                    (
                        u16::from_le_bytes(row[8..10].try_into().unwrap()) as usize,
                        word(row, 4),
                    )
                })
            })
            .unwrap();
        let quote = openmt2001::business_conversion::quote(
            building as u32,
            kind,
            &session.world.categories,
            &session.world.buildings,
            &session.world.catalog.info,
            assets.image,
        )
        .unwrap();
        let posting = AccountPosting {
            player: 0,
            amount: -quote.to_f32(),
        };
        assert!(posting.amount < 0.);
        let mut expected = session.world.ledgers.players.clone();
        expected[0].post(posting.amount, 0x14).unwrap();
        let buildings = session.world.buildings.clone();
        let businesses = session.world.ledgers.businesses.clone();
        session
            .submit_command(
                live_commands::Command::Convert(openmt2001::business_conversion::Command {
                    kind,
                    building: building as u32,
                    owner: 1,
                }),
                assets,
                |_| panic!("command executed before its delay"),
            )
            .unwrap();
        session.world.clock.timing[0] += 25;
        let mut feedback = Feedback::default();
        let result = session.drain_commands(assets, |event| {
            assert_eq!(event, live_commands::Event::AccountPosted(posting));
            feedback.receive(&Event::Command(event))?;
            // Stop at the presentation boundary to inspect the preceding stores.
            Err("stop after refurbishment feedback".into())
        });
        assert_eq!(result, Err("stop after refurbishment feedback".into()));
        assert_eq!(feedback.postings, [posting]);
        feedback.assert_calls(&[false]);
        for (actual, expected) in session.world.ledgers.players.iter().zip(expected) {
            assert_eq!(actual.cash, expected.cash);
            for category in 0..38 {
                assert_eq!(actual.ledger(category), expected.ledger(category));
            }
        }
        assert_eq!(session.world.buildings, buildings);
        assert_eq!(session.world.ledgers.businesses, businesses);
    });
}
fn simulation_test_policy() -> openmt2001::session::tick::Policy {
    use openmt2001::{hourly, scheduler};
    openmt2001::session::tick::Policy {
        scheduler: scheduler::Controls {
            iterations: 1,
            abort: false,
            alternate_script_active: false,
            network_enabled: false,
            network_override: false,
            deadline: u32::MAX,
            gate_32: false,
            gate_32_offset: false,
            before_six_sound: 0,
        },
        calendar: openmt2001::clock::Controls {
            years_per_day: 0,
            suppress_rollover_query: false,
            rollover_events_enabled: false,
            autosave_disabled: true,
        },
        hourly: hourly::Controls {
            suppressed: false,
            world_mode: 1,
            before_six_sound: 0,
            network_enabled: false,
            network_override: false,
        },
        camera: [0., 20., 0.],
        distance_disabled: false,
        notifications: false,
        effect_model: 0,
        market_share_prior: [0.; 6],
        player_message_prior: [0; 48],
        ai_event_gate: false,
    }
}
