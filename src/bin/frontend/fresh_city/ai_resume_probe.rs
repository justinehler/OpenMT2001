//! Two supplied human saves with a native-format AI player pair appended to
//! exercise the retained AI controller against both restored city states.
//! This is a controlled synthetic opponent, not a supplied AI save.
use super::test_observers::{assert_account_postings, cash};
use super::*;
use openmt2001::ai_messages::offers::auction::admission;
use openmt2001::save::SaveFile;

/// Keep the live auction action/controls bindings, with the same city-view
/// scene return values and detached audio as the headless renderer stand-in.
#[derive(Default)]
struct ProbeAuction {
    scene: Option<crate::live_host::Auction>,
    camera_active: bool,
    omitted: std::collections::BTreeSet<u32>,
    bid_gate: std::cell::Cell<bool>,
}

impl ProbeAuction {
    fn event(&mut self, event: openmt2001::live_commands::Event) -> Result<u32, String> {
        use openmt2001::ai_messages::offers::auction::{activation, closure};
        use openmt2001::live_commands::Event;
        match event {
            Event::AuctionScenePrepare => {
                if self.scene.is_some() {
                    return Err("Auction renderer already owns an active scene".into());
                }
                self.scene = Some(crate::live_host::Auction::default());
                self.bid_gate.set(false);
                Ok(0) // Previous interface mode: city view.
            }
            Event::AuctionActivation(activation::Call::Camera) => {
                if self.scene.is_none() || self.camera_active {
                    return Err("Auction camera requires an inactive prepared scene".into());
                }
                self.camera_active = true;
                Ok(0) // Previous renderer transition, as in headless::auction_stand_in.
            }
            Event::AuctionActivation(activation::Call::Ambient) => Ok(0), // Detached audio.
            Event::AuctionActivation(activation::Call::Action(call))
            | Event::AuctionAction(call) => crate::live_host::auction_action(
                call,
                &mut self.scene,
                &mut self.omitted,
                0, // Both supplied saves are non-tutorial sessions.
                &None,
                &self.bid_gate,
            ),
            Event::AuctionActivation(activation::Call::RefreshPanel) => {
                self.scene
                    .as_mut()
                    .ok_or("Auction labels have no prepared scene")?
                    .participants
                    .clear();
                Ok(0)
            }
            Event::AuctionActivation(activation::Call::NormalSpeed) => Ok(0), // Session owns speed.
            Event::AuctionTimeShow { .. } => {
                self.scene.as_ref().ok_or("Auction Time has no scene")?;
                Ok(0)
            }
            Event::AuctionClosure(closure::Call::RestorePresentation) => {
                if self.scene.is_none() || !self.camera_active {
                    return Err("Auction restore has no retained scene".into());
                }
                self.scene = None;
                self.camera_active = false;
                self.bid_gate.set(false);
                Ok(0)
            }
            // The session already owns announcement visibility, category locks,
            // settlement, messages and queued transfers. These are UI/audio only.
            Event::AuctionClosure(
                closure::Call::CloseAnnouncement
                | closure::Call::CategoryView(_)
                | closure::Call::CategoryStatus(_)
                | closure::Call::AudioReset4af140
                | closure::Call::Tutorial(_)
                | closure::Call::ReleaseAmbient(_),
            )
            | Event::AuctionAdmission(admission::Call::ReleaseAmbient(_)) => Ok(0),
            other => Err(format!("unbound probe auction event {other:?}")),
        }
    }

    fn pass(&mut self, session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
        use crate::auction_controls::{Bidder, Call, TickCall};
        if !self.scene.as_ref().is_some_and(|scene| scene.visible) {
            return;
        }
        let player = session
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == session.roster.status.current_player)
            .unwrap()
            .bytes[0x1bc];
        let record = &session.world.city.auction.controller.record;
        let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let bidder = Bidder {
            current: word(0x58),
            winner: word(0x60),
            price: word(0x6c),
            passed: record[usize::from(player) * 12 + 0x10],
            withdrawn: record[usize::from(player) * 12 + 0x12],
        };
        let mut controls = std::mem::take(&mut self.scene.as_mut().unwrap().controls);
        controls
            .tick(bidder, player, session.tutorial_state, |call| match call {
                TickCall::ClearText
                | TickCall::Price(_)
                | TickCall::TimerVisible(_)
                | TickCall::Tutorial(_) => Ok(None),
                TickCall::Withdraw(_) => Err("probe pass policy missed a human turn".into()),
            })
            .unwrap();
        // headless --auctions pass clicks button 3 only while enabled. That
        // button withdraws (action 2), rather than the separate pass-bid action.
        if controls.buttons_enabled & 8 != 0 {
            controls
                .button(3, bidder.current, player, bidder.price, |call| match call {
                    Call::Action(command) => session
                        .apply_auction_action(command, assets, |event| self.event(event))
                        .map(|_| true),
                    Call::HideTimer => Ok(true),
                    other => Err(format!("unexpected probe pass callback {other:?}")),
                })
                .unwrap();
        }
        self.scene.as_mut().unwrap().controls = controls;
    }
}

#[test]
fn auction_probe_retains_live_action_and_scene_contract() {
    use openmt2001::ai_messages::offers::auction::{Call, activation, closure};
    use openmt2001::live_commands::Event;
    let mut auction = ProbeAuction::default();
    assert!(
        auction
            .event(Event::AuctionAction(Call::RefreshTurn(0)))
            .is_err()
    );
    assert!(
        auction
            .event(Event::AuctionActivation(activation::Call::Camera))
            .is_err()
    );
    assert_eq!(auction.event(Event::AuctionScenePrepare), Ok(0));
    assert!(auction.event(Event::AuctionScenePrepare).is_err());
    assert_eq!(
        auction.event(Event::AuctionActivation(activation::Call::Camera)),
        Ok(0)
    );
    assert!(
        auction
            .event(Event::AuctionActivation(activation::Call::Camera))
            .is_err()
    );
    auction.bid_gate.set(true);
    assert_eq!(
        auction.event(Event::AuctionAction(Call::ShowPanel(0x565b9c))),
        Ok(0)
    );
    assert!(auction.scene.as_ref().unwrap().visible);
    assert!(!auction.bid_gate.get());
    assert!(
        auction
            .event(Event::AuctionAction(Call::ShowPanel(0)))
            .is_err()
    );
    assert_eq!(
        auction.event(Event::AuctionClosure(closure::Call::RestorePresentation)),
        Ok(0)
    );
    assert!(auction.scene.is_none());
    assert!(!auction.camera_active);
    assert!(
        auction
            .event(Event::AuctionClosure(closure::Call::RestorePresentation))
            .is_err()
    );
    assert_eq!(auction.event(Event::AuctionScenePrepare), Ok(0));
}

#[test]
#[ignore = "requires original saves and headless scheduler assets"]
fn continues_inserted_ai_from_supplied_saves() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| run(false))
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "requires original saved city, controlled delayed AI bakery and scheduler assets"]
fn inserted_ai_applies_overnight_retail_terms() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| run(true))
        .unwrap()
        .join()
        .unwrap();
}

fn submit_controlled_ai_bakery(session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) -> u32 {
    for category in 0..52 {
        let mut ai_context = crate::build_panel::Context::from_session(session, category).unwrap();
        ai_context.current_owner = 2;
        if !matches!(ai_context.entry(), crate::build_panel::Entry::Open) {
            continue;
        }
        if !matches!(
            crate::build_panel::Context::from_session(session, category)
                .unwrap()
                .entry(),
            crate::build_panel::Entry::Open
        ) {
            continue;
        }
        for x in 0..=5 {
            for y in 0..=5 {
                let request = crate::build_panel::Request {
                    category: category as i32,
                    origin: [x, y],
                    size: [2, 2],
                    height: 2,
                    packed_model: 0,
                    kind: 0,
                    quality: 0,
                    displayed_quote: 0.,
                    business: 0,
                };
                let inspected = session.inspect_build(&request, assets).unwrap();
                if !inspected.allowed() {
                    continue;
                }
                // This command is controlled input, admitted through the real
                // lot/model inspection and quoted for the synthetic AI owner.
                // Later management decisions use the untouched AI timetable.
                let mut command = inspected.command;
                command.owner = 2;
                let quote = openmt2001::live_commands::quote(
                    &mut command,
                    &mut session.world.city,
                    &session.roster,
                    assets.image,
                    session.world.clock.hour(),
                )
                .unwrap();
                let allowance =
                    i32::from_le_bytes(assets.image.at(0x541c7c, 4).unwrap().try_into().unwrap());
                if !openmt2001::construction::affordable(
                    session.world.ledgers.players[1].cash,
                    allowance,
                    quote as f32,
                    false,
                ) {
                    continue;
                }
                session
                    .submit_command(
                        openmt2001::live_commands::Command::Build(command),
                        assets,
                        |_| panic!("controlled bakery applied before its native delay"),
                    )
                    .unwrap();
                println!(
                    "controlled AI bakery admitted at tick{} category{category} quote{quote}",
                    session.world.clock.timing[0]
                );
                return category as u32;
            }
        }
    }
    panic!("supplied Sandbox city has no legal controlled AI bakery lot");
}

fn run(controlled_retail: bool) {
    let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let built = read("OPENMT2001_BUILD_FILE");
    let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = read("OPENMT2001_PARAMETERS");
    let timetable = read("OPENMT2001_TIMETABLE");
    let scripts = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .join("scripts");
    let era_styles = load_era_styles(
        Path::new(&openmt2001::fixtures::path("PARAMETERS"))
            .parent()
            .unwrap(),
    )
    .unwrap();
    let names = std::array::from_fn(|_| Vec::new());
    for (key, scenario, difficulty) in [
        ("OPENMT2001_SAVE_TWO", "sandbox", 0),
        ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1),
    ] {
        if controlled_retail && key != "OPENMT2001_SAVE_TWO" {
            continue;
        }
        let saved = read(key);
        let source = SaveFile::parse(&saved).unwrap();
        let saved_players = source.players().unwrap();
        assert_eq!(
            saved_players.len(),
            1,
            "supplied source save unexpectedly has AI"
        );
        let player_section = source.sections.iter().find(|s| s.tag == 0x40).unwrap();
        let insert_at = player_section.offset + 8 + player_section.payload.len();
        let mut metadata = *saved_players[0].metadata;
        metadata[..4].copy_from_slice(&1u32.to_le_bytes());
        metadata[40..44].copy_from_slice(&2u32.to_le_bytes());
        let mut opponent = *saved_players[0].bytes;
        // The supplied row is human. These are the native-format AI identity
        // fields used by the existing saved-player constructor test.
        opponent[0x14c..0x150].copy_from_slice(&5u32.to_le_bytes());
        opponent[0x150..0x154].copy_from_slice(&1u32.to_le_bytes());
        opponent[0x1b4..0x1b8].fill(0); // no inherited human held card
        let mut pair = Vec::new();
        for (tag, payload) in [
            (0x80000u32, metadata.as_slice()),
            (0x40, opponent.as_slice()),
        ] {
            pair.extend_from_slice(&tag.to_le_bytes());
            pair.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            pair.extend_from_slice(payload);
        }
        let mut with_opponent = saved[..insert_at].to_vec();
        with_opponent.extend_from_slice(&pair);
        with_opponent.extend_from_slice(&saved[insert_at..]);
        let mut session = Session::load_original(
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
                era_styles: era_styles.clone(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario,
                difficulty,
                selected_character: 4,
                profile_name: b"Saved AI diagnostic",
                character_names: &names,
            },
            Policy {
                // Exercise auctions, construction and overnight retail after the startup reseed.
                seed: 2,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            &with_opponent,
        )
        .unwrap();
        assert_eq!(session.roster.manager.players.len(), 2);
        assert!(session.roster.status.has_ai);
        let ai = &mut session.roster.manager.players[1];
        // The copied human row is disabled for AI ticking until this byte is cleared.
        ai.bytes[0x1403] = 0;
        let initial_cash = f32::from_le_bytes(ai.bytes[4..8].try_into().unwrap());
        let start_tick = session.world.clock.timing[0];
        let start_hour = session.world.clock.hour();
        let assets = CommandAssets {
            image: &image,
            board: &board,
            file: &file,
        };
        let mut policy = tick::Policy {
            scheduler: openmt2001::scheduler::Controls {
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
            hourly: openmt2001::hourly::Controls {
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
        };
        let mut completed = 0;
        let mut last_hour = start_hour;
        let mut first_error = None;
        let mut outcome_notice = None;
        let mut ai_business_opened = false;
        let mut construction_notice = None;
        let mut construction_postings = Vec::new();
        let mut recent_postings = std::collections::VecDeque::new();
        let mut prior_retail = std::collections::BTreeMap::new();
        let mut settings_change = None;
        let mut settings_applied = None;
        let mut retail_restocked = false;
        let mut controlled_bakery = None;
        let mut auction_announcements = 0;
        let mut auction = ProbeAuction::default();
        let horizon = if key == "OPENMT2001_SAVE_TWO" {
            14_200
        } else {
            6_700
        };
        for _ in 0..horizon {
            let before_cash = cash(&session);
            let mut postings = Vec::new();
            let mut auction_admission = Vec::new();
            let mut property_transfers = Vec::new();
            let result = session.advance_session(&assets, &mut policy, |event, _| match event {
                tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                    0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                )) => Ok(0),
                tick::Event::Tick(openmt2001::scheduler::Event::BeforeSixSound) => Ok(0),
                tick::Event::Script { commands: [], .. } => Ok(0),
                tick::Event::ScriptCommands([]) => Ok(0),
                tick::Event::TransportSound(_) => Ok(0),
                tick::Event::TransportEnvironment { .. } => Ok(0),
                tick::Event::Calendar(openmt2001::level_runtime::CalendarEvent::Clock(
                    openmt2001::clock::Event::Hour19
                    | openmt2001::clock::Event::Hour7
                    | openmt2001::clock::Event::RolloverQuery,
                )) => Ok(0),
                tick::Event::Calendar(openmt2001::level_runtime::CalendarEvent::Hourly(
                    openmt2001::hourly::Event::Helper(_) | openmt2001::hourly::Event::Query,
                )) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::OutcomeNotice {
                    player,
                    reason,
                }) => {
                    outcome_notice = Some((player, reason));
                    Ok(0)
                }
                tick::Event::Command(openmt2001::live_commands::Event::Outcome(_)) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::Construction(
                    openmt2001::construction::Event::Appearance(_),
                )) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::AccountPosted(posting)) => {
                    postings.push(posting);
                    if recent_postings.len() == 4 {
                        recent_postings.pop_front();
                    }
                    recent_postings.push_back((posting.player, posting.amount));
                    Ok(0)
                }
                tick::Event::AccountPosted(posting) => {
                    postings.push(posting);
                    Ok(0)
                }
                tick::Event::Command(openmt2001::live_commands::Event::AuctionAdmission(
                    call @ (admission::Call::ShowAnnouncement | admission::Call::CategoryStatus(_)),
                )) => {
                    auction_admission.push(call);
                    Ok(0)
                }
                tick::Event::Command(
                    event @ (openmt2001::live_commands::Event::AuctionScenePrepare
                    | openmt2001::live_commands::Event::AuctionActivation(_)
                    | openmt2001::live_commands::Event::AuctionAction(_)
                    | openmt2001::live_commands::Event::AuctionTimeShow { .. }
                    | openmt2001::live_commands::Event::AuctionClosure(_)
                    | openmt2001::live_commands::Event::AuctionAdmission(
                        admission::Call::ReleaseAmbient(_),
                    )),
                ) => auction.event(event),
                tick::Event::Ai(openmt2001::ai_runtime::Call::AuctionAction(call)) => {
                    auction.event(openmt2001::live_commands::Event::AuctionAction(call))
                }
                tick::Event::Command(
                    openmt2001::live_commands::Event::PropertyTransferNotice {
                        category,
                        owner,
                        amount,
                    },
                ) => {
                    // LiveHost publishes a notice after the session settles the
                    // transfer. Check that notification against the world below.
                    property_transfers.push((category, owner, amount));
                    Ok(0)
                }
                tick::Event::Command(openmt2001::live_commands::Event::PropertyOwnership(
                    openmt2001::category_ownership::Seam::BuildingAppearance { .. },
                )) => Ok(0), // LiveHost's renderer-only appearance refresh.
                // Property-view locks are already updated by the session;
                // LiveHost only refreshes the property panel at this boundary.
                tick::Event::Command(openmt2001::live_commands::Event::PropertyView(_)) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::Construction(
                    openmt2001::construction::Event::RefreshInterface,
                )) => {
                    assert_eq!(recent_postings.len(), 4);
                    construction_postings.extend(recent_postings.drain(..));
                    Ok(0)
                }
                tick::Event::Pending(
                    openmt2001::live_commands::PendingCall::World(
                        openmt2001::pending_world::Event::Progress { .. },
                    ),
                    _,
                ) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::RefreshInterface) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::Business(
                    openmt2001::business_binding::Call::RefreshBuilding { .. },
                )) => Ok(0),
                tick::Event::Command(openmt2001::live_commands::Event::ConstructionNotice {
                    building,
                    owner,
                    structure_kind,
                    business,
                    business_kind,
                    category,
                }) => {
                    construction_notice = Some((
                        building,
                        owner,
                        structure_kind,
                        business,
                        business_kind,
                        category,
                    ));
                    Ok(0)
                }
                _ => Err(format!("unbound post-load event {event:?}")),
            });
            if let Err(error) = result {
                first_error = Some(error);
                break;
            }
            assert_account_postings(&before_cash, &cash(&session), &postings);
            for (category, owner, amount) in property_transfers {
                assert_eq!(session.world.categories[category as usize][5], owner);
                assert!(
                    postings.iter().any(|posting| {
                        posting.player == usize::from(owner) - 1
                            && posting.amount == -(amount as f32)
                    }),
                    "property transfer omitted its purchase posting"
                );
            }
            policy.ai_event_gate = auction.bid_gate.get();
            auction.pass(&mut session, &assets);
            if !auction_admission.is_empty() {
                let auction = &session.world.city.auction;
                let word = |at| {
                    u32::from_le_bytes(auction.controller.record[at..at + 4].try_into().unwrap())
                };
                let category = word(4);
                assert_eq!(
                    auction_admission,
                    [
                        admission::Call::ShowAnnouncement,
                        admission::Call::CategoryStatus(category)
                    ],
                    "auction admission must announce before publishing category status"
                );
                assert_ne!(auction.controller.reference, 0);
                assert_eq!(auction.controller.active_count, 1);
                assert_eq!(auction.controller.record[0x7d], 1);
                assert!(auction.presentation.visible);
                assert_eq!(
                    auction.presentation.announcement,
                    [category, word(0x5c), word(0x88)]
                );
                assert!((word(0x5c) as usize) < session.roster.manager.players.len());
                assert_eq!(auction.presentation.countdown, 120 * 50);
                assert_eq!(session.world.categories[category as usize][0x6c], 1);
                auction_announcements += 1;
            }
            completed += 1;
            if outcome_notice.is_some() {
                break;
            }
            ai_business_opened |= session
                .world
                .ledgers
                .businesses
                .iter()
                .any(|b| b[0x21c] != 0 && b[10] == 2);
            if controlled_retail && controlled_bakery.is_none() && ai_business_opened {
                controlled_bakery = Some(submit_controlled_ai_bakery(&mut session, &assets));
            }
            for (slot, row) in session
                .world
                .ledgers
                .businesses
                .iter()
                .enumerate()
                .filter(|(_, b)| b[0x21c] != 0 && b[10] == 2 && b[..4] == [0; 4])
            {
                let live = (
                    i16::from_le_bytes(row[0x1b8..0x1ba].try_into().unwrap()),
                    u32::from_le_bytes(row[0x1c4..0x1c8].try_into().unwrap()),
                );
                let pending = (
                    i16::from_le_bytes(row[0x1e0..0x1e2].try_into().unwrap()),
                    u32::from_le_bytes(row[0x1ec..0x1f0].try_into().unwrap()),
                );
                if let Some((old_live, old_pending)) = prior_retail.insert(slot, (live, pending)) {
                    if old_pending != pending && settings_change.is_none() {
                        settings_change = Some((
                            session.world.clock.timing[0],
                            slot,
                            old_live,
                            pending,
                            session.world.clock.hour(),
                        ));
                        println!(
                            "{key} AI stock/price pending change tick{} business{slot}: live{old_live:?} pending{old_pending:?}->{pending:?}",
                            session.world.clock.timing[0]
                        );
                    }
                    if old_live != live {
                        println!(
                            "{key} AI stock/price live change tick{} business{slot}: {old_live:?}->{live:?}",
                            session.world.clock.timing[0]
                        );
                    }
                }
                if let Some((changed, selected, old_live, expected, _)) = settings_change
                    && selected == slot
                {
                    let now = session.world.clock.timing[0];
                    if now < changed + 25 {
                        assert_eq!(
                            live, old_live,
                            "AI retail terms applied before 25-tick deadline"
                        );
                    } else {
                        assert_eq!(live, expected, "AI retail terms missed 25-tick deadline");
                        if settings_applied.is_none() {
                            settings_applied = Some(now);
                            println!(
                                "{key} AI overnight retail terms applied at tick{now}, submitted{changed}"
                            );
                        }
                    }
                }
            }
            if session.world.clock.hour() != last_hour {
                last_hour = session.world.clock.hour();
                if controlled_retail && session.world.clock.calendar[4] >= 1 && last_hour == 6 {
                    let (_, slot, _, pending, _) = settings_change
                        .expect("morning arrived before AI retail settings decision");
                    let row = &session.world.ledgers.businesses[slot];
                    let stock = i16::from_le_bytes(row[0x80..0x82].try_into().unwrap());
                    let cost = f32::from_le_bytes(row[0x30..0x34].try_into().unwrap());
                    assert_eq!(
                        stock, pending.0,
                        "morning replenishment ignored applied AI target"
                    );
                    assert!(
                        stock > 0 && cost < 0.,
                        "AI replenishment omitted stock or its cost"
                    );
                    retail_restocked = true;
                    println!(
                        "{key} AI morning replenishment tick{} stock{stock} expense{cost}",
                        session.world.clock.timing[0]
                    );
                }
                let ai = &session.roster.manager.players[1];
                let owned = session
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|b| b[0x21c] != 0 && b[10] == 2)
                    .count();
                let structures = session
                    .world
                    .buildings
                    .iter()
                    .filter(|b| b[0x3f] != 0 && b[0x2d] == 2)
                    .count();
                let retail: Vec<_> = session
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b[0x21c] != 0 && b[10] == 2 && b[..4] == [0; 4])
                    .map(|(slot, b)| {
                        (
                            slot,
                            u16::from_le_bytes(b[0x80..0x82].try_into().unwrap()),
                            i16::from_le_bytes(b[0x1b8..0x1ba].try_into().unwrap()),
                            i16::from_le_bytes(b[0x1e0..0x1e2].try_into().unwrap()),
                            f32::from_le_bytes(b[0x1c4..0x1c8].try_into().unwrap()),
                            f32::from_le_bytes(b[0x1ec..0x1f0].try_into().unwrap()),
                        )
                    })
                    .collect();
                println!(
                    "{key} tick{} hour{} cash{} businesses{} buildings{} controller{:?} df8{} strategy{} decision{} action{} owner{} flags{:x}",
                    session.world.clock.timing[0],
                    last_hour,
                    f32::from_le_bytes(ai.bytes[4..8].try_into().unwrap()),
                    owned,
                    structures,
                    &ai.bytes[0x1400..0x1404],
                    ai.bytes[0x1df8],
                    u32::from_le_bytes(ai.bytes[0xb64..0xb68].try_into().unwrap()),
                    u32::from_le_bytes(ai.bytes[0xb54..0xb58].try_into().unwrap()),
                    u32::from_le_bytes(ai.bytes[0x1040..0x1044].try_into().unwrap()),
                    ai.bytes[0x1bc],
                    u32::from_le_bytes(ai.bytes[0x220..0x224].try_into().unwrap())
                );
                println!(
                    "{key} owned retail stock/live-target/pending-target/live-price/pending-price {retail:?}"
                );
            }
            if session.world.clock.timing[0].wrapping_sub(start_tick) >= horizon as u32 - 200 {
                break;
            }
        }
        let ai = &session.roster.manager.players[1];
        println!(
            "{key} completed{completed} tick{} initial-cash{initial_cash} final-cash{} controller{:?} df8{} opened{ai_business_opened} outcome{outcome_notice:?} error{first_error:?}",
            session.world.clock.timing[0],
            f32::from_le_bytes(ai.bytes[4..8].try_into().unwrap()),
            &ai.bytes[0x1400..0x1404],
            ai.bytes[0x1df8]
        );
        println!("{key} first AI retail settings change {settings_change:?}");
        println!("{key} last AI construction notice {construction_notice:?}");
        assert!(first_error.is_none(), "{key}: {first_error:?}");
        assert!(
            completed > 25,
            "saved AI stopped before a real controller interval"
        );
        if key == "OPENMT2001_SAVE_TWO" {
            assert!(
                auction_announcements > 0,
                "resumed AI never announced an auction"
            );
            assert!(
                ai_business_opened,
                "resumed AI did not open an owned business"
            );
            assert!(
                session.world.clock.calendar[4] >= 1 && session.world.clock.hour() >= 6,
                "resumed city stopped before the next morning business boundary"
            );
            let notice = construction_notice.expect("resumed AI construction never opened");
            assert_eq!(notice.1, 2, "construction owner differs from injected AI");
            assert!(
                !construction_postings.is_empty() && construction_postings.len() % 4 == 0,
                "construction omitted native ledger postings"
            );
            assert!(
                construction_postings
                    .iter()
                    .all(|(player, amount)| *player == 1 && *amount <= 0.)
            );
            assert!(
                f32::from_le_bytes(ai.bytes[4..8].try_into().unwrap()) < initial_cash,
                "AI construction did not charge restored player"
            );
            if controlled_retail {
                let category = controlled_bakery.expect("controlled bakery was never admitted");
                let (changed, slot, old_live, pending, hour) = settings_change
                    .expect("normal AI timetable never changed controlled retail terms");
                let row = &session.world.ledgers.businesses[slot];
                assert_eq!(u32::from_le_bytes(row[4..8].try_into().unwrap()), 0);
                let building = usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()));
                assert_eq!(
                    session.population.shopping.building_categories[building],
                    category as i16
                );
                assert_ne!(old_live, pending);
                assert_eq!(
                    hour, 2,
                    "original EOD stock/price timetable did not run overnight"
                );
                assert_eq!(settings_applied, Some(changed + 25));
                assert!(retail_restocked);
            }
        } else {
            assert_eq!(
                outcome_notice,
                Some((0, 2)),
                "saved scenario result boundary changed"
            );
        }
    }
}
