use super::*;

fn content() -> Content {
    Content {
        level: 300001,
        parameter_bits: 10000f32.to_bits(),
        difficulty: 1,
        fingerprint: [9; 32],
    }
}
fn roster() -> Message {
    let rows = std::array::from_fn(|i| {
        let (kind, token, ready, name) = match i {
            0 => (RowKind::Human, 0, true, b"H".to_vec()),
            1 => (RowKind::Ai, 1, false, b"A".to_vec()),
            3 => (RowKind::Closed, -1, false, vec![]),
            _ => (RowKind::Open, -1, false, vec![]),
        };
        Row {
            kind,
            token,
            ready,
            name,
        }
    });
    Message::Roster {
        content: content(),
        rows,
    }
}
fn transfer() -> Message {
    Message::Transfer(Transfer {
        category: 51,
        owner: 6,
        amount: -123,
        due: 0x12345678,
    })
}
fn frame(message: Message, sequence: u64) -> Frame {
    Frame {
        session: [7; 16],
        sequence,
        message,
    }
}
fn bytes(message: Message, sequence: u64) -> Vec<u8> {
    codec::encode(&frame(message, sequence)).unwrap()
}
fn lobby(owner: &Owner) -> Attempt {
    let attempt = owner.begin().unwrap();
    owner.opened(attempt, [7; 16]).unwrap();
    attempt
}
fn playing(owner: &Owner, attempt: Attempt, world: u64) -> SessionBinding {
    owner.begin_start(attempt).unwrap();
    owner.bind_world(attempt, world).unwrap()
}

#[test]
fn one_process_owner_and_explicit_completions() {
    assert!(std::ptr::eq(process(), process()));
    let owner = Owner::new().unwrap();
    assert_eq!(owner.phase(), Ok(Phase::Offline));
    let attempt = owner.begin().unwrap();
    assert_eq!(owner.phase(), Ok(Phase::Opening));
    assert_eq!(owner.bind_world(attempt, 9), Err(Error::Phase));
    assert_eq!(owner.opened(attempt, [0; 16]), Err(Error::Session));
    assert_eq!(owner.phase(), Ok(Phase::Opening));
    owner.opened(attempt, [7; 16]).unwrap();
    owner.begin_start(attempt).unwrap();
    assert_eq!(owner.bind_world(attempt, 0), Err(Error::World));
    assert_eq!(owner.phase(), Ok(Phase::Starting));
    let binding = owner.bind_world(attempt, 9).unwrap();
    assert_eq!(binding.attempt(), attempt);
    assert_eq!(binding.world(), 9);
    assert_eq!(owner.validate_binding(binding), Ok(()));
}

#[test]
fn every_transition_is_checked() {
    // Expected admission for begin/opened/begin_start/bind_world/retire at
    // each phase. Each operation gets an independent owner in that phase.
    let phases = [
        Phase::Offline,
        Phase::Opening,
        Phase::Lobby,
        Phase::Starting,
        Phase::Playing,
        Phase::Stopped,
    ];
    for phase in phases {
        for operation in 0..5 {
            let owner = Owner::new().unwrap();
            let attempt = if phase == Phase::Offline {
                owner.lock().unwrap().attempt
            } else {
                let attempt = owner.begin().unwrap();
                if matches!(phase, Phase::Lobby | Phase::Starting | Phase::Playing) {
                    owner.opened(attempt, [7; 16]).unwrap();
                }
                if matches!(phase, Phase::Starting | Phase::Playing) {
                    owner.begin_start(attempt).unwrap();
                }
                if phase == Phase::Playing {
                    owner.bind_world(attempt, 9).unwrap();
                }
                if phase == Phase::Stopped {
                    owner.retire(attempt).unwrap();
                }
                attempt
            };
            let result = match operation {
                0 => owner.begin().map(|_| ()),
                1 => owner.opened(attempt, [7; 16]),
                2 => owner.begin_start(attempt),
                3 => owner.bind_world(attempt, 9).map(|_| ()),
                _ => owner.retire(attempt),
            };
            let allowed = match operation {
                0 => matches!(phase, Phase::Offline | Phase::Stopped),
                1 => phase == Phase::Opening,
                2 => phase == Phase::Lobby,
                3 => phase == Phase::Starting,
                _ => !matches!(phase, Phase::Offline | Phase::Stopped),
            };
            assert_eq!(result.is_ok(), allowed, "{phase:?}, operation {operation}");
            if !allowed {
                assert_eq!(owner.phase(), Ok(phase));
            }
        }
    }
}

#[test]
fn stale_owner_attempt_world_and_connection_cannot_affect_replacement() {
    let owner = Owner::new().unwrap();
    let a = lobby(&owner);
    let stale = owner.adopt(a, 0, true).unwrap();
    let old = playing(&owner, a, 11);
    owner.retire(a).unwrap();
    let b = lobby(&owner);
    let peer = owner.adopt(b, 0, true).unwrap();
    let binding = playing(&owner, b, 22);
    let data = bytes(transfer(), 0);
    assert_eq!(owner.retire(a), Err(Error::StaleAttempt));
    assert_eq!(owner.opened(a, [7; 16]), Err(Error::StaleAttempt));
    assert_eq!(owner.begin_start(a), Err(Error::StaleAttempt));
    assert_eq!(owner.bind_world(a, 22), Err(Error::StaleAttempt));
    assert_eq!(owner.adopt(a, 1, false), Err(Error::StaleAttempt));
    assert_eq!(owner.validate_binding(old), Err(Error::StaleAttempt));
    assert_eq!(
        owner.receive(stale, Some(binding), &data),
        Err(Error::StaleAttempt)
    );
    let foreign = Owner::new().unwrap();
    assert_eq!(foreign.retire(b), Err(Error::StaleAttempt));
    assert_eq!(foreign.validate_binding(binding), Err(Error::StaleAttempt));
    let wrong = SessionBinding {
        world: 11,
        ..binding
    };
    assert_eq!(owner.receive(peer, Some(wrong), &data), Err(Error::World));
    assert_eq!(
        owner.receive(peer, Some(old), &data),
        Err(Error::StaleAttempt)
    );
    assert_eq!(owner.receive(peer, None, &data), Err(Error::World));
    assert_eq!(owner.phase(), Ok(Phase::Playing));
    assert_eq!(owner.receive(peer, Some(binding), &data), Ok(transfer()));
    // A consumer callback can acquire the lock and retire without deadlocking.
    assert!(owner.state.try_lock().is_ok());
    owner.retire(b).unwrap();
    assert!(owner.validate_binding(binding).is_err());
}

#[test]
fn payload_never_adopts_peer_or_changes_host_and_rejection_is_atomic() {
    let owner = Owner::new().unwrap();
    let attempt = lobby(&owner);
    let host = owner.adopt(attempt, 0, true).unwrap();
    let client = owner.adopt(attempt, 1, false).unwrap();
    assert_eq!(client.slot(), 1);
    assert_eq!(owner.adopt(attempt, 1, false), Err(Error::Peer));
    assert_eq!(owner.adopt(attempt, 2, true), Err(Error::Peer));
    assert_eq!(owner.adopt(attempt, 6, false), Err(Error::Peer));
    let setup = bytes(roster(), 0);
    assert_eq!(owner.receive(client, None, &setup), Err(Error::Peer));
    assert_eq!(
        owner.receive(host, None, &bytes(transfer(), 0)),
        Err(Error::World)
    );
    let mut wrong_session = setup.clone();
    wrong_session[12] = 8;
    assert_eq!(
        owner.receive(host, None, &wrong_session),
        Err(Error::Session)
    );
    assert_eq!(
        owner.receive(host, None, &bytes(roster(), 1)),
        Err(Error::Sequence)
    );
    assert_eq!(owner.receive(host, None, &setup), Ok(roster()));
    assert_eq!(owner.receive(host, None, &setup), Err(Error::Sequence));
    assert_eq!(
        owner.receive(client, None, &bytes(roster(), 1)),
        Err(Error::Peer)
    );
    let start = Message::Start(Start {
        content: content(),
        seed: 0,
    });
    assert_eq!(
        owner.receive(host, None, &bytes(start.clone(), 1)),
        Ok(start)
    );
    assert_eq!(
        owner.phase(),
        Ok(Phase::Lobby),
        "wire Start is not completed world startup"
    );
    let binding = playing(&owner, attempt, 9);
    assert_eq!(
        owner.receive(host, None, &bytes(roster(), 2)),
        Err(Error::Phase)
    );
    assert_eq!(
        owner.receive(client, Some(binding), &bytes(transfer(), 0)),
        Err(Error::Peer)
    );
    assert_eq!(
        owner.receive(host, Some(binding), &bytes(transfer(), 2)),
        Ok(transfer())
    );
}

#[test]
fn every_message_roundtrips_and_transfer_has_independent_wire_bytes() {
    for message in [
        roster(),
        Message::Start(Start {
            content: content(),
            seed: u32::MAX,
        }),
        transfer(),
    ] {
        let expected = frame(message, 0x0807060504030201);
        assert_eq!(
            codec::decode(&codec::encode(&expected).unwrap()),
            Ok(expected)
        );
    }
    let mut expected = b"OMT1\x01\x00\x03\x00\x0a\x00\x00\x00".to_vec();
    expected.extend_from_slice(&[7; 16]);
    expected.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    expected.extend_from_slice(&[51, 6, 0x85, 0xff, 0xff, 0xff, 0x78, 0x56, 0x34, 0x12]);
    assert_eq!(bytes(transfer(), 0x0807060504030201), expected);
}

#[test]
fn all_invalid_frame_classes_and_replay_are_rejected_before_effects() {
    let valid = bytes(roster(), 0);
    let mut invalid = Vec::new();
    for end in 0..valid.len() {
        invalid.push(valid[..end].to_vec());
    }
    for (offset, value) in [
        (0, 0),
        (4, 2),
        (6, 99),
        (77, 4),
        (78, 10),
        (79, 2),
        (80, 32),
        (81, 0),
        (83, 0),
        (88, 0),
        (89, 1),
    ] {
        let mut bad = valid.clone();
        bad[offset] = value;
        invalid.push(bad);
    }
    let mut bad = valid.clone();
    bad[12..28].fill(0);
    invalid.push(bad);
    let mut bad = valid.clone();
    bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    invalid.push(bad);
    let mut bad = valid.clone();
    bad.push(0);
    invalid.push(bad.clone());
    let length = (bad.len() - codec::HEADER) as u32;
    bad[8..12].copy_from_slice(&length.to_le_bytes());
    invalid.push(bad);
    invalid.push(vec![0; codec::MAX_FRAME + 1]);
    // Empty human name, with lengths adjusted so rejection is semantic.
    let mut bad = valid.clone();
    bad.remove(81);
    bad[80] = 0;
    let length = (bad.len() - codec::HEADER) as u32;
    bad[8..12].copy_from_slice(&length.to_le_bytes());
    invalid.push(bad);
    for (range, replacement) in [
        (36..40, vec![0; 4]),
        (40..44, f32::NAN.to_bits().to_le_bytes().to_vec()),
        (44..45, vec![3]),
        (45..77, vec![0; 32]),
    ] {
        let mut bad = valid.clone();
        bad[range].copy_from_slice(&replacement);
        invalid.push(bad);
    }
    for (offset, value) in [(36, 52), (37, 0), (37, 7)] {
        let mut bad = bytes(transfer(), 0);
        bad[offset] = value;
        invalid.push(bad);
    }
    let owner = Owner::new().unwrap();
    let attempt = lobby(&owner);
    let peer = owner.adopt(attempt, 0, true).unwrap();
    let mut effects = 0;
    let mut decoder = codec::Decoder::new([7; 16]).unwrap();
    for bad in invalid {
        assert!(codec::decode(&bad).is_err(), "accepted {bad:?}");
        assert!(decoder.decode(&bad).is_err());
        if owner.receive(peer, None, &bad).is_ok() {
            effects += 1;
        }
    }
    assert_eq!(effects, 0);
    assert_eq!(owner.receive(peer, None, &valid), Ok(roster()));
    assert_eq!(decoder.decode(&bytes(roster(), 1)), Err(Error::Sequence));
    assert_eq!(decoder.decode(&valid), Ok(frame(roster(), 0)));
    assert_eq!(decoder.decode(&valid), Err(Error::Sequence));
    assert_eq!(decoder.decode(&bytes(roster(), 2)), Err(Error::Sequence));
    assert_eq!(decoder.decode(&bytes(roster(), 1)), Ok(frame(roster(), 1)));
}

#[test]
fn encode_validates_rows_content_and_session_too() {
    let mut invalid = frame(roster(), 0);
    invalid.session = [0; 16];
    assert_eq!(codec::encode(&invalid), Err(Error::Session));
    assert!(codec::Decoder::new([0; 16]).is_err());
    let Message::Roster {
        mut content,
        mut rows,
    } = roster()
    else {
        unreachable!()
    };
    rows[1].token = rows[0].token;
    assert_eq!(
        codec::encode(&frame(Message::Roster { content, rows }, 0)),
        Err(Error::Row)
    );
    content.difficulty = 3;
    assert_eq!(
        codec::encode(&frame(Message::Start(Start { content, seed: 1 }), 0)),
        Err(Error::Content)
    );
}
