//! Real pool, production apply_resident and existing operation-wire regressions.
//! Synthetic epoch is explicitly fixture-only. No real native callback success.
use super::super::camera_bridge::{
    self as bridge, PropertyEpoch, ResidentRead as R, ResidentSource,
};
use super::*;
use openmt2001::{
    economy::ResidentAccounts,
    resident::{AuxiliaryPool, Resident},
};
use std::{thread, time::Duration};
const WAIT: Duration = Duration::from_secs(10);
fn state() -> (ResidentAccounts, AuxiliaryPool, ResidentSource) {
    let mut residents = ResidentAccounts::new_pool();
    residents.active_count = 2;
    let mut payload = [0; 330];
    payload[0x7e] = 0xa4;
    residents.nodes[1] = Resident::from_runtime_bytes(payload, 1, 99);
    let mut bytes = [0; 284];
    bytes[0x30..0x32].copy_from_slice(&2i16.to_le_bytes());
    bytes[0x38..0x3c].copy_from_slice(&777u32.to_le_bytes());
    let pool = AuxiliaryPool::from_slots(vec![bytes, [0xab; 284], [0xcd; 284]], 2).unwrap();
    let source = ResidentSource::synthetic(901, &pool);
    (residents, pool, source)
}
fn apply_read(
    residents: &ResidentAccounts,
    pool: &AuxiliaryPool,
    source: &ResidentSource,
    read: R,
) -> Result<SessionReply, RequestError> {
    apply_resident(PropertyEpoch::synthetic(901), residents, pool, source, read)
}
#[test]
fn real_handler_reads_prefix_and_direct_auxiliary_index_without_invented_mapping() {
    let (residents, pool, source) = state();
    assert_eq!(
        apply_read(&residents, &pool, &source, R::Count7a18ec).unwrap(),
        SessionReply::Word(2)
    );
    assert_eq!(
        apply_read(&residents, &pool, &source, R::Flags336(1)).unwrap(),
        SessionReply::Byte(0xa4)
    );
    let SessionReply::ResidentBinding(binding) =
        apply_read(&residents, &pool, &source, R::Lookup6899a0(0)).unwrap()
    else {
        panic!("binding")
    };
    assert_eq!(
        apply_read(&residents, &pool, &source, R::Index38(binding.clone())).unwrap(),
        SessionReply::Word(777)
    );
    assert_eq!(
        apply_read(&residents, &pool, &source, R::Kind30(binding)).unwrap(),
        SessionReply::Word(2)
    );
    for bad in [-1, 2, 777] {
        assert!(apply_read(&residents, &pool, &source, R::Lookup6899a0(bad)).is_err());
    }
    for bad in [-1, 2, 777] {
        assert!(apply_read(&residents, &pool, &source, R::Flags336(bad)).is_err());
    }
}
#[test]
fn fresh_actual_kind_and_index_reads_follow_mutation_and_reject_cross_origin() {
    let (residents, mut pool, source) = state();
    let binding = pool.bind(0).unwrap();
    pool.get_mut(0).unwrap()[0x30..0x32].copy_from_slice(&(-3i16).to_le_bytes());
    pool.get_mut(0).unwrap()[0x38..0x3c].copy_from_slice(&4u32.to_le_bytes());
    assert_eq!(
        apply_read(&residents, &pool, &source, R::Kind30(binding.clone())).unwrap(),
        SessionReply::Word((-3i32) as u32)
    );
    assert_eq!(
        apply_read(&residents, &pool, &source, R::Index38(binding.clone())).unwrap(),
        SessionReply::Word(4)
    );
    let other = AuxiliaryPool::from_slots(pool.slots().to_vec(), 2).unwrap();
    assert!(apply_read(&residents, &other, &source, R::Kind30(binding.clone())).is_err());
    let wrong = ResidentSource::synthetic(902, &pool);
    assert!(apply_read(&residents, &pool, &wrong, R::Kind30(binding.clone())).is_err());
    pool.replace_with(AuxiliaryPool::from_slots(other.slots().to_vec(), 2).unwrap());
    assert!(apply_read(&residents, &pool, &source, R::Validate(binding)).is_err());
}
#[test]
fn actual_operation_rereads_after_callback_and_missing481a50_keeps_prior_pause() {
    use bridge::{Admission, Operation, PauseReason, SessionCall, SessionReply};
    let (mut residents, mut pool, source) = state();
    let sent_source = source.clone();
    let (mut worker, mut inbox) = bridge::channel(901);
    let ui = thread::spawn(move || {
        // Transport-only Install: not claiming a camera/native callback completed.
        inbox
            .wait_service(WAIT, |_| Ok(super::tests::observation()))
            .unwrap();
        inbox
            .wait_service_with_handler(WAIT, |_, port| {
                let resident = |read| SessionCall::Resident {
                    source: sent_source.clone(),
                    read,
                };
                let SessionReply::ResidentBinding(binding) =
                    port.call(resident(R::Lookup6899a0(0))).unwrap()
                else {
                    panic!("binding")
                };
                assert_eq!(
                    port.call(resident(R::Kind30(binding.clone()))).unwrap(),
                    SessionReply::Word(2)
                );
                assert_eq!(
                    port.call(resident(R::Count7a18ec)).unwrap(),
                    SessionReply::Word(2)
                );
                assert_eq!(
                    port.call(resident(R::Flags336(1))).unwrap(),
                    SessionReply::Byte(0xa4)
                );
                port.pause4bac90(PauseReason::Action3).unwrap();
                assert_eq!(
                    port.call(resident(R::Count7a18ec)).unwrap(),
                    SessionReply::Word(1)
                );
                assert_eq!(
                    port.call(resident(R::Flags336(0))).unwrap(),
                    SessionReply::Byte(0xf0)
                );
                assert_eq!(
                    port.call(resident(R::Kind30(binding.clone()))).unwrap(),
                    SessionReply::Word(1)
                );
                assert_eq!(
                    port.call(resident(R::Index38(binding.clone()))).unwrap(),
                    SessionReply::Word(4)
                );
                let error = port
                    .call(resident(R::Participant481a50(binding)))
                    .unwrap_err();
                assert!(error.to_string().contains("reached481a50"));
                // Even a swallowed callback failure defeats attempted completion.
                Ok(super::tests::observation())
            })
            .unwrap_err()
    });
    worker.request(super::tests::install()).unwrap();
    let mut pause = openmt2001::pause::State::default();
    let mut timers = [];
    let catalog = Default::default();
    let mut calls = 0;
    let error = worker
        .request_with_handler(
            Operation::CentralTail {
                admission: Admission::default(),
            },
            |call| {
                calls += 1;
                match call {
                    SessionCall::Resident { source, read } => {
                        apply_read(&residents, &pool, &source, read)
                    }
                    other => {
                        let reply = apply(&mut pause, &mut timers, &catalog, other)?;
                        // Deterministic callback writer on this production handler's
                        // SAME borrowed source records; not an independent roster.
                        residents.active_count = 1;
                        let mut bytes = [0; 330];
                        bytes[0x7e] = 0xf0;
                        residents.nodes[0] = Resident::from_runtime_bytes(bytes, 1, 0);
                        pool.get_mut(0).unwrap()[0x30..0x32].copy_from_slice(&1i16.to_le_bytes());
                        pool.get_mut(0).unwrap()[0x38..0x3c].copy_from_slice(&4u32.to_le_bytes());
                        Ok(reply)
                    }
                }
            },
        )
        .unwrap_err();
    assert_eq!(pause.bytes[3], 1);
    assert_eq!(pause.bytes[8], 1);
    assert_eq!(calls, 10);
    assert!(error.to_string().contains("reached481a50"));
    assert!(ui.join().unwrap().contains("reached481a50"));
}

#[test]
#[ignore = "requires supplied original Session assets"]
fn actual_session_world_capture_handler_and_replacement_are_live() {
    use crate::{
        fresh_city::{self, Inputs, Policy},
        live_host::Host,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    let path = |key: &str| {
        PathBuf::from(std::env::var_os(key).unwrap_or_else(|| panic!("required {key}")))
    };
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

    // Isolate the read fixture using actual Session source storage. This is
    // a controlled writer, not a claim about Scenario3's startup population.
    session.world.ledgers.residents.active_count = 2;
    let mut initial = [0; 330];
    initial[0x7e] = 0xa4;
    session.world.ledgers.residents.nodes[0] = Resident::from_runtime_bytes(initial, 1, 99);
    // Production Host -> Worker -> SAME Session handler, twice around actual
    // source writers. Reuse one captured weak source, not a new read snapshot.
    let source = ResidentSource::capture(&session);
    let allocated = session
        .world
        .creation
        .auxiliary
        .allocate(&[0; 284])
        .unwrap();
    let binding = session.world.creation.auxiliary.bind(allocated).unwrap();
    let expected = session.world.ledgers.residents.active_count;
    let index = 0;
    let first_flag = session.world.ledgers.residents.nodes[index].as_bytes()[0x7e];
    let ui_source = source.clone();
    let ui_binding = binding.clone();
    let (worker, mut inbox) = bridge::channel(source.epoch().serial());
    let mut host = Host::new([0.; 3]);
    host.camera_bridge = Some(worker);
    let ui = thread::spawn(move || {
        inbox
            .wait_service(WAIT, |_| Ok(super::tests::observation()))
            .unwrap();
        for stage in 0..2 {
            inbox
                .wait_service_with_handler(WAIT, |_, services| {
                    let resident = |read| SessionCall::Resident {
                        source: ui_source.clone(),
                        read,
                    };
                    assert_eq!(
                        services.call(resident(R::Count7a18ec)).unwrap(),
                        SessionReply::Word(if stage == 0 { expected } else { 1 })
                    );
                    assert_eq!(
                        services.call(resident(R::Flags336(0))).unwrap(),
                        SessionReply::Byte(if stage == 0 { first_flag } else { 0xd2 })
                    );
                    assert_eq!(
                        services
                            .call(resident(R::Kind30(ui_binding.clone())))
                            .unwrap(),
                        SessionReply::Word(if stage == 0 { 0 } else { 2 })
                    );
                    assert_eq!(
                        services
                            .call(resident(R::Index38(ui_binding.clone())))
                            .unwrap(),
                        SessionReply::Word(if stage == 0 { 0 } else { 777 })
                    );
                    Ok(super::tests::observation())
                })
                .unwrap();
        }
        // Replacement must fail through actual Host before any descriptor read.
        inbox
            .wait_service_with_handler(WAIT, |_, services| {
                services
                    .call(SessionCall::Resident {
                        source: ui_source,
                        read: R::Kind30(ui_binding),
                    })
                    .map_err(|e| e.to_string())?;
                panic!("retired pool must not produce a successful resident reply")
            })
            .unwrap_err()
    });
    host.exchange_camera(super::tests::install()).unwrap();
    let central = || bridge::Operation::CentralTail {
        admission: bridge::Admission::default(),
    };
    host.exchange_camera_with_session(&mut session, central())
        .unwrap();
    session.world.ledgers.residents.active_count = 1;
    let mut bytes = *session.world.ledgers.residents.nodes[index].as_bytes();
    bytes[0x7e] = 0xd2;
    session.world.ledgers.residents.nodes[index] = Resident::from_runtime_bytes(bytes, 1, 99);
    let record = session.world.creation.auxiliary.get_mut(allocated).unwrap();
    record[0x30..0x32].copy_from_slice(&2i16.to_le_bytes());
    record[0x38..0x3c].copy_from_slice(&777u32.to_le_bytes());
    host.exchange_camera_with_session(&mut session, central())
        .unwrap();
    assert_eq!(source, ResidentSource::capture(&session));
    session.world.creation.auxiliary.reset_for_residents();
    assert!(
        host.exchange_camera_with_session(&mut session, central())
            .is_err()
    );
    assert!(ui.join().unwrap().contains("retired/replaced"));
    assert!(binding.validate_live().is_err());
    let fresh = ResidentSource::capture(&session);
    assert_ne!(source, fresh);
    assert_eq!(
        handle(
            &mut session,
            SessionCall::Resident {
                source: fresh,
                read: R::Count7a18ec
            }
        )
        .unwrap(),
        SessionReply::Word(1)
    );
}

use crate::intro::camera_live::{
    chat_ring::Ring,
    continuation as c,
    continuation_bridge::SessionPort,
    navigation::Group,
    owner::Owner,
    ui_selection::{SelectionKey, SelectionState},
};
struct Cycle<'a, 'operation> {
    owner: Owner,
    port: SessionPort<'a, 'operation>,
    trace: Vec<&'static str>,
}
// Only cycle methods are reachable in test_resident_cycle. Unsupported methods
// explicitly fail; no no-op/successful model, input or camera callbacks exist.
impl c::Source for Cycle<'_, '_> {
    fn input(&self) -> c::Result<&openmt2001::native_input::StagedInput> {
        Err("unused input".into())
    }
    fn input_mut(&mut self) -> c::Result<&mut openmt2001::native_input::StagedInput> {
        Err("unused input".into())
    }
    fn byte(&mut self, _: c::Byte) -> c::Result<u8> {
        Err("unused byte".into())
    }
    fn word(&mut self, word: c::Word) -> c::Result<u32> {
        if word != c::Word::ParticipantCount7a18ec {
            return Err("unused word".into());
        }
        self.port.resident_count(&self.owner)
    }
    fn write_word(&mut self, _: c::Word, _: u32) -> c::Result<()> {
        Err("unused store".into())
    }
    fn write_quit93442c(&mut self) -> c::Result<()> {
        Err("unused quit".into())
    }
    fn busy48dbf0(&mut self) -> c::Result<u8> {
        Err("unused busy".into())
    }
    fn resolve_receiver8b2278(&mut self) -> c::Result<c::Receiver> {
        Err("unused receiver".into())
    }
    fn live_device_input(
        &mut self,
        _: c::Receiver,
        _: c::Poll,
    ) -> c::Result<(
        &mut openmt2001::native_input::StagedInput,
        c::LiveDevice<'_>,
    )> {
        Err("unused device".into())
    }
    fn recording_after_device(&mut self, _: c::Receiver, _: c::Poll) -> c::Result<u8> {
        Err("unused recording".into())
    }
    fn secondary_enabled(&mut self) -> c::Result<u8> {
        Err("unused secondary".into())
    }
    fn chat_ring68b4c0(&mut self) -> c::Result<&mut Ring> {
        Err("unused chat".into())
    }
    fn active43c5c0(&mut self, _: Group) -> c::Result<u8> {
        Err("unused activity".into())
    }
    fn mode4b52f0(&mut self) -> c::Result<i32> {
        Err("unused mode".into())
    }
    fn current822d50(&mut self) -> c::Result<Option<SelectionKey>> {
        Ok(self.owner.ui_selection()?.current.map(|v| v.key))
    }
    fn selected_kind30(&mut self, key: SelectionKey) -> c::Result<i16> {
        self.port.resident_kind(&self.owner, key)
    }
    fn participant_player38(&mut self, key: SelectionKey) -> c::Result<u32> {
        self.port.resident_index(&self.owner, key)
    }
    fn participant_flags336(&mut self, index: i32) -> c::Result<u8> {
        self.port.resident_flags(&self.owner, index)
    }
    fn participant6899a0(&mut self, index: i32) -> c::Result<c::ParticipantDescriptor> {
        self.port.resident_lookup(&mut self.owner, index)
    }
    fn publish_current822d50(&mut self, value: c::ParticipantDescriptor) -> c::Result<()> {
        self.port.resident_validate(&self.owner, value)?;
        self.owner.ui_selection_mut()?.current = Some(value.selected());
        self.trace.push("publish822d50");
        Ok(())
    }
    fn transient823168_present(&mut self) -> c::Result<bool> {
        Ok(self.owner.ui_selection()?.transient.is_some())
    }
    fn effect(&mut self, effect: c::Effect) -> c::Result<()> {
        match effect {
            c::Effect::Participant481a50(value) => {
                self.trace.push("481a50");
                self.port.resident_effect(&self.owner, value)
            }
            c::Effect::SetKind4e06c0(_) => {
                self.trace.push("kind9");
                Err("must not reach kind9 before actual481a50 completes".into())
            }
            c::Effect::FreshA41f290Mode5 => {
                self.trace.push("freshA5");
                Err("must not reach freshA5 before actual481a50 completes".into())
            }
            _ => Err("unused effect".into()),
        }
    }
}
#[test]
fn real_cycle_uses_live_descriptor_not_facade_and_retains_current_at_missing481a50() {
    let (residents, mut pool, source) = state();
    pool.get_mut(0).unwrap()[0x38..0x3c].copy_from_slice(&0u32.to_le_bytes());
    let source_for_ui = source.clone();
    let binding = pool.bind(0).unwrap();
    let (mut worker, mut inbox) = bridge::channel(901);
    let ui = thread::spawn(move || {
        inbox
            .wait_service(WAIT, |_| Ok(super::tests::observation()))
            .unwrap();
        let mut owner = Owner::new(openmt2001::camera::bank::Bank::loader_zero_storage(), 0xa5);
        owner
            .install_ui_selection(SelectionState::installed(
                None, None, None, None, None, None, -1, -1,
            ))
            .unwrap();
        owner.install_resident_source(&source_for_ui, 901).unwrap();
        // Wrong copied facade must not suppress the actual kind1/2 branch or
        // replace the actual+38 word0 with facade category999.
        let prior = owner
            .resident_descriptors_mut()
            .unwrap()
            .selected(binding, 99, 999)
            .unwrap()
            .selected();
        owner.ui_selection_mut().unwrap().current = Some(prior);
        owner.ui_selection_mut().unwrap().published = Some(prior);
        owner.ui_selection_mut().unwrap().transient = Some(prior);
        let mut reached = None;
        let error = inbox
            .wait_service_with_handler(WAIT, |_, services| {
                let mut cycle = Cycle {
                    owner,
                    port: SessionPort::new(services),
                    trace: vec![],
                };
                let before = cycle.owner.kind();
                let error = c::test_resident_cycle(&mut cycle).unwrap_err();
                assert!(error.contains("reached481a50"));
                assert_eq!(cycle.trace, vec!["publish822d50", "481a50"]);
                let current = cycle.owner.current_ui_selected().unwrap().unwrap();
                assert_ne!(current.key, prior.key);
                assert_eq!(cycle.owner.kind(), before);
                assert_eq!(cycle.owner.published_ui_selected().unwrap(), Some(prior));
                assert_eq!(cycle.owner.transient_ui_selected().unwrap(), Some(prior));
                reached = Some(current);
                Err(error)
            })
            .unwrap_err();
        assert!(reached.is_some());
        error
    });
    worker.request(super::tests::install()).unwrap();
    let mut flags = vec![];
    let error = worker
        .request_with_handler(
            bridge::Operation::CentralTail {
                admission: bridge::Admission::default(),
            },
            |call| {
                let SessionCall::Resident { source, read } = call else {
                    panic!("only actual resident reads expected")
                };
                if let R::Flags336(index) = &read {
                    flags.push(*index);
                }
                apply_read(&residents, &pool, &source, read)
            },
        )
        .unwrap_err();
    assert_eq!(flags, vec![1]);
    assert!(error.to_string().contains("reached481a50"));
    assert!(ui.join().unwrap().contains("reached481a50"));
}
