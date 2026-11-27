use super::*;
use crate::network::{Content, Error as OwnerError, Start, stream::MAX_OUTPUT_FRAMES};
use std::{io::Write, net::Shutdown, thread};

const WAIT: Duration = Duration::from_secs(3);
fn address() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}
fn until(mut step: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !step() {
        assert!(Instant::now() < deadline, "loopback operation timed out");
        thread::sleep(Duration::from_millis(1));
    }
}
fn start(seed: u32) -> Message {
    Message::Start(Start {
        content: Content {
            level: 300001,
            parameter_bits: 10000f32.to_bits(),
            difficulty: 1,
            fingerprint: [9; 32],
        },
        seed,
    })
}
fn wire(message: Message, sequence: u64) -> Vec<u8> {
    codec::encode(&Frame {
        session: [7; 16],
        sequence,
        message,
    })
    .unwrap()
}
fn host() -> (Owner, Attempt, SocketAddr) {
    let owner = Owner::new().unwrap();
    let attempt = owner.begin().unwrap();
    let address = owner.listen(attempt, address(), [7; 16]).unwrap();
    (owner, attempt, address)
}
fn connected(events: Vec<Event>) -> Option<Connection> {
    let mut connection = None;
    for event in events {
        match event {
            Event::Connected {
                connection: admitted,
                ..
            } => connection = Some(admitted),
            other => panic!("unexpected admission event: {other:?}"),
        }
    }
    connection
}
fn pair() -> (Owner, Attempt, Connection, Owner, Attempt, Connection) {
    let (host, ha, host_address) = host();
    let client = Owner::new().unwrap();
    let ca = client.begin().unwrap();
    client.connect(ca, host_address, WAIT).unwrap();
    assert_eq!(client.phase(), Ok(Phase::Opening));
    assert_eq!(
        client.listen(ca, address(), [9; 16]),
        Err(Error::Protocol(OwnerError::Phase))
    );
    assert!(client.lock().unwrap().peers.iter().all(Option::is_none));
    let (mut hp, mut cp) = (None, None);
    until(|| {
        if let Some(peer) = connected(host.pump(ha, None).unwrap()) {
            hp = Some(peer);
        }
        for event in client.pump(ca, None).unwrap() {
            let Event::Connected {
                connection,
                local_slot,
            } = event
            else {
                panic!("{event:?}")
            };
            assert_eq!(local_slot, 1);
            assert_eq!(connection.slot(), 0);
            cp = Some(connection);
        }
        hp.is_some() && cp.is_some()
    });
    let hp = hp.unwrap();
    assert_eq!(hp.slot(), 1);
    assert_eq!(client.phase(), Ok(Phase::Lobby));
    (host, ha, hp, client, ca, cp.unwrap())
}
fn write(socket: &mut TcpStream, bytes: &[u8]) {
    socket.set_nonblocking(true).unwrap();
    let mut at = 0;
    until(|| match socket.write(&bytes[at..]) {
        Ok(0) => panic!("socket closed while writing"),
        Ok(count) => {
            at += count;
            at == bytes.len()
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            false
        }
        Err(error) => panic!("{error}"),
    });
}

#[test]
fn actual_loopback_admission_ordered_delivery_and_queue_backpressure() {
    let (host, ha, hp, client, ca, cp) = pair();
    assert!(host.pump(ha, None).unwrap().is_empty()); // idle WouldBlock
    assert!(client.pump(ca, None).unwrap().is_empty());
    for seed in 0..MAX_OUTPUT_FRAMES as u32 {
        host.send(hp, start(seed)).unwrap();
    }
    assert_eq!(host.send(hp, start(99)), Err(Error::QueueFull));
    let mut received = Vec::new();
    until(|| {
        assert!(host.pump(ha, None).unwrap().is_empty());
        for event in client.pump(ca, None).unwrap() {
            let Event::Message {
                connection,
                message,
            } = event
            else {
                panic!("{event:?}")
            };
            assert_eq!(connection, cp);
            received.push(*message);
        }
        received.len() == MAX_OUTPUT_FRAMES
    });
    assert_eq!(
        received,
        (0..MAX_OUTPUT_FRAMES as u32).map(start).collect::<Vec<_>>()
    );
    host.send(hp, start(99)).unwrap();
    until(|| {
        host.pump(ha, None).unwrap();
        let events = client.pump(ca, None).unwrap();
        if events.is_empty() {
            return false;
        }
        assert_eq!(
            events,
            [Event::Message {
                connection: cp,
                message: Box::new(start(99))
            }]
        );
        true
    });
    // A client cannot commit authority data, even through a real admitted socket.
    client.send(cp, start(8)).unwrap();
    until(|| {
        client.pump(ca, None).unwrap();
        let events = host.pump(ha, None).unwrap();
        if events.is_empty() {
            return false;
        }
        assert_eq!(
            events,
            [Event::Failed {
                connection: Some(hp),
                error: Error::Protocol(OwnerError::Peer)
            }]
        );
        true
    });
}

#[test]
fn raw_host_fragments_welcome_then_coalesces_frames_before_mid_frame_disconnect() {
    let listener = TcpListener::bind(address()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let client = Owner::new().unwrap();
    let attempt = client.begin().unwrap();
    client
        .connect(attempt, listener.local_addr().unwrap(), WAIT)
        .unwrap();
    let mut socket = None;
    until(|| match listener.accept() {
        Ok((stream, _)) => {
            socket = Some(stream);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("{error}"),
    });
    let mut socket = socket.unwrap();
    socket.set_nonblocking(true).unwrap();
    socket.set_nodelay(true).unwrap();
    // Flush Hello and verify it through the same framed reader.
    client.pump(attempt, None).unwrap();
    let mut input = Stream::new(socket.try_clone().unwrap()).unwrap();
    until(|| {
        let batch = input.read();
        assert_eq!(batch.terminal, None);
        if batch.frames.is_empty() {
            return false;
        }
        assert_eq!(
            codec::decode(&batch.frames[0]).unwrap().message,
            Message::Hello
        );
        true
    });
    let welcome = wire(Message::Welcome { slot: 4 }, 0);
    let mut peer = None;
    for byte in welcome {
        write(&mut socket, &[byte]);
        for event in client.pump(attempt, None).unwrap() {
            let Event::Connected {
                connection,
                local_slot,
            } = event
            else {
                panic!("{event:?}")
            };
            assert_eq!(local_slot, 4);
            peer = Some(connection);
        }
    }
    until(|| {
        if let Some(connection) = connected(client.pump(attempt, None).unwrap()) {
            peer = Some(connection);
        }
        peer.is_some()
    });
    let peer = peer.unwrap();
    let messages: Vec<_> = (0..3).map(start).collect();
    let mut bytes: Vec<_> = messages
        .iter()
        .enumerate()
        .flat_map(|(i, message)| wire(message.clone(), i as u64))
        .collect();
    bytes.extend_from_slice(&wire(start(3), 3)[..codec::HEADER + 2]);
    write(&mut socket, &bytes);
    socket.shutdown(Shutdown::Write).unwrap();
    let mut events = Vec::new();
    until(|| {
        events.extend(client.pump(attempt, None).unwrap());
        events
            .iter()
            .any(|event| matches!(event, Event::Failed { .. }))
    });
    let mut expected: Vec<_> = messages
        .into_iter()
        .map(|message| Event::Message {
            connection: peer,
            message: Box::new(message),
        })
        .collect();
    expected.push(Event::Failed {
        connection: Some(peer),
        error: Error::Truncated,
    });
    assert_eq!(events, expected);
    assert!(client.lock().unwrap().peers[0].is_none());
}

#[test]
fn invalid_or_incomplete_hello_never_adopts_a_connection() {
    for invalid in [wire(Message::Welcome { slot: 1 }, 0), vec![b'O']] {
        let (host, attempt, address) = host();
        let mut socket = TcpStream::connect_timeout(&address, WAIT).unwrap();
        write(&mut socket, &invalid);
        socket.shutdown(Shutdown::Write).unwrap();
        let mut events = Vec::new();
        until(|| {
            events.extend(host.pump(attempt, None).unwrap());
            !events.is_empty()
        });
        assert!(events.iter().all(|event| matches!(
            event,
            Event::Failed {
                connection: None,
                ..
            }
        )));
        assert!(host.lock().unwrap().peers.iter().all(Option::is_none));
        assert_eq!(host.phase(), Ok(Phase::Lobby));
    }
}

#[test]
fn invalid_welcome_and_timeout_never_adopt_a_connection() {
    for timeout in [false, true] {
        let listener = TcpListener::bind(address()).unwrap();
        let client = Owner::new().unwrap();
        let attempt = client.begin().unwrap();
        client
            .connect(attempt, listener.local_addr().unwrap(), WAIT)
            .unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut socket = None;
        until(|| match listener.accept() {
            Ok((stream, _)) => {
                socket = Some(stream);
                true
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
            Err(error) => panic!("{error}"),
        });
        let mut socket = socket.unwrap();
        client.pump(attempt, None).unwrap(); // send Hello before testing Welcome
        if timeout {
            client.lock().unwrap().transport.as_mut().unwrap().links[0]
                .as_mut()
                .unwrap()
                .deadline = Instant::now();
            write(&mut socket, &wire(Message::Welcome { slot: 1 }, 0));
        } else {
            let mut bad = wire(Message::Welcome { slot: 1 }, 0);
            bad[codec::HEADER] = 0; // client may not be assigned the host slot
            write(&mut socket, &bad);
        }
        let mut events = Vec::new();
        until(|| {
            events.extend(client.pump(attempt, None).unwrap());
            !events.is_empty()
        });
        assert_eq!(
            events,
            [Event::Failed {
                connection: None,
                error: if timeout {
                    Error::Timeout
                } else {
                    Error::Protocol(OwnerError::Peer)
                },
            }]
        );
        assert_eq!(client.phase(), Ok(Phase::Opening));
        assert!(client.lock().unwrap().peers.iter().all(Option::is_none));
    }
}

#[test]
fn stale_connect_result_pump_send_and_cleanup_leave_new_attempt_untouched() {
    let (host, ha, hp, client, ca, cp) = pair();
    // A TCP setup result can arrive after its captured attempt was retired.
    let pending_listener = TcpListener::bind(address()).unwrap();
    let pending_socket =
        TcpStream::connect_timeout(&pending_listener.local_addr().unwrap(), WAIT).unwrap();
    let late = Link::new(pending_socket, Admission::Welcome).unwrap();
    client.retire(ca).unwrap();
    let replacement = client.begin().unwrap();
    let replacement_address = client.listen(replacement, address(), [8; 16]).unwrap();
    assert_eq!(
        client.install_client(ca, late),
        Err(Error::Protocol(OwnerError::StaleAttempt))
    );
    assert_eq!(
        client.pump(ca, None),
        Err(Error::Protocol(OwnerError::StaleAttempt))
    );
    assert_eq!(
        client.send(cp, start(1)),
        Err(Error::Protocol(OwnerError::StaleAttempt))
    );
    assert_eq!(client.retire(ca), Err(OwnerError::StaleAttempt));
    assert_eq!(client.phase(), Ok(Phase::Lobby));
    assert_eq!(client.lock().unwrap().session, Some([8; 16]));
    assert_eq!(
        client
            .lock()
            .unwrap()
            .transport
            .as_ref()
            .unwrap()
            .listener
            .as_ref()
            .unwrap()
            .local_addr()
            .unwrap(),
        replacement_address
    );
    // A disconnect from the old remote endpoint affects only that old owner.
    until(|| {
        let events = host.pump(ha, None).unwrap();
        if events.is_empty() {
            return false;
        }
        assert_eq!(
            events,
            [Event::Failed {
                connection: Some(hp),
                error: Error::Closed
            }]
        );
        true
    });
    assert!(client.pump(replacement, None).unwrap().is_empty());
}

#[test]
fn unsolicited_welcome_cannot_admit_before_hello_is_sent() {
    let listener = TcpListener::bind(address()).unwrap();
    let client = Owner::new().unwrap();
    let attempt = client.begin().unwrap();
    let socket = TcpStream::connect_timeout(&listener.local_addr().unwrap(), WAIT).unwrap();
    let probe = socket.try_clone().unwrap();
    let mut link = Link::new(socket, Admission::HelloPending).unwrap();
    link.stream
        .enqueue(
            codec::encode(&Frame {
                session: [0; 16],
                sequence: 0,
                message: Message::Hello,
            })
            .unwrap(),
        )
        .unwrap();
    client.install_client(attempt, link).unwrap();
    // The listener is already connected; no accept wait can block this test.
    listener.set_nonblocking(true).unwrap();
    let mut socket = None;
    until(|| match listener.accept() {
        Ok((stream, _)) => {
            socket = Some(stream);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("{error}"),
    });
    write(&mut socket.unwrap(), &wire(Message::Welcome { slot: 1 }, 0));
    // Wait for bytes to arrive without pumping (and thereby sending Hello).
    let mut byte = [0];
    until(|| match probe.peek(&mut byte) {
        Ok(count) => count > 0,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("{error}"),
    });
    assert_eq!(
        client.pump(attempt, None).unwrap(),
        [Event::Failed {
            connection: None,
            error: Error::Handshake
        }]
    );
    assert_eq!(client.phase(), Ok(Phase::Opening));
    assert!(client.lock().unwrap().peers.iter().all(Option::is_none));
}

#[test]
fn reusing_a_disconnected_slot_does_not_revalidate_its_old_connection() {
    let (host, ha, old, client, ca, _) = pair();
    let address = host
        .lock()
        .unwrap()
        .transport
        .as_ref()
        .unwrap()
        .listener
        .as_ref()
        .unwrap()
        .local_addr()
        .unwrap();
    client.retire(ca).unwrap();
    until(|| {
        host.pump(ha, None)
            .unwrap()
            .iter()
            .any(|event| matches!(event, Event::Failed { .. }))
    });
    let client = Owner::new().unwrap();
    let attempt = client.begin().unwrap();
    client.connect(attempt, address, WAIT).unwrap();
    let mut peer = None;
    until(|| {
        if let Some(connection) = connected(host.pump(ha, None).unwrap()) {
            peer = Some(connection);
        }
        connected(client.pump(attempt, None).unwrap());
        peer.is_some() && client.phase() == Ok(Phase::Lobby)
    });
    let peer = peer.unwrap();
    assert_eq!(peer.slot(), old.slot());
    assert_ne!(peer, old);
    assert_eq!(
        host.send(old, start(0)),
        Err(Error::Protocol(OwnerError::Peer))
    );
    host.send(peer, start(0)).unwrap();
    until(|| {
        host.pump(ha, None).unwrap();
        client
            .pump(attempt, None)
            .unwrap()
            .iter()
            .any(|event| matches!(event, Event::Message { message, .. } if message.as_ref() == &start(0)))
    });
}
