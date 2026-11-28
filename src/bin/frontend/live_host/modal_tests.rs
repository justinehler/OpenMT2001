use super::*;
use openmt2001::{archive::ArchiveIndex, build_file::BuildFile, pe::PeImage};

#[test]
#[ignore = "requires original v1.4 assets; live modal worker callbacks and UI group updates"]
fn original_live_modal_keys_and_timeouts() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let image = PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = BuildFile::parse(&build).unwrap();
            let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let settings = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let exe = openmt2001::fixtures::path("EXE");
            let names = openmt2001::session::character_names(
                &image,
                &exe.parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
            )
            .unwrap();
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            for edges in [[false, true], [true, true]] {
                let mut session = Session::initialize(
                    openmt2001::session::Inputs {
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
                        scripts: settings.parent().unwrap().parent().unwrap(),
                        parameters: std::str::from_utf8(&parameters).unwrap(),
                        era_styles: openmt2001::session::load_era_styles(
                            openmt2001::fixtures::path("PARAMETERS").parent().unwrap(),
                        )
                        .unwrap(),
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario: "SCENARIO1",
                        difficulty: 0,
                        selected_character: 4,
                        profile_name: b"Modal test",
                        character_names: &names,
                    },
                    openmt2001::session::Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    },
                )
                .unwrap();
                let mut host = Host::new([0.; 3]);
                host.begin_presentation(&session).unwrap();
                let mut ui = prompt_queue::State::default();
                ui.advance_to(host.status(&session).prompt_updates);
                let mut request =
                    prompt_queue::scenario_reminder(&session.catalog.levels[0], false, 0);
                let dialog = request.dialog.as_mut().unwrap();
                dialog.timeout = true;
                dialog.timeout_ticks = Some(2);
                dialog.button_count = 0;
                ui.submit(request);
                // Same sequence/pause gates as central_game_update, before the
                // normal group's native repeat loop. Player pause alone does
                // not suppress this UI group.
                for (suppressed, signed_pause) in [(true, 0), (false, 1)] {
                    host.policy.calendar.suppress_rollover_query = suppressed;
                    session.world.city.pause.bytes[3] = signed_pause;
                    host.interface_tick(&mut session, &assets).unwrap();
                    assert_eq!(host.status(&session).prompt_updates, 0);
                    ui.advance_to(0);
                    assert!(ui.take_dialog_replies().is_empty());
                }
                host.policy.calendar.suppress_rollover_query = false;
                session.world.city.pause.bytes[3] = 0;
                host.policy.scheduler.iterations = 2;
                host.interface_tick(&mut session, &assets).unwrap();
                ui.advance_to(host.status(&session).prompt_updates);
                assert!(
                    ui.take_dialog_replies().is_empty(),
                    "N delivered updates survive"
                );
                host.policy.scheduler.iterations = 1;
                host.interface_tick(&mut session, &assets).unwrap();
                ui.advance_to(host.status(&session).prompt_updates);
                let reply = ui.take_dialog_replies();
                assert_eq!(reply.len(), 1, "N+1 expires even with zero buttons");
                host.human_dialog_reply(&mut session, &assets, reply[0])
                    .unwrap();
                assert!(ui.request().is_some());
                ui.finish_dismiss(host.take_prompts(&session));
                assert!(ui.request().is_none());

                let mut confirmation =
                    prompt_queue::scenario_reminder(&session.catalog.levels[0], false, 0);
                confirmation
                    .dialog
                    .as_mut()
                    .unwrap()
                    .outcome(openmt2001::outcome_notice::Call::Timeout(0))
                    .unwrap();
                ui.submit(confirmation);
                host.policy.scheduler.iterations = 4;
                let before = host.prompt_updates;
                for _ in 0..251 {
                    host.interface_tick(&mut session, &assets).unwrap();
                    ui.advance_to(host.prompt_updates);
                }
                assert_eq!(host.prompt_updates - before, 1004);
                assert!(ui.take_dialog_replies().is_empty());
                assert!(ui.request().is_some(), "zero timeout stays open");
                host.auction = Some(Auction::default());
                host.interface_tick(&mut session, &assets).unwrap();
                assert_eq!(
                    host.prompt_updates - before,
                    1004,
                    "hidden normal group receives no updates"
                );
                host.auction = None;
                ui.dialog_button(0);
                let reply = ui.take_dialog_replies()[0];
                host.human_dialog_reply(&mut session, &assets, reply)
                    .unwrap();
                ui.finish_dismiss(host.take_prompts(&session));
                host.policy.scheduler.iterations = 1;

                // Both script producers carry the tutorial policy through the
                // actual worker collector. Their continuation owns dismissal.
                session.tutorial_state = 1;
                for command in [
                    level_script::Command::Prompt,
                    level_script::Command::TutorialMessage,
                ] {
                    session.hooks.commands.push((
                        command,
                        vec![Value::Number(1.), Value::Number(2.), Value::Number(0.)],
                    ));
                    let request = host.take_prompts(&session).into_iter().next().unwrap();
                    let mut tutorial = prompt_queue::State::default();
                    tutorial.advance_to(0);
                    tutorial.submit(request);
                    assert!(tutorial.advance_to(1002).is_empty());
                    assert!(tutorial.request().is_some());
                    assert!(tutorial.take_dialog_replies().is_empty());
                    tutorial.dialog_event1([false; 2]);
                    tutorial.dialog_event1([true, false]);
                    let replies = tutorial.take_dialog_replies();
                    assert_eq!(replies.len(), 1);
                    host.human_dialog_reply(&mut session, &assets, replies[0])
                        .unwrap();
                    tutorial.finish_dismiss(host.take_prompts(&session));
                    assert!(tutorial.request().is_none());
                }
                session.tutorial_state = 0;

                // Admit a real one-button outcome and execute its actual worker
                // callback before closing the frontend's old queue head.
                let winner = session
                    .roster
                    .manager
                    .players
                    .iter()
                    .position(|p| p.reference != session.roster.status.current_player)
                    .unwrap();
                host.set_running(true);
                session
                    .submit_command(
                        openmt2001::live_commands::Command::OutcomeNotice {
                            player: winner as i32,
                            reason: 2,
                        },
                        &assets,
                        |_| Err("early outcome".into()),
                    )
                    .unwrap();
                let mut outcome = None;
                for _ in 0..30 {
                    host.frame(&mut session, &assets, 40);
                    assert!(host.failure.is_none(), "{:?}", host.failure);
                    outcome = host
                        .take_prompts(&session)
                        .into_iter()
                        .find(|r| r.dialog.is_some_and(|d| d.callback == 0x48b490));
                    if outcome.is_some() {
                        break;
                    }
                }
                let request = outcome.expect("live worker outcome");
                assert_eq!(request.dialog.unwrap().button_count, 1);
                ui.submit(request);
                // Queue an independent zero-button modal behind the outcome.
                let mut next =
                    prompt_queue::scenario_reminder(&session.catalog.levels[0], false, 0);
                next.dialog.as_mut().unwrap().button_count = 0;
                ui.submit(next);
                ui.dialog_event1([false; 2]);
                ui.dialog_event1(edges);
                let replies = ui.take_dialog_replies();
                assert_eq!(replies.len(), 1);
                assert_eq!(replies[0].button, u32::from(!edges[0]));
                host.human_dialog_reply(&mut session, &assets, replies[0])
                    .unwrap();
                assert_eq!(
                    host.result_sequence(),
                    Some(openmt2001::outcome_notice::Sequence::Lose)
                );
                assert_eq!(ui.request().unwrap().dialog.unwrap().callback, 0x48b490);
                ui.finish_dismiss(host.take_prompts(&session));
                let replies = ui.take_dialog_replies();
                if edges[0] {
                    assert_eq!(replies.len(), 1);
                    assert_eq!((replies[0].callback, replies[0].button), (0, 1));
                    host.human_dialog_reply(&mut session, &assets, replies[0])
                        .unwrap();
                    ui.finish_dismiss(host.take_prompts(&session));
                    assert!(ui.request().is_none());
                } else {
                    assert!(replies.is_empty());
                    assert_eq!(ui.request().unwrap().dialog.unwrap().callback, 0);
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
