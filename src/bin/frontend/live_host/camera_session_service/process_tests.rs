//! Existing supplied-data setup and actual source joins.
use super::*;
use crate::live_host::camera_bridge::{self as bridge, Admission, Observation, Operation, Pose};
use std::{thread, time::Duration};
const WAIT: Duration = Duration::from_secs(10);
fn observation() -> Observation {
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
fn install() -> Operation {
    Operation::Install {
        property: None,
        admission: Admission::default(),
        capture: openmt2001::camera::bank::Capture::from_records([[0; 196]; 2]),
    }
}
#[test]
#[ignore = "requires supplied original Session and Intro assets"]
fn actual_assets_startup_retained_handle_and_host_service_writers() {
    use crate::{
        fresh_city::{self, Inputs, Policy},
        live_host::Host,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    let path = |key: &str| PathBuf::from(openmt2001::fixtures::path(key));
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
    let (worker, mut inbox) = bridge::channel(812);
    let mut host = Host::new([0.; 3]);
    host.camera_bridge = Some(worker);
    let (source_tx, source_rx) = std::sync::mpsc::channel();
    let ui = thread::spawn(move || {
        let path = |key: &str| PathBuf::from(openmt2001::fixtures::path(key));
        let mut assets = crate::intro::Assets::load_with_source_options(
            crate::intro::Paths {
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
            },
            "DEBUGSTUFF TESTMODE",
        )
        .unwrap();
        source_tx.send(assets.process_runtime.clone()).unwrap();
        assets.activity.store_a4(7);
        assets.activity.store_a8(8);
        inbox.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox
            .wait_service_with_handler(WAIT, |_, services| {
                assert!(
                    assets.process_runtime.mutex_available(),
                    "no source lock across Host/UI rendezvous"
                );
                let fonts = assets.ui_text.fonts_mut();
                let fonts_identity = fonts as *const _;
                let mut port =
                    crate::intro::camera_live::continuation_bridge::SessionPort::new(services);
                use crate::intro::camera_live::continuation::{Byte, Word};
                assert_eq!(port.byte(Byte::Debug8b23c5).unwrap(), 1);
                assert_eq!(port.byte(Byte::Test8b23bc).unwrap(), 1);
                let count = port.word(Word::Count822d38).unwrap();
                port.increment_count4ba580().unwrap();
                assert_eq!(port.word(Word::Count822d38).unwrap(), count.wrapping_add(1));
                assert_eq!(
                    port.word(Word::Scene822d30).unwrap(),
                    1,
                    "same source clock word"
                );
                port.quit93442c().unwrap();
                assert_eq!(fonts as *const _, fonts_identity);
                Err("fault after committed process quit".into())
            })
            .unwrap_err();
        assert_eq!((assets.activity.a4(), assets.activity.a8()), (7, 8));
        assets.process_runtime
    });
    let retained = source_rx.recv_timeout(WAIT).unwrap();
    host.process_runtime = Some(retained.clone());
    host.camera_suppliers = Some(crate::live_host::CameraSuppliers::CameraOnly);
    host.exchange_camera(install()).unwrap();
    assert!(
        host.camera_central_tail(&mut session)
            .unwrap_err()
            .to_string()
            .contains("committed process quit")
    );
    assert_eq!(retained.quit_requested().unwrap(), 1);
    assert!(!retained.exit_ready().unwrap());
    assert_eq!(retained.count822d38().unwrap(), 1);
    let ui_retained = ui.join().unwrap();
    assert_eq!(
        ui_retained
            .byte(super::super::process_runtime::Byte::Debug8b23c5)
            .unwrap(),
        1
    );
    assert!(retained.mutex_available());
    //A real replacement Host/Worker completes another CameraOnly central on
    //the retained process handle. It cannot authorize A's failed quit tail.
    let (worker_b, mut inbox_b) = bridge::channel(814);
    let mut host_b = Host::new([0.; 3]);
    host_b.process_runtime = Some(retained.clone());
    host_b.camera_bridge = Some(worker_b);
    host_b.camera_suppliers = Some(crate::live_host::CameraSuppliers::CameraOnly);
    let ui_b = thread::spawn(move || {
        inbox_b.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox_b.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox_b.retire();
    });
    host_b.exchange_camera(install()).unwrap();
    host_b.camera_central_tail(&mut session).unwrap();
    ui_b.join().unwrap();
    assert!(!retained.exit_ready().unwrap(), "B cannot satisfy failed A");
    retained
        .request_quit(super::super::process_runtime::QuitWriter::Menu)
        .unwrap();
    assert!(
        !retained.exit_ready().unwrap(),
        "outer request cannot forge A completion"
    );
    //Real registered Lua functions -> real event dispatcher/buffer -> actual
    //Host consumer, with both Assets-bound and legacy handle configurations.
    session
        .scripts
        .runtime
        .execute(
            "picking regression event definitions",
            br#"
        function CATCHEVENT_PickingRegression(a)
          if a == 0 then Interface.EnableBlockPicking(8)
          elseif a == 1 or a == 4 then Interface.DisableAll3DPicking()
          elseif a == 2 or a == 5 or a == 7 then Interface.EnableAll3DPicking()
          elseif a == 3 then Interface.EnableBuildingPicking(7)
          elseif a == 6 then Interface.DisableCameraControl() end
        end
    "#,
            &mut |_, _| Err("unexpected callback during function definition".into()),
        )
        .unwrap();
    let command_assets = crate::fresh_city::CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    for bound in [false, true] {
        let mut picks = Host::new([0.; 3]);
        if bound {
            picks.process_runtime = Some(retained.clone());
        }
        picks.commands_seen = session.hooks.commands.len();
        for stage in 0..8 {
            let before = session.hooks.commands.len();
            session
                .dispatch_session_event("PickingRegression", stage, &command_assets, false)
                .unwrap();
            assert!(session.hooks.commands.len() > before);
            picks.collect_script_prompts(&session);
            assert!(picks.failure.is_none());
            let disabled = stage == 1 || stage == 4;
            assert_eq!(picks.tutorial_controls.picking_disabled, disabled);
            let pick = if stage < 3 {
                crate::tutorial_two_interaction::Pick::Block { category_id: 8 }
            } else {
                crate::tutorial_two_interaction::Pick::Building { slot: 7 }
            };
            assert_eq!(
                picks.tutorial_pick_callback(pick),
                if disabled {
                    None
                } else {
                    Some(if stage < 3 { 0 } else { 2 })
                }
            );
            assert!(
                picks.tutorial_interaction.has_pick_gate(),
                "retained policy survives common disable"
            );
            assert_eq!(picks.tutorial_controls.camera_disabled, stage >= 6);
            if bound {
                let x = retained.picking_projection().unwrap();
                assert_eq!(x.common, u8::from(!disabled));
                assert_eq!(x.camera, u8::from(stage < 6));
                if stage < 3 {
                    assert_eq!((x.block, x.block_id), (1, 8));
                } else {
                    assert_eq!((x.building, x.building_id), (1, 7));
                }
            }
        }
    }
    //A distinct successful operation really completes its own quit tail,
    //even when the later protected finalizer fails. Its timers commit first.
    let completed_process = super::super::process_runtime::startup(&executable, "")
        .unwrap()
        .0;
    let (worker_c, mut inbox_c) = bridge::channel(815);
    let mut host_c = Host::new([0.; 3]);
    host_c.process_runtime = Some(completed_process.clone());
    host_c.camera_bridge = Some(worker_c);
    host_c.camera_suppliers = Some(crate::live_host::CameraSuppliers::CameraOnly);
    let ui_c = thread::spawn(move || {
        inbox_c.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox_c
            .wait_service_with_handler(WAIT, |_, services| {
                services.call(SessionCall::Quit93442c).unwrap();
                let mut done = observation();
                done.finalization_fault = Some("late protected finalizer".into());
                done.task_effects.push(bridge::TaskEffect::SetEnabled {
                    mask: 2,
                    enabled: false,
                });
                Ok(done)
            })
            .map_err(|error| assert_eq!(error, "late protected finalizer"))
            .unwrap_err();
        inbox_c.retire();
    });
    let mut selected = 0;
    for timer in &mut session.hooks.timers {
        if u32::from_le_bytes(timer[4..8].try_into().unwrap()) & 2 != 0 {
            timer[0] = 1;
            selected += 1;
        }
    }
    assert!(selected > 0, "explicit enabled timer fixture");
    host_c.exchange_camera(install()).unwrap();
    assert!(
        host_c
            .camera_central_tail(&mut session)
            .unwrap_err()
            .to_string()
            .contains("late protected finalizer")
    );
    ui_c.join().unwrap();
    assert!(completed_process.exit_ready().unwrap());
    assert!(
        session
            .hooks
            .timers
            .iter()
            .filter(|t| u32::from_le_bytes(t[4..8].try_into().unwrap()) & 2 != 0)
            .all(|t| t[0] == 0)
    );
    //Real source sequence writer loans the actual Session to its callback;
    //no source mutex or sibling scene borrow prevents actual Lua/Host work.
    retained
        .start_sequence4b1800(&mut session, |session| {
            assert!(retained.mutex_available());
            assert_eq!(session.world.clock.timing[5], 3);
            assert_eq!(
                retained
                    .byte(super::super::process_runtime::Byte::Suppressed8b0700)
                    .unwrap(),
                0
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(
        retained
            .byte(super::super::process_runtime::Byte::Suppressed8b0700)
            .unwrap(),
        1
    );
    assert!(
        retained
            .end_sequence4b1eb0(|| Err(
                super::super::process_runtime::Error::SequenceCallbackUnavailable
            ))
            .is_err()
    );
    assert_eq!(
        retained
            .byte(super::super::process_runtime::Byte::Suppressed8b0700)
            .unwrap(),
        1
    );
    assert!(
        retained
            .start_sequence4b1800(&mut session, |_| Err(
                super::super::process_runtime::Error::SequenceCallbackUnavailable
            ))
            .is_err()
    );
    assert_eq!(
        retained
            .byte(super::super::process_runtime::Byte::Suppressed8b0700)
            .unwrap(),
        0
    );
    //Real Session containing timing helper, with an explicit failed mode join.
    let before_scene = session.world.clock.timing[5];
    let words = [11u32, 12, 13, 14, 3, 99];
    let payload = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    fn missing_mode(_session: &mut Session<'_>, _mode: i32) -> Result<(), String> {
        Err("missing real mode callback".into())
    }
    assert_ne!(session.roster.status.current_player, 0);
    assert!(
        session
            .restore_source_timing400(&payload, &retained, missing_mode)
            .unwrap_err()
            .contains("mode callback")
    );
    assert_eq!(&session.world.clock.timing[..4], &[11, 12, 13, 14]);
    assert_eq!(session.world.clock.timing[5], before_scene);
    assert!(retained.scene_available().is_err());
}

#[test]
fn checked_operation_process_reads_reread_same_owner_after_real_writer() {
    let process = super::super::process_runtime::test_fixture();
    let retained = process.clone();
    let (mut worker, mut inbox) = bridge::channel(813);
    let ui = thread::spawn(move || {
        inbox.wait_service(WAIT, |_| Ok(observation())).unwrap();
        inbox
            .wait_service_with_handler(WAIT, |_, services| {
                let mut port =
                    crate::intro::camera_live::continuation_bridge::SessionPort::new(services);
                use crate::intro::camera_live::continuation::{Byte, Word};
                assert_eq!(port.word(Word::Count822d38).unwrap(), 0);
                port.increment_count4ba580().unwrap();
                assert_eq!(port.word(Word::Count822d38).unwrap(), 1);
                retained
                    .picking_write(super::super::process_runtime::PickingWrite::DisableAll)
                    .unwrap();
                assert_eq!(port.byte(Byte::Controller920888).unwrap(), 0);
                retained
                    .picking_write(super::super::process_runtime::PickingWrite::EnableAll)
                    .unwrap();
                assert_eq!(port.byte(Byte::Controller920888).unwrap(), 1);
                port.quit93442c().unwrap();
                Err("stop after source effects".into())
            })
            .unwrap_err()
    });
    worker.request(install()).unwrap();
    let central = process
        .begin_central(worker.central_identity().unwrap())
        .unwrap();
    let mut calls = vec![];
    let scene = 2;
    let error = worker
        .request_with_handler(
            Operation::CentralTail {
                admission: Admission::default(),
            },
            |call| {
                assert!(process.mutex_available());
                calls.push(call.clone());
                apply_process(Some(&process), &scene, call)
            },
        )
        .unwrap_err();
    process.fail_central(central).unwrap();
    assert_eq!(calls.len(), 6);
    assert!(error.to_string().contains("stop after source effects"));
    assert_eq!(process.count822d38().unwrap(), 1);
    assert_eq!(process.quit_requested().unwrap(), 1);
    assert!(!process.exit_ready().unwrap());
    assert_eq!(ui.join().unwrap(), "stop after source effects");
}
#[test]
fn source_absence_and_unimplemented_families_are_not_known_zero() {
    let p = super::super::process_runtime::test_fixture();
    let scene = 1;
    assert!(
        apply_process(None, &scene, SessionCall::Count822d38)
            .unwrap_err()
            .to_string()
            .contains("Absent")
    );
    assert!(
        apply_process(
            Some(&p),
            &scene,
            SessionCall::ProcessByte(super::super::process_runtime::Byte::Media8fca98)
        )
        .is_err()
    );
    assert!(apply_process(Some(&p), &scene, SessionCall::Media4b9210).is_err());
    assert!(apply_process(Some(&p), &scene, SessionCall::Sequence4b1980).is_err());
    assert_eq!(
        apply_process(Some(&p), &scene, SessionCall::Scene822d30).unwrap(),
        SessionReply::Word(1)
    );
    p.source_load_unjoined().unwrap();
    assert!(apply_process(Some(&p), &scene, SessionCall::Scene822d30).is_err());
}

#[test]
fn performance_session_reader_keeps_signed_value_and_shared_identity() {
    use super::super::process_runtime;
    let p = process_runtime::test_fixture();
    assert!(apply_process(None, &0, SessionCall::Performance8b26cc).is_err());
    assert_eq!(
        apply_process(Some(&p), &0, SessionCall::Performance8b26cc).unwrap(),
        SessionReply::Signed(2)
    );
    p.set_performance8b26cc(-7).unwrap();
    assert_eq!(
        apply_process(Some(&p.clone()), &0, SessionCall::Performance8b26cc).unwrap(),
        SessionReply::Signed(-7)
    );
}
