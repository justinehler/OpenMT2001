use super::*;
use crate::{local_commands::Queue, scheduler::ClockDomain, session::restore::saved_command};

// Exercise the record builder used by Session save, the native writer and
// parser, and the command decoder and queue insertion used by Session restore,
// without requiring a city asset catalog.
fn round_trip(source: &Queue<Command>, auction: Option<&mut [u8; 344]>) -> Queue<Command> {
    let has_auction = auction.is_some();
    let snapshot = source.snapshot().unwrap();
    let mut headers = Vec::new();
    let mut packets = Vec::new();
    for (domain, outer, admission, direct, command) in &snapshot {
        assert!(!direct);
        let (header, packet) =
            scheduled_record(*domain, *outer, *admission, *direct, command).unwrap();
        headers.push(header);
        packets.push(packet);
    }
    let scheduled: Vec<_> = headers
        .iter()
        .zip(&packets)
        .map(|(header, packet)| writer::Scheduled {
            header,
            packet: Some(packet),
        })
        .collect();
    let countdown = 750;
    let mut categories = [[0; 380]; 52];
    let bytes = writer::encode(
        &mut writer::State {
            compatibility: &[0; 40],
            level: 0,
            difficulty: 0,
            current_cash: 0.,
            players: &[],
            businesses: &[],
            buildings: &[],
            building_base: 0,
            building_sentinel_next: 0,
            categories: &mut categories,
            residents: &[],
            cameras: &[[0; 196]; 2],
            calendar: [0; 5],
            timing: [0; 6],
            pending: &[0; 3588],
            resident_accounts: &[],
            scheduled: &scheduled,
            auction: auction.map(|record| writer::Auction { record, countdown }),
            objective: &[0; 60],
            business_info: &[[0; 116]; 45],
            commodities: &[[0; 28]; 32],
            history: &[0; 0x10f44],
            session_option_word: 0,
        },
        0xc000,
        [0; 4],
    )
    .unwrap();
    let save = SaveFile::parse(&bytes).unwrap();
    assert_eq!(save.auction().unwrap().is_some(), has_auction);
    if let Some(auction) = save.auction().unwrap() {
        assert_eq!(auction[0x7d], 1);
        assert_eq!(word(auction, 0x80), countdown);
    }
    let mut restored = Queue::default();
    for (index, event) in save.scheduled().unwrap().iter().enumerate() {
        let domain = match word(event.header, 4) {
            0 => ClockDomain::Total,
            1 => ClockDomain::Normal,
            _ => panic!("invalid clock selector"),
        };
        let bytes = event.packet.unwrap();
        assert_eq!(bytes, packets[index]);
        restored.restore_queued(
            saved_command(bytes).unwrap(),
            domain,
            word(event.header, 8),
            word(bytes, 12),
            false,
        );
    }
    let actual = restored.snapshot().unwrap();
    assert_eq!(actual.len(), snapshot.len());
    for (actual, expected) in actual.iter().zip(snapshot) {
        assert_eq!(
            matches!(actual.0, ClockDomain::Normal),
            matches!(expected.0, ClockDomain::Normal)
        );
        assert_eq!(
            (actual.1, actual.2, actual.3),
            (expected.1, expected.2, expected.3)
        );
    }
    restored
}

fn drain_at(queue: &mut Queue<Command>, due: u32, mut apply: impl FnMut(&Command)) {
    queue
        .drain(&mut [due - 1; 2], |_| {
            panic!("command fired before saved deadline")
        })
        .unwrap();
    let mut count = 0;
    queue
        .drain(&mut [due; 2], |command| {
            count += 1;
            apply(command);
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 1);
    assert!(queue.snapshot().unwrap().is_empty());
}

#[test]
fn outcome_notice_native_round_trip_preserves_winner_reason_and_due() {
    for player in [-1, 0, 3] {
        for reason in 0..=4 {
            let command = Command::OutcomeNotice { player, reason };
            let due = 1025;
            assert_eq!(
                packet(&command, due).unwrap(),
                words([0x525db8, 9, 28, due, 1, player as u32, reason])
            );
            let mut source = Queue::default();
            source
                .submit(command, 1000, true, |_| panic!("delayed command"))
                .unwrap();
            let mut restored = round_trip(&source, None);
            drain_at(&mut restored, due, |command| {
                assert!(
                    matches!(command, Command::OutcomeNotice { player: p, reason: r } if *p == player && *r == reason)
                );
            });
        }
    }
}

#[test]
fn elimination_native_round_trip_preserves_slot_and_due() {
    for slot in 0..6 {
        let command = Command::Eliminate { slot };
        let due = 2025;
        assert_eq!(
            packet(&command, due).unwrap(),
            words([0x525e10, 10, 24, due, 1, slot as u32])
        );
        let mut source = Queue::default();
        source
            .submit(command, 2000, true, |_| panic!("delayed command"))
            .unwrap();
        let mut restored = round_trip(&source, None);
        drain_at(&mut restored, due, |command| {
            assert!(matches!(command, Command::Eliminate { slot: s } if *s == slot));
        });
    }
}

#[test]
fn room_start_native_round_trip_preserves_announcement_and_due() {
    use crate::ai_messages::offers::auction::admission::{
        self, Call, Controller, Presentation, World,
    };
    for tutorial in [false, true] {
        let mut controller = Controller::default();
        let mut presentation = Presentation::default();
        let mut categories = [[0; 380]; 52];
        let mut source = Queue::default();
        let tick = 1000;
        admission::admit(
            36,
            u32::MAX,
            100,
            &mut World {
                controller: &mut controller,
                categories: &mut categories,
                active: &[1, 1],
                presentation: &mut presentation,
                current_slot: 0,
                tutorial,
                network: [false; 2],
                command_prior: [0; 20],
            },
            |call, _| {
                if let Call::QueueStart {
                    additional_delay,
                    descriptor,
                } = call
                {
                    let mut padded = [0; 52];
                    padded[..20].copy_from_slice(&descriptor);
                    source.submit_with_delay(
                        Command::Auction { descriptor: padded },
                        tick,
                        true,
                        additional_delay,
                        |_| panic!("delayed start"),
                    )?;
                }
                Ok(())
            },
        )
        .unwrap();
        let due = tick + 25 + if tutorial { 5 * 25 } else { 120 * 25 };
        let snapshots = source.snapshot().unwrap();
        assert_eq!(snapshots[0].1, due);
        assert_eq!(
            packet(snapshots[0].4, due).unwrap(),
            words([0x525c70, 0x1e, 20, due, 1])
        );
        // Original saves queue the room-start on the total-clock list
        // (selector 0), like the engine. A normal-list node (selector 1) must
        // also keep its outer deadline and the separate inner deadline.
        let mut normal = Queue::default();
        normal.restore_queued(snapshots[0].4.clone(), ClockDomain::Normal, due, due, false);
        for queue in [&source, &normal] {
            let mut restored = round_trip(queue, Some(&mut controller.record));
            drain_at(&mut restored, due, |command| {
                let Command::Auction { descriptor } = command else {
                    panic!("missing auction start")
                };
                assert_eq!(word(descriptor, 4), 0x1e);
                assert_eq!(word(descriptor, 12), due);
                assert_eq!(descriptor[16], 1);
                assert_eq!(&descriptor[20..], &[0; 32]);
            });
        }
    }
}

#[test]
fn legacy_52_byte_room_start_still_decodes_and_truncated_objects_fail() {
    let mut descriptor = [0xab; 52];
    put(&mut descriptor, 4, 0x1e);
    let Command::Auction { descriptor: actual } = saved_command(&descriptor).unwrap() else {
        panic!("auction descriptor")
    };
    assert_eq!(actual, descriptor);
    for (kind, length) in [(9, 28), (10, 24), (0x1e, 20)] {
        let mut bytes = vec![0; length];
        put(&mut bytes, 4, kind);
        for n in 0..length {
            assert!(
                saved_command(&bytes[..n]).is_err(),
                "kind {kind}, length {n}"
            );
        }
    }
}

#[test]
#[ignore = "requires original auction admission capture"]
fn original_admission_room_start_round_trips_through_native_save() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_ADMISSION")).unwrap();
    let mut input = Input::new(&bytes);
    assert_eq!(input.take(8).unwrap(), b"MTACAD02");
    assert_eq!(input.word().unwrap(), 0x037f);
    let count = input.word().unwrap();
    let mut starts = 0;
    for _ in 0..count {
        input.take(40 + 2 * (344 + 52 * 380 + 36)).unwrap();
        input.word().unwrap(); // admission return
        let calls = input.word().unwrap();
        for _ in 0..calls {
            let n = input.word().unwrap();
            let trace: Vec<_> = (0..n).map(|_| input.word().unwrap()).collect();
            if trace[0] != 1 {
                continue;
            }
            starts += 1;
            assert_eq!(trace.len(), 7);
            let mut descriptor = [0; 52];
            descriptor[..20].copy_from_slice(&words(trace[2..].iter().copied()));
            let mut source = Queue::default();
            source
                .submit_with_delay(
                    Command::Auction { descriptor },
                    1000,
                    true,
                    trace[1],
                    |_| panic!("delayed start"),
                )
                .unwrap();
            let due = 1025 + trace[1];
            let mut expected = descriptor[..20].to_vec();
            put(&mut expected, 8, 20);
            put(&mut expected, 12, due);
            let mut restored = round_trip(&source, None);
            drain_at(&mut restored, due, |command| {
                assert_eq!(packet(command, due).unwrap(), expected);
            });
        }
    }
    input.end().unwrap();
    assert!(starts > 0);
}
