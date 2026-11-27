//! Supplied-data checks on the actual Session/result dispatch path.
use super::*;
use crate::fresh_city::{Inputs, Policy};
use crate::live_host::process_runtime::{Handle, OriginalGraphicsPolicy};
use std::path::{Path, PathBuf};

// Exercise the actual raw current-camera callback. This does not install a
// synthetic bank in production, widen camera modes, or admit fresh autosave.
fn camera_zoom_height(process: &Handle) -> f32 {
    use crate::intro::camera_live::owner::{Input, Owner};
    use openmt2001::camera::{
        bank::{Bank, Capture},
        primary::{Mode12Input, Mode13Input},
    };
    let mut raw = [0u8; 196];
    raw[0] = 12;
    raw[0x70] = 1;
    raw[0x71] = 1;
    raw[0x74] = 10;
    for (at, value) in [(0x38, 20f32), (0x68, 1.), (0x90, 1.), (0xb0, 80.)] {
        raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let low_height = process.performance_nonzero().unwrap();
    let mut owner = Owner::new(
        Bank::from_capture(Capture::from_records([raw, [0; 196]])),
        0,
    );
    owner
        .central(Input {
            mode12: Mode12Input {
                build_visible: false,
                enable_translation: false,
                enable_keyboard: false,
                enable_rotation: false,
                enable_zoom: true,
                buttons: [false, true],
                delta: [0, 2],
                keys: [false; 4],
                sensitivity: 50,
                legacy_yaw: false,
                invert_vertical: false,
                low_height,
            },
            mode13: Mode13Input {
                enable_translation: false,
                enable_keyboard: false,
                enable_zoom: true,
                buttons: [false, true],
                delta: [0, 2],
                keys: [false; 4],
                sensitivity: 50,
                legacy_yaw: false,
                invert_vertical: false,
                low_height,
            },
        })
        .unwrap();
    f32::from_le_bytes(owner.capture().records()[0][0x38..0x3c].try_into().unwrap())
}
fn eval(session: &mut Session<'_>, source: &[u8], assets: &CommandAssets<'_, '_>) {
    session.result_presentation = Some(ResultPresentation::new(Sequence::Complete, (0, 1)));
    session
        .run_result("performance regression", Some(source), &[], assets)
        .unwrap();
    session.result_presentation = None;
}

#[test]
#[ignore = "requires supplied v1.4 scripts, archive, models, roads and parameter assets"]
fn actual_process_lua_cross_vm_level_camera_and_complete_bound() {
    std::thread::Builder::new().stack_size(16*1024*1024).spawn(||{
        let read=|key|std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable=read("OPENMT2001_EXE");let archive=read("OPENMT2001_ARCHIVE_BIN");
        let directory=read("OPENMT2001_ARCHIVE_DIR");let build=read("OPENMT2001_BUILD_FILE");
        let parameters=read("OPENMT2001_PARAMETERS");let timetable=read("OPENMT2001_TIMETABLE");
        let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board=openmt2001::board::Board::from_v14(&image).unwrap();
        let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
        let exe_path=PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let scripts=exe_path.parent().unwrap().join("scripts");
        let language=exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let names=crate::fresh_city::character_names(&image,&language).unwrap();
        let input=|scenario|Inputs{
            tree_locators:None,image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:&scripts,
            parameters:std::str::from_utf8(&parameters).unwrap(),
            era_styles:crate::fresh_city::load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap(),
            timetable:std::str::from_utf8(&timetable).unwrap(),scenario,difficulty:0,
            selected_character:4,profile_name:b"Process performance regression",character_names:&names,
        };
        let policy=||Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]};
        let assets=CommandAssets{image:&image,board:&board,file:&file};
        let process=crate::live_host::process_runtime::startup(&executable,"").unwrap().0;
        assert_eq!(process.performance8b26cc().unwrap(),2);
        let mut a=Session::initialize_in_process(input("SCENARIO1"),policy(),process.clone()).unwrap();
        eval(&mut a,b"Game.performance='1.9'; a_value=Game.performance; difficulty=Game.GetDifficulty(); same_pause=(Game.Pause==Pause)",&assets);
        assert_eq!(a.scripts.runtime.number("a_value").unwrap(),Some(1.));
        assert_eq!(a.scripts.runtime.number("difficulty").unwrap(),Some(0.));
        assert_eq!(a.scripts.runtime.number("same_pause").unwrap(),Some(1.));
        assert_eq!(process.performance8b26cc().unwrap(),1);assert_eq!(camera_zoom_height(&process),18.);
        // Fresh level and second VM inherit1; there is no constructor replay.
        let mut b=Session::initialize_in_process(input("SCENARIO2"),policy(),process.clone()).unwrap();
        eval(&mut b,b"b_value=Game.performance; Game.performance=0",&assets);
        assert_eq!(b.scripts.runtime.number("b_value").unwrap(),Some(1.));
        assert_eq!(process.performance8b26cc().unwrap(),0);assert_eq!(camera_zoom_height(&process),20.);
        eval(&mut a,b"latest=Game.performance; Game.performance=2",&assets);
        assert_eq!(a.scripts.runtime.number("latest").unwrap(),Some(0.));assert_eq!(camera_zoom_height(&process),18.);
        process.graphics_construction4c6810(OriginalGraphicsPolicy::SoftwareKind2Or3).unwrap();
        process.graphics_construction4c6810(OriginalGraphicsPolicy::NormalD3d).unwrap();
        assert_eq!(process.performance8b26cc().unwrap(),0);
        // Recovered4db450 recurrence plus retained fixed-state expectations
        // fix next state and branches; these are not new native measurements.
        // No expected branch is computed by random_zero_to.
        for (bound,path) in [(0,"win_low.tm"),(1,"win_medium.tm"),(2,"win_high.tm")] {
            eval(&mut a,format!("Game.performance={bound}").as_bytes(),&assets);
            a.script_random_state=0x12345678;
            a.begin_result_sequence(Sequence::Complete,(0,1),&assets).unwrap();
            assert_eq!(a.script_random_state,0xb3e97b5b,"one draw including bound0");
            assert_eq!(a.result_presentation().unwrap().flyby.as_deref(),Some(format!("gamedata/cut_scenes/{path}").as_str()));
            assert_eq!(a.world.clock.hour(),21);assert!(a.result_presentation().unwrap().credits);
            assert!(!a.result_presentation().unwrap().bypass_enabled);
            a.result_presentation=None;
        }
        // Initial2 is a cap, not forced-high. Retained second RNG draw is1.
        a.script_random_state=0xb3e97b5b;
        a.begin_result_sequence(Sequence::Complete,(0,1),&assets).unwrap();
        assert_eq!(a.script_random_state,0x5b3f58b2);
        assert_eq!(a.result_presentation().unwrap().flyby.as_deref(),Some("gamedata/cut_scenes/win_medium.tm"));
        a.result_presentation=None;
        // Original hub + in-memory writes prove top-level then INITIALISE
        // assignment is visible when EVENT_Level samples; no file is changed.
        let complete=std::fs::read_to_string(scripts.join("COMPLETE_SEQUENCE/hub.lua")).unwrap();
        let injected=format!("Game.performance=2; top=Game.performance;\n{}",complete.replacen(
            "function EVENTINITIALISE_Level()","function EVENTINITIALISE_Level()\n Game.performance='0.9';",1));
        a.result_presentation=Some(ResultPresentation::new(Sequence::Complete,(0,1)));
        a.script_random_state=0x12345678;
        a.run_result("Complete initialization setter",Some(injected.as_bytes()),&["EVENTINITIALISE_Level","EVENT_Level"],&assets).unwrap();
        assert_eq!(a.scripts.runtime.number("top").unwrap(),Some(2.));assert_eq!(process.performance8b26cc().unwrap(),0);
        assert_eq!(a.script_random_state,0xb3e97b5b);
        assert_eq!(a.result_presentation().unwrap().flyby.as_deref(),Some("gamedata/cut_scenes/win_low.tm"));
        a.result_presentation=None;
        // Failure boundary: no detached fallback; signed setter itself accepts-1.
        eval(&mut b,b"Game.performance=-1",&assets);
        let before=a.script_random_state;
        assert!(a.begin_result_sequence(Sequence::Complete,(0,1),&assets).is_err());
        assert_eq!(a.script_random_state,before);assert_eq!(process.performance8b26cc().unwrap(),-1);
        let mut absent=Session::initialize(input("SCENARIO1"),policy()).unwrap();
        assert!(absent.begin_result_sequence(Sequence::Complete,(0,1),&assets).unwrap_err().contains("process owner absent"));
        // The surviving process outlives both VMs and the old World.
        drop(a);drop(b);assert_eq!(process.performance8b26cc().unwrap(),-1);
        let mut c=Session::initialize_in_process(input("SCENARIO1"),policy(),process.clone()).unwrap();
        eval(&mut c,b"after_rebind=Game.performance",&assets);
        assert_eq!(c.scripts.runtime.number("after_rebind").unwrap(),Some(-1.));
    }).unwrap().join().unwrap();
}
