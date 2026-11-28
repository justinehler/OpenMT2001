//! Modern continuation comparisons, not an
//! independent original mixed-session oracle. Original fixtures stay external.
use super::*;
use crate::fresh_city::{CommandAssets, Inputs, Policy};
use openmt2001::event_cards::{human, lifecycle};
use std::{
    path::Path,
    sync::{Arc, mpsc},
    thread::JoinHandle,
};

#[test]
#[ignore = "requires original assets; mixed card/midnight continuation"]
fn natural_auction_then_pending_card_across_midnight_reload() {
    run(23);
}

#[test]
#[ignore = "requires original assets; mixed card/05-to-06 continuation"]
fn natural_auction_then_pending_card_across_six_reload() {
    run(5);
}

fn presenter(
    exe: Arc<Vec<u8>>,
    directory: Arc<Vec<u8>>,
    archive: Arc<Vec<u8>>,
) -> (mpsc::Sender<crate::auction_bridge::Request>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<crate::auction_bridge::Request>();
    let worker = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view = crate::auction_bridge::View::default();
            for request in rx {
                let result = view.present(
                    request.operation,
                    &renderer,
                    (&exe, &directory, &archive),
                    0,
                );
                request.reply.send(result).unwrap();
            }
        })
        .unwrap();
    (tx, worker)
}

fn step(
    s: &mut Session<'_>,
    h: &mut crate::live_host::Host,
    a: &CommandAssets<'_, '_>,
) -> Vec<crate::prompt::Request> {
    h.frame(s, a, 40);
    assert!(h.failure.is_none(), "Host stopped: {:?}", h.failure);
    let prompts = h.take_prompts(s);
    for prompt in prompts.iter().cloned() {
        if let Some(dialog) = prompt.dialog {
            h.human_dialog_reply(
                s,
                a,
                crate::human_dialog::Reply {
                    callback: dialog.callback,
                    context: dialog.context,
                    button: if dialog.button_count == 2 { 1 } else { 0 },
                },
            )
            .unwrap();
        } else {
            s.dispatch_session_event(
                "InterfacePromptDismiss",
                prompt.dismissal_id as i32,
                a,
                true,
            )
            .unwrap();
        }
    }
    prompts
}

fn accounts(s: &Session<'_>) -> Vec<Vec<u8>> {
    s.world
        .ledgers
        .players
        .iter()
        .map(|a| {
            let mut b = vec![0; 0x1b8];
            a.write_runtime(&mut b).unwrap(); // includes BOTH physical banks and period selector
            b
        })
        .collect()
}

fn queue(s: &Session<'_>) -> Vec<(u32, u32, u32, bool, Vec<u8>)> {
    s.live_commands
        .snapshot()
        .unwrap()
        .into_iter()
        .map(|(domain, outer, admission, direct, c)| {
            let d = match domain {
                openmt2001::scheduler::ClockDomain::Total => 0,
                openmt2001::scheduler::ClockDomain::Normal => 1,
            };
            // A comparison projection only; it does not define the production codec.
            let bytes = match c {
                Command::CategoryReady { category } => words([0xf000, *category]),
                Command::EventCard(c) => words([0xf001, c.card, c.player, c.target as u32]),
                Command::Eliminate { slot } => words([0xf002, *slot as u32]),
                Command::OutcomeNotice { player, reason } => {
                    words([0xf003, *player as u32, *reason])
                }
                c => packet(c, admission).unwrap(),
            };
            (d, outer, admission, direct, bytes)
        })
        .collect()
}

fn equal(a: &mut Session<'_>, b: &mut Session<'_>) {
    let tick = a.world.clock.timing[0];
    assert_eq!(a.world.clock, b.world.clock, "clock at {tick}");
    assert!(
        accounts(a) == accounts(b),
        "cash, BOTH ledger banks and selector at {tick}"
    );
    assert_eq!(
        a.roster.manager.players.len(),
        b.roster.manager.players.len()
    );
    for (x, y) in a
        .roster
        .manager
        .players
        .iter()
        .zip(&b.roster.manager.players)
    {
        assert!(x.bytes == y.bytes, "complete player at {tick}");
        assert_eq!(x.auxiliaries.len(), y.auxiliaries.len());
        for (x, y) in x.auxiliaries.iter().zip(&y.auxiliaries) {
            assert_eq!(x.reference, y.reference);
            assert!(x.bytes == y.bytes, "player auxiliary at {tick}");
        }
    }
    assert!(
        a.world.ledgers.businesses == b.world.ledgers.businesses,
        "businesses at {tick}"
    );
    let residents = |s: &Session<'_>| {
        s.world
            .ledgers
            .residents
            .nodes
            .iter()
            .map(|r| {
                (
                    r.as_bytes().to_vec(),
                    r.active_byte(),
                    r.auxiliary_reference(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        a.world.ledgers.residents.active_count,
        b.world.ledgers.residents.active_count
    );
    assert!(
        residents(a) == residents(b),
        "resident links, allocation and state at {tick}"
    );
    assert_eq!(
        a.world.ledgers.residents.accounts.len(),
        b.world.ledgers.residents.accounts.len()
    );
    for (x, y) in a
        .world
        .ledgers
        .residents
        .accounts
        .iter()
        .zip(&b.world.ledgers.residents.accounts)
    {
        assert!(x.as_bytes() == y.as_bytes(), "resident account at {tick}");
    }
    for (x, y) in a.world.categories.iter().zip(&b.world.categories) {
        // Native stream membership handles12..108 and renderer tail rebuild on load.
        assert_eq!(&x[..12], &y[..12]);
        assert_eq!(&x[108..140], &y[108..140]);
    }
    let chains = |s: &Session<'_>| {
        openmt2001::building_pool::category_chains(
            &s.world.categories,
            &s.world.buildings,
            0x591318,
        )
        .unwrap()
    };
    assert!(
        chains(a) == chains(b),
        "ordered ownership/business chains at {tick}"
    );
    assert_eq!(
        queue(a),
        queue(b),
        "Total then Normal, outer/inner deadlines and stable tie order at {tick}"
    );
    assert_eq!(
        a.world.pending_world.to_bytes(),
        b.world.pending_world.to_bytes()
    );
    let callbacks = |s: &mut Session<'_>| {
        s.scripts
            .events
            .pending()
            .iter()
            .map(|e| {
                (
                    e.remaining,
                    e.argument,
                    s.scripts
                        .runtime
                        .event_reference_identity(e.reference)
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        callbacks(a),
        callbacks(b),
        "ordered named callbacks at {tick}"
    );
    assert_eq!(a.roster.profiles, b.roster.profiles);
    assert_eq!(a.event_cards.cards, b.event_cards.cards);
    assert_eq!(a.event_cards.used, b.event_cards.used);
    assert_eq!(a.event_cards.remaining, b.event_cards.remaining);
    assert_eq!(a.event_cards.rng, b.event_cards.rng);
    assert_eq!(a.random_registry.process, b.random_registry.process);
    assert_eq!(a.script_random_state, b.script_random_state);
    assert_eq!(a.credit_random_state, b.credit_random_state);
    assert_eq!(a.particle_random_state, b.particle_random_state);
    assert_eq!(a.build_random_state, b.build_random_state);
    assert_eq!(
        a.world.creation.initialization_rng.constructor,
        b.world.creation.initialization_rng.constructor
    );
    assert_eq!(
        a.world.creation.initialization_rng.movement,
        b.world.creation.initialization_rng.movement
    );
    assert_eq!(
        a.world.creation.initialization_rng.objective,
        b.world.creation.initialization_rng.objective
    );
    assert_eq!(a.world.creation.identity_rng, b.world.creation.identity_rng);
    assert_eq!(
        a.world.creation.preference_rng,
        b.world.creation.preference_rng
    );
    assert_eq!(
        a.road_traffic.admission.random,
        b.road_traffic.admission.random
    );
}

fn durable(path: &Path, bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    std::fs::read(path).unwrap()
}

fn run(boundary_hour: u32) {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(move || {
        let started = std::time::Instant::now();
        let out = std::path::PathBuf::from(std::env::var("OPENMT2001_MIXED_SAVE_OUTPUT").unwrap())
            .join(format!("hour-{boundary_hour}"));
        std::fs::create_dir(&out).expect("new per-case directory required; never overwrite an old receipt");
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let exe = Arc::new(read("OPENMT2001_EXE"));
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let directory = Arc::new(read("OPENMT2001_ARCHIVE_DIR"));
        let archive = Arc::new(read("OPENMT2001_ARCHIVE_BIN"));
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default.parent().unwrap().parent().unwrap();
        let styles = crate::fresh_city::load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap();
        let exe_path = openmt2001::fixtures::path("EXE");
        let names = crate::fresh_city::character_names(&image, &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        let inputs = || Inputs {
            tree_locators: None, image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(), era_styles: styles.clone(),
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "SCENARIO1", difficulty: 0,
            selected_character: 4, profile_name: b"Mixed boundary regression", character_names: &names,
        };
        let policy = || Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 284] };
        let assets = CommandAssets { image: &image, board: &board, file: &file };
        let camera = openmt2001::flyby::CameraSample { eye: [100.,120.,100.], target: [0.;3], up: [0.,1.,0.], vertical_fov: 0.7 };
        // Fixed presentation pair is explicitly outside this simulation test's claim.
        let cameras = crate::save_menu::cameras(None, camera).unwrap();
        let mut source = Session::initialize(inputs(), policy()).unwrap();
        assert_eq!(source.roster.manager.players.len(), 4);
        assert!(source.world.ledgers.businesses.iter().any(|b| b[0x21c] != 0));
        // Controlled original request32 input, not an allocation/deck/queue edit.
        // Request before time advances so delivery follows the normal human inbox.
        // Card1 is the retained original non-targeted200-dollar financial case.
        assert_eq!(source.event_cards.used[1],0);
        source.human_card(crate::fresh_city::cards::Action::Request(Some(1)), &assets,
            [false,false], |_,_| Ok(())).unwrap();
        let mut host = crate::live_host::Host::new(camera.eye);
        host.begin_presentation(&source).unwrap(); host.running = true;
        let (renderer, render_thread) = presenter(exe.clone(), directory.clone(), archive.clone());
        host.auction_renderer = Some(renderer);
        let mut countdown_seen = false;
        let mut at_boundary = false;
        for _ in 0..40_000 {
            assert!(started.elapsed().as_secs() < 180, "finite preparation deadline");
            step(&mut source, &mut host, &assets);
            if !countdown_seen && source.world.auction.presentation.countdown > 0
                && source.world.auction.controller.reference != 0 {
                let bytes = source.save_v2_with_auction_view(&image, cameras, 1, host.auction_save_state(), 0).unwrap();
                let bytes = durable(&out.join("countdown.sav"), &bytes);
                let saved = SaveFile::parse(&bytes).unwrap();
                let mut loaded = Session::load_original(inputs(), policy(), &bytes).unwrap();
                let native_allocation_next = loaded.allocation_next;
                let continuation = loaded.restore_v2_extension_with_image(&saved, &image).unwrap().unwrap().auction.unwrap();
                continuation_test::load_boundary(&mut source, &loaded, &assets, &saved, native_allocation_next);
                host.restore_auction_widgets(continuation.host.as_ref(), &source).unwrap();
                let mut loaded_host = crate::live_host::Host::new(camera.eye);
        loaded_host.begin_presentation(&loaded).unwrap(); loaded_host.running = true;
                let (tx, worker) = presenter(exe.clone(), directory.clone(), archive.clone());
                loaded_host.auction_renderer = Some(tx);
                loaded_host.activate_saved_auction(&mut loaded).unwrap();
                continuation.apply(&mut loaded).unwrap();
                loaded_host.restore_auction_widgets(continuation.host.as_ref(), &loaded).unwrap();
                equal(&mut source, &mut loaded);
                for _ in 0..16 { step(&mut source, &mut host, &assets); step(&mut loaded, &mut loaded_host, &assets); equal(&mut source, &mut loaded); }
                drop(loaded_host); worker.join().unwrap();
                countdown_seen = true;
            }
            if source.world.clock.hour() == boundary_hour && source.world.clock.timing[2] == 610
                && source.world.clock.timing[5] == 1 && !host.auction_save_state().is_some_and(|s| s.visible) {
                at_boundary = true; break;
            }
        }
        assert!(countdown_seen, "natural opponent auction countdown must actually occur");
        assert!(at_boundary, "did not reach the exact late-hour normal-mode cut within40k frames");
        let human_slot = source.roster.manager.players.iter().position(|p|
            p.reference == source.roster.status.current_player).unwrap();
        // The early request must have traversed the normal inbox into a real
        // held card. Never assign card/deck/player state or retire an AI's card.
        let auxiliary = |s: &Session<'_>| {
            let p = &s.roster.manager.players[human_slot];
            let a = p.auxiliaries.iter().find(|a| a.reference == word(&p.bytes,0x21c)).unwrap();
            (word(&a.bytes,4), word(&a.bytes,0x18))
        };
        let (card, mode) = auxiliary(&source);
        assert_eq!(card,1,"specific request must actually deliver through the original inbox");
        assert_ne!(mode, 0, "requested card must still be held");
        assert_ne!(word(lifecycle::card_row(&image,card).unwrap(),0), 2,
            "held targeted card has no legal target in this bounded case; do not replace it or weaken coverage");
        assert!(source.world.ledgers.businesses.iter().any(|b| b[0x21c] != 0 && b[10] != 0),
            "actual owned businesses required for mixed settlement");
        let tick = source.world.clock.timing[0];
        let cash_before = accounts(&source);
        let mut submitted = 0;
        let mut produced = None;
        source.human_card(crate::fresh_city::cards::Action::Open, &assets, [false,false], |call,_| {
            assert!(matches!(call, human::Call::Present(_) | human::Call::Activate)); Ok(())
        }).unwrap();
        for (domain,outer,inner,direct,c) in source.live_commands.snapshot().unwrap() {
            if let Command::EventCard(c) = c {
                if c.card == card && c.player as usize == human_slot {
                    assert!(matches!(domain,openmt2001::scheduler::ClockDomain::Total));
                    assert_eq!(outer,tick+25); assert_eq!(inner,tick+25); assert!(!direct); submitted += 1; produced = Some(*c);
                }
            }
        }
        assert_eq!(submitted,1,"real human producer must admit one delayed card");
        let produced = produced.unwrap();
        let due = tick + 25;
        let card_row = lifecycle::card_row(&image, card).unwrap();
        assert_eq!(word(card_row, 8), 200, "original card1 amount operand");
        assert_eq!(word(card_row, 12), 0, "original card1 direct financial family");
        assert_eq!(image.f32_at(0x534200).unwrap(), 1., "original currency scale");
        assert_eq!(card_row[16], 0, "original card1 notice gate is disabled; financial posting proves its effect");
        let notice_body = word(card_row, 4).wrapping_add(1);
        let notice_actor: [u8;96] = source.roster.manager.players[human_slot].bytes[0x158..0x1b8].try_into().unwrap();
        let notices = |prompts: &[crate::prompt::Request]| prompts.iter().filter(|p|
            p.title == 1488 && p.body == notice_body && p.card.is_some_and(|c| c.actor == notice_actor)).count();
        let pending = |s: &Session<'_>| s.live_commands.snapshot().unwrap().iter().filter(|(_,_,_,_,c)|
            matches!(c, Command::EventCard(c) if *c == produced)).count();
        assert_eq!(source.event_cards.used[card as usize], 1);

        assert!(accounts(&source) == cash_before, "submission must not apply the effect early");
        // Preserve the future card in both the native and extension queues.
        let bytes = source.save_v2_with_auction_view(&image,cameras,1,host.auction_save_state(),0)
            .expect("a valid pending event-card queue must be saveable");
        let bytes = durable(&out.join("pending-card.sav"), &bytes);
        let saved = SaveFile::parse(&bytes).unwrap();
        let mut loaded = Session::load_original(inputs(),policy(),&bytes).unwrap();
        let native_allocation_next = loaded.allocation_next;
        let metadata = loaded.restore_v2_extension_with_image(&saved,&image).unwrap().unwrap();
        continuation_test::load_boundary(&mut source, &loaded, &assets, &saved, native_allocation_next);
        let mut loaded_host = crate::live_host::Host::new(camera.eye);
        loaded_host.begin_presentation(&loaded).unwrap(); loaded_host.running = true;
        let (tx, worker) = presenter(exe.clone(),directory.clone(),archive.clone());
        loaded_host.auction_renderer = Some(tx);
        if let Some(continuation) = metadata.auction {
            loaded_host.activate_saved_auction(&mut loaded).unwrap();
            continuation.apply(&mut loaded).unwrap();
            loaded_host.restore_auction_widgets(continuation.host.as_ref(),&loaded).unwrap();
        }
        equal(&mut source,&mut loaded);
        let mut crossed = false;
        let mut delivered = false;
        let mut notice_counts = [0usize;2];
        for _ in 0..96 {
            assert!(started.elapsed().as_secs() < 180,"finite continuation deadline");
            let old_hour = source.world.clock.hour();
            let before = accounts(&source);
            let left_prompts = step(&mut source,&mut host,&assets);
            let right_prompts = step(&mut loaded,&mut loaded_host,&assets);
            equal(&mut source,&mut loaded);
            notice_counts[0] += notices(&left_prompts);
            notice_counts[1] += notices(&right_prompts);
            if !delivered && source.world.clock.timing[0] < due {
                for branch in [&source, &loaded] {
                    assert_eq!(pending(branch), 1, "exact command remains until its deadline");
                    assert_eq!(branch.event_cards.used[card as usize], 1);
                }
                assert_eq!(notice_counts, [0,0], "no early card presentation");
            } else if !delivered {
                assert!(crossed, "hour boundary must precede delayed financial delivery");
                assert_eq!(old_hour, source.world.clock.hour(), "delivery cut is separate from hourly settlement");
                for branch in [&source, &loaded] {
                    assert_eq!(pending(branch), 0, "first frame reaching deadline drains exact command");
                    assert_eq!(branch.event_cards.used[card as usize], 0, "actual card release");
                }
                let after = accounts(&source);
                let old = &before[human_slot]; let new = &after[human_slot];
                let money = |b: &[u8], at: usize| f32::from_bits(word(b,at));
                let bank = 0x14 + usize::from(new[0x144])*38*4;
                assert_eq!(old[0x144],new[0x144]);
                assert_eq!(money(new,bank+0x24*4)-money(old,bank+0x24*4),200.,
                    "one original card1 event24 posting, independent of effect helper");
                assert_eq!(money(new,4)-money(old,4),200.,
                    "one cash posting; unrelated same-frame activity is a bounded failure to inspect, not suppress");
                assert_eq!(notice_counts,[0,0],"original card1 suppresses its notice at delivery");
                delivered = true;
            } else {
                assert_eq!(pending(&source),0); assert_eq!(pending(&loaded),0);
                assert_eq!(notice_counts,[0,0],"suppressed card notice must not appear later");
            }
            if source.world.clock.hour() != old_hour {
                assert!(!crossed); crossed = true;
                assert_eq!(source.world.clock.hour(),if boundary_hour == 23 {0} else {6});
                let after = accounts(&source);
                assert_eq!(before.len(),after.len());
                for (a,b) in before.iter().zip(&after) {
                    if boundary_hour == 23 {
                        assert_eq!(b[0x144],u8::from(a[0x144] == 0));
                        let old_bank = 0x14 + usize::from(a[0x144])*38*4;
                        assert_eq!(&a[old_bank..old_bank+38*4],&b[old_bank..old_bank+38*4],"old bank must survive rollover");
                    } else { assert_eq!(a[0x144],b[0x144]); }
                }
                if boundary_hour == 5 {
                    assert!(before.iter().zip(&after).any(|(a,b)|a[4..0x144] != b[4..0x144]),
                        "06 settlement must actually affect at least one player's cash/ledger");
                }
            }
        }
        assert!(crossed); assert!(delivered); assert_eq!(notice_counts,[0,0]);
        assert!(source.live_commands.snapshot().unwrap().iter().all(|(_,_,_,_,c)|
            !matches!(c,Command::EventCard(c) if c.card == card && c.player as usize == human_slot)),
            "the saved delayed command must actually drain after the hour boundary");
        let left = source.save_v2_with_auction_view(&image,cameras,1,host.auction_save_state(),0).unwrap();
        let right = loaded.save_v2_with_auction_view(&image,cameras,1,loaded_host.auction_save_state(),0).unwrap();
        let ls = SaveFile::parse(&left).unwrap(); let rs = SaveFile::parse(&right).unwrap();
        let le = extension::Extension::parse(&ls).unwrap().unwrap();
        let re = extension::Extension::parse(&rs).unwrap().unwrap();
        for section in [2,3,5,6,7,8,9,10,11,12,13,16,17] {
            assert!(le.required(section).unwrap() == re.required(section).unwrap(),"runtime/RNG section{section}");
        }
        durable(&out.join("continued.sav"),&left); durable(&out.join("reloaded.sav"),&right);
        println!("MIXED modern determinism PASS hour{boundary_hour} card{card} cut{tick} end{}; original mixed oracle UNAVAILABLE",source.world.clock.timing[0]);
        drop(loaded_host); worker.join().unwrap(); drop(host); render_thread.join().unwrap();
    }).unwrap().join().unwrap();
}

#[test]
fn pending_event_card_has_native_packet_and_restore_consumer() {
    // Independent retained layout: original command1a is32 bytes, class52515c,
    // delayed flag1, card/player/target at20/24/28. No effect helper predicts it.
    let card = openmt2001::event_cards::lifecycle::Command {
        card: 43,
        player: 2,
        target: 0,
    };
    let encoded = packet(&Command::EventCard(card), 5650)
        .expect("reachable command1a needs a checked native save encoding");
    assert_eq!(encoded.len(), 32);
    for (at, value) in [
        (0, 0x52515c),
        (4, 0x1a),
        (8, 32),
        (12, 5650),
        (20, 43),
        (24, 2),
        (28, 0),
    ] {
        assert_eq!(word(&encoded, at), value, "native command1a word{at}");
    }
    assert_eq!(encoded[16], 1);
    match crate::fresh_city::restore::saved_command(&encoded)
        .expect("native command1a restore consumer")
    {
        Command::EventCard(decoded) => assert_eq!(decoded, card),
        _ => panic!("command1a decoded as another command"),
    }
}

#[test]
fn event_card_codec_preserves_signed_target_and_rejects_wrong_size() {
    for target in [i32::MIN, -1, 0, i32::MAX] {
        let card = lifecycle::Command {
            card: 43,
            player: 2,
            target,
        };
        let encoded = packet(&Command::EventCard(card), u32::MAX - 1).unwrap();
        assert_eq!(word(&encoded, 12), u32::MAX - 1);
        assert!(
            matches!(crate::fresh_city::restore::saved_command(&encoded).unwrap(),
            Command::EventCard(decoded) if decoded == card)
        );
        for length in [20, 24, 28, 31] {
            assert!(crate::fresh_city::restore::saved_command(&encoded[..length]).is_err());
        }
        let mut oversized = encoded;
        oversized.push(0);
        assert!(crate::fresh_city::restore::saved_command(&oversized).is_err());
    }
}
