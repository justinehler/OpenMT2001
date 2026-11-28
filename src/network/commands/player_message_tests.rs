//! Player inbox messages over the real loopback worker: a client's network
//! send reaches the authority's router (0x451720), and a reply for a remote
//! human reaches only that human's peer, whose apply (0x450a30) queues it.
use super::*;
use crate::player_messages::{self, Packet, Route};
use std::time::{Duration, Instant};

/// A live human row with an empty inbox and its scratch record (0x48a1c0).
fn human(slot: u8, reference: u32) -> player_manager::Player {
    let mut bytes = vec![0; 0x220];
    bytes[0x10] = 1;
    bytes[0x1bc] = slot;
    put(&mut bytes, 12, 0x700 + u32::from(slot));
    put(&mut bytes, 0x210, reference + 1);
    player_manager::Player {
        reference,
        bytes,
        auxiliaries: vec![player_manager::Auxiliary {
            reference: reference + 1,
            bytes: [0; 0x30],
        }],
    }
}
fn roster() -> Vec<player_manager::Player> {
    vec![human(0, 0x1000), human(1, 0x2000)]
}

struct Peer {
    adapter: Adapter,
    queue: live_commands::Queue,
    categories: [[u8; 380]; 52],
    pause: crate::pause::State,
    tasks: Vec<[u8; 20]>,
}
impl Peer {
    fn new(adapter: Adapter) -> Self {
        Self {
            adapter,
            queue: Default::default(),
            categories: [[0; 380]; 52],
            pause: Default::default(),
            tasks: crate::timers::startup_tasks([0; 20]).to_vec(),
        }
    }
    /// Pump the worker until a player message arrives. Below the first
    /// ceiling (25) so the authority grants no window meanwhile.
    fn wait_for_message(
        &mut self,
        worker: &crate::network::lobby::worker::Handle,
        tick: u32,
    ) -> Packet {
        let limit = Instant::now() + Duration::from_secs(20);
        loop {
            self.adapter
                .pump(
                    worker,
                    tick,
                    &mut self.queue,
                    &mut self.categories,
                    Targets::default(),
                    &mut self.pause,
                    &mut self.tasks,
                )
                .unwrap();
            if let Some(packet) = self.adapter.next_player_message() {
                return packet;
            }
            assert!(Instant::now() < limit, "player message never arrived");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn kind(packet: &Packet) -> u32 {
    player_manager::word(packet, 4)
}

#[test]
fn client_card_request_reaches_the_authority_deck_and_only_the_client_inbox_takes_the_reply() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let owners = vec![Some(0), Some(peer)];
    let hb = host.bind_world(1).unwrap();
    let cb = client.bind_world(1).unwrap();
    let mut authority = Peer::new(Adapter::new(hb, true, 0, owners.clone()).unwrap());
    let mut replica = Peer::new(Adapter::new(cb, false, peer, owners).unwrap());
    let mut host_players = roster();
    let mut client_players = roster();
    let mut host_arena = player_messages::Arena { next: 0x10_0000 };
    let mut client_arena = player_messages::Arena { next: 0x20_0000 };

    // The client's hourly account countdown asks for a card (0x41cd70 ->
    // 0x48cab0): broadcast kind 0x31 from slot 1. A client's 0x450490 arm
    // hands every player message to the transport.
    let mut request = None;
    player_messages::produce_in(
        [-1, 1, 0x31, 0, 0, 0, 0, -999],
        1,
        &mut client_players,
        &mut client_arena,
        player_messages::Routing {
            current_player: 0x2000,
            network: [true, false],
            prior: [0; 48],
        },
        |route, packet| {
            assert_eq!(route, Route::NetworkSend);
            request = Some(*packet);
            Ok(())
        },
    )
    .unwrap();
    let request = request.unwrap();
    assert_eq!((request[0x14], request[0x15]), (1, 0xff));
    replica
        .adapter
        .send_player_message(&client, Route::NetworkSend, &request)
        .unwrap();
    // A client never forwards to another peer, and the authority never sends
    // to itself.
    assert!(
        replica
            .adapter
            .send_player_message(&client, Route::NetworkRecipient { slot: 0 }, &request)
            .is_err()
    );
    assert!(
        authority
            .adapter
            .send_player_message(&host, Route::NetworkRecipient { slot: 0 }, &request)
            .is_err()
    );

    // The authority's router sends the broadcast to its deck, which answers
    // the sender with kind 0x33 (0x48cb70). Player 1 is a remote human, so the
    // reply is forwarded rather than queued on the host.
    let received = authority.wait_for_message(&host, 10);
    assert_eq!(received, request);
    let mut broadcast = None;
    player_messages::receive(
        &received,
        true,
        &mut host_players,
        &mut host_arena,
        0x1000,
        |route, packet, _| {
            assert_eq!(route, Route::Broadcast);
            broadcast = Some(*packet);
            Ok(())
        },
    )
    .unwrap();
    let mut deck = crate::event_cards::Deck::new([7, 0x343fd, 0x269ec3]);
    let mut drawn = None;
    let handled = deck
        .broadcast(&broadcast.unwrap(), 3, &image, |card, _| {
            drawn = Some(card);
            player_messages::reply_in(
                &received,
                0x33,
                [card, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff],
                &mut host_players,
                &mut host_arena,
                player_messages::Routing {
                    current_player: 0x1000,
                    network: [true, true],
                    prior: [0; 48],
                },
                |route, packet| {
                    assert_eq!(route, Route::NetworkRecipient { slot: 1 });
                    authority.adapter.send_player_message(&host, route, packet)
                },
            )
            .map(|_| ())
        })
        .unwrap();
    assert!(handled);
    let card = drawn.unwrap();
    assert_eq!(deck.used[card as usize], 1);
    assert!(
        host_players
            .iter()
            .all(|p| player_manager::word(&p.bytes, 0x214) == 0),
        "the authority queues nothing for a remote human"
    );

    // The client applies the forward: only its own current player queues it,
    // and its inbox consumer sees the card reply.
    let reply = replica.wait_for_message(&client, 10);
    assert_eq!(kind(&reply), 0x33);
    assert_eq!((reply[0x14], reply[0x15]), (0xff, 1));
    assert_eq!(player_manager::word(&reply, 0x18), card);
    assert!(
        player_messages::receive(
            &reply,
            false,
            &mut client_players,
            &mut client_arena,
            0x2000,
            |route, _, _| panic!("client apply routed {route:?}"),
        )
        .unwrap()
    );
    assert_eq!(player_manager::word(&client_players[0].bytes, 0x214), 0);
    let mut consumed = Vec::new();
    player_messages::consume_human(&mut client_players[1], |call, _| {
        consumed.push(call);
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        consumed[..],
        [player_messages::HumanCall::Packet { kind: 0x33, .. }]
    ));
    // A peer that is not the recipient's leaves the message alone.
    let mut other = roster();
    assert!(
        !player_messages::receive(
            &reply,
            false,
            &mut other,
            &mut client_arena,
            0x1000,
            |_, _, _| { Ok(()) }
        )
        .unwrap()
    );
    assert!(
        other
            .iter()
            .all(|p| player_manager::word(&p.bytes, 0x214) == 0)
    );
    host.cancellation().cancel();
    client.cancellation().cancel();
}

#[test]
fn authority_drops_a_player_message_that_speaks_for_another_peers_row() {
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let owners = vec![Some(0), Some(peer)];
    let hb = host.bind_world(1).unwrap();
    let cb = client.bind_world(1).unwrap();
    let mut authority = Peer::new(Adapter::new(hb, true, 0, owners.clone()).unwrap());
    let replica = Peer::new(Adapter::new(cb, false, peer, owners).unwrap());
    // Sender slot 0 is the authority's own row.
    let forged = player_messages::descriptor([-1, 0, 0x31, 0, 0, 0, 0, 0], 0, [0; 48]);
    let honest = player_messages::descriptor([-1, 1, 0x31, 0, 0, 0, 0, 0], 0, [0; 48]);
    for packet in [forged, honest] {
        replica
            .adapter
            .send_player_message(&client, Route::NetworkSend, &packet)
            .unwrap();
    }
    assert_eq!(authority.wait_for_message(&host, 10), honest);
    assert!(authority.adapter.next_player_message().is_none());
    host.cancellation().cancel();
    client.cancellation().cancel();
}

#[test]
fn player_message_wire_form_is_the_descriptor_and_rejects_world_kinds() {
    use crate::network::{Frame, codec};
    let packet = player_messages::descriptor([1, 0, 0x33, 9, 0, 0, 0, 0], 0, [0x5a; 48]);
    let frame = Frame {
        session: [3; 16],
        sequence: 4,
        message: Message::PlayerMessage(packet),
    };
    let bytes = codec::encode(&frame).unwrap();
    assert_eq!(bytes.len(), codec::HEADER + 48);
    assert_eq!(&bytes[codec::HEADER..], &packet);
    assert_eq!(codec::decode(&bytes).unwrap(), frame);
    // Kinds below 33 are world commands, never inbox messages.
    let mut world = packet;
    world[4..8].copy_from_slice(&0x0fu32.to_le_bytes());
    let mut bad = frame.clone();
    bad.message = Message::PlayerMessage(world);
    assert!(codec::encode(&bad).is_err());
    let mut bad = frame;
    let mut stray = packet;
    stray[0x15] = 6;
    bad.message = Message::PlayerMessage(stray);
    assert!(codec::encode(&bad).is_err());
}
