//! Source-owned pieces use the exact production handler body.
//! The ignored supplied-data test additionally uses actual Host + Session +
//! mutable Intro Assets/commonFonts loans across the real operation stream.
use super::*;
use crate::live_host::camera_bridge::{
    self as bridge, Admission, MissingFamily, Observation, Operation, PauseByte, PauseReason, Pose,
};
use std::{thread, time::Duration};
const WAIT: Duration = Duration::from_secs(10);
pub(super) fn observation() -> Observation {
    Observation {
        raw_capture: None,
        current: Pose {
            eye: [0; 3],
            target: [0; 3],
            up: [0; 3],
        },
        sampled: None,
        strategic_writeback: None,
        finalization_fault: None,
        task_effects: vec![],
    }
}
pub(super) fn install() -> Operation {
    Operation::Install {
        property: None,
        admission: Admission::default(),
        capture: openmt2001::camera::bank::Capture::from_records([[0; 196]; 2]),
    }
}
fn central() -> Operation {
    Operation::CentralTail {
        admission: Admission::default(),
    }
}
#[test]
fn production_handler_body_mutates_actual_pause_and_timers_before_next_ui_read() {
    let (mut worker, mut inbox) = bridge::channel(710);
    let ui = thread::spawn(move || {
        inbox.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox
            .wait_service_with_handler(WAIT, |_, port| {
                assert_eq!(port.pause_byte(PauseByte::SignedReason3_8b22d3).unwrap(), 0);
                port.pause4bac90(PauseReason::Action3).unwrap();
                assert_eq!(port.pause_byte(PauseByte::SignedReason3_8b22d3).unwrap(), 1);
                assert_eq!(port.pause_byte(PauseByte::Latch8b22d8).unwrap(), 1);
                port.unpause4bacc0(PauseReason::Action3).unwrap();
                assert_eq!(port.pause_byte(PauseByte::Latch8b22d8).unwrap(), 0);
                Err("fault after actual effects".into())
            })
            .unwrap_err()
    });
    let mut pause = openmt2001::pause::State::default();
    let mut timers = vec![[0x5a; 20]; 2];
    timers[0][0] = 1;
    timers[0][4..8].copy_from_slice(&2u32.to_le_bytes());
    timers[1][4..8].copy_from_slice(&1u32.to_le_bytes());
    let before = timers.clone();
    let catalog = openmt2001::level_catalog::Catalog::default();
    worker.request(install()).unwrap();
    let mut trace = vec![];
    let error = worker
        .request_with_handler(central(), |call| {
            let reply = apply(&mut pause, &mut timers, &catalog, call.clone())?;
            trace.push((call, pause.bytes, timers[0][0]));
            Ok(reply)
        })
        .unwrap_err();
    assert_eq!(
        error,
        RequestError::Failed("fault after actual effects".into())
    );
    assert_eq!(trace[1].1[3], 1);
    assert_eq!(trace[1].2, 0);
    assert_eq!(trace[4].1[3], 0);
    assert_eq!(trace[4].2, 1);
    assert_eq!(pause.bytes, [0; 9]);
    assert_eq!(timers, before);
    assert_eq!(ui.join().unwrap(), "fault after actual effects");
}
#[test]
fn busy_reads_selected_raw_byte_with_null_and_non_one_domains() {
    let mut pause = Default::default();
    let mut tasks = [];
    let mut catalog = openmt2001::level_catalog::Catalog::default();
    assert_eq!(
        apply(&mut pause, &mut tasks, &catalog, SessionCall::Busy48dbf0).unwrap(),
        SessionReply::Byte(0)
    );
    let mut bytes = [0; openmt2001::level_catalog::RECORD_SIZE];
    bytes[0x85c] = 2;
    catalog
        .levels
        .push(openmt2001::level_catalog::Level { bytes });
    catalog.selected = Some(0);
    assert_eq!(
        apply(&mut pause, &mut tasks, &catalog, SessionCall::Busy48dbf0).unwrap(),
        SessionReply::Byte(2)
    );
    catalog.levels[0].bytes[0x85c] = 0xfe;
    assert_eq!(
        apply(&mut pause, &mut tasks, &catalog, SessionCall::Busy48dbf0).unwrap(),
        SessionReply::Byte(0xfe)
    );
}
#[test]
fn signed_reason_byte_retained_and_missing_family_is_not_success() {
    let mut pause = openmt2001::pause::State { bytes: [0; 9] };
    pause.bytes[3] = 0x80;
    let mut tasks = [];
    let catalog = Default::default();
    assert_eq!(
        apply(
            &mut pause,
            &mut tasks,
            &catalog,
            SessionCall::PauseByte(PauseByte::SignedReason3_8b22d3)
        )
        .unwrap(),
        SessionReply::Byte(0x80)
    );
    apply(
        &mut pause,
        &mut tasks,
        &catalog,
        SessionCall::Unpause4bacc0(PauseReason::Action3),
    )
    .unwrap();
    assert_eq!(
        pause.bytes[3], 0x80,
        "signed JLE must not decrement wrapped reason"
    );
    assert!(
        apply(
            &mut pause,
            &mut tasks,
            &catalog,
            SessionCall::Missing(MissingFamily::Pause43e5f0)
        )
        .is_err()
    );
    assert_eq!(pause.bytes[3], 0x80);
}

#[test]
#[ignore = "requires supplied original Session and Intro assets"]
fn actual_host_session_and_ui_assets_service_fresh_reads_before_late_fault() {
    use crate::{
        fresh_city::{self, Inputs, Policy},
        live_host::Host,
    };
    use std::{fs, path::Path};
    let path = |key: &str| openmt2001::fixtures::path(key);
    let read = |key: &str| fs::read(path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build = read("OPENMT2001_BUILD_FILE");
    let parameters = read("OPENMT2001_PARAMETERS");
    let timetable = read("OPENMT2001_TIMETABLE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let settings = path("OPENMT2001_DEFAULT_SETTINGS");
    let scripts = settings.parent().unwrap().parent().unwrap();
    let names = fresh_city::character_names(&image, &path("OPENMT2001_LANGUAGE")).unwrap();
    let mut session = Session::initialize(
        Inputs {
            tree_locators: None,
            image: &image,
            board: &board,
            file: &file,
            people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                &image, &index, &archive,
            )
            .unwrap(),
            transport_models: openmt2001::traffic::models::Models::from_original(
                &image, &index, &archive,
            )
            .unwrap(),
            road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
            scripts,
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: fresh_city::load_era_styles(
                Path::new(&path("OPENMT2001_PARAMETERS")).parent().unwrap(),
            )
            .unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: "SCENARIO3",
            difficulty: 0,
            selected_character: 4,
            profile_name: b"Host Service Fixture",
            character_names: &names,
        },
        Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 0x11c],
        },
    )
    .unwrap();
    // Real Session deliberately remains on this thread, including its Lua VM.
    let (worker, mut inbox) = bridge::channel(711);
    let mut host = Host::new([0.; 3]);
    host.camera_bridge = Some(worker);
    let ui = thread::spawn(move || {
        let path = |key: &str| openmt2001::fixtures::path(key);
        let mut assets = crate::intro::Assets::load(crate::intro::Paths {
            compatibility: crate::compatibility::Mode::StrictV14,
            installer: path("OPENMT2001_INSTALLER"),
            ffmpeg: path("OPENMT2001_FFMPEG"),
            scene: path("OPENMT2001_SCENE"),
            executable: path("OPENMT2001_EXE"),
            map: path("OPENMT2001_INTRO_MAP"),
            quality: "low".into(),
            language: path("OPENMT2001_LANGUAGE"),
            profile_root: openmt2001::fixtures::scratch("TEXT_PROFILE"),
            save_root: openmt2001::fixtures::scratch("TEXT_SAVES"),
        })
        .unwrap();
        inbox.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox
            .wait_service_with_handler(WAIT, |_, services| {
                // This exclusive loan to the real process boxed manager spans a
                // Host rendezvous without moving Assets or using a cloned manager.
                let fonts = assets.ui_text.fonts_mut();
                let identity = fonts as *const _;
                let old = fonts.current;
                fonts.current = 1;
                services.pause4bac90(PauseReason::Action3).unwrap();
                assert_eq!(
                    services
                        .pause_byte(PauseByte::SignedReason3_8b22d3)
                        .unwrap(),
                    1
                );
                assert_eq!(services.pause_byte(PauseByte::Latch8b22d8).unwrap(), 1);
                assert_eq!(fonts as *const _, identity);
                assert_eq!(fonts.current, 1);
                fonts.current = old;
                Err("late supplied UI fault".into())
            })
            .unwrap_err()
    });
    host.exchange_camera(install()).unwrap().unwrap();
    let prior = session.world.pause.bytes[3];
    // Fixture domain is explicit; no production state clearing to activate.
    assert_eq!(prior, 0, "fixture requires unoccupied reason3");
    let error = host
        .exchange_camera_with_session(&mut session, central())
        .unwrap_err();
    assert_eq!(error, RequestError::Failed("late supplied UI fault".into()));
    assert_eq!(session.world.pause.bytes[3], 1);
    assert_eq!(session.world.pause.bytes[8], 1);
    assert!(
        session
            .hooks
            .timers
            .iter()
            .filter(|t| u32::from_le_bytes(t[4..8].try_into().unwrap()) & 2 != 0)
            .all(|t| t[0] == 0)
    );
    assert_eq!(
        host.exchange_camera_with_session(&mut session, central()),
        Err(error)
    );
    assert_eq!(ui.join().unwrap(), "late supplied UI fault");
}
