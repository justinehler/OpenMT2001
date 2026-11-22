//! Independent Host-level checks for the actual result modal and campaign record.
//! These are controlled outcome injections, not natural competitive playthroughs.

/// The original 0x48c binding writes the reciprocal business/building indexes.
/// Run this during natural Host hours so later construction and sale can expose
/// a stale link even when money and the clock continue to change.
pub(crate) fn assert_live_links(session: &crate::fresh_city::Session<'_>) {
    for (slot, business) in session.world.ledgers.businesses.iter().enumerate() {
        if business[0x21c] == 0 { continue; }
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
        let record = session.world.buildings.get(building as usize)
            .unwrap_or_else(|| panic!("active business {slot} has missing building {building}"));
        let reciprocal = u32::from_le_bytes(record[0x1c..0x20].try_into().unwrap());
        assert_eq!(reciprocal, slot as u32, "active business {slot} has stale building {building} reciprocal link");
        let owner = usize::from(business[10]);
        assert!(owner <= session.roster.manager.players.len(), "active business {slot} owner leaves roster");
    }
}

#[cfg(test)]
mod tests {
    use crate::{campaign_store, fresh_city::{character_names, load_era_styles, CommandAssets, Inputs, Policy, Session}, human_dialog, live_host::Host};
    use openmt2001::{live_commands, outcome_notice::Sequence};
    use std::{fs, path::{Path, PathBuf}};

    #[test]
    #[ignore = "requires original v1.4 scripts and model/road assets"]
    fn host_result_persists_only_after_presentation_completes() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
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
            let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let scripts = exe_path.parent().unwrap().join("scripts");
            let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
            let names = character_names(&image, &language).unwrap();
            let era_styles = load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap();
            let assets = CommandAssets { image: &image, board: &board, file: &file };
            let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let root = PathBuf::from("work/validation").join(format!("result-profile-{}-{nonce}", std::process::id()));
            for (label, local_wins, expected, complete) in [
                ("cancel", false, Sequence::Lose, false),
                ("loss", false, Sequence::Lose, true),
                ("win", true, Sequence::Win, true),
            ] {
                let path = campaign_store::profile_path(&root, label.as_bytes());
                assert!(!path.exists(), "unique validation profile path already exists");
                let mut catalog = campaign_store::load_catalog(&image, &scripts).unwrap();
                campaign_store::load_progress(&mut catalog, &path).unwrap();
                // The GAMESTART registration order unlocks a different first
                // scenario. Represent a profile that can legally launch 1.
                catalog.unlock(200001);
                let selected = catalog.find(200001).unwrap();
                let next_id = catalog.levels[selected + 1..].iter().find(|level| !level.tutorial()).unwrap().id();
                let mut session = Session::initialize_campaign(Inputs {
                    image: &image, board: &board, file: &file,
                    people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                    vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                    transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                    road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                    scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
                    era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario: "SCENARIO1", difficulty: 0, selected_character: 4,
                    profile_name: b"Result validation", character_names: &names,
                }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }, catalog, path.clone()).unwrap();
                let local = session.roster.manager.players.iter().position(|p| p.reference == session.roster.status.current_player).unwrap();
                let winner = if local_wins { local } else { (local + 1) % session.roster.manager.players.len() };
                let mut host = Host::new([0., 20., 0.]);
                host.set_running(true);
                assert!(host.human_dialog_reply(&mut session, &assets, human_dialog::Reply {
                    callback: 0x48b490, context: winner as u32, button: 0,
                }).is_err(), "result callback was accepted before original modal admission");
                session.submit_command(live_commands::Command::OutcomeNotice { player: winner as i32, reason: 2 }, &assets,
                    |_| Err("result command dispatched before its delay".into())).unwrap();
                let due = session.world.clock.timing[0] + 25;
                let mut modal = None;
                for _ in 0..30 {
                    host.frame(&mut session, &assets, 40);
                    assert!(host.failure.is_none(), "Host failed before original modal: {:?}", host.failure);
                    if let Some(request) = host.take_prompts(&session).into_iter().find(|r| r.dialog.is_some_and(|d| d.callback == 0x48b490)) {
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
                let wrong_context = (winner as u32 + 1) % session.roster.manager.players.len() as u32;
                assert!(host.human_dialog_reply(&mut session, &assets, human_dialog::Reply {
                    callback: dialog.callback, context: wrong_context, button: 0,
                }).is_err(), "different outcome context was accepted");
                let frozen = session.world.clock.timing[0];
                for _ in 0..10 { host.frame(&mut session, &assets, 40); }
                assert_eq!(session.world.clock.timing[0], frozen);
                host.human_dialog_reply(&mut session, &assets, human_dialog::Reply {
                    callback: dialog.callback, context: dialog.context, button: 0,
                }).unwrap();
                if expected == Sequence::Win {
                    assert_eq!(session.world.clock.hour(), 21,
                        "WIN_SEQUENCE Calendar.hour did not update the retained city");
                }
                host.human_dialog_reply(&mut session, &assets, human_dialog::Reply {
                    callback: dialog.callback, context: dialog.context, button: 0,
                }).unwrap();
                assert_eq!(host.result_sequence(), Some(expected), "duplicate result reply changed sequence");
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 0,
                    "duplicate result reply applied campaign progression early");
                assert_eq!(host.result_sequence(), Some(expected));
                assert!(!host.running);
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 0);
                assert!(!path.exists(), "profile changed before result sequence completed");

                if !complete {
                    drop(session);
                    let mut reloaded = campaign_store::load_catalog(&image, &scripts).unwrap();
                    campaign_store::load_progress(&mut reloaded, &path).unwrap();
                    let level = &reloaded.levels[reloaded.find(200001).unwrap()];
                    assert_eq!(level.attempts(), 0, "cancelled result counted as a playthrough");
                    assert!(!level.completed()[0]);
                    assert!(!reloaded.levels[reloaded.find(next_id).unwrap()].unlocked());
                    continue;
                }

                // The renderer's completion callback is controlled here; it is
                // deliberately separate from the naturally scheduled modal.
                assert!(host.complete_result_sequence(&mut session).is_err(),
                    "campaign progressed before the result VM requested Game.End");
                session.dispatch_result_event("FlybyEnded", &assets).unwrap();
                assert!(host.complete_result_sequence(&mut session).is_err(),
                    "campaign progressed before CutSceneEnded");
                session.dispatch_result_event("CutSceneEnded", &assets).unwrap();
                assert_eq!(host.complete_result_sequence(&mut session).unwrap(), expected);
                assert!(host.complete_result_sequence(&mut session).is_err());
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 1);
                assert_eq!(session.catalog.selected_level().unwrap().completed()[0], local_wins);
                let mut reloaded = campaign_store::load_catalog(&image, &scripts).unwrap();
                campaign_store::load_progress(&mut reloaded, &path).unwrap();
                let level = &reloaded.levels[reloaded.find(200001).unwrap()];
                assert_eq!(level.attempts(), 1, "{label} attempt lost on profile reload");
                assert_eq!(level.completed()[0], local_wins, "{label} completion changed on reload");
                let next = &reloaded.levels[reloaded.find(next_id).unwrap()];
                assert_eq!(next.unlocked(), local_wins, "{label} next scenario unlock changed on reload");
            }
        }).unwrap().join().unwrap();
    }
}
