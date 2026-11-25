//! Audited nested sources used by the sixteen original campaign hubs.
use openmt2001::lua4::Runtime;
use std::path::Path;

pub fn register_sources(runtime: &mut Runtime, root: &Path) -> Result<(), String> {
    for map in [
        "tempmap",
        "apartmentrim",
        "apartments2",
        "parkfiller",
        "goodsparsemap",
        "goodspread",
        "sparsemap",
        "cheapbuiltmap",
        "mapbuiltupmore",
        "mapbuiltup",
        "developedmap",
        "cheapbuiltbigmap",
        "monopolymap",
        "shoppingmap",
    ] {
        let relative = format!("MAPS/{map}.lua");
        let bytes = std::fs::read(root.join(&relative)).map_err(|e| format!("{relative}: {e}"))?;
        runtime.script_buffer(&format!("scripts/{relative}"), &bytes)?;
    }
    // Scenario5 reuses3 and7 reuses6. The unused5/7 preferences are not loaded.
    for scenario in [1, 2, 3, 4, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16] {
        let relative = format!("SCENARIO{scenario}/initialpref.lua");
        let bytes = std::fs::read(root.join(&relative)).map_err(|e| format!("{relative}: {e}"))?;
        runtime.script_buffer(&format!("scripts/{relative}"), &bytes)?;
    }
    // Original Tutorial 3's initializer explicitly reuses Tutorial 1's
    // supplied ownership/population preferences.
    for tutorial_preferences in ["TUTORIAL1/initialpref.lua", "TUTORIAL4/initialpref.lua", "TUTORIAL8/initialpref.lua", "TUTORIAL9/initialpref.lua", "TUTORIAL12/initialpref.lua"] {
        let bytes = std::fs::read(root.join(tutorial_preferences))
            .map_err(|error| format!("{tutorial_preferences}: {error}"))?;
        runtime.script_buffer(&format!("scripts/{tutorial_preferences}"), &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{Inputs, Policy, Session, character_names, load_era_styles};
    use std::{fs, path::Path};
    /// Separate controlled native comparison; this does not drive the tutorial
    /// script. Only auction storage/one lock byte use native probe priors, and
    /// both are restored before the legal connected tutorial continues.
    fn compare_native_auction_announcement_order(session:&mut Session<'_>,assets:&super::super::CommandAssets<'_,'_>){
        use openmt2001::live_commands;
        let put=|bytes:&mut [u8],at:usize,value:u32|bytes[at..at+4].copy_from_slice(&value.to_le_bytes());
        struct Geometry;
        impl openmt2001::building_pool::GeometryBinding for Geometry{
            fn prepare(&mut self,_:usize,_:&mut [u8;0x394])->Result<(),String>{Err("auction must not prepare geometry".into())}
            fn model_height(&mut self,_:u32)->Result<f32,String>{Err("auction must not query geometry".into())}
        }
        #[derive(Default)]
        struct Observation{event:Option<[u32;3]>,clock_write:std::rc::Rc<std::cell::Cell<bool>>}
        impl live_commands::TutorialAnnouncement for Observation{
            fn start(&mut self,state:&mut openmt2001::level_script::State<'_>,
                presentation:&openmt2001::ai_messages::offers::auction::admission::Presentation,
                _:&mut dyn openmt2001::building_pool::GeometryBinding,_:&mut openmt2001::economy::Utilities)->Result<(),String>{
                assert!(self.event.is_none(),"internal61 must occur once per admitted auction");
                self.event=Some([presentation.countdown,u32::from(presentation.visible),u32::from(state.categories[3][0x6c])]);
                // Deliberate synchronous host mutation tests borrow ownership;
                // Lua property conversion is exercised in the binding regressions.
                state.clock.calendar[0]=13;
                self.clock_write.set(true);
                Ok(())
            }
        }
        let path = std::env::var_os("OPENMT2001_NATIVE_TUTORIAL_AUCTION_ORDER")
            .expect("set OPENMT2001_NATIVE_TUTORIAL_AUCTION_ORDER to the private original comparison fixture");
        let bytes=fs::read(path).unwrap();
        assert_eq!(&bytes[..8],b"MTTAOR01");
        let mut words=bytes[8..].chunks_exact(4).map(|b|u32::from_le_bytes(b.try_into().unwrap()));
        assert_eq!(words.next(),Some(0x037f));assert_eq!(words.next(),Some(3));
        let auction=std::mem::take(&mut session.world.city.auction);
        let lock=session.world.city.categories[3][0x6c];
        for _ in 0..3{
            let tutorial_state=words.next().unwrap() as u8;
            let has_event=words.next().unwrap()!=0;
            let before:[u32;3]=std::array::from_fn(|_|words.next().unwrap());
            let after:[u32;3]=std::array::from_fn(|_|words.next().unwrap());
            session.world.city.auction=Default::default();
            session.world.city.auction.presentation.countdown=0x55667788;
            session.world.city.categories[3][0x6c]=0x7f;
            let mut descriptor=[0;52];put(&mut descriptor,4,0x1d);put(&mut descriptor,20,3);put(&mut descriptor,24,0);put(&mut descriptor,28,1000);
            let mut context=live_commands::CommandContext{image:assets.image,clock:&mut session.world.clock,accounting_enabled:true,
                notifications:session.world.city.ai_demand.ready!=0,tutorial_state,network_enabled:false,network_override:false,utility_factor:1.};
            let mut messages=openmt2001::player_messages::Context{arena:openmt2001::player_messages::Arena{next:session.allocation_next},
                current_player:session.roster.status.current_player,network:[false;2],prior:[0;48]};
            let previous_hour=context.clock.calendar[0];
            let mut observation=Observation::default();
            let clock_write=observation.clock_write.clone();
            let mut observed_live_clock=false;
            live_commands::Runtime{city:&mut session.world.city,roster:&mut session.roster,population:&mut session.population,
                utilities:&mut session.hooks.utilities,board:assets.board,event_cards:&mut session.event_cards,message_context:&mut messages}
                .apply_with_announcement(&live_commands::Command::Auction{descriptor},&mut context,&mut Geometry,Some(&mut observation),|_,clock| {
                    if clock_write.get() {assert_eq!(clock.hour(),13);observed_live_clock=true;}
                    Ok(())
                }).unwrap();
            assert_eq!(observed_live_clock,has_event,"post-announcement command callback sees authoritative clock");
            context.clock.calendar[0]=previous_hour;
            assert_eq!(observation.event,has_event.then_some(before),"native internal61 gate/order, state{tutorial_state}");
            let presentation=&session.world.city.auction.presentation;
            assert_eq!([presentation.countdown,u32::from(presentation.visible),u32::from(session.world.city.categories[3][0x6c])],after,
                "native post-announcement stores, state{tutorial_state}");
        }
        assert!(words.next().is_none());
        session.world.city.auction=auction;session.world.city.categories[3][0x6c]=lock;
    }
    #[test]
    #[ignore = "requires supplied v1.4 bonus scripts and original assets"]
    fn original_bonus_initializes_at_all_difficulties() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        for (address, expected) in [(0x4dbe93, 103.), (0x4dbea7, 76.),
            (0x4dbebb, 77.), (0x4dbecf, 78.)] {
            assert_eq!(image.f32_at(address).unwrap(), expected);
        }
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let executable_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts = executable_path.parent().unwrap().join("scripts");
        let language = executable_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names = super::super::character_names(&image, &language).unwrap();
        let era_styles = load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap()).unwrap();
        let installer = Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap().parent().unwrap().to_path_buf();
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let text = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets.chunks_exact(2).map(|length| {
            let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let value = &text[at..end];
            at = end;
            value
        }).collect::<Vec<_>>();
        assert_eq!(at, text.len());
        let mut catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        catalog.unlock(400001);
        let mut menu = crate::singleplayer::SinglePlayer::new(
            &index, &archive, &installer, &strings, &executable).unwrap();
        menu.open(false, "Bonus initialization", catalog,
            std::path::PathBuf::from("work/economy/bonus-menu-launch.profile"));
        let bonus_row = crate::scenarios::CAMPAIGN.iter().position(|row| row.id == 400001).unwrap();
        menu.activate(5 + bonus_row);
        menu.activate(0);
        assert!(menu.message.is_none(), "unlocked Bonus must enter character setup");
        menu.activate(0);
        let setup = menu.request.take().expect("Bonus Start must submit a live setup");
        assert!(matches!(setup.preview, crate::singleplayer::Preview::Scenario(400001)));
        assert!(matches!(setup.difficulty, crate::singleplayer::Difficulty::Easy));
        assert_eq!(setup.character_index, 4);
        let scenario = match setup.preview {
            crate::singleplayer::Preview::Scenario(id) => crate::scenarios::find(id).unwrap(),
            crate::singleplayer::Preview::Sandbox => panic!("Bonus became Sandbox"),
        };
        for difficulty in 0..3 {
            let inputs = Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: scenario.token, difficulty, selected_character: setup.character_index,
                profile_name: &setup.profile_name, character_names: &names,
            };
            let policy = Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] };
            let mut session = if difficulty == 0 {
                Session::initialize_campaign(inputs, policy, setup.catalog.clone(),
                    setup.profile_path.clone())
            } else {
                Session::initialize(inputs, policy)
            }.unwrap();
            assert_eq!(session.catalog.selected_level().unwrap().id(), 400001);
            assert_eq!(session.roster.manager.players.len(), 1);
            assert_eq!(session.world.ledgers.residents.active_count, 500);
            if difficulty == 0 {
                let mut host = crate::live_host::Host::new([0., 20., 0.]);
                host.frame(&mut session, &super::super::CommandAssets {
                    image: &image, board: &board, file: &file,
                }, 20);
                let status = host.status(&session);
                assert!(status.failure.is_none(), "Bonus retained Host startup");
                assert!(status.interface_ticks > 0);
                assert_eq!(status.ticks, session.world.clock.timing[0]);
                assert!(status.city_people_rows.is_some());
                // Exercise fresh report adapters on supported original
                // Bonus data; its 500 residents have no supplied apartment map.
                let players = crate::player_info::State::from_runtime(
                    &session.roster.manager, &session.world.ledgers.players,
                    session.roster.status.current_player).unwrap();
                let ownership = crate::city_ownership::Ownership::from_runtime(
                    &session.world, &players).unwrap();
                assert_eq!(ownership.groups.len(), 1);
                let rights = crate::city_rights::Rights::from_runtime(
                    &session.world, &players).unwrap().expect("Bonus has a current human player");
                assert_eq!(rights.owner_number, 1);
                assert!(!rights.player_name.is_empty());
                let prestige = crate::city_prestige::Prestige::from_runtime(
                    &session.world, &board).unwrap();
                assert!(prestige.rows.iter().any(|row|
                    matches!(row, crate::city_prestige::Row::Block(_))));
                let polls = crate::consumer_polls::Poll::from_runtime(&session.world).unwrap();
                let housed = polls.iter().map(|poll| u32::from(poll.residents)).sum::<u32>();
                assert_eq!(housed, 0, "Bonus supplies no housed poll respondents");
            }
        }
        // Original Scenario 1 supplies apartments2 and 423 citizens at Easy.
        // Its retained initial resident lists have not yet entered housing;
        // verify that the UI adapter agrees with the shared native poll query.
        let populated = Session::initialize(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                &image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(
                &image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: "SCENARIO1", difficulty: 0, selected_character: 0,
            profile_name: b"Poll report", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
        assert_eq!(populated.world.ledgers.residents.active_count, 423);
        let polls = crate::consumer_polls::Poll::from_runtime(&populated.world).unwrap();
        let housed = polls.iter().map(|poll| u32::from(poll.residents)).sum::<u32>();
        assert_eq!(housed, 0, "retained Scenario1 initial apartment lists are empty");
        let source = &populated.roster.manager.players[0];
        let mut accumulator = openmt2001::player_manager::Player {
            reference: source.reference, bytes: source.bytes.clone(), auxiliaries: vec![],
        };
        let active = populated.world.commodities.resident_inputs().map(|commodity| commodity.active);
        for (category, poll) in polls.iter().enumerate() {
            openmt2001::consumer_poll::clear(&mut accumulator);
            openmt2001::consumer_poll::start(&mut accumulator, category as i32,
                &populated.world.categories, &populated.world.buildings,
                &populated.world.ledgers.businesses, &populated.world.ledgers.residents,
                &active).unwrap();
            let original_counts = accumulator.bytes[0x1c8..0x208].chunks_exact(2)
                .map(|count| u16::from_le_bytes(count.try_into().unwrap())).collect::<Vec<_>>();
            assert_eq!(poll.counts.as_slice(), original_counts, "live poll block{category}");
            assert_eq!(poll.residents, u16::from_le_bytes(
                accumulator.bytes[0x208..0x20a].try_into().unwrap()), "live poll residents block{category}");
        }
        menu.open_tutorial("Tutorial startup", setup.catalog.clone(),
            std::path::PathBuf::from("work/economy/tutorial-menu-launch.profile"));
        assert_eq!(menu.count(), 15);
        #[cfg(feature = "capture")]
        {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let prepared = menu.draw(&renderer, Some(2), false).unwrap();
            if let Ok(path) = std::env::var("OPENMT2001_TUTORIAL_CAPTURE") {
                crate::capture_test::capture_standalone(&renderer, &prepared, &path);
            }
        }
        menu.activate(3);
        menu.activate(0);
        assert!(matches!(menu.request.as_ref().unwrap().preview,
            crate::singleplayer::Preview::Scenario(100001)),
            "Tutorial 2 must be selectable after connected original-asset progression");
        menu.open_tutorial("Tutorial startup", setup.catalog.clone(),
            std::path::PathBuf::from("work/economy/tutorial-menu-launch.profile"));
        menu.activate(4);
        menu.activate(0);
        assert!(matches!(menu.request.as_ref().unwrap().preview,
            crate::singleplayer::Preview::Scenario(100002)), "Tutorial 3 is selectable after complete connected acceptance");
        menu.open_tutorial("Tutorial startup", setup.catalog.clone(),
            std::path::PathBuf::from("work/economy/tutorial-menu-launch.profile"));
        menu.activate(5);
        menu.activate(0);
        assert!(matches!(menu.request.as_ref().unwrap().preview,
            crate::singleplayer::Preview::Scenario(100003)),
            "Tutorial 4 is selectable after its complete control/auction lifecycle acceptance");
        menu.open_tutorial("Tutorial startup", setup.catalog.clone(),
            std::path::PathBuf::from("work/economy/tutorial-menu-launch.profile"));
        menu.activate(0);
        let tutorial_setup = menu.request.take().expect("Tutorial 1 OK must submit a live setup");
        assert!(matches!(tutorial_setup.preview, crate::singleplayer::Preview::Scenario(100000)));
        assert!(matches!(tutorial_setup.difficulty, crate::singleplayer::Difficulty::Easy));
        assert_eq!(tutorial_setup.character_index, 0);
        let tutorial_scenario = crate::scenarios::find(100000).unwrap();
        let mut tutorial = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: tutorial_scenario.token, difficulty: 0,
            selected_character: tutorial_setup.character_index,
            profile_name: &tutorial_setup.profile_name, character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] },
            tutorial_setup.catalog, tutorial_setup.profile_path).unwrap();
        assert_eq!(tutorial.catalog.selected_level().unwrap().id(), 100000);
        assert_eq!(tutorial.tutorial_state, 1);
        assert_eq!(tutorial.roster.manager.players.len(), 1);
        assert_eq!(tutorial.roster.manager.players[0].bytes[0x150..0x154], 0u32.to_le_bytes());
        let mut host = crate::live_host::Host::new([0., 20., 0.]);
        host.frame(&mut tutorial, &super::super::CommandAssets {
            image: &image, board: &board, file: &file,
        }, 20);
        let status = host.status(&tutorial);
        assert!(status.failure.is_none(), "Tutorial 1 retained Host startup");
        assert!(status.interface_ticks > 0);
        assert_eq!(status.prompts.len(), 1);
        assert_eq!(status.prompts[0].dismissal_id, 2);
        assert!(status.tutorial_controls.all_disabled);
        assert!(status.tutorial_controls.picking_disabled);
        assert!(status.tutorial_controls.camera_disabled);
        tutorial.dispatch_session_event("InterfacePromptDismiss", 2,
            &super::super::CommandAssets { image: &image, board: &board, file: &file },
            false).unwrap();
        let next = host.status(&tutorial);
        assert!(next.failure.is_none(), "Tutorial 1 prompt dismissal");
        assert_eq!(next.prompts.len(), 1);
        assert_eq!(next.prompts[0].dismissal_id, 3);
        assert_eq!(next.tutorial_arrows, vec![[507, 18]]);
        let assets = super::super::CommandAssets { image: &image, board: &board, file: &file };
        for id in 3..=5 {
            tutorial.dispatch_session_event("InterfacePromptDismiss", id, &assets, false).unwrap();
            let current = host.status(&tutorial);
            assert!(current.failure.is_none(), "Tutorial 1 prompt {id}");
            if id < 5 {
                assert_eq!(current.prompts[0].dismissal_id, (id + 1) as u32);
            } else {
                assert!(current.prompts.is_empty());
                assert_eq!(current.tutorial_message.unwrap().dismissal_id, 6);
                assert_eq!(current.tutorial_enabled_controls, vec![103]);
                assert_eq!(current.tutorial_arrows, vec![[547, 355]]);
            }
        }
        host.tutorial_event(&mut tutorial, &assets, 103).unwrap();
        let after_navigation = host.status(&tutorial);
        assert!(after_navigation.failure.is_none());
        assert_eq!(after_navigation.prompts[0].dismissal_id, 7);
        assert!(after_navigation.tutorial_message.is_none());
        assert!(after_navigation.tutorial_enabled_controls.is_empty());
        for id in 7..=10 {
            tutorial.dispatch_session_event("InterfacePromptDismiss", id, &assets, false).unwrap();
            let current = host.status(&tutorial);
            assert!(current.failure.is_none(), "Tutorial 1 prompt {id}");
            if id < 10 {
                assert_eq!(current.prompts[0].dismissal_id, (id + 1) as u32);
            } else {
                assert!(current.prompts.is_empty());
                assert_eq!(current.tutorial_message.unwrap().dismissal_id, 11);
                assert_eq!(current.tutorial_enabled_controls, vec![77, 78]);
            }
        }
        for _ in 0..6 {
            host.tutorial_event(&mut tutorial, &assets, 77).unwrap();
        }
        let comparison = host.status(&tutorial);
        assert_eq!(comparison.tutorial_message.unwrap().dismissal_id, 12);
        assert_eq!(comparison.tutorial_enabled_controls, vec![76]);
        host.tutorial_event(&mut tutorial, &assets, 76).unwrap();
        let closed = host.status(&tutorial);
        assert_eq!(closed.tutorial_message.unwrap().dismissal_id, 13);
        assert_eq!(closed.tutorial_enabled_controls, vec![76]);
        host.tutorial_event(&mut tutorial, &assets, 76).unwrap();
        let reopened = host.status(&tutorial);
        assert_eq!(reopened.prompts[0].dismissal_id, 14);
        assert!(reopened.tutorial_message.is_none());
        assert!(reopened.tutorial_enabled_controls.is_empty());
        tutorial.dispatch_session_event("InterfacePromptDismiss", 14, &assets, false).unwrap();
        assert!(tutorial.hooks.commands.iter().any(|(command, _)|
            matches!(command, openmt2001::level_script::Command::ResultEnd)));
        host.frame(&mut tutorial, &assets, 20);
        let finished = host.status(&tutorial);
        assert!(finished.failure.is_none(), "Tutorial 1 completion");
        assert!(finished.tutorial_completed);
        assert!(tutorial.catalog.selected_level().unwrap().completed()[0]);

        // TUTORIAL2 shares the native optional-player path and opens on the
        // same original tempmap. Follow its retained prompts and pick gates.
        let tutorial_two = crate::scenarios::find(100001).unwrap();
        let mut second = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: tutorial_two.token, difficulty: 0, selected_character: 0,
            profile_name: b"Tutorial2", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] },
            crate::campaign_store::load_catalog(&image, &scripts).unwrap(),
            std::path::PathBuf::from("work/economy/tutorial2-startup.profile")).unwrap();
        assert_eq!(second.catalog.selected_level().unwrap().id(), 100001);
        assert_eq!(second.tutorial_state, 1);
        assert_eq!(second.roster.manager.players.len(), 1);
        let mut second_host = crate::live_host::Host::new([0., 20., 0.]);
        second_host.frame(&mut second, &assets, 20);
        let second_status = second_host.status(&second);
        assert!(second_status.failure.is_none(), "Tutorial 2 original startup: {:?}", second_status.failure);
        assert_eq!(second_status.prompts.len(), 1);
        assert_eq!(second_status.prompts[0].dismissal_id, 2);
        assert!(second_status.tutorial_controls.all_disabled);
        second_host.running = true;
        second_host.frame(&mut second, &assets, 80);
        let view = second_host.status(&second);
        assert!(view.failure.is_none(), "Tutorial 2 camera startup: {:?}", view.failure);
        assert!(matches!(view.tutorial_camera_events.as_slice(),
            [crate::live_host::TutorialCameraAction::Mode { mode: 12, .. },
                crate::live_host::TutorialCameraAction::PlanView]),
            "Tutorial 2 camera callbacks: {:?}; clock {:?}; pending {:?}; commands {:?}",
            view.tutorial_camera_events, second.world.clock.timing,
            second.scripts.events.pending(),
            second.hooks.commands.iter().map(|(command, _)| format!("{command:?}"))
                .collect::<Vec<_>>());
        second.dispatch_session_event("InterfacePromptDismiss", 2, &assets, false).unwrap();
        let block = second_host.status(&second);
        assert_eq!(block.prompts[0].dismissal_id, 3);
        assert!(matches!(block.tutorial_camera_events.as_slice(),
            [.., crate::live_host::TutorialCameraAction::CityView,
                crate::live_host::TutorialCameraAction::Mode { mode: 12, .. }]));
        second.dispatch_session_event("InterfacePromptDismiss", 3, &assets, false).unwrap();
        let building = second_host.status(&second);
        assert_eq!(building.prompts[0].dismissal_id, 4);
        assert!(matches!(building.tutorial_camera_events.last(),
            Some(crate::live_host::TutorialCameraAction::Mode { mode: 13, target: 0 })));
        // Native 4d9d80 treats Mode13's target as a business slot. Tempmap's
        // first AddBusiness is the Indiana Avenue deli; its runtime parent
        // must survive original geometry assembly with an entrance for 481cb0.
        let suncols = read("OPENMT2001_SUNCOLS");
        let mut scene = second.scene(&build,
            openmt2001::render_scene::OriginalAssets {
                executable: &executable, directory: &directory, archive: &archive,
            }, &suncols, None, true).unwrap();
        let businesses = scene.inspection_buildings.iter().filter(|row|
            row.initialized_business.as_ref().is_some_and(|b| b.slot == 0))
            .collect::<Vec<_>>();
        assert_eq!(businesses.len(), 1, "Tutorial2 business slot0 rendered once");
        let first = businesses[0];
        let building_center = first.center;
        let building_translation = first.translation;
        assert_eq!(first.category, 8, "tempmap business0 is Indiana Avenue");
        assert!(first.camera_entrance.is_some(), "native building camera needs entrance");
        assert!(scene.building_count > 0 && scene.vertex_count > 0);
        let (mut controls, mut angle) = crate::navigation::building_entry(
            board.blocks[first.category].center,
            [first.translation[0], first.translation[2]],
            first.camera_entrance, first.kind == 2, 0., 0.,
        ).expect("Tutorial2 business0 resolves original building camera");
        let (eye, target) = crate::navigation::building_tick(&mut controls, &mut angle,
            [false; 4], [0; 2]).unwrap();
        let camera = openmt2001::flyby::CameraSample {
            eye, target, up: [0., 1., 0.], vertical_fov: 45.,
        };
        assert!(scene.reframe(camera).is_ok(), "retained scene accepts Mode13 camera");
        let world_center: [f32; 3] = std::array::from_fn(|i|
            building_translation[i] + building_center[i]);
        let clip = glam::Mat4::from_cols_array(&scene.camera)
            * glam::Vec4::new(world_center[0], world_center[1], world_center[2], 1.);
        assert!(clip.w > 0. && (clip.x / clip.w).abs() < 1.
            && (clip.y / clip.w).abs() < 1.,
            "Tutorial2 deli is inside Mode13 view: eye={eye:?} target={target:?} center={world_center:?} clip={clip:?}");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        renderer.prepare(&scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
            scene.camera, scene.fog).unwrap();
        let locator_assets = crate::locator::Assets::load(
            &index, &archive, &installer, &strings, &executable).unwrap();
        let mut locator = locator_assets.runtime_state(
            &crate::locator::RuntimeInputs::from_world(&second.world)).unwrap();
        let players = crate::player_info::State::from_runtime(
            &second.roster.manager, &second.world.ledgers.players,
            second.roster.status.current_player).unwrap();
        locator.attach_ownership(Some(crate::city_ownership::Ownership::from_runtime(
            &second.world, &players).unwrap()));
        locator.attach_rights(crate::city_rights::Rights::from_runtime(
            &second.world, &players).unwrap());
        locator.attach_players(players);
        locator.attach_people(crate::city_people::Report::from_world(&second.world, &image, &second.roster.manager).unwrap());
        locator.attach_prestige(Some(crate::city_prestige::Prestige::from_runtime(
            &second.world, &board).unwrap()));
        locator.attach_polls(Some(crate::consumer_polls::Poll::from_runtime(&second.world).unwrap()));
        assert!(locator_assets.contains(&locator, [32, 180]));
        for control in 24..=27 {
            let point = (64..380).step_by(4)
                .flat_map(|x| (60..150).step_by(4).map(move |y| [x, y]))
                .find(|&point| locator.tutorial_control_at(&locator_assets, Some(point)) == Some(control))
                .unwrap_or_else(|| panic!("supported fresh Strategic View tab {control} is unavailable"));
            locator.mouse(&locator_assets, &scene, Some(point), true);
            locator.mouse(&locator_assets, &scene, Some(point), false);
            assert_eq!(locator.take_tutorial_control(), Some(control));
        }

        second.dispatch_session_event("InterfacePromptDismiss", 4, &assets, false).unwrap();
        let compass = second_host.status(&second);
        assert!(compass.failure.is_none(), "Tutorial 2 compass prompt: {:?}", compass.failure);
        assert_eq!(compass.tutorial_message.unwrap().dismissal_id, 5);
        let toggle = image.f32_at(0x4dbe57).unwrap() as u32;
        assert_eq!(image.f32_at(0x4dbcef).unwrap(), 14.);
        assert_eq!(compass.tutorial_enabled_controls, vec![toggle]);
        second_host.tutorial_event(&mut second, &assets, toggle).unwrap();
        let block_pick = second_host.status(&second);
        assert!(block_pick.failure.is_none(), "Tutorial 2 block pick: {:?}", block_pick.failure);
        assert_eq!(block_pick.tutorial_message.unwrap().dismissal_id, 6);
        assert_eq!(block_pick.tutorial_interaction.prompt_offset(), [0., 150.]);
        assert_eq!(block_pick.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::Block { category_id: 8 }), Some(0));
        assert_eq!(block_pick.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::BlockEntry { category_id: 8 }), Some(1));
        assert_eq!(block_pick.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::Block { category_id: 7 }), None);
        let selection = crate::selection::Assets::load(&executable, &index, &archive).unwrap();
        let navigator = crate::navigation::Navigation::load(&index, &archive, &executable).unwrap();
        scene.reframe(navigator.camera).unwrap();
        let center = selection.center(8);
        let clip = glam::Mat4::from_cols_array(&scene.camera)
            * glam::Vec4::new(center[0], center[1], center[2], 1.);
        assert!(clip.w > 0., "Indiana Avenue must be in front of the strategic camera");
        let anchor = [((clip.x / clip.w + 1.) * 640.) as i32,
            ((1. - clip.y / clip.w) * 480.) as i32];
        let strategic_hit = (-6..=6).flat_map(|row| (-6..=6).map(move |col|
            [anchor[0] + col * 12, anchor[1] + row * 12]))
            .filter_map(|point| selection.pick(navigator.camera, &scene, point, None))
            .find(|hit| selection.blocks[hit.category].id == 8)
            .unwrap_or_else(|| panic!("Indiana Avenue lacks a strategic pick near {anchor:?}"));
        assert_eq!(selection.blocks[strategic_hit.category].id, 8,
            "original Tutorial 2 arrow targets Indiana Avenue");
        assert!(second_host.tutorial_pick(&mut second, &assets,
            crate::tutorial_two_interaction::Pick::Block { category_id: 8 }).unwrap());
        let single_selection = second_host.status(&second);
        assert!(single_selection.failure.is_none());
        assert_eq!(single_selection.tutorial_message.unwrap().dismissal_id, 6,
            "native Picked3D(0) selection must not complete the double-click instruction");
        assert!(single_selection.prompts.is_empty());
        assert!(second_host.tutorial_pick(&mut second, &assets,
            crate::tutorial_two_interaction::Pick::BlockEntry { category_id: 8 }).unwrap());
        let selected_block = second_host.status(&second);
        assert!(selected_block.failure.is_none(), "Tutorial 2 selected block: {:?}", selected_block.failure);
        assert_eq!(selected_block.prompts[0].dismissal_id, 7);
        assert_eq!(selected_block.tutorial_interaction.prompt_offset(), [0., 0.]);
        assert_eq!(selected_block.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::Block { category_id: 8 }), None);

        second.dispatch_session_event("InterfacePromptDismiss", 7, &assets, false).unwrap();
        let building_pick = second_host.status(&second);
        assert!(building_pick.failure.is_none(), "Tutorial 2 building pick: {:?}", building_pick.failure);
        assert_eq!(building_pick.tutorial_message.unwrap().dismissal_id, 8);
        assert_eq!(building_pick.tutorial_interaction.prompt_offset(), [0., -100.]);
        assert_eq!(building_pick.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::Building { slot: 0 }), Some(2));
        let mut block_camera = crate::navigation::block_entry(selection.center(8));
        let (eye, target) = block_camera.tick([false; 4]);
        let camera = openmt2001::flyby::CameraSample {
            eye, target, up: [0., 1., 0.], vertical_fov: 45.,
        };
        scene.reframe(camera).unwrap();
        let building_hit = scene.inspection_buildings.iter().enumerate()
            .filter(|(_, building)| building.category == 8)
            .find_map(|(_, building)| {
                let center: [f32; 3] = std::array::from_fn(|axis|
                    building.translation[axis] + building.center[axis]);
                let clip = glam::Mat4::from_cols_array(&scene.camera)
                    * glam::Vec4::new(center[0], center[1], center[2], 1.);
                (clip.w > 0.).then(|| {
                    let point = [((clip.x / clip.w + 1.) * 640.) as i32,
                        ((1. - clip.y / clip.w) * 480.) as i32];
                    selection.pick(camera, &scene, point, None)
                }).flatten().filter(|hit| hit.category == 8 && hit.building.is_some())
            }).expect("Tutorial 2 settled Block View can pick an Indiana building");
        assert!(second_host.tutorial_pick(&mut second, &assets,
            crate::tutorial_two_interaction::Pick::Building {
                slot: building_hit.building.unwrap() }).unwrap());
        let selected_building = second_host.status(&second);
        assert!(selected_building.failure.is_none(), "Tutorial 2 selected building: {:?}", selected_building.failure);
        assert_eq!(selected_building.prompts[0].dismissal_id, 9);
        assert_eq!(selected_building.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::Building { slot: 0 }), None);
        for dismissal in 9..=11 {
            second.dispatch_session_event("InterfacePromptDismiss", dismissal, &assets, false).unwrap();
            let status = second_host.status(&second);
            assert!(status.failure.is_none(), "Tutorial 2 prompt {dismissal}: {:?}", status.failure);
            if dismissal < 11 {
                assert_eq!(status.prompts[0].dismissal_id, (dismissal + 1) as u32);
            } else {
                assert_eq!(status.tutorial_message.unwrap().dismissal_id, 12);
                assert_eq!(status.tutorial_enabled_controls,
                    vec![14, 103]);
                assert_eq!(status.tutorial_interaction.callback(
                    crate::tutorial_two_interaction::Pick::Block { category_id: 8 }), Some(0));
            }
        }
        second_host.tutorial_event(&mut second, &assets, 14).unwrap();
        let learned = second_host.status(&second);
        assert!(learned.failure.is_none(), "Tutorial 2 learned views: {:?}", learned.failure);
        assert_eq!(learned.prompts[0].dismissal_id, 13);
        second.dispatch_session_event("InterfacePromptDismiss", 13, &assets, false).unwrap();
        second_host.frame(&mut second, &assets, 20);
        let completed = second_host.status(&second);
        assert!(completed.failure.is_none(), "Tutorial 2 completion: {:?}", completed.failure);
        assert!(completed.tutorial_completed);
    }

    #[test]
    #[ignore = "requires supplied v1.4 Tutorial 3 scripts and original assets"]
    fn original_tutorial_three_strategic_flow() {
        tutorial_three_to_five(None);
    }

    #[test]
    #[ignore = "requires original inputs; verifies strict v1.4 Tutorial5 rights defect"]
    fn original_tutorial_five_rights_frontier() {
        tutorial_three_to_five(Some(crate::compatibility::Mode::StrictV14));
    }

    #[test]
    #[ignore = "requires original Tutorial5 scripts and assets; modern documented-blocker repair"]
    fn original_tutorial_five_compatibility_flow() {
        tutorial_three_to_five(Some(crate::compatibility::Mode::Modern));
    }


    #[test]
    #[ignore = "requires supplied original Tutorial8 inputs"]
    fn original_tutorial_eight_strict_rights_gate() {
        tutorial_eight(crate::compatibility::Mode::StrictV14);
    }
    #[test]
    #[ignore = "requires supplied original Tutorial8 inputs"]
    fn original_tutorial_eight_three_business_flow() {
        tutorial_eight(crate::compatibility::Mode::Modern);
    }
    fn tutorial_eight(mode: crate::compatibility::Mode) {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = openmt2001::archive::ArchiveIndex::parse(
            &directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let executable_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts = executable_path.parent().unwrap().join("scripts");
        let language = executable_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names = super::super::character_names(&image, &language).unwrap();
        let era_styles = load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap()).unwrap();
        let installer = Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap().parent().unwrap().to_path_buf();
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let text = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets.chunks_exact(2).map(|length| {
            let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let value = &text[at..end];
            at = end;
            value
        }).collect::<Vec<_>>();
        assert_eq!(at, text.len());
        let assets = super::super::CommandAssets { image: &image, board: &board, file: &file };

        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
        }
        let scratch = Scratch(std::path::PathBuf::from(std::env::var_os("OPENMT2001_TEST_SCRATCH").unwrap())
            .join(format!("tutorial8-{}-{}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        fs::create_dir_all(&scratch.0).unwrap();
        let profile = scratch.0.join("progress.profile");
        let mut session = Session::initialize_campaign_compatible(Inputs {
            tree_locators: None,
            image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),era_styles,
            timetable:std::str::from_utf8(&timetable).unwrap(),
            scenario:crate::scenarios::find(100008).unwrap().token,difficulty:0,
            selected_character:0,profile_name:b"Tutorial8",character_names:&names,
        },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]},
            crate::campaign_store::load_catalog(&image,&scripts).unwrap(),profile.clone(),mode).unwrap();
        let mut host=crate::live_host::Host::new([0.,20.,0.]);
        host.begin_presentation(&session).unwrap();
        host.frame(&mut session,&assets,20);
        let prompt_assets=crate::prompt::Assets::load(&index,&archive,&installer,&strings,&image).unwrap();
        let ack=|session:&mut Session<'_>,host:&mut crate::live_host::Host,id| {
            let status=host.status(session);
            let request=status.prompts.last().cloned().unwrap();
            assert_eq!(request.dismissal_id,id);
            let mut modal=crate::prompt::State::default();
            let _=modal.submit(request);
            let bounds=prompt_assets.button_at(status.tutorial_interaction.prompt_offset());
            modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
            assert_eq!(modal.left_button(true,bounds),None);
            assert_eq!(modal.left_button_with(false,bounds,|_,id| {
                session.dispatch_session_event("InterfacePromptDismiss",id as i32,&assets,false).unwrap();
            }),Some(id));
        };
        let buildings=|session:&Session<'_>| session.world.buildings.iter().filter(|row|row[0x3f]!=0).count();
        assert_eq!(buildings(&session),4,"original shoppingmap supplies four buildings");
        let businesses_before=session.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count();
        ack(&mut session,&mut host,2);
        assert_eq!(host.status(&session).tutorial_enabled_controls,vec![73]);
        host.tutorial_event(&mut session,&assets,73).unwrap();
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,4);
        assert!(host.status(&session).tutorial_enabled_controls.contains(&23));
        let scene=session.scene(&build,openmt2001::render_scene::OriginalAssets{
            executable:&executable,directory:&directory,archive:&archive},
            &read("OPENMT2001_SUNCOLS"),None,true).unwrap();
        let locator_assets=crate::locator::Assets::load(&index,&archive,&installer,&strings,&executable).unwrap();
        let mut locator=locator_assets.runtime_state(&crate::locator::RuntimeInputs::from_world(&session.world)).unwrap();
        locator.attach_polls(Some(crate::consumer_polls::Poll::from_runtime(&session.world).unwrap()));
        let poll_point=(0..640).step_by(4).flat_map(|x|(0..480).step_by(4).map(move|y|[x,y]))
            .find(|&point|locator.tutorial_control_at(&locator_assets,Some(point))==Some(23)).unwrap();
        locator.mouse(&locator_assets,&scene,Some(poll_point),true);
        locator.mouse(&locator_assets,&scene,Some(poll_point),false);
        assert_eq!(locator.take_tutorial_control(),Some(23));
        host.tutorial_event(&mut session,&assets,23).unwrap();
        let status=host.status(&session);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id,5);
        let kentucky=(0..52).find(|&id|status.tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::Block{category_id:id})==Some(0)).unwrap();
        assert!(!host.tutorial_pick(&mut session,&assets,crate::tutorial_two_interaction::Pick::Block{
            category_id:(kentucky+1)%52}).unwrap());
        assert!(host.tutorial_pick(&mut session,&assets,crate::tutorial_two_interaction::Pick::Block{category_id:kentucky}).unwrap());
        ack(&mut session,&mut host,6);
        assert!(host.tutorial_pick(&mut session,&assets,crate::tutorial_two_interaction::Pick::BlockEntry{category_id:kentucky}).unwrap());
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,8);
        let context=crate::build_panel::Context::from_session(&session,kentucky as usize).unwrap();
        host.tutorial_event(&mut session,&assets,11).unwrap();
        println!("Tutorial8 {mode:?}: Kentucky{id} owner{owner} current{current} rights{rights} pending{pending} entry{entry:?}",
            id=kentucky,owner=context.category_record[5],current=context.current_owner,
            rights=u32::from_le_bytes(context.category_record[0x78..0x7c].try_into().unwrap()),
            pending=context.category_record[0x6d],entry=context.entry());
        if mode==crate::compatibility::Mode::StrictV14 {
            assert_eq!(context.entry(),crate::build_panel::Entry::Message{text:1202,show_rights_action:true});
            assert_eq!(&context.category_record[0x78..0x7c],&[0;4]);
            return;
        }
        assert_eq!(context.entry(),crate::build_panel::Entry::Open);
        let planning_assets=crate::build_view::Assets::load(&index,&archive,&installer,&strings,&executable).unwrap();
        let click=|planning:&mut crate::build_view::State,point:[i32;2]|{
            planning.pointer(Some(point));planning.mouse(true);planning.mouse(false);
            planning.take_tutorial_control()
        };
        // Supplied US prompt bodies2157..2159 request Bakery, Grocery Store,
        // and Cafe. Stale Lua comments name different businesses.
        for (ordinal,(use_index,expected_business)) in [(1,0u32),(12,5),(4,12)].into_iter().enumerate() {
            if ordinal!=0 {host.tutorial_event(&mut session,&assets,11).unwrap();}
            let status=host.status(&session);
            assert_eq!(status.tutorial_build_type,0);
            assert_eq!(status.tutorial_build_use,use_index);
            assert!(!status.tutorial_enabled_controls.contains(&98));
            assert!(!status.tutorial_enabled_controls.contains(&99));
            host.tutorial_event(&mut session,&assets,98).unwrap();
            let mut planning=planning_assets.open(crate::build_panel::Context::from_session(&session,kentucky as usize).unwrap()).unwrap();
            planning.set_tutorial_build_type(status.tutorial_build_type);
            planning.set_tutorial_build_use(status.tutorial_build_use);
            assert_eq!(click(&mut planning,[100,155]),None);
            assert_eq!(planning.step,0,"wrong type rejected");
            assert_eq!(click(&mut planning,[100,100]),Some(86));
            host.tutorial_event(&mut session,&assets,86).unwrap();
            assert_eq!(click(&mut planning,[100,186]),Some(87));
            host.tutorial_event(&mut session,&assets,87).unwrap();
            assert_eq!(planning.step,1,"wrong use rejected");
            'uses: for scroll in 0..45 {
                if scroll>0 {planning.scroll(-1);}
                for row in 0..10 {
                    if click(&mut planning,[100,150+row*18])==Some(87) {
                        host.tutorial_event(&mut session,&assets,87).unwrap();
                        if planning.step==2 {break 'uses;}
                    }
                }
            }
            assert_eq!(planning.step,2,"authored use must be selectable through the actual list");
            let block=board.blocks.iter().find(|block|block.id==kentucky as usize).unwrap();
            let mut navigation=crate::navigation::block_entry([block.center[0],0.,block.center[1]]);
            let (eye,target)=navigation.tick([false;4]);
            planning.project_grid(openmt2001::flyby::CameraSample{eye,target,up:[0.,1.,0.],vertical_fov:45.},block.center).unwrap();
            let centers=crate::build_panel::cell_centers(planning.grid.as_ref().unwrap());
            let mut placed=false;
            'footprints:for width in 1..=6usize {for depth in 1..=6usize {
                for x in 0..=7-width {for y in 0..=7-depth {
                    let start=centers[x*7+y].map(|v|v as i32);
                    let end=centers[(x+width-1)*7+y+depth-1].map(|v|v as i32);
                    planning.pointer(Some(start));if !planning.tutorial_drag_at(){continue;}
                    planning.mouse(true);planning.pointer(Some(end));planning.mouse(false);
                    if planning.take_placement_request(){placed=true;break 'footprints;}
                }}
            }}
            assert!(placed,"legal footprint for authored use{use_index}");
            let choices=host.complete_build_drag(&mut session,&assets).unwrap();
            planning.apply_placement_choices(choices);
            let request=planning.request().unwrap();
            assert_eq!(request.business,expected_business,
                "Tutorial8 use{use_index} must construct the business named by its supplied US prompt");
            let quote=session.inspect_build(&request,&assets).unwrap();
            assert!(quote.allowed(),"legal original catalog build use{use_index}");
            planning.quote_request=Some(request.clone());planning.quote=Some(quote);planning.needs_quote=false;
            assert_eq!(click(&mut planning,[140,550]),Some(97));
            assert!(planning.submit);
            host.tutorial_event(&mut session,&assets,97).unwrap();
            let queued=session.live_commands.snapshot().unwrap().len();
            assert!(session.submit_build(&request,&assets,|event|Err(format!("unexpected synchronous construction {event:?}"))).unwrap().submitted);
            assert_eq!(buildings(&session),4+ordinal);
            host.set_running(true);
            assert_eq!(session.live_commands.snapshot().unwrap().len(),queued+1);
            // Build command admission creates a pending structure first. Wait
            // through genuine pending-world construction and delayed Open too.
            for _ in 0..1000 {
                host.frame(&mut session,&assets,40);
                assert!(host.status(&session).failure.is_none());
                if session.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count()==businesses_before+ordinal+1 {break;}
            }
            assert_eq!(session.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count(),businesses_before+ordinal+1,
                "running pending-world construction must open the authored business");
            assert_eq!(buildings(&session),5+ordinal);
            let new_business=session.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).nth(businesses_before+ordinal).unwrap();
            assert_eq!(u32::from_le_bytes(new_business[4..8].try_into().unwrap()),request.business);
            let building_slot=u16::from_le_bytes(new_business[8..10].try_into().unwrap()) as usize;
            let building=&session.world.buildings[building_slot];
            assert_ne!(building[0x3f],0);
            assert_eq!(u32::from_le_bytes(building[0x40..0x44].try_into().unwrap()),0x617628+kentucky*380);
            assert_eq!(request.kind,0);
            println!("Tutorial8 built use{use_index}, catalog{}, slot{building_slot}, origin{:?}, size{:?}, height{}",request.business,request.origin,request.size,request.height);
            assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,9+ordinal as u32,
                "genuine internal99 advances the original hub exactly once");
            host.frame(&mut session,&assets,40);
            assert_eq!(buildings(&session),5+ordinal);
        }
        assert_eq!(buildings(&session),7);
        assert_eq!(session.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count(),businesses_before+3);
        host.tutorial_event(&mut session,&assets,73).unwrap();
        ack(&mut session,&mut host,12);
        ack(&mut session,&mut host,13);
        host.frame(&mut session,&assets,20);
        assert!(host.status(&session).tutorial_completed);
        let recorded=fs::read(&profile).unwrap();
        host.frame(&mut session,&assets,40);
        assert_eq!(fs::read(&profile).unwrap(),recorded);
        let mut progression=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut progression,&profile).unwrap();
        assert!(progression.levels.iter().find(|level|level.id()==100008).unwrap().completed()[0]);
        assert!(progression.levels.iter().find(|level|level.id()==100009).unwrap().unlocked());
    }

    fn tutorial_three_to_five(fifth_mode: Option<crate::compatibility::Mode>) {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = openmt2001::archive::ArchiveIndex::parse(
            &directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let executable_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts = executable_path.parent().unwrap().join("scripts");
        let language = executable_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names = super::super::character_names(&image, &language).unwrap();
        let era_styles = load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap()).unwrap();
        let installer = Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap().parent().unwrap().to_path_buf();
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let text = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets.chunks_exact(2).map(|length| {
            let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let value = &text[at..end];
            at = end;
            value
        }).collect::<Vec<_>>();
        assert_eq!(at, text.len());
        let assets = super::super::CommandAssets { image: &image, board: &board, file: &file };
        let toggle = image.f32_at(0x4dbe57).unwrap() as u32;
        // Tutorial 3 starts on Pacific Avenue and reuses the wired compass
        // control before opening the Strategic View information sequence.
        let tutorial_three = crate::scenarios::find(100002).unwrap();
        let profile = std::path::PathBuf::from(format!("work/economy/tutorial3-{}-{}.profile", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        if profile.exists() { fs::remove_file(&profile).unwrap(); }
        let mut third = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: tutorial_three.token, difficulty: 0, selected_character: 0,
            profile_name: b"Tutorial3", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] },
            crate::campaign_store::load_catalog(&image, &scripts).unwrap(),
            profile.clone()).unwrap();
        assert_eq!(third.roster.manager.players.len(), 2);
        assert!(third.roster.status.has_ai);
        assert_eq!(third.roster.manager.players[1].bytes[0x1403], 1, "native SetTwiddleThumbs stops the linked IRON player");
        assert_eq!(third.roster.manager.players.iter().map(|player|
            u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()))
            .collect::<Vec<_>>(), vec![4, 6], "original Tutorial 3 registers HAT and IRON");
        let scene = third.scene(&build, openmt2001::render_scene::OriginalAssets {
            executable: &executable, directory: &directory, archive: &archive,
        }, &read("OPENMT2001_SUNCOLS"), None, true).unwrap();
        let mut third_host = crate::live_host::Host::new([0., 20., 0.]);
        third_host.frame(&mut third, &assets, 20);
        let opening = third_host.status(&third);
        assert!(opening.failure.is_none(), "Tutorial 3 startup: {:?}", opening.failure);
        assert_eq!(opening.tutorial_message.unwrap().dismissal_id, 2);
        assert_eq!(opening.tutorial_enabled_controls, vec![toggle]);
        third_host.tutorial_event(&mut third, &assets, toggle).unwrap();
        let strategic = third_host.status(&third);
        assert!(strategic.failure.is_none(), "Tutorial 3 compass: {:?}", strategic.failure);
        assert_eq!(strategic.prompts[0].dismissal_id, 3);
        assert_eq!(strategic.tutorial_interaction.prompt_offset(), [0., 150.]);
        for dismissal in 3..=5 {
            third.dispatch_session_event("InterfacePromptDismiss", dismissal, &assets, false).unwrap();
            let status = third_host.status(&third);
            assert!(status.failure.is_none(), "Tutorial 3 prompt {dismissal}: {:?}", status.failure);
            assert_eq!(status.prompts[0].dismissal_id, (dismissal + 1) as u32);
            if dismissal == 5 {
                assert_eq!(status.tutorial_enabled_controls, vec![0, 24, 25, 26, 27]);
                assert_eq!(status.tutorial_arrows, vec![[83, 50]]);
            }
        }
        let runtime_inputs = crate::locator::RuntimeInputs::from_world(&third.world);
        let locator_assets = crate::locator::Assets::load(
            &index, &archive, &installer, &strings, &executable).unwrap();
        let mut locator = locator_assets.runtime_state(&runtime_inputs).unwrap();
        let players = crate::player_info::State::from_runtime(
            &third.roster.manager, &third.world.ledgers.players,
            third.roster.status.current_player).unwrap();
        let ownership = crate::city_ownership::Ownership::from_runtime(&third.world, &players).unwrap();
        let rights = crate::city_rights::Rights::from_runtime(&third.world, &players).unwrap();
        locator.attach_people(crate::city_people::Report::from_world(&third.world, &image, &third.roster.manager).unwrap());
        locator.attach_players(players);
        locator.attach_prestige(Some(crate::city_prestige::Prestige::from_runtime(
            &third.world, &board).unwrap()));
        locator.attach_ownership(Some(ownership));
        locator.attach_rights(rights);
        locator.attach_polls(Some(crate::consumer_polls::Poll::from_runtime(&third.world).unwrap()));
        assert!(locator_assets.contains(&locator, [32, 180]),
            "fresh tutorial must expose Player Information button");
        let tab_points = (24..=27).map(|id| {
            (64..380).step_by(4).flat_map(|x| (60..150).step_by(4).map(move |y| [x, y]))
                .find(|&point| locator.tutorial_control_at(&locator_assets, Some(point)) == Some(id))
                .unwrap_or_else(|| panic!("fresh Strategic View tab {id} is not hit-testable"))
        }).collect::<Vec<_>>();
        for (index, point) in tab_points.into_iter().enumerate() {
            locator.mouse(&locator_assets, &scene, Some(point), true);
            locator.mouse(&locator_assets, &scene, Some(point), false);
            let control = locator.take_tutorial_control().unwrap();
            assert_eq!(control, 24 + index as u32);
            third_host.tutorial_event(&mut third, &assets, control).unwrap();
            let status = third_host.status(&third);
            assert!(status.failure.is_none(), "Tutorial 3 strategic tab: {:?}", status.failure);
            assert!(status.prompts.is_empty(), "information tabs retain prompt 6");
        }
        third.dispatch_session_event("InterfacePromptDismiss", 6, &assets, false).unwrap();
        let player_instruction = third_host.status(&third);
        assert_eq!(player_instruction.tutorial_message.unwrap().dismissal_id, 7);
        assert!(player_instruction.tutorial_enabled_controls.contains(&21));
        let player_button = [32, 180];
        assert_eq!(locator.tutorial_control_at(&locator_assets, Some(player_button)), Some(21));
        locator.mouse(&locator_assets, &scene, Some(player_button), true);
        locator.mouse(&locator_assets, &scene, Some(player_button), false);
        third_host.tutorial_event(&mut third, &assets,
            locator.take_tutorial_control().unwrap()).unwrap();
        let player_prompt = third_host.status(&third);
        assert!(player_prompt.failure.is_none(), "Tutorial 3 player view: {:?}", player_prompt.failure);
        assert_eq!(player_prompt.prompts[0].dismissal_id, 8);
        let token = (64..380).step_by(4)
            .flat_map(|x| (60..150).step_by(4).map(move |y| [x, y]))
            .find(|&point| locator.tutorial_control_at(&locator_assets, Some(point)) == Some(24))
            .expect("first player token has native tutorial control 24");
        locator.mouse(&locator_assets, &scene, Some(token), true);
        locator.mouse(&locator_assets, &scene, Some(token), false);
        third_host.tutorial_event(&mut third, &assets,
            locator.take_tutorial_control().unwrap()).unwrap();
        assert!(third_host.status(&third).failure.is_none());
        third.dispatch_session_event("InterfacePromptDismiss", 8, &assets, false).unwrap();
        let locator_instruction = third_host.status(&third);
        assert!(locator_instruction.failure.is_none(), "Tutorial 3 locator instruction: {:?}", locator_instruction.failure);
        assert_eq!(locator_instruction.tutorial_message.unwrap().dismissal_id, 9);
        assert!(locator_instruction.tutorial_enabled_controls.contains(&22));

        locator.refresh_scene(&scene);
        let click = |locator: &mut crate::locator::Locator, control| {
            let point = (24..400).step_by(2)
                .flat_map(|x| (40..400).step_by(2).map(move |y| [x, y]))
                .find(|&point| locator.tutorial_control_at(&locator_assets, Some(point)) == Some(control))
                .unwrap_or_else(|| panic!("Tutorial 3 Locator control {control} is missing"));
            locator.mouse(&locator_assets, &scene, Some(point), true);
            let building = locator.mouse(&locator_assets, &scene, Some(point), false);
            (locator.take_tutorial_control().unwrap(), building)
        };
        let (control, _) = click(&mut locator, 22);
        third_host.tutorial_event(&mut third, &assets, control).unwrap();
        let owned = third_host.status(&third);
        assert_eq!(owned.tutorial_message.unwrap().dismissal_id, 10);
        assert_eq!(owned.tutorial_enabled_controls, vec![0, 6, 30]);
        let (control, building) = click(&mut locator, 30);
        assert_eq!(control, 6);
        let selected = &scene.inspection_buildings[building.expect("owned-business link selects actual building")];
        assert_eq!(selected.saved_business.as_ref().map(|b| b.record[0xa])
            .or_else(|| selected.initialized_business.as_ref().map(|b| b.record[0xa])), Some(1));
        third_host.tutorial_event(&mut third, &assets, control).unwrap();
        assert_eq!(third_host.status(&third).tutorial_message.unwrap().dismissal_id, 11);
        third_host.tutorial_event(&mut third, &assets, toggle).unwrap();
        assert_eq!(third_host.status(&third).tutorial_message.unwrap().dismissal_id, 12);
        let (control, _) = click(&mut locator, 25);
        third_host.tutorial_event(&mut third, &assets, control).unwrap();
        assert_eq!(third_host.status(&third).prompts[0].dismissal_id, 13);
        let (control, _) = click(&mut locator, 31);
        third_host.tutorial_event(&mut third, &assets, control).unwrap();
        assert_eq!(third_host.status(&third).tutorial_message.unwrap().dismissal_id, 14);
        let (control, _) = click(&mut locator, 26);
        third_host.tutorial_event(&mut third, &assets, control).unwrap();
        assert_eq!(third_host.status(&third).tutorial_message.unwrap().dismissal_id, 15);
        let (control, _) = click(&mut locator, 23);
        third_host.tutorial_event(&mut third, &assets, control).unwrap();
        assert_eq!(third_host.status(&third).tutorial_message.unwrap().dismissal_id, 16);
        let mut constants = openmt2001::lua4::Runtime::new().unwrap();
        openmt2001::initial_conditions::Bindings::register_block_constants(&mut constants, &image).unwrap();
        let illinois = constants.number("ILLINOIS_AVENUE").unwrap().unwrap() as u32;
        assert!(third_host.tutorial_pick(&mut third, &assets,
            crate::tutorial_two_interaction::Pick::Block { category_id: illinois }).unwrap());
        assert_eq!(third_host.status(&third).prompts[0].dismissal_id, 17);
        third.dispatch_session_event("InterfacePromptDismiss", 17, &assets, false).unwrap();
        third_host.frame(&mut third, &assets, 20);
        let completed = third_host.status(&third);
        assert!(completed.failure.is_none(), "Tutorial 3 completion: {:?}", completed.failure);
        assert!(completed.tutorial_completed);
        assert!(third.catalog.selected_level().unwrap().completed()[0]);
        println!("Tutorial3: original script flow completed and progression recorded");
        let recorded = fs::read(&profile).unwrap();
        third_host.frame(&mut third, &assets, 20);
        assert_eq!(fs::read(&profile).unwrap(), recorded, "completion is recorded once");
        let mut progression = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut progression, &profile).unwrap();
        assert!(progression.levels.iter().find(|l| l.id() == 100002).unwrap().completed()[0]);
        assert!(progression.levels.iter().find(|l| l.id() == 100003).unwrap().unlocked());
        let fourth_token = crate::scenarios::find(100003).unwrap().token;
        let mut fourth = Session::initialize_campaign(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts: &scripts, parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: era_styles.clone(), timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: fourth_token, difficulty: 0, selected_character: 0,
            profile_name: b"Tutorial3", character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }, progression, profile.clone()).unwrap();
        let mut fourth_host = crate::live_host::Host::new([0., 20., 0.]);
        fourth_host.frame(&mut fourth, &assets, 20);
        let next = fourth_host.status(&fourth);
        assert!(next.failure.is_none(), "Tutorial 4 connected startup: {:?}", next.failure);
        assert_eq!(next.prompts[0].dismissal_id, 2);
        assert_eq!(fourth.world.ledgers.residents.active_count, 100);
        assert_eq!(fourth.roster.manager.players[1].bytes[0x1403], 1);
        compare_native_auction_announcement_order(&mut fourth,&assets);

        // Continue through the next original tutorial using retained report
        // controls and a legal offer. Native comparison above is separate from
        // this script/command progression; no61 is injected into this flow.
        let pacific=fourth.scripts.runtime.number("PACIFIC_AVENUE").unwrap().unwrap() as usize;
        fourth_host.running=true;
        fourth_host.frame(&mut fourth,&assets,80);
        let camera=fourth_host.status(&fourth);
        assert!(camera.failure.is_none(),"Tutorial4 camera startup: {:?}",camera.failure);
        assert!(camera.tutorial_camera_events.iter().any(|event|matches!(event,
            crate::live_host::TutorialCameraAction::Mode{mode:12,target} if *target==pacific as u32)),
            "original BlockCam selects Pacific Avenue: {:?}",camera.tutorial_camera_events);
        let fourth_scene=fourth.scene(&build,openmt2001::render_scene::OriginalAssets{
            executable:&executable,directory:&directory,archive:&archive},&read("OPENMT2001_SUNCOLS"),None,true).unwrap();
        let players=crate::player_info::State::from_runtime(&fourth.roster.manager,&fourth.world.ledgers.players,fourth.roster.status.current_player).unwrap();
        let report=crate::block_information::Report::from_runtime(&fourth.world,&players,&image).unwrap().unwrap();
        let mut property=crate::utility_report::View::default();
        property.select(Some(pacific));
        let click_property=|property:&mut crate::utility_report::View,id:u32|{
            let point=(0..400).step_by(2).flat_map(|x|(50..650).step_by(2).map(move|y|[x,y]))
                .find(|&point|locator_assets.property_tutorial_control_at(property,Some(point),&report,pacific)==Some(id))
                .unwrap_or_else(||panic!("original Tutorial4 property control{id} has no retained hitbox"));
            locator_assets.property_pointer(property,Some(point),&report,pacific);
            assert!(property.mouse(true));
            locator_assets.property_pointer(property,Some(point),&report,pacific);
            assert!(property.mouse(false));
            property.take_tutorial_control().unwrap()
        };
        for dismissal in [2,3]{fourth.dispatch_session_event("InterfacePromptDismiss",dismissal,&assets,false).unwrap();}
        assert_eq!(fourth_host.status(&fourth).tutorial_message.unwrap().dismissal_id,4);
        let control=click_property(&mut property,16);
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        assert_eq!(fourth_host.status(&fourth).tutorial_message.unwrap().dismissal_id,5);
        let control=click_property(&mut property,32);
        assert_eq!(control,6);
        let slot=property.requested_slot.take().expect("LandRent row links an actual business");
        let selected=fourth_scene.inspection_buildings.iter().find(|building|
            building.category==pacific && building.runtime_slot==Some(slot)).unwrap_or_else(||panic!(
                "LandRent building slot{slot} in category{pacific}; scene identities {:?}; record category {:x}; active{} model{:x}; categorylist {:?}; rows {:?}",
                fourth_scene.inspection_buildings.iter().filter(|building|building.category==pacific || building.runtime_slot==Some(slot))
                    .map(|building|(building.category,building.runtime_slot)).collect::<Vec<_>>(),
                u32::from_le_bytes(fourth.world.buildings[slot][0x40..0x44].try_into().unwrap()),
                fourth.world.buildings[slot][0x3f],
                u32::from_le_bytes(fourth.world.buildings[slot][0x170..0x174].try_into().unwrap()),
                fourth.world.categories[pacific][12..140].chunks_exact(4).map(|bytes|i32::from_le_bytes(bytes.try_into().unwrap())).collect::<Vec<_>>(),
                report.land_rent[pacific]));
        assert_eq!(selected.category,pacific);
        let expected_owner=report.land_rent[pacific].iter().find_map(|row|match row{
            crate::block_information::land_rent::Row::Business(b) if b.slot==slot=>Some(b.owner),_=>None}).unwrap();
        assert_eq!(selected.initialized_business.as_ref().unwrap().record[0xa],expected_owner);
        assert!(crate::navigation::building_entry(board.blocks[pacific].center,[selected.translation[0],selected.translation[2]],
            selected.camera_entrance,selected.kind==2,0.,0.).is_some(),"linked business enters the retained native camera");
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        assert_eq!(fourth_host.status(&fourth).tutorial_message.unwrap().dismissal_id,6);
        let business_assets=crate::business_view::Assets::load(&index,&archive,&installer,&strings,&executable).unwrap();
        assert!(business_assets.go_to_block_contains([20.,288.]));
        fourth_host.tutorial_event(&mut fourth,&assets,14).unwrap();
        assert_eq!(fourth_host.status(&fourth).tutorial_message.unwrap().dismissal_id,7);
        let control=click_property(&mut property,17);
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        assert_eq!(fourth_host.status(&fourth).prompts[0].dismissal_id,8);
        fourth.dispatch_session_event("InterfacePromptDismiss",8,&assets,false).unwrap();
        for (control,dismissal) in [(33,10),(34,11)]{
            let actual=click_property(&mut property,control);
            assert_eq!(actual,control);
            fourth_host.tutorial_event(&mut fourth,&assets,actual).unwrap();
            assert_eq!(fourth_host.status(&fourth).tutorial_message.unwrap().dismissal_id,dismissal);
        }
        assert!(!property.previous_profit,"real period controls return to today's report");
        let control=click_property(&mut property,18);
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        assert_eq!(fourth_host.status(&fourth).prompts[0].dismissal_id,12);
        for dismissal in [12,13]{fourth.dispatch_session_event("InterfacePromptDismiss",dismissal,&assets,false).unwrap();}
        let control=click_property(&mut property,19);
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        assert_eq!(fourth_host.status(&fourth).prompts.last().unwrap().dismissal_id,15);
        for dismissal in [15,16]{fourth.dispatch_session_event("InterfacePromptDismiss",dismissal,&assets,false).unwrap();}
        assert_eq!(fourth_host.status(&fourth).tutorial_message.unwrap().dismissal_id,17);
        let control=click_property(&mut property,10);
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        let request=property.take_property_request().expect("Lease button opens the real retained offer panel");
        assert!(!request.submit);assert_eq!(request.category,pacific as u32);
        let inspection=fourth_host.inspect_property_offer(&mut fourth,&assets,request.category,request.panel,true).unwrap();
        assert_eq!(inspection.panel.visible,1);assert_eq!(inspection.panel.action_visible,1);
        assert_eq!(inspection.panel.amount_visible,1);
        assert!(inspection.panel.amount>=inspection.panel.minimum && inspection.panel.amount<=inspection.panel.maximum);
        property.apply_property_inspection(request.category,Ok(inspection));
        let offer=fourth_host.status(&fourth);
        assert_eq!(offer.tutorial_message.unwrap().dismissal_id,18);
        assert!(!offer.tutorial_enabled_controls.contains(&61),"internal61 is independent of GUI enablement");
        let control=click_property(&mut property,35);
        fourth_host.tutorial_event(&mut fourth,&assets,control).unwrap();
        let request=property.take_property_request().expect("Confirm button submits its legal panel");
        assert!(request.submit);
        let submitted=fourth_host.property_offer(&mut fourth,&assets,request.category,request.panel).unwrap();
        assert!(submitted.submitted,"legal original cash/ownership admits the city auction");
        property.apply_property_inspection(request.category,Ok(submitted));
        fourth_host.running=true;
        let mut announcement=None;
        for _ in 0..100{
            fourth_host.frame(&mut fourth,&assets,40);
            let status=fourth_host.status(&fourth);
            assert!(status.failure.is_none(),"Tutorial4 queued auction: {:?}",status.failure);
            if fourth.world.pause.has_reason(){announcement=Some(status);break;}
        }
        assert!(fourth.world.pause.has_reason(),"original internal61 pauses synchronously");
        assert_eq!(fourth.world.city.auction.presentation.countdown,250);
        assert!(fourth.world.city.auction.presentation.visible);
        assert_eq!(fourth.world.city.categories[pacific][0x6c],1);
        let announcement=announcement.expect("paused auction retains its delivered prompt status");
        assert!(announcement.tutorial_message.is_none());
        assert_eq!(announcement.prompts[0].dismissal_id,19,"automatic61 followed the original dismissal to prompt18");
        for dismissal in [19,20]{fourth.dispatch_session_event("InterfacePromptDismiss",dismissal,&assets,false).unwrap();}
        fourth_host.frame(&mut fourth,&assets,20);
        let completed=fourth_host.status(&fourth);
        assert!(completed.failure.is_none(),"Tutorial4 completion: {:?}",completed.failure);
        assert!(completed.tutorial_completed);
        let recorded=fs::read(&profile).unwrap();
        fourth_host.frame(&mut fourth,&assets,20);
        assert_eq!(fs::read(&profile).unwrap(),recorded,"Tutorial4 completion is recorded once");
        let mut progression=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut progression,&profile).unwrap();
        assert!(progression.levels.iter().find(|level|level.id()==100003).unwrap().completed()[0]);
        assert!(progression.levels.iter().find(|level|level.id()==100004).unwrap().unlocked());
        println!("Tutorial4: property/auction flow completed; Tutorial5 unlocked");
        if fifth_mode.is_none() {
            fs::remove_file(&profile).unwrap();
            return;
        }
        let fifth_token=crate::scenarios::find(100004).unwrap().token;
        let mut fifth=Session::initialize_campaign_compatible(Inputs{
            tree_locators: None,
            image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),era_styles:era_styles.clone(),
            timetable:std::str::from_utf8(&timetable).unwrap(),scenario:fifth_token,difficulty:0,
            selected_character:0,profile_name:b"Tutorial3",character_names:&names,
        },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]},progression,profile.clone(),fifth_mode.unwrap()).unwrap();
        let mut fifth_host=crate::live_host::Host::new([0.,20.,0.]);
        fifth_host.begin_presentation(&fifth).unwrap();
        fifth_host.frame(&mut fifth,&assets,20);
        let fifth_start=fifth_host.status(&fifth);
        assert!(fifth_start.failure.is_none(),"Tutorial5 connected startup: {:?}",fifth_start.failure);
        assert_eq!(fifth_start.tutorial_message.unwrap().dismissal_id,2);
        assert_eq!(fifth_start.tutorial_build_type,-1);
        assert_eq!(fifth.world.ledgers.residents.active_count,100);
        assert_eq!(fifth.roster.manager.players.len(),1);
        fifth_host.tutorial_event(&mut fifth,&assets,toggle).unwrap();
        assert_eq!(fifth_host.status(&fifth).tutorial_message.unwrap().dismissal_id,3);
        let fifth_scene=fifth.scene(&build,openmt2001::render_scene::OriginalAssets{
            executable:&executable,directory:&directory,archive:&archive},&read("OPENMT2001_SUNCOLS"),None,true).unwrap();
        let fifth_players=crate::player_info::State::from_runtime(&fifth.roster.manager,
            &fifth.world.ledgers.players,fifth.roster.status.current_player).unwrap();
        let fifth_rights=crate::city_rights::Rights::from_runtime(&fifth.world,&fifth_players).unwrap();
        let mut fifth_locator=locator_assets.runtime_state(&crate::locator::RuntimeInputs::from_world(&fifth.world)).unwrap();
        fifth_locator.attach_people(crate::city_people::Report::from_world(&fifth.world,&image,&fifth.roster.manager).unwrap());
        fifth_locator.attach_players(fifth_players);
        fifth_locator.attach_rights(fifth_rights);
        let rights_point=(64..380).step_by(4).flat_map(|x|(60..150).step_by(4).map(move|y|[x,y]))
            .find(|&point|fifth_locator.tutorial_control_at(&locator_assets,Some(point))==Some(26)).unwrap();
        fifth_locator.mouse(&locator_assets,&fifth_scene,Some(rights_point),true);
        fifth_locator.mouse(&locator_assets,&fifth_scene,Some(rights_point),false);
        let control=fifth_locator.take_tutorial_control().unwrap();
        assert_eq!(control,26);
        fifth_host.tutorial_event(&mut fifth,&assets,control).unwrap();
        assert_eq!(fifth_host.status(&fifth).tutorial_message.unwrap().dismissal_id,4);
        assert!(fifth_host.tutorial_pick(&mut fifth,&assets,crate::tutorial_two_interaction::Pick::BlockEntry{
            category_id:pacific as u32}).unwrap());
        assert_eq!(fifth_host.status(&fifth).prompts.last().unwrap().dismissal_id,5);
        fifth.dispatch_session_event("InterfacePromptDismiss",5,&assets,false).unwrap();
        assert_eq!(fifth_host.status(&fifth).tutorial_message.unwrap().dismissal_id,6);
        let planning_assets=crate::build_view::Assets::load(&index,&archive,&installer,&strings,&executable).unwrap();
        let context=crate::build_panel::Context::from_session(&fifth,pacific).unwrap();
        // Original41ff49 and the Main producer emit11 after inspecting the
        // entry, including when the rights panel denies the planner.
        fifth_host.tutorial_event(&mut fifth,&assets,11).unwrap();
        if fifth_mode == Some(crate::compatibility::Mode::StrictV14) {
            assert_eq!(context.entry(), crate::build_panel::Entry::Message {
                text: 1202, show_rights_action: true,
            });
            assert_eq!(&context.category_record[0x78..0x7c], &[0; 4]);
            println!("Tutorial5 strict: original v1.4 rights gate1202 retained");
            fs::remove_file(&profile).unwrap();
            return;
        }
        assert!(matches!(context.entry(),crate::build_panel::Entry::Open),
            "Tutorial5 Pacific admission {:?}; category{pacific} owner{} current{} rights{} pending{}",
            context.entry(),context.category_record[5],context.current_owner,
            u32::from_le_bytes(context.category_record[0x78..0x7c].try_into().unwrap()),
            context.category_record[0x6d]);
        let mut planning=planning_assets.open(context).unwrap();
        let status=fifth_host.status(&fifth);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id,7);
        assert_eq!(status.tutorial_build_type,0);
        assert_eq!(status.tutorial_enabled_controls,vec![86],"original type constant admits the actual GUI widget");
        planning.set_tutorial_build_type(status.tutorial_build_type);
        let planner_click=|planning:&mut crate::build_view::State,id:u32|{
            let point=(0..320).flat_map(|y|(0..196).map(move|x|[x*2,y*2]))
                .find(|&point|{planning.pointer(Some(point));planning.tutorial_control_at()==Some(id)})
                .unwrap_or_else(||panic!("original Tutorial5 control{id} has no actual enabled producer at stage{}",planning.step));
            planning.pointer(Some(point));planning.mouse(true);planning.mouse(false);
            planning.take_tutorial_control().unwrap_or_else(||panic!("same-target planner control{id} emits no callback"))
        };
        let control=planner_click(&mut planning,86);
        fifth_host.tutorial_event(&mut fifth,&assets,control).unwrap();
        assert_eq!(planning.step,1);
        assert_eq!(fifth_host.status(&fifth).tutorial_message.unwrap().dismissal_id,8);
        assert_eq!(fifth_host.status(&fifth).tutorial_enabled_controls,vec![87]);
        let control=planner_click(&mut planning,87);
        fifth_host.tutorial_event(&mut fifth,&assets,control).unwrap();
        assert_eq!(planning.step,2);
        assert_eq!(fifth_host.status(&fifth).tutorial_message.unwrap().dismissal_id,9);
        let block=board.blocks.iter().find(|block|block.id==pacific).unwrap();
        let mut navigation=crate::navigation::block_entry([block.center[0],0.,block.center[1]]);
        let (eye,target)=navigation.tick([false;4]);
        planning.project_grid(openmt2001::flyby::CameraSample{
            eye,target,up:[0.,1.,0.],vertical_fov:45.,
        },block.center).unwrap();
        let centers=crate::build_panel::cell_centers(planning.grid.as_ref().unwrap());
        let mut placed=None;
        'footprints:for width in 1..=6usize{for depth in 1..=6usize{
            for x in 0..=7-width{for y in 0..=7-depth{
                let start=centers[x*7+y].map(|v|v as i32);
                let end=centers[(x+width-1)*7+y+depth-1].map(|v|v as i32);
                planning.pointer(Some(start));if !planning.tutorial_drag_at(){continue;}
                planning.mouse(true);planning.pointer(Some(end));planning.mouse(false);
                if planning.take_placement_request(){placed=Some((start,end));break 'footprints;}
            }}
        }}
        let (start,end)=placed.expect("original Tutorial5 selected use has a legal visible footprint");
        assert_eq!(planning.step,2,"native88 runs before visible controller stage3");
        let other_streams=(fifth.random_registry.process,fifth.script_random_state,
            fifth.credit_random_state,fifth.particle_random_state);
        let previous_build=fifth.build_random_state;
        let choices=fifth_host.complete_build_drag(&mut fifth,&assets).unwrap();
        assert_ne!(fifth.build_random_state,previous_build);
        assert_eq!((fifth.random_registry.process,fifth.script_random_state,
            fifth.credit_random_state,fifth.particle_random_state),other_streams);
        let status=fifth_host.status(&fifth);
        assert_eq!(status.prompts.last().unwrap().dismissal_id,10,
            "actual worker88 dismisses the original drag message before stage3 reply");
        assert_eq!(planning.step,2);
        planning.apply_placement_choices(choices);
        assert_eq!(planning.step,3);
        let request=planning.request().unwrap();
        let quote=fifth.inspect_build(&request,&assets).unwrap();
        assert!(quote.allowed(),"Tutorial5 actual footprint/design quote: locked={} placement={} affordable={} preview={}",
            quote.category_locked,quote.placement_allowed,quote.affordable,quote.preview.is_some());
        planning.quote_request=Some(request);planning.quote=Some(quote);planning.needs_quote=false;
        fifth.dispatch_session_event("InterfacePromptDismiss",10,&assets,false).unwrap();
        let status=fifth_host.status(&fifth);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id,11);
        assert!(status.tutorial_enabled_controls.contains(&89));
        let control=planner_click(&mut planning,89);
        assert_eq!(control,89);assert_eq!(planning.step,2);
        fifth_host.tutorial_event(&mut fifth,&assets,control).unwrap();
        assert_eq!(fifth_host.status(&fifth).tutorial_message.unwrap().dismissal_id,12);
        planning.pointer(Some(start));assert!(planning.tutorial_drag_at());planning.mouse(true);
        planning.pointer(Some(end));planning.mouse(false);
        assert!(planning.take_placement_request());
        let choices=fifth_host.complete_build_drag(&mut fifth,&assets).unwrap();
        assert_eq!(planning.step,2);
        assert_eq!(fifth_host.status(&fifth).prompts.last().unwrap().dismissal_id,13);
        planning.apply_placement_choices(choices);
        let refresh_quote=|planning:&mut crate::build_view::State,session:&mut Session<'_>|{
            let request=planning.request().unwrap();
            let quote=session.inspect_build(&request,&assets).unwrap();
            assert!(quote.allowed(),"Tutorial5 changed original form remains legally buildable");
            planning.quote_request=Some(request);planning.quote=Some(quote);planning.needs_quote=false;
        };
        refresh_quote(&mut planning,&mut fifth);
        for dismissal in [13,14]{
            fifth.dispatch_session_event("InterfacePromptDismiss",dismissal,&assets,false).unwrap();
        }
        let mut next_modal = fifth_host.status(&fifth);
        assert_eq!(next_modal.prompts.last().unwrap().dismissal_id,15);
        // These are original modal prompts, not required widget callbacks.
        // Acknowledging15/16/17 advances the authored floor/quality/design
        // explanations; do not invent callbacks93/95/92 beneath an active modal.
        let prompt_assets = crate::prompt::Assets::load(&index, &archive, &installer, &strings, &image).unwrap();
        for dismissal in [15, 16, 17] {
            let request = next_modal.prompts.last().cloned().unwrap();
            assert_eq!(request.dismissal_id, dismissal);
            let mut modal = crate::prompt::State::default();
            let _ = modal.submit(request);
            let bounds = prompt_assets.button_at(next_modal.tutorial_interaction.prompt_offset());
            modal.cursor([(bounds[0]+bounds[2])*0.5, (bounds[1]+bounds[3])*0.5]);
            assert_eq!(modal.left_button(true, bounds), None);
            assert_eq!(modal.left_button_with(false, bounds, |_, id| {
                fifth.dispatch_session_event("InterfacePromptDismiss", id as i32, &assets, false).unwrap();
            }), Some(dismissal));
            next_modal = fifth_host.status(&fifth);
        }
        assert_eq!(next_modal.tutorial_message.unwrap().dismissal_id,18);
        refresh_quote(&mut planning,&mut fifth);
        let request=planning.request().unwrap();
        let queued=fifth.live_commands.snapshot().unwrap().len();
        assert_eq!(planner_click(&mut planning,97),97);
        assert!(planning.submit);
        // Native424a28 dispatches97 before the construction producer424abe.
        fifth_host.tutorial_event(&mut fifth,&assets,97).unwrap();
        let accepted_status = fifth_host.status(&fifth);
        assert_eq!(accepted_status.prompts.last().unwrap().dismissal_id,19);
        let submission=fifth.submit_build(&request,&assets,|event|
            Err(format!("Tutorial5 legal queued build unexpectedly executed synchronously: {event:?}"))).unwrap();
        assert!(submission.submitted);
        assert_eq!(fifth.live_commands.snapshot().unwrap().len(),queued+1);
        // The live worker runs independently of the displayed prompt. Allow
        // the genuine25-tick command delay before acknowledging the final text.
        let buildings_before = fifth.world.buildings.iter().filter(|row| row[0x3f] != 0).count();
        fifth_host.set_running(true);
        for _ in 0..100 {
            fifth_host.frame(&mut fifth,&assets,40);
            assert!(fifth_host.status(&fifth).failure.is_none(),"Tutorial5 queued construction failed");
            if fifth.live_commands.snapshot().unwrap().len() < queued+1 { break; }
        }
        assert!(fifth.live_commands.snapshot().unwrap().len() < queued+1,
            "the actual running scheduler must consume the delayed construction");
        assert!(fifth.world.buildings.iter().filter(|row| row[0x3f] != 0).count() > buildings_before,
            "legal submission must create an actual retained building");
        let status=fifth_host.status(&fifth);
        let request = accepted_status.prompts.last().cloned().unwrap();
        assert_eq!(request.dismissal_id,19);
        let mut modal=crate::prompt::State::default();
        let _=modal.submit(request);
        let bounds=prompt_assets.button_at(status.tutorial_interaction.prompt_offset());
        modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
        assert_eq!(modal.left_button(true,bounds),None);
        assert_eq!(modal.left_button_with(false,bounds,|_,id| {
            fifth.dispatch_session_event("InterfacePromptDismiss",id as i32,&assets,false).unwrap();
        }),Some(19));
        fifth_host.frame(&mut fifth,&assets,20);
        assert!(fifth_host.status(&fifth).tutorial_completed);
        let recorded=fs::read(&profile).unwrap();
        fifth_host.frame(&mut fifth,&assets,20);
        assert_eq!(fs::read(&profile).unwrap(),recorded,"Tutorial5 completion is recorded once");
        let mut progression=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut progression,&profile).unwrap();
        assert!(progression.levels.iter().find(|level|level.id()==100004).unwrap().completed()[0]);
        assert!(progression.levels.iter().find(|level|level.id()==100005).unwrap().unlocked());
    }
    #[test]
    #[ignore = "requires original Tutorial6 scripts, models and management assets"]
    fn original_tutorial_six_management_flow() {
        let read=|key|fs::read(std::env::var(key).unwrap()).unwrap();
        let executable=read("OPENMT2001_EXE");
        let archive=read("OPENMT2001_ARCHIVE_BIN");
        let directory=read("OPENMT2001_ARCHIVE_DIR");
        let build=read("OPENMT2001_BUILD_FILE");
        let parameters=read("OPENMT2001_PARAMETERS");
        let timetable=read("OPENMT2001_TIMETABLE");
        let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board=openmt2001::board::Board::from_v14(&image).unwrap();
        let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
        let exe_path=std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts=exe_path.parent().unwrap().join("scripts");
        let language=exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names=character_names(&image,&language).unwrap();
        let parameter_path=std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let era_styles=load_era_styles(parameter_path.parent().unwrap()).unwrap();
        let installer=parameter_path.parent().unwrap().parent().unwrap();
        let offsets=fs::read(language.join("tycoon.off")).unwrap();
        let text=fs::read(language.join("tycoon.bin")).unwrap();
        let mut at=0;
        let strings=offsets.chunks_exact(2).map(|length|{
            let end=at+u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let value=&text[at..end];at=end;value
        }).collect::<Vec<_>>();
        assert_eq!(at,text.len());
        let business_assets=crate::business_view::Assets::load(&index,&archive,installer,&strings,&executable).unwrap();
        // Internal original-script admission is deliberately independent of
        // menu/profile eligibility while Tutorial5's startup is under review.
        let mut session=Session::initialize(Inputs{
            tree_locators: None,
            image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),era_styles:era_styles.clone(),
            timetable:std::str::from_utf8(&timetable).unwrap(),scenario:crate::scenarios::find(100005).unwrap().token,
            difficulty:0,selected_character:0,profile_name:b"Tutorial6",character_names:&names,
        },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]}).unwrap();
        let assets=super::super::CommandAssets{image:&image,board:&board,file:&file};
        let profile=std::path::PathBuf::from(format!("work/economy/tutorial6-lifecycle-{}-{}.lua",
            std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        crate::campaign_store::save_progress(&session.catalog,&profile).unwrap();
        session.campaign_profile=Some(profile.clone());
        let prompt_assets=crate::prompt::Assets::load(&index,&archive,installer,&strings,&image).unwrap();
        let mut host=crate::live_host::Host::new([0.,20.,0.]);
        host.begin_presentation(&session).unwrap();
        host.set_running(true);
        host.frame(&mut session,&assets,20);
        let status=host.status(&session);
        assert!(status.failure.is_none(),"Tutorial6 original startup: {:?}",status.failure);
        assert_eq!(status.prompts.last().unwrap().dismissal_id,2);
        assert_eq!(session.tutorial_state,1);
        assert_eq!(session.world.ledgers.residents.active_count,100);
        assert_eq!(session.roster.manager.players.len(),1);
        assert_eq!(session.world.ledgers.businesses[7][10],1);
        assert_eq!(session.world.ledgers.businesses[8][10],0);
        session.dispatch_session_event("InterfacePromptDismiss",2,&assets,false).unwrap();
        let gate=host.status(&session).tutorial_interaction;
        assert_eq!(gate.callback(crate::tutorial_two_interaction::Pick::Building{slot:7}),None);
        assert_eq!(gate.callback(crate::tutorial_two_interaction::Pick::Building{slot:8}),Some(2));
        let mut scene=session.scene(&build,openmt2001::render_scene::OriginalAssets{
            executable:&executable,directory:&directory,archive:&archive},
            &read("OPENMT2001_SUNCOLS"),None,true).unwrap();
        let pacific=session.scripts.runtime.number("PACIFIC_AVENUE").unwrap().unwrap() as usize;
        let block=board.blocks.iter().find(|block|block.id==pacific).unwrap();
        let mut camera=crate::navigation::block_entry([block.center[0],0.,block.center[1]]);
        let (eye,target)=camera.tick([false;4]);
        let camera=openmt2001::flyby::CameraSample{eye,target,up:[0.,1.,0.],vertical_fov:45.};
        scene.reframe(camera).unwrap();
        let selection=crate::selection::Assets::load(&executable,&index,&archive).unwrap();
        let picked=(0..960).step_by(4).flat_map(|y|(0..1280).step_by(4).map(move|x|[x,y]))
            .find_map(|point|selection.pick(camera,&scene,point,None)
                .and_then(|hit|hit.building).filter(|&building|
                    scene.inspection_buildings[building].runtime_slot==Some(8)))
            .expect("original City cafe8 is actually pickable in the Pacific scene");
        let selected=scene.inspection_buildings[picked].runtime_slot.unwrap();
        assert!(host.tutorial_pick(&mut session,&assets,
            crate::tutorial_two_interaction::Pick::Building{slot:selected}).unwrap());
        assert_eq!(host.status(&session).prompts.last().unwrap().dismissal_id,4);
        let inspection=session.inspect_business_operations(selected,business_assets.bounds(),&image).unwrap();
        assert!(!inspection.owned);
        assert!(!inspection.report.expenses.is_empty(),"actual City business admits detail arrows");
        assert!(!inspection.permissions.convert && !inspection.permissions.demolish);
        session.dispatch_session_event("InterfacePromptDismiss",4,&assets,false).unwrap();
        let status=host.status(&session);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id,5);
        assert_eq!(status.tutorial_enabled_controls,vec![37]);
        let mut business=crate::business_view::View::default();
        business.select(Some(picked));
        business.inspection=Some(inspection);
        let business_control=|view:&mut crate::business_view::View,id:u32| {
            let point=(0..480).flat_map(|y|(0..640).map(move|x|[x as f32,y as f32]))
                .find(|&point|view.tutorial_control_at(&business_assets,Some(point))==Some(id))
                .unwrap_or_else(||panic!("actual business control{id} has no admitted hit"));
            view.mouse(&business_assets,Some(point),true);
            view.mouse(&business_assets,Some(point),false);
            view.take_tutorial_control().unwrap_or_else(||panic!("actual business control{id} emitted no callback"))
        };
        for (detail,next) in [(1,6),(2,7),(0,8)] {
            let control=business_control(&mut business,37);
            assert_eq!(control,37);
            host.tutorial_event(&mut session,&assets,control).unwrap();
            assert_eq!(business.detail,detail);
            assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,next);
        }
        assert_eq!(host.status(&session).tutorial_enabled_controls,vec![16]);
        let control=business_control(&mut business,16);
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(host.status(&session).prompts.last().unwrap().dismissal_id,9);
        session.dispatch_session_event("InterfacePromptDismiss",9,&assets,false).unwrap();
        let control=business_control(&mut business,10);
        assert_eq!(business.navigation.take(),Some(1));
        host.tutorial_event(&mut session,&assets,control).unwrap();
        let adjustment_assets=crate::business_adjustment::Assets::load(
            &index,&archive,installer,&strings,&executable).unwrap();
        let owner=host.status(&session).business_owner;
        let mut adjustment=adjustment_assets.open_for_owner(&scene,picked,owner).unwrap().unwrap();
        assert!(adjustment.edit_target.is_none(),"City adjustment is inspection only");
        let adjustment_control=|state:&mut crate::business_adjustment::State,id:u32| {
            let point=(0..960).flat_map(|y|(0..1280).map(move|x|[x,y]))
                .find(|&point|state.tutorial_control_at(&adjustment_assets,Some(point))==Some(id))
                .unwrap_or_else(||panic!("actual adjustment control{id} has no admitted hit"));
            state.pointer(&adjustment_assets,Some(point));
            state.mouse(true);
            state.mouse(false);
            state.take_tutorial_control()
        };
        let control=adjustment_control(&mut adjustment,53).unwrap();
        assert!(adjustment.edit_request.is_none(),"City OK closes without Apply");
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,12);
        let owned=(0..960).step_by(4).flat_map(|y|(0..1280).step_by(4).map(move|x|[x,y]))
            .find_map(|point|selection.pick(camera,&scene,point,None)
                .and_then(|hit|hit.building).filter(|&building|
                    scene.inspection_buildings[building].runtime_slot==Some(7)))
            .expect("original owned business7 is actually pickable");
        let owned_slot=scene.inspection_buildings[owned].runtime_slot.unwrap();
        assert!(host.tutorial_pick(&mut session,&assets,
            crate::tutorial_two_interaction::Pick::Building{slot:owned_slot}).unwrap());
        session.dispatch_session_event("InterfacePromptDismiss",13,&assets,false).unwrap();
        business.select(Some(owned));
        business.inspection=Some(session.inspect_business_operations(owned_slot,business_assets.bounds(),&image).unwrap());
        let control=business_control(&mut business,10);
        host.tutorial_event(&mut session,&assets,control).unwrap();
        // Callback7 comes from an admitted numeric producer, not a registered
        // enabled widget. Queued disabled widget input is ignored without mutation.
        let commands_before = session.hooks.commands.len();
        let businesses_before = session.world.ledgers.businesses.clone();
        let prompt_before = host.status(&session).tutorial_message.unwrap().dismissal_id;
        host.tutorial_event(&mut session,&assets,7).unwrap();
        assert_eq!(session.hooks.commands.len(), commands_before);
        assert_eq!(session.world.ledgers.businesses, businesses_before);
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id, prompt_before);
        let mut adjustment=adjustment_assets.open_for_owner(&scene,owned,owner).unwrap().unwrap();
        let target=adjustment.edit_target.expect("owned retail adjustment admits actual edits");
        assert!(adjustment_control(&mut adjustment,41).is_none(),"numeric callback waits for authoritative edit");
        let action=adjustment.edit_request.take().unwrap();
        let old=session.world.ledgers.businesses[target.business];
        adjustment.edit_inflight=Some(action);
        adjustment.receive_edit(crate::business_edit::edit(&mut session,&assets,target,action));
        let (control,event)=adjustment.take_tutorial_producer().unwrap();
        assert_eq!((control,event),(41,7));
        assert_ne!(session.world.ledgers.businesses[target.business][0x1ec..0x1f0],old[0x1ec..0x1f0]);
        host.tutorial_producer_event(&mut session,&assets,control,event).unwrap();
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,16);
        let control=adjustment_control(&mut adjustment,53).unwrap();
        let action=adjustment.edit_request.take().unwrap();
        assert_eq!(action,crate::business_edit::Action::Apply);
        let result=crate::business_edit::edit(&mut session,&assets,target,action);
        adjustment.edit_inflight=Some(action);
        adjustment.receive_edit(result);
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(host.status(&session).tutorial_enabled_controls,vec![11]);
        let control=business_control(&mut business,11);
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(business.mode,2);
        assert!(business.action.is_none(),"Sell tab displays quote without selling the authored business");
        assert_eq!(host.status(&session).prompts.last().unwrap().dismissal_id,18);
        session.dispatch_session_event("InterfacePromptDismiss",18,&assets,false).unwrap();
        let control=business_control(&mut business,12);
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(business.mode,3);
        business.inspection=Some(session.inspect_business_operations(owned_slot,business_assets.bounds(),&image).unwrap());
        assert!(!business.inspection.as_ref().unwrap().conversions.is_empty(),"native fit query admits a real refurbishment");
        let point=(0..480).flat_map(|y|(0..640).map(move|x|[x as f32,y as f32]))
            .find(|&point|business.tutorial_control_at(&business_assets,Some(point))==Some(40))
            .expect("actual eligible refurbishment list row is hit-testable");
        business.mouse(&business_assets,Some(point),true);
        business.mouse(&business_assets,Some(point),false);
        let (control,event)=business.take_tutorial_producer().unwrap();
        assert_eq!((control,event),(40,7));
        let selected=business.selected.unwrap();
        let quoted=business.inspection.as_ref().unwrap().conversions[selected].clone();
        assert!(quoted.price.is_finite() && quoted.price>=0.);
        host.tutorial_producer_event(&mut session,&assets,control,event).unwrap();
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,21);
        let control=business_control(&mut business,38);
        let action=business.action.take().unwrap();
        assert_eq!(action,openmt2001::business_controls::Action::Convert{selected_label:quoted.label});
        assert!(host.business_operation(&mut session,&assets,owned_slot,action).unwrap());
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(host.status(&session).tutorial_enabled_controls,vec![13]);
        assert_eq!(session.world.buildings[owned_slot][0x3d],1,"actual refurbishment producer marks pending conversion");
        for _ in 0..100 {
            if session.world.buildings[owned_slot][0x3d]==0 {break;}
            host.frame(&mut session,&assets,40);
            assert!(host.status(&session).failure.is_none(),"natural refurbishment consumer: {:?}",host.status(&session).failure);
        }
        assert_eq!(session.world.buildings[owned_slot][0x3d],0,"native queued refurbishment completes under the original running timer");
        let converted=u32::from_le_bytes(session.world.buildings[owned_slot][0x1c..0x20].try_into().unwrap()) as usize;
        assert_eq!(u32::from_le_bytes(session.world.ledgers.businesses[converted][4..8].try_into().unwrap()),quoted.kind);
        let refreshed=session.scene(&build,openmt2001::render_scene::OriginalAssets{
            executable:&executable,directory:&directory,archive:&archive},
            &read("OPENMT2001_SUNCOLS"),None,true).unwrap();
        let converted_scene=refreshed.inspection_buildings.iter()
            .find(|building|building.runtime_slot==Some(owned_slot))
            .expect("completed conversion remains in the actual live scene");
        assert_eq!(converted_scene.business().unwrap().business_type,quoted.kind,
            "refreshed scene exposes the actual converted business");
        business.inspection=Some(session.inspect_business_operations(owned_slot,business_assets.bounds(),&image).unwrap());
        let control=business_control(&mut business,13);
        host.tutorial_event(&mut session,&assets,control).unwrap();
        assert_eq!(business.mode,4);
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,23);
        let control=business_control(&mut business,39);
        let action=business.action.take().unwrap();
        assert_eq!(action,openmt2001::business_controls::Action::Demolish);
        assert!(host.business_operation(&mut session,&assets,owned_slot,action).unwrap());
        host.tutorial_event(&mut session,&assets,control).unwrap();
        let demolition_status=host.status(&session);
        let final_prompt=demolition_status.prompts.last().unwrap().clone();
        assert_eq!(final_prompt.dismissal_id,24);
        // Native demolition has two floor-count*25 waits around the model
        // descent (0.3 per pending tick). Allow the actual model to finish.
        for _ in 0..1000 {
            if session.world.buildings[owned_slot][0x3f]==0 {break;}
            host.frame(&mut session,&assets,40);
            assert!(host.status(&session).failure.is_none(),"natural demolition consumer: {:?}",host.status(&session).failure);
        }
        assert_eq!(session.world.buildings[owned_slot][0x3f],0,"actual queued demolition removes the building");
        let refreshed=session.scene(&build,openmt2001::render_scene::OriginalAssets{
            executable:&executable,directory:&directory,archive:&archive},
            &read("OPENMT2001_SUNCOLS"),None,true).unwrap();
        assert!(refreshed.inspection_buildings.iter()
            .all(|building|building.runtime_slot!=Some(owned_slot)),
            "completed demolition also removes the building from the actual live scene");
        let mut modal=crate::prompt::State::default();
        let _=modal.submit(final_prompt);
        let bounds=prompt_assets.button_at(host.status(&session).tutorial_interaction.prompt_offset());
        modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
        assert_eq!(modal.left_button(true,bounds),None);
        assert_eq!(modal.left_button_with(false,bounds,|_,dismissal|{
            session.dispatch_session_event("InterfacePromptDismiss",i32::try_from(dismissal).unwrap(),&assets,false).unwrap();
        }),Some(24));
        host.frame(&mut session,&assets,20);
        let completed=host.status(&session);
        assert!(completed.failure.is_none(),"Tutorial6 original End: {:?}",completed.failure);
        assert!(completed.tutorial_completed);
        let recorded=fs::read(&profile).unwrap();
        host.frame(&mut session,&assets,40);
        assert_eq!(fs::read(&profile).unwrap(),recorded,"actual Tutorial6 End records the isolated profile once");
        let mut progression=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut progression,&profile).unwrap();
        assert_eq!(progression.levels.iter().find(|level|level.id()==100005).unwrap().completed(),[true,false,false]);
        assert_eq!(progression.levels.iter().find(|level|level.id()==100005).unwrap().attempts(),0);
        assert_eq!(progression.levels.iter().find(|level|level.id()==100004).unwrap().completed(),[false;3],
            "internal6 admission never fabricates unresolved Tutorial5 completion");
        let mut next=Session::initialize_campaign(Inputs{
            tree_locators: None,
            image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),era_styles:era_styles.clone(),
            timetable:std::str::from_utf8(&timetable).unwrap(),scenario:crate::scenarios::find(100006).unwrap().token,
            difficulty:0,selected_character:0,profile_name:b"Tutorial6",character_names:&names,
        },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]},progression,profile.clone()).unwrap();
        let mut next_host=crate::live_host::Host::new([0.,20.,0.]);
        next_host.begin_presentation(&next).unwrap();next_host.set_running(true);
        next_host.frame(&mut next,&assets,20);
        let status=next_host.status(&next);
        assert!(status.failure.is_none(),"original6A startup: {:?}",status.failure);
        assert_eq!(next.tutorial_state,1);
        assert!(status.tutorial_controls.all_disabled && status.tutorial_controls.picking_disabled);
        assert!(!status.tutorial_controls.camera_disabled);
        let mut modal=crate::prompt::State::default();
        for request in status.prompts{let _=modal.submit(request);}
        for dismissal in [2,3]{
            assert_eq!(modal.request().unwrap().dismissal_id,dismissal);
            let bounds=prompt_assets.button_at(next_host.status(&next).tutorial_interaction.prompt_offset());
            modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
            assert_eq!(modal.left_button(true,bounds),None);
            assert_eq!(modal.left_button_with(false,bounds,|presenter,id|{
                next.dispatch_session_event("InterfacePromptDismiss",i32::try_from(id).unwrap(),&assets,false).unwrap();
                for request in next_host.status(&next).prompts{let _=presenter.submit(request);}
            }),Some(dismissal));
        }
        next_host.frame(&mut next,&assets,20);
        let status=next_host.status(&next);
        assert!(status.failure.is_none(),"original6A End: {:?}",status.failure);
        assert!(status.tutorial_completed);
        let recorded=fs::read(&profile).unwrap();
        next_host.frame(&mut next,&assets,40);
        assert_eq!(fs::read(&profile).unwrap(),recorded,"original6A End also records once");
        let mut reopened=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut reopened,&profile).unwrap();
        for id in [100005,100006]{assert_eq!(reopened.levels.iter().find(|level|level.id()==id).unwrap().completed(),[true,false,false]);}
        assert_eq!(reopened.levels.iter().find(|level|level.id()==100004).unwrap().completed(),[false;3]);
        let mut seventh=Session::initialize_campaign(Inputs{
            tree_locators: None,
            image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),era_styles,
            timetable:std::str::from_utf8(&timetable).unwrap(),scenario:crate::scenarios::find(100007).unwrap().token,
            difficulty:0,selected_character:0,profile_name:b"Tutorial6",character_names:&names,
        },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]},reopened,profile.clone()).unwrap();
        let mut seventh_host=crate::live_host::Host::new([0.,20.,0.]);
        seventh_host.begin_presentation(&seventh).unwrap();seventh_host.set_running(true);
        seventh_host.frame(&mut seventh,&assets,20);
        let status=seventh_host.status(&seventh);
        assert!(status.failure.is_none(),"original7 startup: {:?}",status.failure);
        assert!(!status.event_list_visible);
        let mut modal=crate::prompt::State::default();
        for request in status.prompts{let _=modal.submit(request);}
        for dismissal in 2..=7{
            assert_eq!(modal.request().unwrap().dismissal_id,dismissal);
            let bounds=prompt_assets.button_at(seventh_host.status(&seventh).tutorial_interaction.prompt_offset());
            modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
            assert_eq!(modal.left_button(true,bounds),None);
            assert_eq!(modal.left_button_with(false,bounds,|presenter,id|{
                seventh.dispatch_session_event("InterfacePromptDismiss",i32::try_from(id).unwrap(),&assets,false).unwrap();
                for request in seventh_host.status(&seventh).prompts{let _=presenter.submit(request);}
            }),Some(dismissal));
            if dismissal==2{
                let status=seventh_host.status(&seventh);
                assert!(status.event_list_visible && !status.event_list_settled);
                seventh_host.frame(&mut seventh,&assets,20);
                assert!(seventh_host.status(&seventh).event_list_settled,
                    "original Show opening clears at the first actual interface callback");
            }
        }
        assert_eq!(seventh_host.status(&seventh).tutorial_message.unwrap().dismissal_id,8);
        let comparison_assets=crate::comparison::Assets::load(&index,&archive,installer,&strings,&executable).unwrap();
        let players=crate::player_info::State::from_runtime(&seventh.roster.manager,
            &seventh.world.ledgers.players,seventh.roster.status.current_player).unwrap();
        let data=crate::comparison::Data::from_runtime(&seventh.world,&players.records,&image,0x591318).unwrap();
        let mut comparison=crate::comparison::View::new(data);
        assert_eq!(comparison.state.mode,0);
        let right=(0..960).flat_map(|y|(0..1280).map(move|x|[x,y]))
            .find(|&point|comparison_assets.button_at(&comparison.state,point)==Some(2)).unwrap();
        comparison.pointer(&comparison_assets,Some(right));
        assert_eq!(comparison.mouse(true),None);
        comparison.pointer(&comparison_assets,None);
        assert_eq!(comparison.mouse(false),None);
        assert_eq!(comparison.state.mode,0,"canceled press produces neither a mode nor tutorial event");
        for mode in [1,3,4,5]{
            assert!(seventh_host.status(&seventh).tutorial_enabled_controls.contains(&78));
            comparison.pointer(&comparison_assets,Some(right));
            assert_eq!(comparison.mouse(true),None);
            assert_eq!(comparison.mouse(false),Some(2));
            seventh_host.tutorial_event(&mut seventh,&assets,78).unwrap();
            assert_eq!(comparison.state.mode,mode);
        }
        assert_eq!(comparison.state.label(),1535);
        let status=seventh_host.status(&seventh);
        assert!(!status.tutorial_enabled_controls.contains(&78));
        for request in status.prompts{let _=modal.submit(request);}
        for dismissal in 9..=16{
            assert_eq!(modal.request().unwrap().dismissal_id,dismissal);
            let bounds=prompt_assets.button_at(seventh_host.status(&seventh).tutorial_interaction.prompt_offset());
            modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
            assert_eq!(modal.left_button(true,bounds),None);
            assert_eq!(modal.left_button_with(false,bounds,|presenter,id|{
                seventh.dispatch_session_event("InterfacePromptDismiss",i32::try_from(id).unwrap(),&assets,false).unwrap();
                for request in seventh_host.status(&seventh).prompts{let _=presenter.submit(request);}
            }),Some(dismissal));
        }
        seventh_host.frame(&mut seventh,&assets,20);
        let status=seventh_host.status(&seventh);
        assert!(status.failure.is_none(),"original7 End: {:?}",status.failure);
        assert!(status.tutorial_completed);
        let recorded=fs::read(&profile).unwrap();
        seventh_host.frame(&mut seventh,&assets,40);
        assert_eq!(fs::read(&profile).unwrap(),recorded,"original7 profile records once");
        let mut final_profile=crate::campaign_store::load_catalog(&image,&scripts).unwrap();
        crate::campaign_store::load_progress(&mut final_profile,&profile).unwrap();
        for id in [100005,100006,100007]{assert_eq!(final_profile.levels.iter().find(|level|level.id()==id).unwrap().completed(),[true,false,false]);}
        assert_eq!(final_profile.levels.iter().find(|level|level.id()==100004).unwrap().completed(),[false;3]);
        // Internal original6 admission is deliberate while5 remains under
        // startup review. This does not prove5-to6 profile/menu progression.
    }
    #[test]
    #[ignore = "requires original v1.4 Tutorial 12 script, map, assets and GPU"]
    fn original_tutorial_twelve_lease_auction_flow() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let scripts = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent().unwrap().join("scripts");
        let parameters = read("OPENMT2001_PARAMETERS");
        let parameter_path = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let era_styles = load_era_styles(parameter_path.parent().unwrap()).unwrap();
        let timetable = read("OPENMT2001_TIMETABLE");
        let executable_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = executable_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names = character_names(&image, &language).unwrap();
        let installer = parameter_path.parent().unwrap().parent().unwrap();
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let text = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets.chunks_exact(2).map(|length| {
            let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let value = &text[at..end]; at = end; value
        }).collect::<Vec<_>>();
        assert_eq!(at, text.len());
        let locator_assets = crate::locator::Assets::load(&index, &archive, installer, &strings, &executable).unwrap();
        let auction_assets = crate::auction_panel::Assets::load(&index, &archive, installer, &strings, &executable).unwrap();
        let mut session = Session::initialize(Inputs {
            tree_locators: None,
            image: &image, board: &board, file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(), era_styles,
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "TUTORIAL12",
            difficulty: 0, selected_character: 0, profile_name: b"Monopoly tutorial",
            character_names: &names,
        }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
        let profile = std::path::PathBuf::from(format!("work/economy/tutorial12-{}-{}.profile",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        crate::campaign_store::save_progress(&session.catalog, &profile).unwrap();
        session.campaign_profile = Some(profile.clone());
        let assets = super::super::CommandAssets { image: &image, board: &board, file: &file };
        assert_eq!(session.tutorial_state, 1);
        assert_eq!(session.roster.manager.players.len(), 2);
        assert_eq!((session.world.objective.kind, session.world.objective.target), (0, 0.));
        assert_eq!(session.scripts.runtime.number("TUT_BUILD_RIGHTS_BUTTON").unwrap(), Some(91.));
        let mut host = crate::live_host::Host::new([0., 20., 0.]);
        let (renderer_sender, renderer_receiver) = std::sync::mpsc::channel::<crate::auction_bridge::Request>();
        let presenter = std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(move || {
            let exe = read("OPENMT2001_EXE");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view = crate::auction_bridge::View::default();
            let mut counts = [0; 3];
            for request in renderer_receiver {
                let kind = match request.operation {
                    crate::auction_bridge::Operation::Prepare { .. } => 0,
                    crate::auction_bridge::Operation::Activate => 1,
                    crate::auction_bridge::Operation::Restore => 2,
                };
                let result = view.present(request.operation, &renderer, (&exe, &directory, &archive), 3);
                assert!(result.is_ok(), "Tutorial12 auction renderer: {result:?}");
                counts[kind] += 1;
                request.reply.send(result).unwrap();
            }
            counts
        }).unwrap();
        host.auction_renderer = Some(renderer_sender);
        host.begin_presentation(&session).unwrap();
        host.set_running(true);
        host.frame(&mut session, &assets, 20);
        // BlockCam and SetPlanView are original scheduled events, not direct
        // effects of the Level callback that creates the first prompt.
        host.frame(&mut session, &assets, 80);
        let status = host.status(&session);
        assert!(status.failure.is_none(), "Tutorial12 startup: {:?}", status.failure);
        assert_eq!(status.prompts[0].dismissal_id, 2);
        assert_eq!(status.tutorial_camera_events.len(), 2, "original BlockCam and SetPlanView");
        session.dispatch_session_event("InterfacePromptDismiss", 2, &assets, false).unwrap();
        let target = session.scripts.runtime.number("ILLINOIS_AVENUE").unwrap().unwrap() as u32;
        assert_eq!(host.status(&session).tutorial_interaction.callback(
            crate::tutorial_two_interaction::Pick::BlockEntry { category_id: target }), Some(1));
        assert!(host.tutorial_pick(&mut session, &assets,
            crate::tutorial_two_interaction::Pick::BlockEntry { category_id: target }).unwrap());
        let status = host.status(&session);
        assert!(status.failure.is_none(), "Tutorial12 block entry: {:?}", status.failure);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id, 4);
        let green = session.scripts.runtime.number("TUT_MPP_GREENBUTTON_2").unwrap().unwrap() as u32;
        assert!(status.tutorial_enabled_controls.contains(&green));
        let players = crate::player_info::State::from_runtime(&session.roster.manager,
            &session.world.ledgers.players, session.roster.status.current_player).unwrap();
        let report = crate::block_information::Report::from_runtime(&session.world, &players, &image).unwrap().unwrap();
        let mut property = crate::utility_report::View::default();
        property.select(Some(target as usize));
        let click_property = |property: &mut crate::utility_report::View, control: u32| {
            let point = (0..400).step_by(2).flat_map(|x| (50..650).step_by(2).map(move |y| [x,y]))
                .find(|&point| locator_assets.property_tutorial_control_at(property,
                    Some(point), &report, target as usize) == Some(control)).unwrap();
            locator_assets.property_pointer(property, Some(point), &report, target as usize);
            assert!(property.mouse(true));
            locator_assets.property_pointer(property, Some(point), &report, target as usize);
            assert!(property.mouse(false));
            property.take_tutorial_control().unwrap()
        };
        assert_eq!(click_property(&mut property, green), green);
        host.tutorial_event(&mut session, &assets, green).unwrap();
        let request = property.take_property_request().unwrap();
        assert!(!request.submit);
        let inspected = host.inspect_property_offer(&mut session, &assets,
            request.category, request.panel, true).unwrap();
        assert_eq!(inspected.panel.action_visible, 1);
        property.apply_property_inspection(request.category, Ok(inspected));
        let status = host.status(&session);
        assert!(status.failure.is_none(), "Tutorial12 lease-panel callback: {:?}", status.failure);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id, 5);
        let buy = session.scripts.runtime.number("TUT_MPP_BLOCK_BUYBLOCK").unwrap().unwrap() as u32;
        assert!(status.tutorial_enabled_controls.contains(&buy));
        assert_eq!(click_property(&mut property, buy), buy);
        host.tutorial_event(&mut session, &assets, buy).unwrap();
        let request = property.take_property_request().unwrap();
        assert!(request.submit);
        let submitted = host.property_offer(&mut session, &assets, request.category, request.panel).unwrap();
        assert!(submitted.submitted);
        property.apply_property_inspection(request.category, Ok(submitted));
        let status = host.status(&session);
        assert!(status.failure.is_none(), "Tutorial12 buy-block callback: {:?}", status.failure);
        assert!(status.tutorial_message.is_none(), "original prompt4 unpauses pending auction");
        for _ in 0..1500 {
            host.frame(&mut session, &assets, 40);
            let status = host.status(&session);
            assert!(status.failure.is_none(), "Tutorial12 auction admission: {:?}", status.failure);
            if status.auction.as_ref().is_some_and(|auction| auction.visible) { break; }
        }
        assert!(host.status(&session).auction.as_ref().is_some_and(|auction| auction.visible),
            "original lease must enter the native auction room");
        assert_eq!(session.hooks.forced_auction_handlers.len(), 3);
        let mut auction_view = crate::auction_panel::View::default();
        let mut human_bids = 0;
        for _ in 0..4000 {
            host.frame(&mut session, &assets, 40);
            let status = host.status(&session);
            assert!(status.failure.is_none(), "Tutorial12 auction closure: {:?}", status.failure);
            if status.tutorial_message.is_some_and(|message| message.dismissal_id == 6) { break; }
            if let Some(auction) = status.auction.as_ref().filter(|auction|
                auction.visible && auction.controls.buttons_enabled & 1 != 0) {
                let bounds = auction_assets.buttons[0];
                let point = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
                assert_eq!(auction_view.mouse(&auction_assets, auction, Some(point), true), None);
                assert_eq!(auction_view.mouse(&auction_assets, auction, Some(point), false), Some(0));
                host.auction_button(&mut session, &assets, 0).unwrap();
                human_bids += 1;
            }
        }
        assert_eq!(host.status(&session).tutorial_message.map(|message| message.dismissal_id), Some(6),
            "native auction must settle and publish original Tutorial12 value-info prompt; human bids={human_bids}");
        assert!(host.status(&session).auction.is_none(), "original auction room must close before prompt5");
        // Native464bf0 queues command16 after restoring the room and emitting
        // TutorialEvent64. Its category ownership transfer is delayed one
        // local command turn; prompt5 can therefore appear while owner is0.
        for _ in 0..100 {
            if session.world.categories[target as usize][5] == 1 { break; }
            host.frame(&mut session, &assets, 40);
            assert!(host.status(&session).failure.is_none(), "Tutorial12 delayed transfer failed");
        }
        assert_eq!(session.world.categories[target as usize][5], 1,
            "original command16 must transfer Illinois to the human after auction closure; human bids={human_bids}");
        assert_eq!(u32::from_le_bytes(session.world.categories[target as usize][0x78..0x7c].try_into().unwrap()), 0,
            "ownership transfer must clear Illinois building rights before prompt8 activation");
        let players = crate::player_info::State::from_runtime(&session.roster.manager,
            &session.world.ledgers.players, session.roster.status.current_player).unwrap();
        let report = crate::block_information::Report::from_runtime(&session.world, &players, &image).unwrap().unwrap();
        let mut property = crate::utility_report::View::default();
        property.select(Some(target as usize));
        for control in [19, 11] {
            let point = (0..400).step_by(2).flat_map(|x| (50..650).step_by(2).map(move |y| [x,y]))
                .find(|&point| locator_assets.property_tutorial_control_at(&property,
                    Some(point), &report, target as usize) == Some(control)).unwrap();
            locator_assets.property_pointer(&mut property, Some(point), &report, target as usize);
            assert!(property.mouse(true));
            locator_assets.property_pointer(&mut property, Some(point), &report, target as usize);
            assert!(property.mouse(false));
            assert_eq!(property.take_tutorial_control(), Some(control));
            assert!(host.status(&session).tutorial_enabled_controls.contains(&control));
            host.tutorial_event(&mut session, &assets, control).unwrap();
            if control == 19 {
                let status = host.status(&session);
                assert_eq!(status.prompts[0].dismissal_id, 7, "original value-info callback opens modal prompt6");
                session.dispatch_session_event("InterfacePromptDismiss", 7, &assets, false).unwrap();
                assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id, 8);
            }
        }
        let status = host.status(&session);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id, 9,
            "original build control opens Tutorial12 rights prompt8");
        assert!(status.tutorial_enabled_controls.contains(&91));
        assert_eq!(crate::build_panel::Context::from_session(&session, target as usize).unwrap().entry(),
            crate::build_panel::Entry::Message { text: 1202, show_rights_action: true });
        // The original 4055f0 child at [[main+390]+4] is V2's visible
        // Activate Building Rights hit102; 41ffef applies 4050d0 before
        // 41fff4 publishes TutorialEvent91. Follow that actual UI producer.
        property.open_property_rights(target as usize, 0);
        let request = property.take_rights_request().unwrap();
        assert!(!request.activate);
        property.apply_rights_inspection(target,
            host.property_rights(&mut session, &assets, target, request.panel_mode, false));
        let point = (0..400).step_by(2).flat_map(|x| (50..650).step_by(2).map(move |y| [x,y]))
            .find(|&point| locator_assets.property_tutorial_control_at(&property,
                Some(point), &report, target as usize) == Some(91)).unwrap();
        locator_assets.property_pointer(&mut property, Some(point), &report, target as usize);
        assert_eq!(property.property_control_hover, Some(102));
        assert!(property.mouse(true));
        locator_assets.property_pointer(&mut property, Some(point), &report, target as usize);
        assert!(property.mouse(false));
        assert!(property.take_tutorial_control().is_none(),
            "rights tutorial event belongs after worker action, not the UI mouse release");
        let request = property.take_rights_request().unwrap();
        assert!(request.activate);
        let activated = host.property_rights(&mut session, &assets,
            request.category, request.panel_mode, true).unwrap();
        assert_eq!(activated.message, Some(1406), "native rights command remains pending");
        assert_eq!(session.world.categories[target as usize][0x6d], 1);
        let status = host.status(&session);
        assert!(status.failure.is_none(), "Tutorial12 rights producer: {:?}", status.failure);
        assert_eq!(status.tutorial_message.unwrap().dismissal_id, 10,
            "event91 advances original Tutorial12 prompt9 after rights action");
        assert_eq!((status.tutorial_build_type, status.tutorial_build_use), (1, 1),
            "original prompt9 restricts the builder to residential type and Use index1");
        // 4050d0's command19 remains queued after the tutorial callback. The
        // native command consumer451546 clears pending+6d and sets rights+78
        // to1; only then may 404eb0 admit the ordinary planner.
        for _ in 0..500 {
            let row = &session.world.categories[target as usize];
            if row[0x6d] == 0 && u32::from_le_bytes(row[0x78..0x7c].try_into().unwrap()) == 1 {
                break;
            }
            host.frame(&mut session, &assets, 40);
            assert!(host.status(&session).failure.is_none(), "Tutorial12 rights-delay command failed");
        }
        let row = &session.world.categories[target as usize];
        assert_eq!((row[0x6d], u32::from_le_bytes(row[0x78..0x7c].try_into().unwrap())), (0, 1),
            "original command19 must admit hotel construction after the rights wait");
        let context = crate::build_panel::Context::from_session(&session, target as usize).unwrap();
        assert_eq!(context.entry(), crate::build_panel::Entry::Open);
        let planning_assets = crate::build_view::Assets::load(&index, &archive, installer,
            &strings, &executable).unwrap();
        let mut planning = planning_assets.open(context).unwrap();
        planning.set_tutorial_build_type(host.status(&session).tutorial_build_type);
        planning.set_tutorial_build_use(host.status(&session).tutorial_build_use);
        // Original planner type1 (residential) is the only admitted type.
        planning.pointer(Some([200, 156]));
        assert_eq!(planning.tutorial_control_at(), Some(86));
        assert!(!planning.mouse(true));
        assert!(planning.mouse(false));
        assert_eq!(planning.take_tutorial_control(), Some(86));
        host.tutorial_event(&mut session, &assets, 86).unwrap();
        assert_eq!(planning.step, 1, "hotel tutorial requires a Use-list choice");
        // 42504d compares the selected list index with SetTutorialUse(1).
        // Use row1 is visible without scrolling in the original list.
        planning.pointer(Some([200, 175]));
        assert_eq!(planning.tutorial_control_at(), Some(87));
        assert!(!planning.mouse(true));
        assert!(planning.mouse(false));
        assert_eq!(planning.take_tutorial_control(), Some(87));
        assert_eq!(planning.step, 2, "native Use index1 admits the hotel footprint stage");
        host.tutorial_event(&mut session, &assets, 87).unwrap();
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id, 11,
            "original prompt10 follows the permitted residential Use selection");
        let block = board.blocks.iter().find(|block| block.id == target as usize).unwrap();
        let mut navigation = crate::navigation::block_entry([block.center[0], 0., block.center[1]]);
        let (eye, target_eye) = navigation.tick([false; 4]);
        planning.project_grid(openmt2001::flyby::CameraSample {
            eye, target: target_eye, up: [0., 1., 0.], vertical_fov: 45.,
        }, block.center).unwrap();
        let centers = crate::build_panel::cell_centers(planning.grid.as_ref().unwrap());
        let mut chosen = false;
        'footprints: for width in 1..=6usize { for depth in 1..=6usize {
            for x in 0..=7-width { for y in 0..=7-depth {
                let start = centers[x*7+y].map(|v| v as i32);
                let end = centers[(x+width-1)*7+y+depth-1].map(|v| v as i32);
                planning.pointer(Some(start));
                if !planning.tutorial_drag_at() { continue; }
                planning.mouse(true);
                planning.pointer(Some(end));
                planning.mouse(false);
                if planning.take_placement_request() { chosen = true; break 'footprints; }
            }}
        }}
        assert!(chosen, "original Hotel Use43 has a visible legal Illinois footprint");
        let choices = host.complete_build_drag(&mut session, &assets).unwrap();
        planning.apply_placement_choices(choices);
        assert_eq!(planning.step, 3);
        let request = planning.request().unwrap();
        assert_eq!((request.category, request.kind, request.business), (target as i32, 1, 43),
            "original residential Use index1 is Hotel43");
        let quote = session.inspect_build(&request, &assets).unwrap();
        assert!(quote.allowed(), "Tutorial12 Hotel quote: locked={} placement={} affordable={} preview={}",
            quote.category_locked, quote.placement_allowed, quote.affordable, quote.preview.is_some());
        planning.quote_request = Some(request.clone());
        planning.quote = Some(quote);
        planning.needs_quote = false;
        let accept = (0..400).step_by(2).flat_map(|x| (0..650).step_by(2).map(move |y| [x,y]))
            .find(|&point| { planning.pointer(Some(point)); planning.tutorial_control_at() == Some(97) }).unwrap();
        planning.pointer(Some(accept));
        assert!(!planning.mouse(true));
        assert!(planning.mouse(false));
        assert_eq!(planning.take_tutorial_control(), Some(97));
        assert!(planning.submit);
        // Native424a28 publishes Accept97 before 424abe queues construction.
        host.tutorial_event(&mut session, &assets, 97).unwrap();
        let queued = session.live_commands.snapshot().unwrap().len();
        let submission = session.submit_build(&request, &assets, |event|
            Err(format!("Tutorial12 hotel unexpectedly built synchronously: {event:?}"))).unwrap();
        assert!(submission.submitted);
        assert_eq!(session.live_commands.snapshot().unwrap().len(), queued + 1);
        // Native4b0810 advances construction upward for height*25 ticks,
        // animates the transition, then counts downward for height*25 more.
        // OpenBusiness command11 and tutorial99 occur after that full cycle.
        let completion_budget = (request.height.max(0) as usize * 50 + 2_000).min(10_000);
        let mut final_prompt = None;
        let mut other_prompts = Vec::new();
        for _ in 0..completion_budget {
            host.frame(&mut session, &assets, 40);
            let status = host.status(&session);
            assert!(status.failure.is_none(), "Tutorial12 hotel construction: {:?}", status.failure);
            for prompt in status.prompts {
                if prompt.dismissal_id == 12 {
                    final_prompt = Some(prompt);
                } else {
                    other_prompts.push(prompt);
                }
            }
            if final_prompt.is_some() { break; }
        }
        let status = host.status(&session);
        assert_eq!(final_prompt.map(|prompt| prompt.dismissal_id), Some(12),
            "native construction completion99 must create Tutorial12 final prompt11; height={} running={} paused={} pending={} first={:?} queue={} clock={:?} message={:?} other_prompts={other_prompts:?}",
            request.height, status.running, status.script_paused, session.world.pending_world.count,
            session.world.pending_world.records[0], session.live_commands.snapshot().unwrap().len(),
            &session.world.clock.timing[..2], status.tutorial_message);
        session.dispatch_session_event("InterfacePromptDismiss", 12, &assets, false).unwrap();
        host.frame(&mut session, &assets, 40);
        assert!(host.status(&session).tutorial_completed, "original prompt12 Game.End(true)");
        let recorded = fs::read(&profile).unwrap();
        host.frame(&mut session, &assets, 40);
        assert_eq!(fs::read(&profile).unwrap(), recorded, "Tutorial12 completion records once");
        let mut progression = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        crate::campaign_store::load_progress(&mut progression, &profile).unwrap();
        assert_eq!(progression.levels.iter().find(|level| level.id() == 100012).unwrap().completed(),
            [true, false, false]);
        drop(host.auction_renderer.take());
        assert_eq!(presenter.join().unwrap(), [1,1,1], "native auction scene prepared, activated and restored once");
    }
    #[test]
    #[ignore = "requires original v1.4 Tutorial 10/11 scripts and assets"]
    fn original_tutorial_ten_eleven_utility_railroad_flow() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let scripts = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent().unwrap().join("scripts");
        let parameters = read("OPENMT2001_PARAMETERS");
        let parameter_path = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let era_styles = load_era_styles(parameter_path.parent().unwrap()).unwrap();
        let timetable = read("OPENMT2001_TIMETABLE");
        let language = parameter_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names = character_names(&image, &language).unwrap();
        let assets = super::super::CommandAssets { image: &image, board: &board, file: &file };
        for (id, first, subsequent, last_prompt) in [
            (100010, "TELECOMS", ["ELECTRIC_COMPANY", "WATER_WORKS", "GASWORKS"], 10),
            (100011, "B_O_RAILROAD", ["PENSYLVANIA_RAILROAD", "READING_RAILROAD", "SHORTLINE_RAILROAD"], 12),
        ] {
            let scenario = crate::scenarios::find(id).unwrap();
            let mut session = Session::initialize(Inputs {
                tree_locators: None,
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"), scripts: &scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(), era_styles: era_styles.clone(),
                timetable: std::str::from_utf8(&timetable).unwrap(), scenario: scenario.token,
                difficulty: 0, selected_character: 0, profile_name: b"Utilities tutorial",
                character_names: &names,
            }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] }).unwrap();
            let profile = std::path::PathBuf::from(format!("work/economy/tutorial{id}-{}-{}.profile",
                std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            crate::campaign_store::save_progress(&session.catalog, &profile).unwrap();
            session.campaign_profile = Some(profile.clone());
            let mut host = crate::live_host::Host::new([0., 20., 0.]);
            host.begin_presentation(&session).unwrap();
            host.set_running(true);
            host.frame(&mut session, &assets, 20);
            let mut status = host.status(&session);
            assert!(status.failure.is_none(), "Tutorial{id} startup: {:?}", status.failure);
            assert_eq!(status.prompts[0].dismissal_id, 2);
            session.dispatch_session_event("InterfacePromptDismiss", 2, &assets, false).unwrap();
            let target = session.scripts.runtime.number(first).unwrap().unwrap() as u32;
            status = host.status(&session);
            assert_eq!(status.tutorial_interaction.callback(
                crate::tutorial_two_interaction::Pick::BlockEntry { category_id: target }), Some(1));
            assert!(host.tutorial_pick(&mut session, &assets,
                crate::tutorial_two_interaction::Pick::BlockEntry { category_id: target }).unwrap());
            status = host.status(&session);
            assert_eq!(status.prompts[0].dismissal_id, 4);
            session.dispatch_session_event("InterfacePromptDismiss", 4, &assets, false).unwrap();
            let info = session.scripts.runtime.number("TUT_MPP_INFOBUTTON_2").unwrap().unwrap() as u32;
            status = host.status(&session);
            assert!(status.tutorial_enabled_controls.contains(&info));
            host.tutorial_event(&mut session, &assets, info).unwrap();
            status = host.status(&session);
            for dismissal in if id == 100010 { vec![6] } else { vec![6, 7, 8, 9] } {
                assert_eq!(status.prompts[0].dismissal_id, dismissal);
                session.dispatch_session_event("InterfacePromptDismiss", dismissal as i32, &assets, false).unwrap();
                status = host.status(&session);
            }
            for (i, name) in subsequent.into_iter().enumerate() {
                let target = session.scripts.runtime.number(name).unwrap().unwrap() as u32;
                assert!(status.tutorial_enabled_controls.contains(&103), "Tutorial{id} Navigator stage{i}");
                assert_eq!(status.tutorial_interaction.callback(
                    crate::tutorial_two_interaction::Pick::BlockEntry { category_id: target }), Some(1));
                if (id == 100010 && i == 0) || (id == 100011 && i == 1) {
                    // The actual Navigator commits control103 after choosing a
                    // target. The scripts also admit the 3D block-entry path.
                    host.tutorial_event(&mut session, &assets, 103).unwrap();
                } else {
                    assert!(host.tutorial_pick(&mut session, &assets,
                        crate::tutorial_two_interaction::Pick::BlockEntry { category_id: target }).unwrap());
                }
                status = host.status(&session);
                if id == 100010 && i == 0 {
                    assert_eq!(status.prompts[0].dismissal_id, 8);
                    session.dispatch_session_event("InterfacePromptDismiss", 8, &assets, false).unwrap();
                    status = host.status(&session);
                }
            }
            assert_eq!(status.prompts[0].dismissal_id, last_prompt + 1);
            session.dispatch_session_event("InterfacePromptDismiss", (last_prompt + 1) as i32, &assets, false).unwrap();
            host.frame(&mut session, &assets, 20);
            assert!(host.status(&session).tutorial_completed, "Tutorial{id} original End");
            let recorded = fs::read(&profile).unwrap();
            host.frame(&mut session, &assets, 20);
            assert_eq!(fs::read(&profile).unwrap(), recorded, "Tutorial{id} records completion once");
            let mut progression = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
            crate::campaign_store::load_progress(&mut progression, &profile).unwrap();
            assert_eq!(progression.levels.iter().find(|level| level.id() == id).unwrap().completed(), [true, false, false]);
        }
    }
    #[test]
    #[ignore = "requires actual v1.4 scripts, original model/road assets"]
    fn original_tutorial_nine_auction_flow() {
        let read=|key|fs::read(std::env::var(key).unwrap()).unwrap();
        let executable=read("OPENMT2001_EXE");
        let archive=read("OPENMT2001_ARCHIVE_BIN");
        let directory=read("OPENMT2001_ARCHIVE_DIR");
        let build=read("OPENMT2001_BUILD_FILE");
        let parameters=read("OPENMT2001_PARAMETERS");
        let timetable=read("OPENMT2001_TIMETABLE");
        let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board=openmt2001::board::Board::from_v14(&image).unwrap();
        let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
        let exe_path=std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts=exe_path.parent().unwrap().join("scripts");
        let language=exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names=character_names(&image,&language).unwrap();
        let parameter_path=std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let era_styles=load_era_styles(parameter_path.parent().unwrap()).unwrap();
        let installer=parameter_path.parent().unwrap().parent().unwrap();
        let offsets=fs::read(language.join("tycoon.off")).unwrap();
        let text=fs::read(language.join("tycoon.bin")).unwrap();
        let mut at=0;
        let strings=offsets.chunks_exact(2).map(|length|{
            let end=at+u16::from_le_bytes(length.try_into().unwrap()) as usize;
            let value=&text[at..end];at=end;value
        }).collect::<Vec<_>>();
        assert_eq!(at,text.len());
        // Internal original-script admission is deliberately independent of
        // menu/profile eligibility while Tutorial5's startup is under review.
        let mut session=Session::initialize(Inputs{
            tree_locators: None,
            image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),era_styles:era_styles.clone(),
            timetable:std::str::from_utf8(&timetable).unwrap(),scenario:crate::scenarios::find(100009).unwrap().token,
            difficulty:0,selected_character:0,profile_name:b"Tutorial9",character_names:&names,
        },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]}).unwrap();
        let assets=super::super::CommandAssets{image:&image,board:&board,file:&file};
        let profile=std::path::PathBuf::from(format!("work/economy/tutorial9-lifecycle-{}-{}.lua",
            std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        crate::campaign_store::save_progress(&session.catalog,&profile).unwrap();
        session.campaign_profile=Some(profile.clone());
        let prompt_assets=crate::prompt::Assets::load(&index,&archive,installer,&strings,&image).unwrap();
        let (renderer_sender,renderer_receiver)=std::sync::mpsc::channel::<crate::auction_bridge::Request>();
        let presenter=std::thread::Builder::new().stack_size(32*1024*1024).spawn(move ||{
            let exe=read("OPENMT2001_EXE");let directory=read("OPENMT2001_ARCHIVE_DIR");let archive=read("OPENMT2001_ARCHIVE_BIN");
            let mut descriptor=wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows){descriptor.backends=wgpu::Backends::DX12;}
            let renderer=pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let mut view=crate::auction_bridge::View::default();let mut counts=[0;3];
            for request in renderer_receiver{
                let kind=match request.operation{
                    crate::auction_bridge::Operation::Prepare{..}=>0,
                    crate::auction_bridge::Operation::Activate=>1,
                    crate::auction_bridge::Operation::Restore=>2,
                };
                // Original SetPlanView is a retained presentation request;
                // Prepare returns the real prior strategic interface mode3.
                let result=view.present(request.operation,&renderer,(&exe,&directory,&archive),3);
                assert!(result.is_ok(),"original9 actual renderer: {result:?}");counts[kind]+=1;
                request.reply.send(result).unwrap();
            }
            counts
        }).unwrap();
        let mut host=crate::live_host::Host::new([0.,20.,0.]);
        host.auction_renderer=Some(renderer_sender);
        host.begin_presentation(&session).unwrap();
        host.set_running(true);
        host.frame(&mut session,&assets,20);
        let status=host.status(&session);
        assert!(status.failure.is_none(),"original9 startup: {:?}",status.failure);
        assert_eq!(session.roster.manager.players.len(),4);
        let locator_assets=crate::locator::Assets::load(&index,&archive,installer,&strings,&executable).unwrap();
        let auction_assets=crate::auction_panel::Assets::load(&index,&archive,installer,&strings,&executable).unwrap();
        let mut modal=crate::prompt::State::default();
        for request in status.prompts{let _=modal.submit(request);}
        let modal_continue=|session:&mut Session<'_>,host:&mut crate::live_host::Host,modal:&mut crate::prompt::State,id:u32|{
            assert_eq!(modal.request().unwrap().dismissal_id,id);
            let status=host.status(session);
            assert!(status.failure.is_none(),"original9 modal{id}: {:?}",status.failure);
            let bounds=prompt_assets.button_at(status.tutorial_interaction.prompt_offset());
            modal.cursor([(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5]);
            assert_eq!(modal.left_button(true,bounds),None);
            assert_eq!(modal.left_button_with(false,bounds,|presenter,dismissal|{
                session.dispatch_session_event("InterfacePromptDismiss",i32::try_from(dismissal).unwrap(),&assets,false).unwrap();
                for request in host.status(session).prompts{let _=presenter.submit(request);}
            }),Some(id));
        };
        modal_continue(&mut session,&mut host,&mut modal,2);
        let pacific=session.scripts.runtime.number("PACIFIC_AVENUE").unwrap().unwrap() as usize;
        assert!(host.tutorial_pick(&mut session,&assets,crate::tutorial_two_interaction::Pick::BlockEntry{category_id:pacific as u32}).unwrap());
        let players=crate::player_info::State::from_runtime(&session.roster.manager,&session.world.ledgers.players,session.roster.status.current_player).unwrap();
        let report=crate::block_information::Report::from_runtime(&session.world,&players,&image).unwrap().unwrap();
        let mut property=crate::utility_report::View::default();property.select(Some(pacific));
        let property_click=|property:&mut crate::utility_report::View,id:u32|{
            let point=(0..400).step_by(2).flat_map(|x|(50..650).step_by(2).map(move|y|[x,y]))
                .find(|&point|locator_assets.property_tutorial_control_at(property,Some(point),&report,pacific)==Some(id)).unwrap();
            locator_assets.property_pointer(property,Some(point),&report,pacific);assert!(property.mouse(true));
            locator_assets.property_pointer(property,Some(point),&report,pacific);assert!(property.mouse(false));
            property.take_tutorial_control().unwrap()
        };
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,4);
        let control=property_click(&mut property,10);host.tutorial_event(&mut session,&assets,control).unwrap();
        let request=property.take_property_request().unwrap();assert!(!request.submit);
        let inspected=host.inspect_property_offer(&mut session,&assets,request.category,request.panel,true).unwrap();
        assert_eq!(inspected.panel.action_visible,1);property.apply_property_inspection(request.category,Ok(inspected));
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,5);
        let control=property_click(&mut property,35);host.tutorial_event(&mut session,&assets,control).unwrap();
        let request=property.take_property_request().unwrap();assert!(request.submit);
        let submitted=host.property_offer(&mut session,&assets,request.category,request.panel).unwrap();assert!(submitted.submitted);
        property.apply_property_inspection(request.category,Ok(submitted));
        let status=host.status(&session);assert!(status.failure.is_none());
        for request in status.prompts{let _=modal.submit(request);}
        modal_continue(&mut session,&mut host,&mut modal,6);
        modal_continue(&mut session,&mut host,&mut modal,7);
        assert_eq!(session.hooks.forced_auction_handlers.len(),3);
        // The native announcement/interface clock queues Start; the normal
        // auction progress consumer delivers63 inline. No tutorial injection.
        for _ in 0..1500{
            host.frame(&mut session,&assets,40);
            let status=host.status(&session);assert!(status.failure.is_none(),"original9 auction start: {:?}",status.failure);
            for request in status.prompts{let _=modal.submit(request);}
            if modal.request().is_some_and(|request|request.dismissal_id==8){break;}
        }
        assert_eq!(modal.request().unwrap().dismissal_id,8,"native63 must create original room introduction");
        for id in 8..=16{modal_continue(&mut session,&mut host,&mut modal,id);}
        let word=|row:&[u8],at|u32::from_le_bytes(row[at..at+4].try_into().unwrap());
        let opening=word(&session.world.auction.controller.record,0x6c);
        let modal_wait=|session:&mut Session<'_>,host:&mut crate::live_host::Host,modal:&mut crate::prompt::State,id:u32|{
            for _ in 0..400{
                host.frame(session,&assets,40);
                let status=host.status(session);assert!(status.failure.is_none(),"original9 forced flow to{id}: {:?}",status.failure);
                let produced=status.prompts.iter().filter(|request|request.dismissal_id==id).count();
                if produced!=0{assert_eq!(produced,1,"the actual scheduled-script prompt{id} publishes once through the command cursor");}
                for request in status.prompts{let _=modal.submit(request);}
                if modal.request().is_some_and(|request|request.dismissal_id==id){break;}
            }
            assert_eq!(modal.request().map(|request|request.dismissal_id),Some(id),
                "original9 waiting{id}: ticks={} pause={:?} prompt={:?} counter={:?} pending={:?} controller={:02x?}",
                host.status(session).ticks,session.world.pause.bytes,
                session.scripts.runtime.number("g_currentprompt").unwrap(),
                session.scripts.runtime.number("g_counter").unwrap(),session.scripts.events.pending(),
                &session.world.auction.controller.record[0x54..0x80]);
        };
        modal_wait(&mut session,&mut host,&mut modal,17);
        assert_eq!(word(&session.world.auction.controller.record,0x6c),opening.wrapping_add(150),"original scheduled AI1/AI2 bids add50/100 before AI3 pass");
        assert_eq!(session.scripts.runtime.number("g_counter").unwrap(),Some(1.));
        modal_continue(&mut session,&mut host,&mut modal,17);
        modal_continue(&mut session,&mut host,&mut modal,18);
        let mut auction_view=crate::auction_panel::View::default();
        let human_bid=|session:&mut Session<'_>,host:&mut crate::live_host::Host,view:&mut crate::auction_panel::View|{
            let status=host.status(session);assert!(status.tutorial_enabled_controls.contains(&65));
            let auction=status.auction.unwrap();let bounds=auction_assets.buttons[0];
            let point=[(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5];
            assert_eq!(view.mouse(&auction_assets,&auction,Some(point),true),None);
            assert_eq!(view.mouse(&auction_assets,&auction,Some(point),false),Some(0));
            host.auction_button(session,&assets,0).unwrap();
        };
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,19);
        human_bid(&mut session,&mut host,&mut auction_view);
        assert_eq!(session.scripts.runtime.number("g_counter").unwrap(),Some(2.));
        modal_wait(&mut session,&mut host,&mut modal,20);
        modal_continue(&mut session,&mut host,&mut modal,20);
        modal_continue(&mut session,&mut host,&mut modal,21);
        assert_eq!(host.status(&session).tutorial_message.unwrap().dismissal_id,22);
        human_bid(&mut session,&mut host,&mut auction_view);
        assert_eq!(session.scripts.runtime.number("g_counter").unwrap(),Some(3.));
        modal_wait(&mut session,&mut host,&mut modal,23);
        assert_eq!(word(&session.world.auction.controller.record,0x60),0,"actual pullout consumers elect the human winner");
        modal_continue(&mut session,&mut host,&mut modal,23);
        host.frame(&mut session,&assets,20);
        let status=host.status(&session);assert!(status.failure.is_none(),"original9 End: {:?}",status.failure);assert!(status.tutorial_completed);assert!(!status.running);
        let recorded=fs::read(&profile).unwrap();host.frame(&mut session,&assets,80);assert_eq!(fs::read(&profile).unwrap(),recorded);
        let mut reopened=crate::campaign_store::load_catalog(&image,&scripts).unwrap();crate::campaign_store::load_progress(&mut reopened,&profile).unwrap();
        assert_eq!(reopened.levels.iter().find(|level|level.id()==100009).unwrap().completed(),[true,false,false]);
        assert_eq!(reopened.levels.iter().find(|level|level.id()==100004).unwrap().completed(),[false;3]);
        drop(host.auction_renderer.take());
        assert_eq!(presenter.join().unwrap(),[1,1,0],"actual original-assets room prepares/activates once; authored End precedes settlement Restore");
        // Authored End precedes the positive-priced lease settlement. This
        // proves tutorial completion, not subsequent auction ownership transfer.
    }
    #[test]
    #[ignore = "requires actual v1.4 scripts, original model/road assets"]
    fn all_original_campaign_scenarios_initialize() {
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
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let executable_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts = executable_path.parent().unwrap().join("scripts");
        let language = executable_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let names = character_names(&image, &language).unwrap();
        let mut failures = vec![];
        let mut successes = 0;
        for number in 1..=16 {
            for difficulty in 0..3 {
                let scenario = format!("SCENARIO{number}");
                let result = Session::initialize(
                    Inputs {
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
                        era_styles: load_era_styles(
                            Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
                                .parent()
                                .unwrap(),
                        )
                        .unwrap(),
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario: &scenario,
                        difficulty,
                        selected_character: 4,
                        profile_name: b"Campaign initialization",
                        character_names: &names,
                    },
                    Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    },
                );
                match result {
                    Ok(mut session) => {
                        assert_eq!(
                            session.catalog.selected_level().unwrap().id(),
                            200000 + number
                        );
                        assert_eq!(
                            session.roster.manager.players.len(),
                            session.world.ledgers.players.len()
                        );
                        let objectives = [
                            (4, 1945.),
                            (3, 3000.),
                            (6, 100.),
                            (6, 100.),
                            (6, 200.),
                            (1, 3.),
                            (2, 2.),
                            (3, 6000.),
                            (4, 1960.),
                            (5, 25000.),
                            (6, 3000.),
                            (5, 20000.),
                            (1, 5.),
                            (5, 40000.),
                            (2, 3.),
                            (4, 2000.),
                        ];
                        assert_eq!(
                            (session.world.objective.kind, session.world.objective.target),
                            objectives[number as usize - 1]
                        );
                        assert_eq!(
                            session.world.objective.flag,
                            u8::from(!matches!(number, 3 | 4))
                        );
                        println!(
                            "{scenario}/{difficulty}: players={}, population={}, businesses={}, objective={}/{}",
                            session.roster.manager.players.len(),
                            session.world.ledgers.residents.active_count,
                            session
                                .world
                                .ledgers
                                .businesses
                                .iter()
                                .filter(|b| b[0x21c] != 0)
                                .count(),
                            session.world.objective.kind,
                            session.world.objective.target
                        );
                        let before = session.world.ledgers.residents.active_count;
                        let assets = super::super::CommandAssets {
                            image: &image,
                            board: &board,
                            file: &file,
                        };
                        session.world.clock.calendar[0] = 14;
                        match session.dispatch_session_event("HourlyUpdate", 14, &assets, true) {
                            Ok(()) => {
                                let (cap, added) = match number {
                                    2 => (350, 10),
                                    6 | 7 | 9 | 10 => (350, 5),
                                    8 => (600, 16),
                                    11 => (430, 16),
                                    12 => (900, 8),
                                    13 => (800, 5),
                                    14 => (632, 5),
                                    15 => (1000, 12),
                                    16 => (1200, 10),
                                    _ => (0, 0),
                                };
                                assert_eq!(
                                    session.world.ledgers.residents.active_count,
                                    before + if before < cap { added } else { 0 },
                                    "{scenario}/{difficulty} afternoon creation"
                                );
                                println!(
                                    "{scenario}/{difficulty}: afternoon population {before}→{}",
                                    session.world.ledgers.residents.active_count
                                );
                            }
                            Err(error) => {
                                failures
                                    .push(format!("{scenario}/{difficulty} afternoon: {error}"));
                                continue;
                            }
                        }
                        if difficulty == 0 && matches!(number, 2 | 3 | 4 | 5 | 12) {
                            let first = session.hooks.commands.len();
                            let context = session
                                .world
                                .script_context(
                                    &image,
                                    &session.resident_models,
                                    openmt2001::level_startup::HostContext {
                accounting_enabled: true,
                                        difficulty,
                                        notifications: true,
                                        auxiliary_template: [0; 0x11c],
                                        category_base: 0x617628,
                                        building_base: 0x591318,
                                    },
                                )
                                .unwrap();
                            let players: Vec<_> = session
                                .roster
                                .manager
                                .players
                                .iter()
                                .map(|p| {
                                    Some(openmt2001::business_binding::Player {
                                        identity: p.reference,
                                        kind: u32::from_le_bytes(p.bytes[0x154..0x158].try_into().unwrap()),
                                    })
                                })
                                .collect();
                            let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
                                &board,
                                &file,
                                std::mem::take(&mut session.models),
                            );
                            let result = session.scripts.trigger_event(
                                ("CreatePrompt", 0, 1),
                                &mut session.world.script_state(&players),
                                &context,
                                &mut geometry,
                                &mut session.hooks,
                            );
                            session.models = std::mem::take(geometry.registrar_mut());
                            if let Err(error) = result {
                                failures.push(format!("{scenario}/{difficulty} prompt: {error}"));
                                continue;
                            }
                            assert!(session.world.pause.has_reason());
                            assert!(session.hooks.commands[first..].iter().any(
                                |(command, args)| matches!(
                                    command,
                                    openmt2001::level_script::Command::Prompt
                                ) && matches!(
                                    args.as_slice(),
                                    [
                                        openmt2001::lua4::Value::Number(_),
                                        openmt2001::lua4::Value::Number(_),
                                        openmt2001::lua4::Value::Number(_),
                                        openmt2001::lua4::Value::Number(0.)
                                    ]
                                )
                            ));
                            session
                                .dispatch_session_event("InterfacePromptDismiss", 1, &assets, true)
                                .unwrap();
                            assert!(!session.world.pause.has_reason());
                        }
                        successes += 1;
                    }
                    Err(error) => {
                        println!("{scenario}/{difficulty}: {error}");
                        failures.push(format!("{scenario}/{difficulty}: {error}"));
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{successes}/48 initialized; remaining:\n{}",
            failures.join("\n")
        );
    }
}
