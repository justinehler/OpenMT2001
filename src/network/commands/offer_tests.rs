//! Human-to-human property offers between connected peers over the real
//! loopback worker. A kind-35 offer (0x48a640) travels as a player message,
//! the owner answers it (0x48b350), and the answer is either a kind-29
//! acceptance the authority schedules for every peer (0x451720 -> 0x465330)
//! or a kind-38 reply (0x48cb70) that keeps the offer's terms.
use super::*;
use crate::{
    human_reply,
    network::{lobby::worker, synchronization::Synchronization},
    player_messages::{self, HumanCall, Packet, Route},
    property_offer,
};
use std::{
    thread,
    time::{Duration, Instant},
};

const CATEGORY: u32 = 9;
const AMOUNT: u32 = 3000;
const IDENTITY: u32 = 77;

/// The authority's game (slot 0) and one client (slot 1), both live.
struct Pair {
    host: TestSession,
    client: TestSession,
    host_adapter: Adapter,
    client_adapter: Adapter,
    host_sync: Synchronization,
    client_sync: Synchronization,
    host_worker: worker::Handle,
    client_worker: worker::Handle,
    host_arena: player_messages::Arena,
    client_arena: player_messages::Arena,
}
impl Pair {
    fn new(image: &crate::pe::PeImage<'_>, board: &crate::board::Board, owner: u8) -> Self {
        let mut host = TestSession::new(image, board);
        let mut client = TestSession::new(image, board);
        let (host_worker, client_worker, peer) = worker::tests::playing_pair(false);
        let hb = host_worker
            .bind_world(host.world.topology.epoch().serial())
            .unwrap();
        let cb = client_worker
            .bind_world(client.world.topology.epoch().serial())
            .unwrap();
        let peers = vec![Some(0), Some(peer)];
        let host_adapter = Adapter::new(hb, true, 0, peers.clone()).unwrap();
        let client_adapter = Adapter::new(cb, false, peer, peers.clone()).unwrap();
        let mut host_sync = Synchronization::new(hb, true, 0, peers.clone()).unwrap();
        let mut client_sync = Synchronization::new(cb, false, peer, peers).unwrap();
        client.roster.status.current_player = client.roster.manager.players[1].reference;
        for (s, sync) in [(&mut host, &mut host_sync), (&mut client, &mut client_sync)] {
            s.world.clock.timing[5] = 1;
            sync.enable(&mut s.tasks).unwrap();
            // Two participants: the broadcaster's signed count is above one.
            put(&mut s.roster.manager.bytes, 12, 2);
            s.world.city.categories[CATEGORY as usize][5] = owner;
            for player in &mut s.roster.manager.players {
                // Sender context (+0xc) and the inbox scratch record (+0x210).
                let scratch = player.reference + 0x800;
                let context = 0x700 + u32::from(player.bytes[0x1bc]);
                put(&mut player.bytes, 12, context);
                put(&mut player.bytes, 0x210, scratch);
                player.auxiliaries.push(player_manager::Auxiliary {
                    reference: scratch,
                    bytes: [0; 0x30],
                });
            }
        }
        Self {
            host,
            client,
            host_adapter,
            client_adapter,
            host_sync,
            client_sync,
            host_worker,
            client_worker,
            host_arena: player_messages::Arena { next: 0x80_0000 },
            client_arena: player_messages::Arena { next: 0x90_0000 },
        }
    }
    /// One frame on each peer: pump the transport, then run the simulation,
    /// whose command drain is the live Runtime's (auction admission included).
    fn frame(&mut self, image: &crate::pe::PeImage<'_>, board: &crate::board::Board) {
        self.host.auction_frame(
            image,
            board,
            &mut self.host_adapter,
            &mut self.host_sync,
            &self.host_worker,
            true,
        );
        self.client.auction_frame(
            image,
            board,
            &mut self.client_adapter,
            &mut self.client_sync,
            &self.client_worker,
            true,
        );
    }
    fn frames_until(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        what: &str,
        mut done: impl FnMut(&mut Self) -> bool,
    ) {
        let until = Instant::now() + Duration::from_secs(60);
        while !done(self) {
            assert!(Instant::now() < until, "{what} never happened");
            self.frame(image, board);
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn session(&mut self, slot: u8) -> &mut TestSession {
        if slot == 0 {
            &mut self.host
        } else {
            &mut self.client
        }
    }
    /// The proposer's human offer (0x48a640) to the owner, routed exactly as
    /// `Session::submit_property_offer_panel` routes it.
    fn offer(&mut self, proposer: u8) -> Packet {
        let authority = proposer == 0;
        let session = self.session(proposer);
        let args = crate::ai_messages::offers::producers::property(
            0x48a640,
            proposer,
            CATEGORY,
            (AMOUNT as f32).to_bits(),
            IDENTITY,
            &session.world.city.categories,
        )
        .unwrap();
        let remaining = player_manager::word(&session.roster.manager.bytes, 12);
        let mut context = player_messages::Context {
            arena: player_messages::Arena { next: 0x70_0000 },
            current_player: session.roster.status.current_player,
            network: [true, authority],
            prior: [0; 48],
        };
        let mut sent = Vec::new();
        assert!(
            property_offer::route(
                args,
                remaining,
                &mut session.roster.manager.players,
                &mut context,
                |dispatch| {
                    sent.push(dispatch);
                    Ok(())
                },
            )
            .unwrap()
        );
        let [property_offer::Dispatch::External { route, packet }] = sent[..] else {
            panic!("offer dispatched {sent:?}");
        };
        // A client hands it to the authority; the authority sends it to the
        // remote owner's peer.
        if authority {
            assert_eq!(route, Route::NetworkRecipient { slot: 1 });
            self.host_adapter
                .send_player_message(&self.host_worker, route, &packet)
                .unwrap();
        } else {
            assert_eq!(route, Route::NetworkSend);
            self.client_adapter
                .send_player_message(&self.client_worker, route, &packet)
                .unwrap();
        }
        packet
    }
    /// Wait for the next player message on `slot`'s peer and route or apply it
    /// as `Session::receive_connected_messages` does. Returns the packet.
    fn receive(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        slot: u8,
    ) -> Packet {
        let mut packet = None;
        self.frames_until(image, board, "player message", |pair| {
            packet = if slot == 0 {
                pair.host_adapter.next_player_message()
            } else {
                pair.client_adapter.next_player_message()
            };
            packet.is_some()
        });
        let packet = packet.unwrap();
        let (session, arena, authority) = if slot == 0 {
            (&mut self.host, &mut self.host_arena, true)
        } else {
            (&mut self.client, &mut self.client_arena, false)
        };
        let current = session.roster.status.current_player;
        let mut forwarded = Vec::new();
        assert!(
            player_messages::receive(
                &packet,
                authority,
                &mut session.roster.manager.players,
                arena,
                current,
                |route, packet, _| {
                    match route {
                        Route::PropertyRefresh { .. } => (),
                        other => forwarded.push((other, *packet)),
                    }
                    Ok(())
                },
            )
            .unwrap()
        );
        assert!(forwarded.is_empty(), "{forwarded:?}");
        packet
    }
    /// The owner's modal (0x48bc70) and its answer (0x48b350), as
    /// `Session::respond_human_dialog` sends it.
    fn answer(&mut self, owner: u8, button: u32) {
        let authority = owner == 0;
        let session = self.session(owner);
        let references: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.reference)
            .collect();
        let mut shown = Vec::new();
        player_messages::consume_human(
            &mut session.roster.manager.players[owner as usize],
            |call, _| {
                shown.push(call);
                Ok(())
            },
        )
        .unwrap();
        assert!(
            matches!(shown[..], [HumanCall::Packet { kind: 0x23, .. }]),
            "{shown:?}"
        );
        let mut answers = Vec::new();
        human_reply::offer_response(
            &mut session.roster.manager.players[owner as usize],
            &mut human_reply::World {
                categories: &mut session.world.city.categories,
                player_references: &references,
            },
            &human_reply::Inputs {
                button,
                message_kind: 0x23,
                command_prior: [0; 52],
            },
            |call, _, _| {
                answers.push(call);
                Ok(())
            },
        )
        .unwrap();
        let [answer] = answers[..] else {
            panic!("owner answered {answers:?}");
        };
        let tick = session.world.clock.timing[0];
        match answer {
            human_reply::Call::Auction { descriptor } => {
                let (adapter, worker, session) = if authority {
                    (&mut self.host_adapter, &self.host_worker, &mut self.host)
                } else {
                    (
                        &mut self.client_adapter,
                        &self.client_worker,
                        &mut self.client,
                    )
                };
                adapter
                    .submit_offer_acceptance(
                        worker,
                        descriptor,
                        owner,
                        &mut session.queue,
                        &mut session.world.city.categories,
                        tick,
                    )
                    .unwrap();
            }
            human_reply::Call::Refuse { packet, .. } => {
                let current = session.roster.status.current_player;
                let mut arena = player_messages::Arena { next: 0x60_0000 };
                let mut sent = Vec::new();
                player_messages::reply_in(
                    &packet,
                    0x26,
                    [0x5f5e0ff; 4],
                    &mut session.roster.manager.players,
                    &mut arena,
                    player_messages::Routing {
                        current_player: current,
                        network: [true, authority],
                        prior: [0; 48],
                    },
                    |route, packet| {
                        if !matches!(route, Route::PropertyRefresh { .. }) {
                            sent.push((route, *packet));
                        }
                        Ok(())
                    },
                )
                .unwrap();
                let [(route, packet)] = sent[..] else {
                    panic!("refusal routed {sent:?}");
                };
                if authority {
                    assert_eq!(route, Route::NetworkRecipient { slot: 1 });
                    self.host_adapter
                        .send_player_message(&self.host_worker, route, &packet)
                        .unwrap();
                } else {
                    assert_eq!(route, Route::NetworkSend);
                    self.client_adapter
                        .send_player_message(&self.client_worker, route, &packet)
                        .unwrap();
                }
            }
        }
    }
    fn inbox(&self, slot: u8) -> u32 {
        let session = if slot == 0 { &self.host } else { &self.client };
        player_manager::word(&session.roster.manager.players[slot as usize].bytes, 0x214)
    }
    /// Pop `slot`'s next inbox message and return it.
    fn pop(&mut self, slot: u8) -> Packet {
        let player = &mut self.session(slot).roster.manager.players[slot as usize];
        let scratch = player_messages::pop(player, |_, _| Ok(()))
            .unwrap()
            .expect("an inbox message");
        player
            .auxiliaries
            .iter()
            .find(|a| a.reference == scratch)
            .unwrap()
            .bytes
    }
    fn assert_same_auction(&self) {
        let (h, c) = (&self.host.world.city, &self.client.world.city);
        assert_eq!(h.auction.controller.active_count, 1);
        assert_eq!(c.auction.controller.active_count, 1);
        assert_eq!(
            h.auction.controller.record[..0x8c],
            c.auction.controller.record[..0x8c]
        );
        assert_eq!(h.categories, c.categories);
    }
    fn close(self) {
        self.host_worker.cancellation().cancel();
        self.client_worker.cancellation().cancel();
    }
}

/// The offer's payload words and context survive into the reply unchanged.
fn assert_keeps_terms(reply: &Packet, offer: &Packet) {
    assert_eq!(player_manager::word(reply, 4), 0x26);
    assert_eq!(reply[0x18..0x2c], offer[0x18..0x2c]);
    assert_eq!(player_manager::word(reply, 0x18), CATEGORY);
    assert_eq!(player_manager::word(reply, 0x1c), AMOUNT);
    assert_eq!(player_manager::word(reply, 0x20), IDENTITY);
}

fn accepted_offer_opens_the_same_auction_on_both_peers(proposer: u8) {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let owner = 1 - proposer;
    let mut pair = Pair::new(&image, &board, owner + 1);
    let offer = pair.offer(proposer);
    assert_eq!((offer[0x14], offer[0x15]), (proposer, owner));
    assert_eq!(pair.receive(&image, &board, owner), offer);
    assert_eq!(pair.inbox(owner), 1);
    assert_eq!(
        pair.inbox(proposer),
        0,
        "the proposer's peer queues nothing"
    );
    pair.answer(owner, 0);
    pair.frames_until(&image, &board, "auction admission on both peers", |p| {
        p.host.world.city.auction.controller.active_count == 1
            && p.client.world.city.auction.controller.active_count == 1
    });
    pair.assert_same_auction();
    // The auction opens with the proposer's bid on the offered property.
    let record = &pair.host.world.city.auction.controller.record;
    assert_eq!(player_manager::word(record, 4), CATEGORY);
    assert_eq!(player_manager::word(record, 0x54), u32::from(proposer));
    assert_eq!(player_manager::word(record, 0x6c), AMOUNT);
    assert_eq!(player_manager::word(record, 0x50), u32::from(owner));
    // Neither peer drew a refusal.
    assert_eq!(pair.inbox(proposer), 0);
    pair.close();
}

fn rejected_offer_returns_its_terms_to_the_proposer(proposer: u8) {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let owner = 1 - proposer;
    let mut pair = Pair::new(&image, &board, owner + 1);
    let offer = pair.offer(proposer);
    pair.receive(&image, &board, owner);
    pair.answer(owner, 1);
    let reply = pair.receive(&image, &board, proposer);
    assert_eq!((reply[0x14], reply[0x15]), (owner, proposer));
    assert_keeps_terms(&reply, &offer);
    assert_eq!(pair.inbox(proposer), 1);
    assert_eq!(pair.inbox(owner), 0);
    assert_keeps_terms(&pair.pop(proposer), &offer);
    // Nothing else changes on either peer.
    for _ in 0..40 {
        pair.frame(&image, &board);
    }
    for s in [&pair.host, &pair.client] {
        assert_eq!(s.world.city.auction.controller.active_count, 0);
        assert_eq!(s.world.city.categories[CATEGORY as usize][5], owner + 1);
    }
    pair.close();
}

#[test]
fn host_offer_accepted_by_the_client_opens_the_same_auction_on_both_peers() {
    accepted_offer_opens_the_same_auction_on_both_peers(0);
}

#[test]
fn client_offer_accepted_by_the_host_opens_the_same_auction_on_both_peers() {
    accepted_offer_opens_the_same_auction_on_both_peers(1);
}

#[test]
fn host_offer_rejected_by_the_client_returns_its_terms() {
    rejected_offer_returns_its_terms_to_the_proposer(0);
}

#[test]
fn client_offer_rejected_by_the_host_returns_its_terms() {
    rejected_offer_returns_its_terms_to_the_proposer(1);
}

#[test]
fn acceptance_during_another_auction_answers_kind_38_with_the_original_terms() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut pair = Pair::new(&image, &board, 2);
    // Another lot is already being auctioned.
    let mut busy = [0; 52];
    put(&mut busy, 4, 0x1d);
    busy[16] = 1;
    put(&mut busy, 20, 3);
    put(&mut busy, 24, u32::MAX);
    put(&mut busy, 28, 500);
    let tick = pair.host.world.clock.timing[0];
    pair.host_adapter
        .submit_auction(
            &pair.host_worker,
            busy,
            0,
            &mut pair.host.queue,
            &mut pair.host.world.city.categories,
            tick,
        )
        .unwrap();
    pair.frames_until(&image, &board, "first auction admission", |p| {
        p.host.world.city.auction.controller.active_count == 1
            && p.client.world.city.auction.controller.active_count == 1
    });
    // The host's human offers for the client's property and the client accepts.
    let offer = pair.offer(0);
    pair.receive(&image, &board, 1);
    pair.answer(1, 0);
    // Both peers refuse the second admission; only the authority answers its
    // requester, here the host's own human.
    pair.frames_until(&image, &board, "busy admission reply", |p| p.inbox(0) == 1);
    let reply = pair.pop(0);
    assert_eq!((reply[0x14], reply[0x15]), (0xff, 0));
    assert_keeps_terms(&reply, &offer);
    assert_eq!(pair.inbox(1), 0);
    for s in [&pair.host, &pair.client] {
        // The first auction keeps its lot and opening terms.
        let record = &s.world.city.auction.controller.record;
        assert_eq!(s.world.city.auction.controller.active_count, 1);
        assert_eq!(player_manager::word(record, 4), 3);
        assert_eq!(player_manager::word(record, 0x6c), 500);
        assert_eq!(s.world.city.categories[CATEGORY as usize][5], 2);
    }
    pair.close();
}

#[test]
fn client_city_offer_runs_the_authority_broadcaster_for_both_peers() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    // With one participant the broadcaster sells directly: a kind-36 reply to
    // the buyer, then a kind-22 transfer to it. With two it opens an auction
    // whose initiating bidder is the buyer.
    for remaining in [1, 2] {
        let mut pair = Pair::new(&image, &board, 0);
        for s in [&mut pair.host, &mut pair.client] {
            put(&mut s.roster.manager.bytes, 12, remaining);
        }
        let offer = pair.offer(1);
        assert_eq!((offer[0x14], offer[0x15]), (1, 0xff));
        // The authority's router meets the broadcast and its broadcaster arm,
        // as `Session::receive_connected_messages` runs them.
        let mut received = None;
        pair.frames_until(&image, &board, "city offer at the authority", |p| {
            received = p.host_adapter.next_player_message();
            received.is_some()
        });
        let received = received.unwrap();
        assert_eq!(received, offer);
        let current = pair.host.roster.status.current_player;
        let mut broadcast = None;
        player_messages::receive(
            &received,
            true,
            &mut pair.host.roster.manager.players,
            &mut pair.host_arena,
            current,
            |route, packet, _| {
                assert_eq!(route, Route::Broadcast);
                broadcast = Some(*packet);
                Ok(())
            },
        )
        .unwrap();
        let broadcast = broadcast.unwrap();
        assert!(property_offer::is_offer_broadcast(&broadcast));
        let mut dispatched = Vec::new();
        property_offer::broadcast(
            broadcast,
            remaining,
            &mut pair.host.roster.manager.players,
            &mut pair.host_arena,
            player_messages::Routing {
                current_player: current,
                network: [true, true],
                prior: [0; 48],
            },
            |dispatch| {
                dispatched.push(dispatch);
                Ok(())
            },
        )
        .unwrap();
        let tick = pair.host.world.clock.timing[0];
        for dispatch in dispatched {
            match dispatch {
                property_offer::Dispatch::External { route, packet } => {
                    assert_eq!(route, Route::NetworkRecipient { slot: 1 });
                    assert_eq!(remaining, 1);
                    assert_eq!(player_manager::word(&packet, 4), 0x24);
                    pair.host_adapter
                        .send_player_message(&pair.host_worker, route, &packet)
                        .unwrap();
                }
                property_offer::Dispatch::Queue { descriptor, .. } => pair
                    .host_adapter
                    .submit_auction(
                        &pair.host_worker,
                        descriptor,
                        0,
                        &mut pair.host.queue,
                        &mut pair.host.world.city.categories,
                        tick,
                    )
                    .unwrap(),
            }
        }
        if remaining == 1 {
            let reply = pair.receive(&image, &board, 1);
            assert_eq!(player_manager::word(&reply, 4), 0x24);
            assert_eq!(reply[0x18..0x2c], offer[0x18..0x2c]);
            pair.frames_until(&image, &board, "transfer on both peers", |p| {
                p.host.world.city.categories[CATEGORY as usize][5] == 2
                    && p.client.world.city.categories[CATEGORY as usize][5] == 2
            });
            assert_eq!(
                pair.host.world.city.categories,
                pair.client.world.city.categories
            );
            assert_eq!(
                pair.host.world.city.ledgers.players[1].cash,
                pair.client.world.city.ledgers.players[1].cash
            );
            assert_eq!(pair.host.world.city.auction.controller.active_count, 0);
        } else {
            pair.frames_until(&image, &board, "auction admission on both peers", |p| {
                p.host.world.city.auction.controller.active_count == 1
                    && p.client.world.city.auction.controller.active_count == 1
            });
            pair.assert_same_auction();
            let record = &pair.client.world.city.auction.controller.record;
            assert_eq!(player_manager::word(record, 0x54), 1);
            assert_eq!(player_manager::word(record, 0x6c), AMOUNT);
            assert_eq!(pair.inbox(1), 0);
        }
        pair.close();
    }
}

#[test]
fn only_the_accepting_owners_peer_may_send_its_acceptance() {
    let (host, client, peer) = worker::tests::playing_pair(false);
    let peers = vec![Some(0), Some(peer)];
    let hb = host.bind_world(1).unwrap();
    let cb = client.bind_world(1).unwrap();
    let authority = Adapter::new(hb, true, 0, peers.clone()).unwrap();
    let mut replica = Adapter::new(cb, false, peer, peers).unwrap();
    let mut acceptance = [0; 52];
    put(&mut acceptance, 4, 0x1d);
    acceptance[16] = 1;
    put(&mut acceptance, 20, CATEGORY);
    put(&mut acceptance, 24, 0);
    put(&mut acceptance, 28, AMOUNT);
    let mut queue = live_commands::Queue::default();
    let mut categories = [[0; 380]; 52];
    // The client may not accept for the host's row, nor send an unscheduled
    // or non-admission descriptor.
    assert!(
        replica
            .submit_offer_acceptance(&client, acceptance, 0, &mut queue, &mut categories, 100)
            .is_err()
    );
    let mut immediate = acceptance;
    immediate[16] = 0;
    assert!(
        replica
            .submit_offer_acceptance(&client, immediate, 1, &mut queue, &mut categories, 100)
            .is_err()
    );
    // The authority accepts the client's acceptance only from the client.
    let request = Request::auction(0, 2, acceptance, 0);
    assert!(authority.authorize(peer, &request).is_ok());
    assert_eq!(authority.authorize(0, &request), Err(Error::Peer));
    let forged = Request::auction(0, 1, acceptance, 0);
    assert_eq!(authority.authorize(peer, &forged), Err(Error::Peer));
    host.cancellation().cancel();
    client.cancellation().cancel();
}
