use crate::{
    campaign_store,
    fresh_city::{CommandAssets, Inputs, Policy, Session, character_names, load_era_styles},
    human_dialog,
    live_host::Host,
};
use openmt2001::{live_commands, outcome_notice::Sequence};
use std::{fs, path::Path};

#[test]
#[ignore = "requires original v1.4 scripts and model/road assets"]
fn host_result_persists_only_after_presentation_completes() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
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
            let index =
                openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let exe_path = openmt2001::fixtures::path("EXE");
            let scripts = exe_path.parent().unwrap().join("scripts");
            let language = exe_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa");
            let names = character_names(&image, &language).unwrap();
            let era_styles = load_era_styles(
                Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                    .parent()
                    .unwrap(),
            )
            .unwrap();
            let installer = openmt2001::fixtures::path("PARAMETERS");
            let style = crate::block_sidewalk_labels::load_style(
                &image,
                installer.parent().unwrap().parent().unwrap(),
            )
            .unwrap();
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = openmt2001::fixtures::scratch("RESULT_ROUTE")
                .join(format!("result-profile-{}-{nonce}", std::process::id()));
            for (label, local_wins, expected, complete, scenario, level_id) in [
                ("cancel", false, Sequence::Lose, false, "SCENARIO1", 200001),
                ("loss", false, Sequence::Lose, true, "SCENARIO1", 200001),
                ("win", true, Sequence::Win, true, "SCENARIO1", 200001),
                (
                    "complete",
                    true,
                    Sequence::Complete,
                    true,
                    "SCENARIO16",
                    200016,
                ),
            ] {
                let path = campaign_store::profile_path(&root, label.as_bytes());
                assert!(
                    !path.exists(),
                    "unique validation profile path already exists"
                );
                let mut catalog = campaign_store::load_catalog(&image, &scripts).unwrap();
                campaign_store::load_progress(&mut catalog, &path).unwrap();
                // The GAMESTART registration order unlocks a different first
                // scenario. Represent a profile that can legally launch this level.
                catalog.unlock(level_id);
                let selected = catalog.find(level_id).unwrap();
                let next_id = if level_id == 200016 {
                    400001
                } else {
                    catalog.levels[selected + 1..]
                        .iter()
                        .find(|level| !level.tutorial())
                        .unwrap()
                        .id()
                };
                let process = crate::live_host::process_runtime::startup(&executable, "")
                    .unwrap()
                    .0;
                let mut session =
                    Session::initialize_campaign_with_style_and_compatibility_in_process(
                        Inputs {
                            tree_locators: None,
                            image: &image,
                            board: &board,
                            file: &file,
                            people: openmt2001::people::PeopleModels::load(
                                &image, &index, &archive,
                            )
                            .unwrap(),
                            vehicle_models:
                                openmt2001::traffic::road::models::Models::from_original(
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
                            difficulty: 0,
                            selected_character: 4,
                            profile_name: b"Result validation",
                            character_names: &names,
                        },
                        Policy {
                            seed: 0x12345678,
                            auxiliary_prior_bytes: [0; 0x11c],
                        },
                        catalog,
                        path.clone(),
                        &style,
                        crate::compatibility::Mode::StrictV14,
                        Some(process),
                    )
                    .unwrap();
                let local = session
                    .roster
                    .manager
                    .players
                    .iter()
                    .position(|p| p.reference == session.roster.status.current_player)
                    .unwrap();
                let winner = if local_wins {
                    local
                } else {
                    (local + 1) % session.roster.manager.players.len()
                };
                let mut host = Host::new([0., 20., 0.]);
                host.set_running(true);
                assert!(
                    host.human_dialog_reply(
                        &mut session,
                        &assets,
                        human_dialog::Reply {
                            callback: 0x48b490,
                            context: winner as u32,
                            button: 0,
                        }
                    )
                    .is_err(),
                    "result callback was accepted before original modal admission"
                );
                session
                    .submit_command(
                        live_commands::Command::OutcomeNotice {
                            player: winner as i32,
                            reason: 2,
                        },
                        &assets,
                        |_| Err("result command dispatched before its delay".into()),
                    )
                    .unwrap();
                let due = session.world.clock.timing[0] + 25;
                let mut modal = None;
                for _ in 0..30 {
                    host.frame(&mut session, &assets, 40);
                    assert!(
                        host.failure.is_none(),
                        "Host failed before original modal: {:?}",
                        host.failure
                    );
                    if let Some(request) = host
                        .take_prompts(&session)
                        .into_iter()
                        .find(|r| r.dialog.is_some_and(|d| d.callback == 0x48b490))
                    {
                        modal = request.dialog;
                        break;
                    }
                }
                let dialog = modal.expect("original outcome modal did not reach Host");
                assert_eq!(session.world.clock.timing[0], due);
                assert_eq!(dialog.context, winner as u32);
                assert_eq!(dialog.timeout_ticks, Some(1500));
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 0);
                assert!(!path.exists(), "profile changed before result presentation");
                let wrong_context =
                    (winner as u32 + 1) % session.roster.manager.players.len() as u32;
                assert!(
                    host.human_dialog_reply(
                        &mut session,
                        &assets,
                        human_dialog::Reply {
                            callback: dialog.callback,
                            context: wrong_context,
                            button: 0,
                        }
                    )
                    .is_err(),
                    "different outcome context was accepted"
                );
                let frozen = session.world.clock.timing[0];
                for _ in 0..10 {
                    host.frame(&mut session, &assets, 40);
                }
                assert_eq!(session.world.clock.timing[0], frozen);
                host.human_dialog_reply(
                    &mut session,
                    &assets,
                    human_dialog::Reply {
                        callback: dialog.callback,
                        context: dialog.context,
                        button: 0,
                    },
                )
                .unwrap();
                if expected == Sequence::Win {
                    assert_eq!(
                        session.world.clock.hour(),
                        21,
                        "WIN_SEQUENCE Calendar.hour did not update the retained city"
                    );
                }
                host.human_dialog_reply(
                    &mut session,
                    &assets,
                    human_dialog::Reply {
                        callback: dialog.callback,
                        context: dialog.context,
                        button: 0,
                    },
                )
                .unwrap();
                assert_eq!(
                    host.result_sequence(),
                    Some(expected),
                    "duplicate result reply changed sequence"
                );
                assert_eq!(
                    session.catalog.selected_level().unwrap().attempts(),
                    0,
                    "duplicate result reply applied campaign progression early"
                );
                assert_eq!(host.result_sequence(), Some(expected));
                assert!(!host.running);
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 0);
                assert!(
                    !path.exists(),
                    "profile changed before result sequence completed"
                );

                if !complete {
                    drop(session);
                    let mut reloaded = campaign_store::load_catalog(&image, &scripts).unwrap();
                    campaign_store::load_progress(&mut reloaded, &path).unwrap();
                    let level = &reloaded.levels[reloaded.find(level_id).unwrap()];
                    assert_eq!(
                        level.attempts(),
                        0,
                        "cancelled result counted as a playthrough"
                    );
                    assert!(!level.completed()[0]);
                    assert!(!reloaded.levels[reloaded.find(next_id).unwrap()].unlocked());
                    continue;
                }

                // Drive the same controller and retained host events as the
                // live presentation, without requiring an audio device.
                assert!(
                    host.complete_result_sequence(&mut session).is_err(),
                    "campaign progressed before the result VM requested Game.End"
                );
                let presentation = session.result_presentation().unwrap();
                let mut controller = crate::result_controller::Controller::new(
                    presentation.bypass_enabled,
                    presentation.credits,
                    presentation.credit_rows,
                );
                controller.set_identifier(presentation.cut_scene_identifier);
                for _ in 0..300 {
                    assert!(!controller.tick(false));
                    if controller.take_credit_construction_edge() {
                        session.construct_result_credits(&image).unwrap();
                    }
                    host.frame(&mut session, &assets, 20);
                }
                host.dispatch_result_event(&mut session, &assets, "FlybyEnded")
                    .unwrap();
                let presentation = session.result_presentation().unwrap();
                controller.set_identifier(presentation.cut_scene_identifier);
                controller.set_bypass(presentation.bypass_enabled);
                assert!(
                    host.complete_result_sequence(&mut session).is_err(),
                    "campaign progressed before CutSceneEnded"
                );
                let mut ended = false;
                for tick in 0..10000 {
                    host.frame(&mut session, &assets, 20);
                    let finished = controller.tick(tick == 0 && expected != Sequence::Complete);
                    if controller.take_credit_construction_edge() {
                        session.construct_result_credits(&image).unwrap();
                    }
                    if finished {
                        ended = true;
                        host.dispatch_result_event(&mut session, &assets, "CutSceneEnded")
                            .unwrap();
                        break;
                    }
                }
                assert!(
                    ended,
                    "result controller did not reach the original end callback"
                );
                assert_eq!(
                    host.complete_result_sequence(&mut session).unwrap(),
                    expected
                );
                assert!(host.complete_result_sequence(&mut session).is_err());
                let status = host.status(&session);
                assert_eq!(
                    status.result_review,
                    Some(crate::live_host::ResultReview::Statistics)
                );
                assert!(status.stats.is_some());
                host.stats_action(&mut session, openmt2001::stats_panel::Action::Close)
                    .unwrap();
                assert_eq!(
                    host.status(&session).result_review,
                    Some(crate::live_host::ResultReview::Menu(
                        crate::live_host::ResultMenu::Scenarios
                    ))
                );
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 1);
                assert_eq!(
                    session.catalog.selected_level().unwrap().completed()[0],
                    local_wins
                );
                let mut reloaded = campaign_store::load_catalog(&image, &scripts).unwrap();
                campaign_store::load_progress(&mut reloaded, &path).unwrap();
                let level = &reloaded.levels[reloaded.find(level_id).unwrap()];
                assert_eq!(
                    level.attempts(),
                    1,
                    "{label} attempt lost on profile reload"
                );
                assert_eq!(
                    level.completed()[0],
                    local_wins,
                    "{label} completion changed on reload"
                );
                let next = &reloaded.levels[reloaded.find(next_id).unwrap()];
                assert_eq!(
                    next.unlocked(),
                    local_wins,
                    "{label} next scenario unlock changed on reload"
                );
            }
            fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
