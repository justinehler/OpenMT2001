//! CPU regressions at the real Session calendar seam.
//! Controlled clock/hourly controls isolate serialization, not natural-game or
//! camera-provider acceptance. No fallback handler admits unknown simulation work.
use super::*;
use crate::fresh_city::{CommandAssets, Inputs, Policy};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = openmt2001::fixtures::scratch("TEST_SCRATCH");
        std::fs::create_dir_all(&parent).unwrap();
        let root = parent.join(format!(
            "autosave-live-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

use openmt2001::scheduler;

fn presentation(event: crate::fresh_city::tick::Event<'_>) -> Result<u32, String> {
    use crate::fresh_city::tick::Event;
    match event {
        // Existing Host's documented render-only boundaries. This CPU test
        // deliberately exercises no renderer/provider or original oracle.
        Event::Tick(scheduler::Event::Helper(0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0))
        | Event::Script { .. }
        | Event::ScriptCommands(_)
        | Event::EraMusic { .. }
        | Event::TransportSound(_)
        | Event::TransportEnvironment { .. } => Ok(0),
        other => Err(format!("Unsupported CPU regression boundary: {other:?}")),
    }
}
fn intro(session: &mut Session<'_>) -> Vec<(u32, i32)> {
    session
        .scripts
        .events
        .pending()
        .into_iter()
        .filter_map(|event| {
            let (name, _) = session
                .scripts
                .runtime
                .event_reference_identity(event.reference)
                .unwrap()
                .unwrap();
            (name == "EVENT_BusinessIntro1940").then_some((event.remaining, event.argument))
        })
        .collect()
}
#[test]
#[ignore = "requires supplied original data; controlled real Session Autosave/modern load"]
fn callback_bytes_preserve_transient_live_owners_and_exact_modern_era_queue() {
    run(true);
}
#[test]
#[ignore = "requires supplied original data; controlled real Session Autosave/modern load"]
fn ordinary_midnight_does_not_manufacture_modern_era_introduction() {
    run(false);
}
fn run(era: bool) {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(move|| {
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
        let assets=CommandAssets{image:&image,board:&board,file:&file};
        let mut source=Session::initialize(inputs(),policy()).unwrap();
        assert_eq!(source.session_option_word, 0);
        // Exercise a raw Sandbox option independently of this fixture's level and logic mode.
        source.session_option_word = 2;
        // Controlled one-step midnight. Literal expected words below are
        // independent of the encoder; this is not a natural midnight claim.
        source.world.clock.calendar=if era{[23,9,0,239,9]}else{[23,0,0,23,0]};
        source.world.clock.timing=[1000,624,624,14999,0,1];
        source.world.settings.tweaks[4..8].copy_from_slice(&1u32.to_le_bytes());
        source.hooks.utilities.owners=[1,2,3,4];
        source.hooks.utilities.enabled=[true,false,true,false];
        source.hooks.utilities.totals=[5.,6.,7.,8.];
        source.allocation_next=0x1f000000;
        source.live_commands.restore_queued(Command::CategoryReady{category:0},
            scheduler::ClockDomain::Normal,650,650,true);
        let category_ids:Vec<_>=source.world.categories.iter().map(|category| {
            let count=word(category,8) as i32;
            let mut pointer=word(category,0x15c);let mut ids=Vec::new();
            for _ in 0..count.max(0) {
                assert_ne!(pointer,0x590f80,"fixture uses real building links");
                let row=&source.world.buildings[((pointer-0x591318)/916) as usize];
                ids.push(word(row,0));pointer=word(row,0x44);
            }
            ids
        }).collect();
        assert!(category_ids.iter().any(|ids|!ids.is_empty()),
            "the supplied Session must exercise at least one populated native ID chain");
        for category in &mut source.world.categories {
            for i in 0..24 {put(category,12+i*4,0xfacefeed);}
        }
        let camera=openmt2001::flyby::CameraSample {
            eye:[100.,120.,100.],target:[0.;3],up:[0.,1.,0.],vertical_fov:0.7,
        };
        let cameras=crate::save_menu::cameras(None,camera).unwrap();
        let mut host=crate::live_host::Host::new(camera.eye);
        host.policy.hourly.suppressed=true;
        let auction_host=host.auction_save_state();
        let mut captured=None;let mut calls=0;
        source.advance_session(&assets,&mut host.policy,|event,clock| {
            if let crate::fresh_city::tick::Event::Autosave(request)=event {
                calls+=1;
                assert_eq!(clock.calendar,if era{[0,0,1,239,10]}else{[0,1,0,23,1]});
                assert_eq!(clock.timing,[1001,625,0,0,0,1]);
                let bytes=request.encode_once(cameras,1,auction_host.clone(),0)?;
                assert!(request.encode_once(cameras,1,auction_host.clone(),0).is_err());
                captured=Some(bytes);Ok(0)
            }else{presentation(event)}
        }).unwrap();
        assert_eq!(calls,1);assert_eq!(source.world.clock.calendar[3],if era{240}else{24});
        for (category,ids) in source.world.categories.iter().zip(category_ids) {
            for (i,id) in ids.iter().enumerate(){assert_eq!(word(category,12+i*4),*id);}
            for i in ids.len()..24 {assert_eq!(word(category,12+i*4),0xfacefeed);}
        }

        let bytes=captured.unwrap();let saved=SaveFile::parse(&bytes).unwrap();
        assert_eq!(saved.session_option_word().unwrap(), 2);
        assert_eq!(saved.clock().unwrap().calendar,if era{[0,0,1,239,10]}else{[0,1,0,23,1]});
        let ext=extension::Extension::parse(&saved).unwrap().unwrap();
        let economy=ext.required(12).unwrap();
        assert_eq!((0..12).map(|i|word(economy,i*4)).collect::<Vec<_>>(),
            [1,2,3,4,1,0,1,0,0x40a00000,0x40c00000,0x40e00000,0x41000000]);
        assert_eq!(word(ext.required(3).unwrap(),32),0x1f000000);
        assert_eq!(verified_modern_presentation_queue(&saved).unwrap(),[true]);
        assert_eq!(intro(&mut source),if era{vec![(1249,0)]}else{vec![]});
        // Later Session state cannot enter the callback's completed owned bytes.
        source.hooks.utilities.totals=[91.,92.,93.,94.];source.allocation_next=0x2f000000;
        let mut loaded=Session::load_original(inputs(),policy(),&bytes).unwrap();
        assert_eq!(loaded.session_option_word, 2);
        assert_eq!(loaded.saved_logic_mode, Some(1));
        let native_allocation_next=loaded.allocation_next;
        loaded.restore_v2_extension_with_image(&saved,&image).unwrap();
        let manual = loaded.save_v2(&image, cameras, 1).unwrap();
        assert_eq!(SaveFile::parse(&manual).unwrap().session_option_word().unwrap(), 2);
        assert_eq!(loaded.world.clock.calendar[3],if era{239}else{23});
        assert_eq!(loaded.hooks.utilities.owners,[1,2,3,4]);
        assert_eq!(loaded.hooks.utilities.totals,[5.,6.,7.,8.]);assert_eq!(loaded.allocation_next,native_allocation_next.max(0x1f000000));
        assert_eq!(intro(&mut loaded),if era{vec![(1250,0)]}else{vec![]});
        let queue=loaded.live_commands.snapshot().unwrap();assert_eq!(queue.len(),1);
        assert!(matches!(queue[0],(scheduler::ClockDomain::Normal,650,650,true,Command::CategoryReady{category:0})));
        drop(queue);
        let mut resumed_host=crate::live_host::Host::new(camera.eye);
        resumed_host.policy.hourly.suppressed=true;
        loaded.advance_session(&assets,&mut resumed_host.policy,|event,_|presentation(event)).unwrap();
        assert_eq!(intro(&mut loaded),if era{vec![(1249,0)]}else{vec![]});
        assert_eq!(loaded.world.clock.calendar[3],if era{239}else{23});
        // The default real Host attempts autosave but honestly reports missing
        // selected authority, and still completes calendar and script tick.
        let mut recovering=Session::initialize(inputs(),policy()).unwrap();
        recovering.session_option_word = 2;
        recovering.world.clock.calendar=[23,0,0,23,0];
        recovering.world.clock.timing=[1000,624,624,14999,0,1];
        recovering.world.settings.tweaks[4..8].copy_from_slice(&1u32.to_le_bytes());
        let mut real_host=crate::live_host::Host::new(camera.eye);real_host.running=true;
        real_host.policy.hourly.suppressed=true;real_host.tick(&mut recovering,&assets);
        assert!(real_host.failure.is_none(),"{:?}",real_host.failure);
        assert_eq!(recovering.world.clock.calendar,[0,1,0,24,1]);
        assert!(matches!(real_host.autosave_status,Some(crate::save_menu::AutosaveStatus::Failed{
            error:crate::save_menu::AutosaveFailure::MissingSelectedProfile,..})));
        let scratch=Scratch::new();let root=crate::save_menu::selected_profile_root(&scratch.0,b"Selected");
        let prior=crate::save_menu::PreparedAutosave::prepare(&root).unwrap().publish(bytes.clone()).unwrap();
        real_host.autosave_profile_root=Some(root);
        recovering.world.clock.calendar=[23,0,0,23,0];recovering.world.clock.timing=[1000,624,624,14999,0,1];
        real_host.tick(&mut recovering,&assets);
        assert!(real_host.failure.is_none(),"{:?}",real_host.failure);
        assert_eq!(recovering.world.clock.calendar,[0,1,0,24,1]);
        assert!(matches!(real_host.autosave_status,Some(crate::save_menu::AutosaveStatus::Failed{
            error:crate::save_menu::AutosaveFailure::UnsupportedCamera,..})));
        assert_eq!(std::fs::read(&prior).unwrap(),bytes);
        assert_eq!(std::fs::read_dir(prior.parent().unwrap()).unwrap().count(),1);
        // The actual Host derives93bbb9 from network state, without a new option.
        recovering.world.clock.calendar=[23,0,0,23,0];recovering.world.clock.timing=[1000,624,624,14999,0,1];
        real_host.autosave_status=None;real_host.policy.scheduler.network_enabled=true;
        real_host.tick(&mut recovering,&assets);
        assert!(real_host.failure.is_none(),"{:?}",real_host.failure);
        assert!(real_host.policy.calendar.autosave_disabled);assert!(real_host.autosave_status.is_none());
        assert_eq!(recovering.world.clock.calendar,[0,1,0,24,1]);
        assert_eq!(std::fs::read(&prior).unwrap(),bytes);

        if !era {
            // Controlled real consumer coverage; the production Intro producer
            // remains a separate dependency, not fabricated by this fixture.
            use crate::save_menu::camera_source;
            let (wrong_publisher,wrong_reader)=camera_source::channel(loaded.world.topology.epoch());
            assert!(real_host.install_ui_camera_source(&recovering,wrong_reader).is_err());
            drop(wrong_publisher);
            let (mut publisher,reader)=camera_source::channel(recovering.world.topology.epoch());
            real_host.install_ui_camera_source(&recovering,reader).unwrap();
            assert!(real_host.has_ui_camera_source());
            let (spare_publisher,spare_reader)=camera_source::channel(recovering.world.topology.epoch());
            assert!(real_host.install_ui_camera_source(&recovering,spare_reader).is_err(),
                "a second publisher must not replace the installed source");
            drop(spare_publisher);
            real_host.policy.scheduler.network_enabled=false;
            recovering.world.clock.calendar=[23,0,0,23,0];
            recovering.world.clock.timing=[1000,624,624,14999,0,1];
            real_host.tick(&mut recovering,&assets);
            assert!(real_host.failure.is_none(),"{:?}",real_host.failure);
            assert!(matches!(real_host.autosave_status,Some(crate::save_menu::AutosaveStatus::Failed{
                error:crate::save_menu::AutosaveFailure::UnsupportedState(_),..})));
            assert_eq!(std::fs::read(&prior).unwrap(),bytes,"no missing-source fallback");

            let mut expected=cameras;
            expected[1][..4].copy_from_slice(&13u32.to_le_bytes());
            expected[1][0x4c..0x50].copy_from_slice(&333f32.to_le_bytes());
            expected[1][0xb0]=0xa7; // opaque byte must survive both publishers
            publisher.publish(openmt2001::camera::bank::Capture::from_records(expected),0).unwrap();
            recovering.world.clock.calendar=[23,0,0,23,0];
            recovering.world.clock.timing=[1000,624,624,14999,0,1];
            real_host.tick(&mut recovering,&assets);
            assert!(real_host.failure.is_none(),"{:?}",real_host.failure);
            assert!(matches!(&real_host.autosave_status,Some(crate::save_menu::AutosaveStatus::Saved(path)) if path==&prior));
            let automatic=std::fs::read(&prior).unwrap();
            assert_eq!(SaveFile::parse(&automatic).unwrap().session_option_word().unwrap(), 2);
            assert_eq!(SaveFile::parse(&automatic).unwrap().cameras().unwrap(),[&expected[0],&expected[1]]);

            let root=real_host.autosave_profile_root.clone().unwrap();
            let target=crate::save_menu::Target::NewName("SameSnapshot".into());
            assert!(real_host.save_selected_from_ui_snapshot(&mut recovering,&image,&root,&target,1).is_err(),
                "manual consumer requires the actual Options hold");
            real_host.options(&mut recovering.world.pause,&mut recovering.hooks.timers,true).unwrap();
            let pause=recovering.world.pause.clone();let timers=recovering.hooks.timers.clone();
            let manual=real_host.save_selected_from_ui_snapshot(&mut recovering,&image,&root,&target,1).unwrap();
            let manual_bytes=std::fs::read(&manual).unwrap();
            assert_eq!(SaveFile::parse(&manual_bytes).unwrap().session_option_word().unwrap(), 2);
            assert_eq!(SaveFile::parse(&manual_bytes).unwrap().cameras().unwrap(),[&expected[0],&expected[1]]);
            assert_eq!(recovering.world.pause,pause,"live Options hold must remain owned");
            assert_eq!(recovering.hooks.timers,timers,"serialization cannot release live timers");
            publisher.retire().unwrap();
            assert!(real_host.save_selected_from_ui_snapshot(&mut recovering,&image,&root,
                &crate::save_menu::Target::ExistingSlot("SameSnapshot.sav".into()),1).is_err());
            assert_eq!(std::fs::read(&manual).unwrap(),manual_bytes);
            real_host.options(&mut recovering.world.pause,&mut recovering.hooks.timers,false).unwrap();
            recovering.world.clock.calendar=[23,0,0,23,0];
            recovering.world.clock.timing=[1000,624,624,14999,0,1];
            real_host.tick(&mut recovering,&assets);
            assert!(real_host.failure.is_none(),"retired save source is recoverable: {:?}",real_host.failure);
            assert!(matches!(real_host.autosave_status,Some(crate::save_menu::AutosaveStatus::Failed{
                error:crate::save_menu::AutosaveFailure::UnsupportedState(_),..})));
            assert_eq!(std::fs::read(&prior).unwrap(),automatic);
            assert_eq!(std::fs::read_dir(prior.parent().unwrap()).unwrap().count(),2);
        }

    }).unwrap().join().unwrap();
}
#[test]
#[ignore = "requires original tutorial assets; real Host Autosave and normal Session load"]
fn tutorial_midnight_publishes_overwrites_and_restarts_original_scripts() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
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
            timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "TUTORIAL1", difficulty: 0,
            selected_character: 4, profile_name: b"Tutorial autosave", character_names: &names,
        };
        let policy = || Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 284] };
        let assets=CommandAssets{image:&image,board:&board,file:&file};
        let mut source = Session::initialize(inputs(), policy()).unwrap();
        assert_eq!(source.tutorial_state, 1);
        let camera = openmt2001::flyby::CameraSample {
            eye: [100., 120., 100.], target: [0.; 3], up: [0., 1., 0.], vertical_fov: 0.7,
        };
        let cameras = crate::save_menu::cameras(None, camera).unwrap();
        let scratch = Scratch::new();
        let root = crate::save_menu::selected_profile_root(&scratch.0, b"Tutorial autosave");
        source.campaign_profile = Some(crate::campaign_store::profile_path(&scratch.0, b"Tutorial autosave"));
        let mut host = crate::live_host::Host::new(camera.eye);
        host.running = true;
        host.autosave_profile_root = Some(root.clone());
        // The retained tutorial supplies its zero-year clock and original
        // hourly/daily query gates; the real Host owns autosave publication.
        host.frame(&mut source, &assets, 0);
        assert!(host.failure.is_none(), "{:?}", host.failure);
        assert!(host.status(&source).tutorial_controls.all_disabled);
        assert!(!source.scripts.events.pending().is_empty());
        source.scripts.runtime.execute("progress before autosave",
            b"g_currentprompt=9; g_counter=123", &mut |_, _| Err("unexpected binding".into())).unwrap();
        let (mut publisher, reader) = crate::save_menu::camera_source::channel(source.world.topology.epoch());
        host.install_ui_camera_source(&source, reader).unwrap();
        publisher.publish(openmt2001::camera::bank::Capture::from_records(cameras), 0).unwrap();
        source.world.clock.calendar = [23, 0, 0, 23, 0];
        source.world.clock.timing = [1000, 624, 624, 14999, 0, 1];
        host.tick(&mut source, &assets);
        assert!(host.failure.is_none(), "{:?}", host.failure);
        let destination = root.join("saves/autosave.sav");
        assert!(matches!(&host.autosave_status, Some(crate::save_menu::AutosaveStatus::Saved(path)) if path == &destination));
        let bytes = std::fs::read(&destination).unwrap();
        let saved = SaveFile::parse(&bytes).unwrap();
        assert_eq!(saved.level_metadata().unwrap()[0], 100000);
        assert_eq!(source.session_option_word, 0);
        assert_eq!(saved.session_option_word().unwrap(), 0);
        assert_eq!(saved.clock().unwrap().calendar, [0, 0, 0, 23, 1]);
        assert_eq!(saved.clock().unwrap().timing, [1001, 625, 0, 0, 0, 1]);
        assert_eq!(source.world.clock.calendar, [0, 0, 0, 0, 1]);
        let ext = extension::Extension::parse(&saved).unwrap().unwrap();
        assert_eq!(ext.required(13).unwrap(), 0u32.to_le_bytes());
        assert_eq!(source.save_v2(&image, cameras, 1).unwrap_err(), "Manual Save is disabled in tutorials");
        // Same native initialization and extension restoration as the Load route.
        let mut loaded = Session::load_original(inputs(), policy(), &bytes).unwrap();
        let metadata = loaded.restore_v2_extension_with_image(&saved, &image).unwrap().unwrap();
        assert_eq!(metadata.profile_filename, source.campaign_profile.as_ref().map(|p|p.file_name().unwrap().to_str().unwrap().to_owned()));
        assert_eq!(loaded.tutorial_state, 1);
        assert_eq!(loaded.world.clock.calendar, [0, 0, 0, 23, 1]);
        assert!(loaded.scripts.events.pending().is_empty());
        loaded.scripts.runtime.execute("read tutorial progress",
            b"function ReadPrompt() return g_currentprompt end; function ReadCounter() return g_counter end",
            &mut |_, _| Err("unexpected binding".into())).unwrap();
        assert_eq!(loaded.scripts.runtime.call_number("ReadPrompt", &[], &mut |_, _| Err("unexpected binding".into())).unwrap(), 0.);
        assert_eq!(loaded.scripts.runtime.call_number("ReadCounter", &[], &mut |_, _| Err("unexpected binding".into())).unwrap(), 0.);
        let mut resumed = crate::live_host::Host::new(camera.eye);
        resumed.running = true;
        resumed.frame(&mut loaded, &assets, 0);
        assert!(resumed.failure.is_none(), "{:?}", resumed.failure);
        let status = resumed.status(&loaded);
        assert!(status.tutorial_session && status.tutorial_controls.all_disabled);
        assert!(!status.prompts.is_empty());
        assert_eq!(loaded.scripts.runtime.call_number("ReadPrompt", &[], &mut |_, _| Err("unexpected binding".into())).unwrap(), 1.);
        assert!(!loaded.scripts.events.pending().is_empty(), "fresh Level must schedule its camera callback");
        // A later midnight overwrites the same slot, including zero-year days.
        source.world.clock.calendar = [23, 0, 0, 23, 1];
        source.world.clock.timing = [2000, 1249, 624, 14999, 0, 1];
        host.tick(&mut source, &assets);
        assert!(host.failure.is_none(), "{:?}", host.failure);
        assert!(matches!(&host.autosave_status, Some(crate::save_menu::AutosaveStatus::Saved(path)) if path == &destination));
        let replaced = std::fs::read(&destination).unwrap();
        assert_ne!(bytes, replaced);
        assert_eq!(SaveFile::parse(&replaced).unwrap().clock().unwrap().calendar, [0, 0, 0, 23, 2]);
        assert_eq!(std::fs::read_dir(destination.parent().unwrap()).unwrap().count(), 1);
        source.world.clock.calendar = [23, 0, 0, 23, 2];
        source.world.clock.timing = [3000, 1874, 624, 14999, 0, 1];
        host.policy.scheduler.network_enabled = true;
        host.autosave_status = None;
        host.tick(&mut source, &assets);
        assert!(host.failure.is_none(), "{:?}", host.failure);
        assert!(host.autosave_status.is_none());
        assert_eq!(source.world.clock.calendar, [0, 0, 0, 0, 3]);
        assert_eq!(std::fs::read(&destination).unwrap(), replaced);
    }).unwrap().join().unwrap();
}
