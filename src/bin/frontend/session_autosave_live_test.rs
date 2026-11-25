//! CPU regressions at the real Session calendar seam.
//! Controlled clock/hourly controls isolate serialization, not natural-game or
//! camera-provider acceptance. No fallback handler admits unknown simulation work.
use super::*;
use crate::fresh_city::{CommandAssets,Inputs,Policy};
use std::{path::{Path,PathBuf},sync::Arc};
struct Scratch(PathBuf);
impl Scratch {
    fn new()->Self {
        let parent=std::env::var_os("OPENMT2001_TEST_SCRATCH").map(PathBuf::from).unwrap_or_else(||PathBuf::from("work/test-scratch"));
        std::fs::create_dir_all(&parent).unwrap();
        let root=parent.join(format!("autosave-live-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();Self(root)
    }
}
impl Drop for Scratch {fn drop(&mut self){std::fs::remove_dir_all(&self.0).unwrap();}}

use openmt2001::scheduler;

fn presentation(event:crate::fresh_city::tick::Event<'_>)->Result<u32,String> {
    use crate::fresh_city::tick::Event;
    match event {
        // Existing Host's documented render-only boundaries. This CPU test
        // deliberately exercises no renderer/provider or original oracle.
        Event::Tick(scheduler::Event::Helper(0x4805b0|0x450080|0x4eb5b0|0x4b5db0))
        |Event::Script{..}|Event::ScriptCommands(_)|Event::EraMusic{..}
        |Event::TransportSound(_)|Event::TransportEnvironment{..}=>Ok(0),
        other=>Err(format!("Unsupported CPU regression boundary: {other:?}")),
    }
}
fn intro(session:&mut Session<'_>)->Vec<(u32,i32)> {
    session.scripts.events.pending().into_iter().filter_map(|event| {
        let (name,_)=session.scripts.runtime.event_reference_identity(event.reference).unwrap().unwrap();
        (name=="EVENT_BusinessIntro1940").then_some((event.remaining,event.argument))
    }).collect()
}
#[test]
#[ignore="requires supplied original data; controlled real Session Autosave/modern load"]
fn callback_bytes_preserve_transient_live_owners_and_exact_modern_era_queue() {run(true);}
#[test]
#[ignore="requires supplied original data; controlled real Session Autosave/modern load"]
fn ordinary_midnight_does_not_manufacture_modern_era_introduction() {run(false);}
fn run(era:bool) {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(move|| {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
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
        let default = std::path::PathBuf::from(std::env::var("OPENMT2001_DEFAULT_SETTINGS").unwrap());
        let scripts = default.parent().unwrap().parent().unwrap();
        let styles = crate::fresh_city::load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap();
        let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
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
        loaded.restore_v2_extension_with_image(&saved,&image).unwrap();
        assert_eq!(loaded.world.clock.calendar[3],if era{239}else{23});
        assert_eq!(loaded.hooks.utilities.owners,[1,2,3,4]);
        assert_eq!(loaded.hooks.utilities.totals,[5.,6.,7.,8.]);assert_eq!(loaded.allocation_next,0x1f000000);
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
        assert_eq!(std::fs::read(prior).unwrap(),bytes);

    }).unwrap().join().unwrap();
}
