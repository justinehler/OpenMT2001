pub use openmt2001::session::result::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn complete_credits_have_native_constructor_count() {
        let mut presentation = ResultPresentation::new(Sequence::Complete, (0, 1));
        presentation
            .apply(&level_script::Command::ResultCredits, &[Value::Number(1.)])
            .unwrap();
        assert!(presentation.credits);
        assert_eq!(presentation.credit_rows, 32);
    }

    #[test]
    fn result_identifiers_select_original_language_rows() {
        let mut presentation = ResultPresentation::new(Sequence::Win, (0, 1));
        assert_eq!(presentation.text_id(), None);
        for (identifier, expected) in [
            (1., Some(1767)),
            (2., Some(1768)),
            (3., Some(1769)),
            (4., None),
        ] {
            presentation
                .apply(
                    &level_script::Command::ResultCutSceneIdentifier,
                    &[Value::Number(identifier)],
                )
                .unwrap();
            assert_eq!(presentation.text_id(), expected);
        }
    }

    #[test]
    #[ignore = "requires original v1.4 US language data"]
    fn result_text_ids_address_supplied_localized_glyphs() {
        let exe_path = openmt2001::fixtures::path("EXE");
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let lengths = std::fs::read(language.join("tycoon.off")).unwrap();
        let glyphs = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let mut selected = Vec::new();
        for (index, length) in lengths.chunks_exact(2).enumerate() {
            let end = at + usize::from(u16::from_le_bytes(length.try_into().unwrap()));
            if (1767..=1769).contains(&index) {
                selected.push(glyphs[at..end].to_vec());
            }
            at = end;
        }
        assert_eq!(at, glyphs.len());
        assert_eq!(
            selected,
            [
                b"9OU\x07VE\x007ON".to_vec(),
                b"9OU\x07VE\x00,OST".to_vec(),
                b"9OU\x07VE\x00COMPLETED\x00THE\x00GAME\x0e".to_vec(),
            ]
        );
    }

    #[test]
    #[ignore = "requires original v1.4 4b1bc0 row-constructor fixture"]
    fn complete_credit_count_matches_original_calls() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_RESULT_CREDITS")).unwrap();
        assert_eq!(&bytes[..8], b"MTRCRED1");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        let rows = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 16 + rows * 24);
        let mut presentation = ResultPresentation::new(Sequence::Complete, (0, 1));
        presentation
            .apply(&level_script::Command::ResultCredits, &[Value::Number(1.)])
            .unwrap();
        assert_eq!(presentation.credit_rows, rows);
    }

    #[test]
    #[ignore = "requires original v1.4 credit-constructor fixture and executable"]
    fn complete_credit_rows_match_native_constructor_arguments() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_RESULT_CREDITS")).unwrap();
        assert_eq!(&bytes[..8], b"MTRCRED1");
        let image_bytes = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&image_bytes).unwrap();
        let mut state = 0x1234_5678;
        let descriptors = credit_descriptors(&image, &mut state).unwrap();
        assert_eq!(descriptors.len(), 32);
        assert_eq!(bytes.len(), 16 + descriptors.len() * 24);
        for (index, row) in descriptors.iter().enumerate() {
            let expected: [u32; 6] = std::array::from_fn(|column| {
                let offset = 16 + index * 24 + column * 4;
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
            });
            assert_eq!(*row, expected, "credit row {index}");
        }
        assert_eq!(state, 3_701_378_197);
        let mut state = 0x1234_5678;
        let resolved = credit_lines(&image, &mut state).unwrap();
        assert_eq!(resolved.len(), 32);
        assert_eq!(resolved[0].heading_id, Some(1771));
        assert_eq!(
            resolved[1].names[0],
            b"Keith Taylor".iter().map(|&b| b - 32).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires original v1.4 scripts, archive, model, road and parameter assets"]
    fn result_hubs_keep_final_history_and_game_end_gate() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
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
                    openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
                        .unwrap();
                let exe_path = openmt2001::fixtures::path("EXE");
                let scripts = exe_path.parent().unwrap().join("scripts");
                let language = exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa");
                let names = super::super::character_names(&image, &language).unwrap();
                let era_styles = super::super::load_era_styles(
                    Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                        .parent()
                        .unwrap(),
                )
                .unwrap();
                let assets = CommandAssets {
                    image: &image,
                    board: &board,
                    file: &file,
                };
                let process = crate::live_host::process_runtime::startup(&executable, "")
                    .unwrap()
                    .0;
                let mut session = Session::initialize_in_process(
                    super::super::Inputs {
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
                        scripts: &scripts,
                        parameters: std::str::from_utf8(&parameters).unwrap(),
                        era_styles,
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario: "SCENARIO1",
                        difficulty: 0,
                        selected_character: 4,
                        profile_name: b"Complete validation",
                        character_names: &names,
                    },
                    super::super::Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    },
                    process,
                )
                .unwrap();
                let mut expected_state = session.script_random_state;
                let selected =
                    random_zero_to(&mut expected_state, &[Value::Number(2.)]).unwrap() as u32;
                session
                    .begin_result_sequence(Sequence::Complete, (0, 1), &assets)
                    .unwrap();
                let result = session.result_presentation().unwrap().clone();
                assert_eq!(session.script_random_state, expected_state);
                assert_eq!(
                    result.flyby.as_deref(),
                    Some(match selected {
                        0 => "gamedata/cut_scenes/win_low.tm",
                        1 => "gamedata/cut_scenes/win_medium.tm",
                        2 => "gamedata/cut_scenes/win_high.tm",
                        _ => unreachable!(),
                    })
                );
                assert_eq!(session.world.clock.hour(), 21);
                assert_eq!(result.fireworks, Some(1));
                assert!(result.credits);
                assert_eq!(result.credit_rows, 32);
                assert!(
                    result.credit_lines.is_empty(),
                    "credits constructed before native title fade"
                );
                assert!(!result.bypass_enabled);
                assert_eq!(result.cut_scene_identifier, Some(3));
                assert_eq!(result.text_id(), Some(1769));
                assert_eq!(result.end_requested, None);
                let credit_rng_before = session.credit_random_state;
                let mut controller = crate::result_controller::Controller::new(false, true, 32);
                controller.set_identifier(Some(3));
                for tick in 1..=256 {
                    controller.tick(false);
                    assert_eq!(controller.take_credit_construction_edge(), tick == 256);
                }
                assert_eq!(session.credit_random_state, credit_rng_before);
                session.construct_result_credits(&image).unwrap();
                assert_eq!(
                    session.result_presentation().unwrap().credit_lines.len(),
                    32
                );
                assert_ne!(session.credit_random_state, credit_rng_before);
                assert!(session.construct_result_credits(&image).is_err());
                session
                    .dispatch_result_event("FlybyEnded", &assets)
                    .unwrap();
                let result = session.result_presentation().unwrap();
                assert!(result.fixed_camera);
                assert!(result.bypass_enabled);
                assert_eq!(result.end_requested, None);
                let before = session.history.count(0, 2).unwrap();
                session
                    .dispatch_result_event("CutSceneEnded", &assets)
                    .unwrap();
                assert_eq!(
                    session.result_presentation().unwrap().end_requested,
                    Some(1)
                );
                assert_eq!(session.history.count(0, 2), Some((before + 1).min(480)));
                let state = crate::stats_panel::State::show(
                    session.world.objective.kind,
                    false,
                    session.roster.manager.players.len(),
                )
                .unwrap();
                let snapshot =
                    crate::stats_panel::Snapshot::from_session(&session, &state).unwrap();
                assert_eq!(snapshot.history, session.history.as_bytes());
                let history = snapshot.history;
                session
                    .dispatch_result_event("CutSceneEnded", &assets)
                    .unwrap();
                assert_eq!(
                    session.history.as_bytes(),
                    history,
                    "duplicate end must not sample twice"
                );
                for (sequence, expected_end) in [(Sequence::Win, 1), (Sequence::Lose, 2)] {
                    session.result_presentation = None;
                    session
                        .begin_result_sequence(sequence, (0, 1), &assets)
                        .unwrap();
                    let count = session.history.count(0, 2).unwrap();
                    session
                        .dispatch_result_event("FlybyEnded", &assets)
                        .unwrap();
                    session
                        .dispatch_result_event("CutSceneEnded", &assets)
                        .unwrap();
                    assert_eq!(
                        session.result_presentation().unwrap().end_requested,
                        Some(expected_end)
                    );
                    assert_eq!(session.history.count(0, 2), Some((count + 1).min(480)));
                    let history = session.history.as_bytes().to_vec();
                    session
                        .dispatch_result_event("CutSceneEnded", &assets)
                        .unwrap();
                    assert_eq!(session.history.as_bytes(), history);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
// performance_tests.rs needs intro, so it builds with the game binary
// (game.rs `fresh_city_result_host_tests`, #99).

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use super::{CommandAssets, Session};
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::level_script::random::zero_to as random_zero_to;
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::{
    business_binding, level_script, level_startup, lua4::Value, outcome_notice::Sequence,
};
