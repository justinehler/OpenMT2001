use super::*;
use crate::network::{Frame, codec};

fn owner() -> (Owner, SessionBinding, Connection) {
    let owner = Owner::new().unwrap();
    let attempt = owner.begin().unwrap();
    owner.opened(attempt, [7; 16]).unwrap();
    let peer = owner.adopt(attempt, 4, false).unwrap();
    owner.begin_start(attempt).unwrap();
    let binding = owner.bind_world(attempt, 1).unwrap();
    (owner, binding, peer)
}
fn ingress(
    owner: &Owner,
    binding: SessionBinding,
    connection: Connection,
    message: Message,
    sequence: u64,
) -> Result<Ingress> {
    let bytes = codec::encode(&Frame {
        session: [7; 16],
        sequence,
        message,
    })?;
    Ok(Ingress {
        binding,
        connection,
        message: owner.receive(connection, Some(binding), &bytes)?,
    })
}
fn transfer() -> Transfer {
    Transfer {
        category: 7,
        owner: 3,
        amount: 500,
        due: 0,
    }
}

fn construction(owner: i8) -> crate::construction::Command {
    crate::construction::Command {
        category: 7,
        rectangle: [0, 0, 1, 1],
        height: 2,
        variation: 3,
        business: 4,
        kind: 0,
        quality: 1,
        owner,
        components: [101.25, 23.5, 7.75, 2.5],
    }
}

#[test]
fn construction_checks_compact_owner_replay_schema_and_authority_only_ai() {
    let (owner, binding, peer) = owner();
    let mut host = Adapter::new(binding, true, 0, vec![Some(0), None, Some(4)]).unwrap();
    let mut client = Adapter::new(binding, false, 4, vec![Some(0), None, Some(4)]).unwrap();
    let request = Request::construction(0, &construction(3));
    let admitted = ingress(
        &owner,
        binding,
        peer,
        Message::CommandRequest(request.clone()),
        0,
    )
    .unwrap();
    let Message::CommandCommit(commit) = host.receive(&owner, admitted, 100).unwrap() else {
        panic!()
    };
    assert_eq!(commit.due, 125);
    assert_eq!(commit.request, request);
    assert_eq!(
        commit.request.construction_payload().unwrap().packet(),
        construction(3).packet()
    );
    let replay = ingress(&owner, binding, peer, Message::CommandRequest(request), 1).unwrap();
    assert_eq!(host.receive(&owner, replay, 100), Err(Error::Sequence));
    for (sequence, wrong_owner) in [(2, 1), (3, 2)] {
        let request = Request::construction(1, &construction(wrong_owner));
        let wrong = ingress(
            &owner,
            binding,
            peer,
            Message::CommandRequest(request),
            sequence,
        )
        .unwrap();
        assert_eq!(host.receive(&owner, wrong, 100), Err(Error::Peer));
    }
    // AI has no peer: only the local authority can submit its owner, using
    // the same serial/order stream as its human player's commands.
    let ai = host.next_construction(&construction(2)).unwrap();
    let Message::CommandCommit(ai) = host.request(&owner, ai, 100).unwrap() else {
        panic!()
    };
    assert_eq!(ai.order, 1);
    assert_eq!(ai.due, 125);
    let forbidden = client.next_construction(&construction(2)).unwrap();
    assert_eq!(client.request(&owner, forbidden, 100), Err(Error::Peer));
    let mut mismatched = Request::construction(1, &construction(3));
    mismatched.owner = 1;
    assert!(mismatched.validate().is_err());
    for length in [0, 7, 39, 41, 60, 128] {
        let request = Request {
            serial: 0,
            owner: 3,
            kind: 0x0f,
            payload: vec![0; length],
        };
        assert_eq!(request.validate(), Err(Error::Unsupported));
    }
    let valid = construction(3);
    for invalid in [
        crate::construction::Command {
            category: -1,
            ..valid.clone()
        },
        crate::construction::Command {
            category: 52,
            ..valid.clone()
        },
        crate::construction::Command {
            rectangle: [0, 0, 7, 1],
            ..valid.clone()
        },
        crate::construction::Command {
            components: [f32::NAN; 4],
            ..valid.clone()
        },
        crate::construction::Command {
            kind: 3,
            ..valid.clone()
        },
        crate::construction::Command { owner: 0, ..valid },
    ] {
        assert_eq!(
            Request::construction(0, &invalid).validate(),
            Err(Error::Row)
        );
    }
    assert_eq!(host.next_commit, 2);
}

#[test]
fn admitted_connection_and_original_compact_owner_authorize_once() {
    let (owner, binding, peer) = owner();
    // Distinct lobby peer4, compact gameplay owner3, character token irrelevant.
    let mut host = Adapter::new(binding, true, 0, vec![Some(0), None, Some(4)]).unwrap();
    let request = Request::transfer(0, transfer());
    let admitted = ingress(
        &owner,
        binding,
        peer,
        Message::CommandRequest(request.clone()),
        0,
    )
    .unwrap();
    let Message::CommandCommit(commit) = host.receive(&owner, admitted, 100).unwrap() else {
        panic!()
    };
    assert_eq!(
        commit,
        Commit {
            order: 0,
            due: 125,
            request: Request::transfer(0, transfer())
        }
    );
    // Fresh transport envelope cannot replay an already committed request.
    let replay = ingress(&owner, binding, peer, Message::CommandRequest(request), 1).unwrap();
    assert_eq!(host.receive(&owner, replay, 100), Err(Error::Sequence));
    let wrong = Request::transfer(
        1,
        Transfer {
            owner: 1,
            ..transfer()
        },
    );
    let wrong = ingress(&owner, binding, peer, Message::CommandRequest(wrong), 2).unwrap();
    assert_eq!(host.receive(&owner, wrong, 100), Err(Error::Peer));
    assert_eq!(host.next_commit, 1);
    // Old transport sequence is rejected before the adapter sees it.
    assert_eq!(
        ingress(
            &owner,
            binding,
            peer,
            Message::CommandRequest(Request::transfer(2, transfer())),
            2
        )
        .unwrap_err(),
        Error::Sequence
    );
    owner.retire(binding.attempt()).unwrap();
    let stale = Ingress {
        binding,
        connection: peer,
        message: Message::CommandRequest(Request::transfer(2, transfer())),
    };
    assert_eq!(host.receive(&owner, stale, 100), Err(Error::Phase));
}

#[test]
fn every_unqualified_family_and_auction_transfer_schema_is_explicitly_unsupported() {
    for kind in [
        0,
        1,
        8,
        9,
        0x10,
        0x11,
        0x12,
        0x13,
        0x14,
        0x15,
        0x17,
        0x18,
        0x19,
        0x1a,
        0x1b,
        0x1c,
        0x1d,
        0x1e,
        0x1f,
        0x20,
        0x21,
        0x31,
        u32::MAX,
    ] {
        assert_eq!(
            Request {
                serial: 0,
                owner: 3,
                kind,
                payload: vec![0; 7]
            }
            .validate(),
            Err(Error::Unsupported)
        );
    }
    for length in [0, 6, 8, 28, 32, 52, 128] {
        assert_eq!(
            Request {
                serial: 0,
                owner: 3,
                kind: 0x16,
                payload: vec![0; length]
            }
            .transfer_payload(),
            Err(Error::Unsupported)
        );
    }
}

#[cfg(feature = "lua4")]
#[test]
fn actual_live_queue_retains_authority_order_and_client_never_adds_delay() {
    let (owner, binding, peer) = owner();
    let mut host = Adapter::new(binding, true, 0, vec![Some(0), None, Some(4)]).unwrap();
    let mut client = Adapter::new(binding, false, 4, vec![Some(0), None, Some(4)]).unwrap();
    let mut left = crate::live_commands::Queue::default();
    let mut right = crate::live_commands::Queue::default();
    for serial in 0..2 {
        let request = Request::transfer(
            serial,
            Transfer {
                amount: 500 + serial as i32,
                ..transfer()
            },
        );
        let admitted = ingress(
            &owner,
            binding,
            peer,
            Message::CommandRequest(request),
            serial,
        )
        .unwrap();
        let Message::CommandCommit(commit) = host.receive(&owner, admitted, 100).unwrap() else {
            panic!()
        };
        host.enqueue(&commit, &mut left, &mut [[0; 0x17c]; 52], 100)
            .unwrap();
        client
            .enqueue(&commit, &mut right, &mut [[0; 0x17c]; 52], 102)
            .unwrap();
        assert_eq!(
            client.enqueue(&commit, &mut right, &mut [[0; 0x17c]; 52], 102),
            Err(Error::Sequence)
        );
    }
    assert_eq!(client.admit_deadline(3, 150), Err(Error::Sequence));
    client.admit_deadline(2, 150).unwrap();
    for queue in [&mut left, &mut right] {
        let mut effects = Vec::new();
        queue
            .drain(&mut [124, 0], |_| panic!("early property effect"))
            .unwrap();
        queue
            .drain(&mut [125, 0], |command| {
                let crate::live_commands::Command::PropertyTransfer { amount, .. } = command else {
                    panic!()
                };
                effects.push(*amount);
                Ok(())
            })
            .unwrap();
        assert_eq!(effects, [500, 501]);
        queue
            .drain(&mut [150, 0], |_| panic!("duplicate property effect"))
            .unwrap();
    }
}

#[cfg(feature = "lua4")]
#[path = "runtime_tests.rs"]
mod runtime;

#[test]
fn gameplay_wire_schemas_round_trip_and_are_bounded() {
    let messages = [
        Message::CommandRequest(Request::construction(26, &construction(3))),
        Message::CommandCommit(Commit {
            order: 30,
            due: 125,
            request: Request::construction(26, &construction(3)),
        }),
        Message::CommandRequest(Request::transfer(27, transfer())),
        Message::CommandCommit(Commit {
            order: 31,
            due: 125,
            request: Request::transfer(27, transfer()),
        }),
        Message::CommandDeadline {
            after: 32,
            tick: 150,
        },
        Message::CommandRejected {
            serial: 27,
            unsupported: true,
        },
    ];
    for message in messages {
        let frame = Frame {
            session: [7; 16],
            sequence: 8,
            message,
        };
        let bytes = codec::encode(&frame).unwrap();
        assert_eq!(codec::decode(&bytes).unwrap(), frame);
        for length in 0..bytes.len() {
            assert!(codec::decode(&bytes[..length]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(codec::decode(&trailing), Err(Error::TrailingBytes));
    }
    assert_eq!(
        codec::encode(&Frame {
            session: [7; 16],
            sequence: 0,
            message: Message::CommandRequest(Request {
                serial: 0,
                owner: 1,
                kind: 0x0f,
                payload: vec![0; 129]
            })
        }),
        Err(Error::Oversized)
    );
}

#[cfg(feature = "lua4")]
#[test]
fn only_admitted_ordered_deadline_ingress_resumes_reason_one_and_same_tasks() {
    let owner = Owner::new().unwrap();
    let attempt = owner.begin().unwrap();
    owner.opened(attempt, [7; 16]).unwrap();
    let authority = owner.adopt(attempt, 0, true).unwrap();
    owner.begin_start(attempt).unwrap();
    let binding = owner.bind_world(attempt, 44).unwrap();
    let mut client = Adapter::new(binding, false, 4, vec![Some(0), Some(4)]).unwrap();
    let mut pause = crate::pause::State::default();
    let mut tasks = crate::timers::startup_tasks([0x5a; 20]);
    let before = tasks.clone();
    pause_at_limit(&mut pause, &mut tasks).unwrap();
    let paused = tasks.clone();
    // A future deadline cannot pass a missing shared commit.
    let wrong = ingress(
        &owner,
        binding,
        authority,
        Message::CommandDeadline { after: 1, tick: 50 },
        0,
    )
    .unwrap();
    let Message::CommandDeadline { after, tick } = client.receive(&owner, wrong, 25).unwrap()
    else {
        panic!()
    };
    assert!(
        client
            .apply_deadline(after, tick, 25, &mut pause, &mut tasks)
            .is_err()
    );
    assert_eq!(tasks, paused);
    assert_eq!(pause.bytes[1], 1);
    assert_eq!(client.deadline(), 25);
    let admitted = ingress(
        &owner,
        binding,
        authority,
        Message::CommandDeadline { after: 0, tick: 50 },
        1,
    )
    .unwrap();
    let Message::CommandDeadline { after, tick } = client.receive(&owner, admitted, 25).unwrap()
    else {
        panic!()
    };
    client
        .apply_deadline(after, tick, 25, &mut pause, &mut tasks)
        .unwrap();
    assert_eq!(pause.bytes[1], 0);
    assert_eq!(tasks, before);
    pause_at_limit(&mut pause, &mut tasks).unwrap();
    assert_eq!(
        ingress(
            &owner,
            binding,
            authority,
            Message::CommandDeadline { after: 0, tick: 75 },
            1
        )
        .unwrap_err(),
        Error::Sequence
    );
    assert_eq!(pause.bytes[1], 1);
    assert_eq!(tasks, paused);
    owner.retire(attempt).unwrap();
    let stale = Ingress {
        binding,
        connection: authority,
        message: Message::CommandDeadline { after: 0, tick: 75 },
    };
    assert_eq!(client.receive(&owner, stale, 50), Err(Error::Phase));
    assert_eq!(pause.bytes[1], 1);
    assert_eq!(tasks, paused);
}
