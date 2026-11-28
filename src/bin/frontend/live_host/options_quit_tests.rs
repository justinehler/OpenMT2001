//! In-city Options > Quit on a live city: the 49e970 confirmation opens over
//! Options, No (49e940 reply 1) keeps the city, and Yes abandons it once and
//! returns where 4ba313..4ba385 sends it.
use super::*;
use openmt2001::{archive::ArchiveIndex, build_file::BuildFile, pe::PeImage};

#[test]
#[ignore = "requires original v1.4 assets, USA language and TUTORIAL1/SCENARIO1 scripts"]
fn original_live_options_quit_no_keeps_city_and_yes_returns_to_picker() {
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
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let installer = parameter.parent().unwrap().parent().unwrap();
            let exe = openmt2001::fixtures::path("EXE");
            let language = exe
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa");
            let names = openmt2001::session::character_names(&image, &language).unwrap();
            let offsets = std::fs::read(language.join("tycoon.off")).unwrap();
            let bytes = std::fs::read(language.join("tycoon.bin")).unwrap();
            let mut at = 0;
            let strings = offsets
                .as_chunks::<2>()
                .0
                .iter()
                .map(|n| {
                    let end = at + usize::from(u16::from_le_bytes(*n));
                    let row = &bytes[at..end];
                    at = end;
                    row
                })
                .collect::<Vec<_>>();
            // The frontend prompt_panel.tm owner that 49e970 raises (88c0d4).
            let prompts =
                crate::prompt::Assets::load(&index, &archive, installer, &strings, &image).unwrap();
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            for (scenario, target) in [
                ("TUTORIAL1", ResultMenu::Tutorials),
                ("SCENARIO1", ResultMenu::Scenarios),
            ] {
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
                            parameter.parent().unwrap(),
                        )
                        .unwrap(),
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario,
                        difficulty: 0,
                        selected_character: 4,
                        profile_name: b"Quit test",
                        character_names: &names,
                    },
                    openmt2001::session::Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    },
                )
                .unwrap();
                assert_eq!(session.session_option_word, 0, "{scenario}");
                let mut host = Host::new([0.; 3]);
                host.begin_presentation(&session).unwrap();
                let attempts = session.catalog.selected_level().unwrap().attempts();
                let options = crate::game_options::Assets::test_menu();
                let mut menu = crate::game_options::View::default();
                for (button, keeps_city) in [(1, true), (0, false)] {
                    host.options(&mut session.world.pause, &mut session.hooks.timers, true)
                        .unwrap();
                    menu.open(true);
                    assert_eq!(menu.mouse(&options, Some([65., 5.]), true), None);
                    assert_eq!(
                        menu.mouse(&options, Some([65., 5.]), false),
                        Some(crate::game_options::Button::Quit)
                    );
                    // Drawing the confirmation through the city's path is not fatal.
                    prompts.frontend_prompt_surfaces(&menu.quit_prompt).unwrap();
                    let request = menu.quit_prompt.request().unwrap();
                    assert_eq!((request.title, request.body), (1763, 1764));
                    let decode = |id: u32| {
                        strings[id as usize]
                            .iter()
                            .map(|b| b + 32)
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(decode(request.title), b"QUIT GAME");
                    assert_eq!(decode(request.body), b"Do you want to quit this game?");
                    assert_eq!(
                        request.dialog.unwrap().buttons.map(decode),
                        [b"Yes".to_vec(), b"No".to_vec()]
                    );
                    // Click through the shifted button geometry the city uses.
                    let buttons = prompts.frontend_dialog_buttons(2);
                    let [l, t, r, b] = buttons[button];
                    menu.quit_prompt.cursor([(l + r) * 0.5, (t + b) * 0.5]);
                    menu.quit_prompt.dialog_mouse(true, &buttons);
                    menu.quit_prompt.dialog_mouse(false, &buttons);
                    let replies = menu.quit_prompt.take_dialog_replies();
                    assert_eq!(replies.len(), 1);
                    assert_eq!(replies[0].button, button as u32);
                    host.human_dialog_reply(&mut session, &assets, replies[0])
                        .unwrap();
                    menu.quit_prompt.finish_dismiss(vec![]);
                    assert!(menu.quit_prompt.request().is_none());
                    let status = host.status(&session);
                    if keeps_city {
                        assert!(
                            host.game_active && menu.open,
                            "{scenario}: No keeps Options"
                        );
                        assert_eq!(status.result_review, None);
                        menu.close();
                        host.options(&mut session.world.pause, &mut session.hooks.timers, false)
                            .unwrap();
                    } else {
                        assert!(!host.game_active && !host.running);
                        assert_eq!(
                            status.result_review,
                            Some(ResultReview::Menu(target)),
                            "{scenario}"
                        );
                    }
                }
                assert_eq!(
                    session.catalog.selected_level().unwrap().attempts(),
                    attempts,
                    "{scenario}: abandonment counts no attempt"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
