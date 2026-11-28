//! Synthetic load bootstrap through the normal controller and delayed terms queue.
use crate::{
    ai_script::{AiScript, Initialization},
    economy::{BusinessDayInputs, BusinessDaySettings},
    player_manager::{Manager, Player, put, word},
};

fn controller_image() -> Vec<u8> {
    // Only the final virtual call needs image data in this controlled decision.
    let mut bytes = vec![0; 0x108];
    bytes[..2].copy_from_slice(b"MZ");
    put(&mut bytes, 0x3c, 0x40);
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x46..0x48].copy_from_slice(&1u16.to_le_bytes());
    bytes[0x54..0x56].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x58..0x5a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut bytes, 0x74, 0x400000);
    put(&mut bytes, 0x84, 0x1000);
    put(&mut bytes, 0x88, 8);
    put(&mut bytes, 0x8c, 0x100);
    put(&mut bytes, 0x104, 0x123456);
    bytes
}

#[test]
fn loaded_ai_dispatches_0230_retail_and_applies_delayed_terms() {
    let mut player = Player {
        reference: 0x20000000,
        bytes: vec![0; 0x1e50],
        auxiliaries: vec![],
    };
    // Native post-load reset: initialized, idle, deadline zero.
    put(&mut player.bytes, 0, 0x401000);
    player.bytes[0x10] = 1;
    put(&mut player.bytes, 0x154, 1);
    player.bytes[0x1bc] = 1;
    put(&mut player.bytes, 0x240, 2);
    put(&mut player.bytes, 0xb68, u32::MAX);
    player.bytes[0x1401] = 1;
    player.bytes[0x238..0x23b].fill(1);
    put(&mut player.bytes, 0x224, 7);
    put(&mut player.bytes, 0x228, 0x343fd);
    put(&mut player.bytes, 0x22c, 0x269ec3);
    let mut manager = Manager::initialize([0; 40]);
    manager.players.push(player);
    let mut profiles = [[0; 0x650]; 10];
    put(&mut profiles[0], 0, 0x20000000);
    profiles[0][4] = 1;
    put(&mut profiles[0], 0x614, 5000);
    let mut scripts = AiScript::new().unwrap();
    let source = br#"
function EVENT_LoadHseKeepingTimetable(a)
 AiChars[a]:ResetTimeTable(HSE)
 AiChars[a]:LoadTimeTable(HSE,2,30,TT_EOD_STOCK_AND_PRICE)
 AiChars[a]:LoadTimeTable(HSE,2,30,TT_RENT)
 AiChars[a]:LoadTimeTable(HSE,3,30,TT_RENT)
end
function EVENT_LoadBuyBlockTimetable(a) AiChars[a]:ResetTimeTable(BLK) end
function EVENT_LoadDifficultyVariables(a) end
function EVENT_LoadBuildTimes(a) end
function EVENT_LoadScoreData() end
"#;
    let initialize =
        |scripts: &mut AiScript, manager: &mut Manager, profiles: &mut [[u8; 0x650]], loaded| {
            let mut state = Initialization::<64> {
                precision: crate::x87::Precision::Pc64,
                profiles,
                manager,
                category_counts: &mut [0; 52 * 45],
                chains: &std::array::from_fn(|_| vec![]),
                buildings: &[],
                businesses: &[],
                difficulty: 0,
                current_hour: 0,
                // A nonzero saved clock exposed the accidental fresh-game count.
                count_tick: Some(10002),
            };
            if loaded {
                scripts.initialize_loaded(&mut state, source, None)
            } else {
                scripts.initialize(&mut state, source, None)
            }
            .unwrap();
        };
    initialize(&mut scripts, &mut manager, &mut profiles, true);
    assert_eq!(word(&manager.players[0].bytes, 0x224), 7);
    assert_eq!(&manager.players[0].bytes[0xb4c..0xb4e], &[0; 2]);
    // Fresh starts still use the randomized build-time countdown.
    let mut fresh = Manager::initialize([0; 40]);
    fresh.players.push(Player {
        reference: manager.players[0].reference,
        bytes: manager.players[0].bytes.clone(),
        auxiliaries: vec![],
    });
    initialize(&mut scripts, &mut fresh, &mut profiles.clone(), false);
    assert_ne!(word(&fresh.players[0].bytes, 0x224), 7);
    assert_ne!(&fresh.players[0].bytes[0xb4c..0xb4e], &[0; 2]);

    let bytes = controller_image();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let mut buildings = [[0; 64]; 1];
    buildings[0][8] = 1;
    let mut settings = BusinessDaySettings {
        capacity_rates: [0.; 45],
        utility_rates: [[0.; 4]; 45],
        catalog_masks: [0; 45],
    };
    settings.capacity_rates[0] = 40.;
    let mut catalogs = crate::shopping::Catalogs {
        commodities: std::array::from_fn(|_| vec![]),
        fractions: std::array::from_fn(|_| vec![]),
    };
    catalogs.commodities[0] = vec![0];
    catalogs.fractions[0] = vec![1.];
    let inputs = BusinessDayInputs {
        buildings: &buildings,
        building_categories: &[],
        category_owners: &[0; 52],
        catalogs: &catalogs,
        unit_prices: &[1.; 32],
        settings: &settings,
    };
    let mut businesses = vec![[0; 544]; 600];
    let row = &mut businesses[0];
    row[10] = 2;
    row[0x21c] = 1;
    row[0x1b8..0x1ba].copy_from_slice(&20i16.to_le_bytes());
    row[0x1e0..0x1e2].copy_from_slice(&20i16.to_le_bytes());
    put(row, 0x1c4, 30f32.to_bits());
    put(row, 0x1ec, 30f32.to_bits());
    put(row, 0x20c, u32::MAX);
    let mut entry = [0; 28];
    entry[0] = 1;
    let owned = crate::ai_ownership::BusinessList {
        entries: vec![entry],
        active_count: 1,
    };
    let mut commands = crate::economy::LocalBusinessCommands::default();
    let mut seen = vec![];
    let mut applied = vec![];
    for day_tick in 0..=1575 {
        let tick = 10002 + day_tick;
        crate::ai_events::poll_players(&mut manager.players, &mut profiles, day_tick as i32)
            .unwrap();
        crate::ai_tick::advance(
            &mut manager.players[0],
            &mut profiles,
            &mut crate::ai_tick::Controls {
                normal_tick: tick,
                day_tick,
                event_gate: false,
            },
            &image,
            |call, player, profiles, controls| {
                match call.address {
                    0x47d6a0 => return Ok(controls.normal_tick + 125),
                    0x47b8f0 => return Ok(1),
                    // Control only the planning result; the real initiation must
                    // leave idle state before HSE consumption can raise decisions.
                    0x47b240 => put(&mut player.bytes, 0x1040, 5),
                    0x47b3e0 => {
                        crate::ai_strategy::initiate(player, &image, |_, _| Ok(0))?;
                    }
                    0x467370 | 0x467440 => crate::ai_events::consume_player(
                        player,
                        profiles,
                        day_tick as i32,
                        call.address == 0x467370,
                        &[0; 52],
                        |_, _, _| panic!("unexpected timetable warning"),
                    )?,
                    0x468670 => {
                        let mut flags = word(&player.bytes, 0x220);
                        crate::ai_dispatch::advance(&mut flags, &mut false, |event, _, _, _| {
                            seen.push((day_tick, event));
                            if event == crate::ai_dispatch::Event::BusinessSettings {
                                crate::ai_business::update(
                                    &owned,
                                    &mut businesses,
                                    &inputs,
                                    player.bytes[0x1bc],
                                    tick,
                                    &mut commands,
                                    |_, _| panic!("settings applied before the 25-tick delay"),
                                )?;
                            }
                            Ok(0)
                        })?;
                        put(&mut player.bytes, 0x220, flags);
                    }
                    _ => {}
                }
                Ok(0)
            },
        )
        .unwrap();
        commands
            .drain(&mut [tick; 2], |command| {
                applied.push(day_tick);
                crate::save::restore::apply_business_terms_without_tenant_callback(
                    &mut businesses,
                    command.business_index,
                    &command.terms,
                )
            })
            .unwrap();
        if day_tick < 1575 {
            assert_eq!(word(&businesses[0], 0x1c4), 30f32.to_bits());
        }
    }
    assert_eq!(
        seen,
        [
            (1550, crate::ai_dispatch::Event::BusinessSettings),
            (1551, crate::ai_dispatch::Event::RentSettings),
        ]
    );
    assert_eq!(applied, [1575]);
    assert_eq!(&businesses[0][0x1b8..0x1ba], &25i16.to_le_bytes());
    assert_eq!(&businesses[0][0x1e0..0x1e2], &25i16.to_le_bytes());
    assert_eq!(word(&businesses[0], 0x1c4), 33f32.to_bits());
    assert_eq!(word(&businesses[0], 0x1ec), 33f32.to_bits());
}

#[test]
fn loaded_ai_prepares_auction_on_next_strategy_deadline() {
    let mut player = Player {
        reference: 0x20000000,
        bytes: vec![0; 0x1e50],
        auxiliaries: vec![],
    };
    put(&mut player.bytes, 0, 0x401000);
    put(&mut player.bytes, 0x154, 1);
    put(&mut player.bytes, 0x240, 2);
    player.bytes[0x1401] = 1;
    put(&mut player.bytes, 0x224, 7);
    put(&mut player.bytes, 0x228, 0x343fd);
    put(&mut player.bytes, 0x22c, 0x269ec3);
    let mut manager = Manager::initialize([0; 40]);
    manager.players.push(player);
    let mut profiles = [[0; 0x650]; 10];
    put(&mut profiles[0], 0, 0x20000000);
    profiles[0][4] = 1;
    let source = br#"
function EVENT_LoadHseKeepingTimetable(a) AiChars[a]:ResetTimeTable(HSE) end
function EVENT_LoadBuyBlockTimetable(a) AiChars[a]:ResetTimeTable(BLK) end
function EVENT_LoadDifficultyVariables(a) end
function EVENT_LoadBuildTimes(a) end
function EVENT_LoadScoreData() end
"#;
    AiScript::new()
        .unwrap()
        .initialize_loaded(
            &mut Initialization::<64> {
                precision: crate::x87::Precision::Pc64,
                profiles: &mut profiles,
                manager: &mut manager,
                category_counts: &mut [0; 52 * 45],
                chains: &std::array::from_fn(|_| vec![]),
                buildings: &[],
                businesses: &[],
                difficulty: 0,
                current_hour: 0,
                count_tick: Some(10002),
            },
            source,
            None,
        )
        .unwrap();
    assert_eq!(word(&manager.players[0].bytes, 0x224), 7);
    assert_eq!(&manager.players[0].bytes[0xb4c..0xb4e], &[0; 2]);
    let bytes = controller_image();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let mut selected = vec![];
    let mut prepared = vec![];
    for tick in 10002..=10127 {
        crate::ai_tick::advance(
            &mut manager.players[0],
            &mut profiles,
            &mut crate::ai_tick::Controls {
                normal_tick: tick,
                day_tick: 0,
                event_gate: false,
            },
            &image,
            |call, player, profiles, controls| {
                match call.address {
                    0x47d6a0 => return Ok(controls.normal_tick + 125),
                    0x47b8f0 => return Ok(1),
                    0x47b240 => {
                        selected.push(tick);
                        // Control the selected target, then use real action
                        // initiation and continuation to reach auction admission.
                        put(&mut player.bytes, 0x1040, 2);
                        put(&mut player.bytes, 0xb68, 3);
                        put(&mut player.bytes, 0xcb8, 500);
                    }
                    0x47b3e0 => {
                        assert!(crate::ai_strategy::initiate(player, &image, |call, _| {
                            assert_eq!(call, crate::ai_strategy::Call::CategoryPrice(3));
                            Ok(100)
                        })?);
                    }
                    0x467690 => {
                        crate::ai_strategy::continue_strategy(player, profiles, |call, _, _| {
                            match call {
                                crate::ai_strategy::ContinuationCall::Viable => return Ok(1),
                                crate::ai_strategy::ContinuationCall::Prepare {
                                    category,
                                    amount,
                                } => {
                                    prepared.push((tick, category, amount));
                                }
                                crate::ai_strategy::ContinuationCall::Action(_) => {}
                            }
                            Ok(0)
                        })?
                    }
                    _ => {}
                }
                Ok(0)
            },
        )
        .unwrap();
    }
    assert_eq!(selected, [10002]);
    assert_eq!(prepared, [(10127, 3, 100)]);
    assert_eq!(word(&manager.players[0].bytes, 0x240), 4);
    assert_eq!(manager.players[0].bytes[0x13c5], 1);
    assert_eq!(
        u16::from_le_bytes(manager.players[0].bytes[0xb4c..0xb4e].try_into().unwrap()),
        10252
    );
}
