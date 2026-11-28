//! Era-audio tests that run the Session tick through the live Host. They build
//! with the game binary; era_audio itself is in crates/frontend (#99).
use crate::era_audio::test_player as player;
use crate::era_audio::*;
use openmt2001::pe::PeImage;
use openmt2001::{clock, hourly, level_runtime::HourlyContext, scheduler};
use std::sync::mpsc;

// Synthetic PE data supplies an empty board with one road. No original assets
// or audio device are needed to exercise the Session tick's hourly adapter.
fn board_image() -> Vec<u8> {
    let mut bytes = vec![0; 0x140000];
    let put = |bytes: &mut [u8], at, value: u32| {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    };
    bytes[..2].copy_from_slice(b"MZ");
    put(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut bytes, 0xb4, 0x400000);
    // One section maps virtual addresses to the same file offsets minus base.
    put(&mut bytes, 0xc4, 0x1000);
    put(&mut bytes, 0xc8, 0x13f000);
    put(&mut bytes, 0xcc, 0x1000);
    for y in 1..6 {
        for x in 1..5 {
            bytes[0x1366b8 + y * 113 + x] = b'.';
        }
    }
    bytes
}

#[test]
fn session_tick_pre_six_adapter_releases_each_morning_through_era_audio_requests() {
    struct City;
    impl hourly::World for City {
        fn players(&self) -> &[openmt2001::economy::PlayerAccount] {
            &[]
        }
    }
    let bytes = board_image();
    let image = PeImage::parse(&bytes).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let mut host = crate::live_host::Host::new([0., 20., 0.]);
    let (sender, receiver) = mpsc::channel::<Request>();
    host.era_audio = Some(sender);
    let worker = std::thread::spawn(move || {
        let (mut player, _output) = player();
        let mut operations = Vec::new();
        for request in receiver {
            operations.push(request.operation);
            // Unlike tolerant Player::apply, this assertion fails if the
            // previous morning was not released at its hourly boundary.
            if request.operation == Operation::BeforeSixCreate {
                assert!(player.before_six.is_none());
            }
            let result = player.apply(request.operation);
            request.reply.send(result).unwrap();
        }
        assert!(player.before_six.is_none());
        operations
    });
    let mut clock = clock::Clock {
        calendar: [5, 0, 0, 5, 0],
        timing: [3664, 3664, 539, 3664, 0, 1],
    };
    let mut utilities = openmt2001::economy::Utilities::new([0; 4]);
    let mut context = HourlyContext {
        board: &board,
        calendar: host.policy.calendar,
        hourly: host.policy.hourly,
        utilities: &mut utilities,
    };
    context.calendar.autosave_disabled = true;
    let mut creates = Vec::new();
    let mut releases = Vec::new();
    // Continuous scheduler/clock advancement through both morning boundaries.
    for _ in 0..15_086 {
        scheduler::advance(
            &mut clock,
            &mut host.policy.scheduler,
            |event, clock, scheduler| {
                match event {
                    scheduler::Event::BeforeSixSound => {
                        assert_eq!((clock.hour(), clock.timing[2]), (5, 540));
                        let handle = request(&host.era_audio, Operation::BeforeSixCreate)?;
                        assert_ne!(handle, 0);
                        creates.push(handle);
                        return Ok(handle);
                    }
                    scheduler::Event::AdvanceHour => {
                        openmt2001::session::tick::advance_scheduled_hour(
                            scheduler,
                            &mut context,
                            |_, context| {
                                clock.try_advance_hour(&mut context.calendar, |event, clock, _| {
                                    if event == clock::Event::Hourly {
                                        hourly::advance_world(
                                            clock,
                                            &mut context.hourly,
                                            &mut City,
                                            |event, clock, _, _| {
                                                if let hourly::Event::StopSound(handle) = event {
                                                    assert_eq!(
                                                        (clock.hour(), clock.timing[2]),
                                                        (6, 0)
                                                    );
                                                    request(
                                                        &host.era_audio,
                                                        Operation::BeforeSixRelease(handle),
                                                    )?;
                                                    releases.push(handle);
                                                }
                                                Ok(false)
                                            },
                                        )?;
                                    }
                                    Ok(false)
                                })
                            },
                        )?;
                    }
                    _ => {}
                }
                Ok(0)
            },
        )
        .unwrap();
    }
    assert_eq!(creates.len(), 2);
    assert_eq!(releases, creates);
    assert_eq!(clock.calendar[0], 6);
    assert_eq!(clock.calendar[4], 1);
    assert_eq!(host.policy.scheduler.before_six_sound, 0);
    assert_eq!(context.hourly.before_six_sound, 0);
    host.policy.scheduler.before_six_sound = 99;
    let failure = openmt2001::session::tick::advance_scheduled_hour(
        &mut host.policy.scheduler,
        &mut context,
        |_, context| {
            assert_eq!(context.hourly.before_six_sound, 99);
            context.hourly.before_six_sound = 0;
            Err("failure after releasing the cue".into())
        },
    );
    assert_eq!(failure.unwrap_err(), "failure after releasing the cue");
    assert_eq!(host.policy.scheduler.before_six_sound, 0);
    drop(host);
    assert_eq!(
        worker.join().unwrap(),
        [
            Operation::BeforeSixCreate,
            Operation::BeforeSixRelease(creates[0]),
            Operation::BeforeSixCreate,
            Operation::BeforeSixRelease(creates[1])
        ]
    );
}

#[test]
#[ignore = "requires supplied original city assets; actual Session and Host tick with synthetic PCM"]
fn session_keeps_running_across_two_pre_six_mornings_with_live_host_and_era_audio() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(session_mornings)
        .unwrap()
        .join()
        .unwrap();
}

fn session_mornings() {
    use openmt2001::session::{self as fresh_city, CommandAssets, Inputs, Policy, Session};
    let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let build = read("BUILD_FILE");
    let parameters = read("PARAMETERS");
    let timetable = read("TIMETABLE");
    let image = PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
    let exe = openmt2001::fixtures::path("EXE");
    let names = fresh_city::character_names(
        &image,
        &exe.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa"),
    )
    .unwrap();
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
            road_routes: read("ROAD_ROUTE_TABLE"),
            scripts: default.parent().unwrap().parent().unwrap(),
            parameters: std::str::from_utf8(&parameters).unwrap(),
            era_styles: fresh_city::load_era_styles(
                openmt2001::fixtures::path("PARAMETERS").parent().unwrap(),
            )
            .unwrap(),
            timetable: std::str::from_utf8(&timetable).unwrap(),
            scenario: "SCENARIO1",
            difficulty: 0,
            selected_character: 4,
            profile_name: b"Morning audio regression",
            character_names: &names,
        },
        Policy {
            seed: 0x12345678,
            auxiliary_prior_bytes: [0; 284],
        },
    )
    .unwrap();
    let assets = CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    let mut host = crate::live_host::Host::new([0., 20., 0.]);
    let (sender, receiver) = mpsc::channel::<Request>();
    host.era_audio = Some(sender);
    let worker = std::thread::spawn(move || {
        let (mut player, _output) = player();
        let mut operations = Vec::new();
        for request in receiver {
            operations.push(request.operation);
            match request.operation {
                Operation::BeforeSixCreate => assert!(player.before_six.is_none()),
                Operation::BeforeSixRelease(handle) => {
                    assert_eq!(player.before_six.as_ref().unwrap().0, handle)
                }
                other => panic!("unexpected morning audio operation {other:?}"),
            }
            request.reply.send(player.apply(request.operation)).unwrap();
        }
        assert!(player.before_six.is_none());
        operations
    });
    host.set_running(true);
    let mut handles = Vec::new();
    for day in 0..2 {
        // Controlled clock placement excludes unrelated daytime AI/modal work.
        // Every tick from 05:51 through 06:00 uses actual Session/Host dispatch.
        session.world.clock.calendar = [5, day, 0, day * 24 + 5, day];
        session.world.clock.timing = [day * 15_000 + 3664, day * 15_000 + 3664, 539, 3664, 0, 1];
        session.world.pause.bytes = [0; 9];
        for tick in 0..86 {
            host.tick(&mut session, &assets);
            assert!(host.failure.is_none(), "{:?}", host.failure);
            assert!(host.running);
            assert!(!session.world.pause.has_reason());
            assert_eq!(session.world.clock.timing[0], day * 15_000 + 3665 + tick);
            assert_eq!(
                host.policy.scheduler.before_six_sound,
                host.policy.hourly.before_six_sound
            );
            if tick == 0 {
                assert_ne!(host.policy.scheduler.before_six_sound, 0);
                handles.push(host.policy.scheduler.before_six_sound);
            } else if tick < 85 {
                assert_eq!(
                    host.policy.scheduler.before_six_sound,
                    handles[day as usize]
                );
            }
        }
        assert_eq!(
            (session.world.clock.hour(), session.world.clock.timing[2]),
            (6, 0)
        );
        assert_eq!(host.policy.scheduler.before_six_sound, 0);
        assert_eq!(host.policy.hourly.before_six_sound, 0);
    }
    drop(host);
    assert_eq!(
        worker.join().unwrap(),
        [
            Operation::BeforeSixCreate,
            Operation::BeforeSixRelease(handles[0]),
            Operation::BeforeSixCreate,
            Operation::BeforeSixRelease(handles[1])
        ]
    );
}
