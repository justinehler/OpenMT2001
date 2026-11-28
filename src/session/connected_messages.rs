//! Player inbox messages between connected peers (0x450490 -> 0x451fe0 ->
//! 0x451720 on the authority, 0x450a30 on a client).
use super::{Session, tick::Event};
use crate::{
    live_commands,
    player_messages::{self, Packet, Route},
    property_offer,
};

/// Broadcast kinds the original broadcaster (0x48c1c0) answers from the deck:
/// draw a card, draw a named card, release a card.
fn is_card_broadcast(packet: &Packet) -> bool {
    matches!(
        u32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]),
        0x31 | 0x32 | 0x34
    )
}

/// What the authority sends once a frame's received messages are routed, in
/// the order the router produced it.
enum Outgoing {
    Message(Route, Packet),
    /// A command the broadcaster scheduled (kind 29 or 22), for every peer.
    Command([u8; 52]),
}

impl Session<'_> {
    /// Route every player message the peers sent since the last frame. The
    /// authority runs the broadcaster (0x48c1c0) for card requests and city
    /// offers and sends each reply for a remote human to that human's peer; a
    /// client queues a message only for its own current player. Presentation
    /// effects go to `boundary`.
    pub(super) fn receive_connected_messages(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        prior: Packet,
        boundary: &mut impl FnMut(Event<'_>, &crate::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        let Some(connection) = &mut self.connected else {
            return Ok(());
        };
        let authority = connection.commands.is_host();
        let current = self.roster.status.current_player;
        let remaining = crate::player_manager::word(&self.roster.manager.bytes, 12);
        let mut arena = player_messages::Arena {
            next: self.allocation_next,
        };
        let outgoing = std::cell::RefCell::new(Vec::new());
        let world = &mut self.world;
        let players = &mut self.roster.manager.players;
        let deck = &mut self.event_cards;
        let routing = player_messages::Routing {
            current_player: current,
            network: [true, true],
            prior,
        };
        let result = (|| {
            while let Some(packet) = connection.commands.next_player_message() {
                let broadcast = std::cell::Cell::new(None);
                let mut deliver = |route: Route, packet: &Packet| -> Result<(), String> {
                    match route {
                        Route::NetworkRecipient { .. } => outgoing
                            .borrow_mut()
                            .push(Outgoing::Message(route, *packet)),
                        Route::Broadcast
                            if is_card_broadcast(packet)
                                || property_offer::is_offer_broadcast(packet) =>
                        {
                            broadcast.set(Some(*packet));
                        }
                        Route::PropertyRefresh { category } => {
                            // 0x451720/0x450a30 refresh the property view (0x405260)
                            // before the panel boundary.
                            crate::category_ownership::property_view(
                                world
                                    .city
                                    .categories
                                    .get_mut(category as usize)
                                    .ok_or("received property refresh leaves category pool")?,
                                0,
                                1,
                            );
                            boundary(
                                Event::Command(live_commands::Event::PlayerMessageRoute {
                                    route,
                                    packet: *packet,
                                }),
                                &world.clock,
                            )?;
                        }
                        _ => {
                            boundary(
                                Event::Command(live_commands::Event::PlayerMessageRoute {
                                    route,
                                    packet: *packet,
                                }),
                                &world.clock,
                            )?;
                        }
                    }
                    Ok(())
                };
                player_messages::receive(
                    &packet,
                    authority,
                    players,
                    &mut arena,
                    current,
                    |route, packet, _| deliver(route, packet),
                )?;
                let Some(request) = broadcast.get() else {
                    continue;
                };
                if property_offer::is_offer_broadcast(&request) {
                    // A peer's offer to the city meets the same broadcaster
                    // arm as a local one.
                    property_offer::broadcast(
                        request,
                        remaining,
                        players,
                        &mut arena,
                        routing,
                        |dispatch| match dispatch {
                            property_offer::Dispatch::Queue { descriptor, .. } => {
                                outgoing.borrow_mut().push(Outgoing::Command(descriptor));
                                Ok(())
                            }
                            property_offer::Dispatch::External { route, packet } => {
                                deliver(route, &packet)
                            }
                        },
                    )?;
                    continue;
                }
                // 0x48c1c0 answers a card request with a kind-0x33 reply (0x48cb70)
                // to its sender while the deck update is still in progress.
                deck.broadcast(&request, world.clock.calendar[4], image, |card, _| {
                    player_messages::reply_in(
                        &request,
                        0x33,
                        [card, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff],
                        players,
                        &mut arena,
                        routing,
                        |route, packet| deliver(route, packet),
                    )
                    .map(|_| ())
                })?;
            }
            Ok::<(), String>(())
        })();
        self.allocation_next = arena.next;
        result?;
        for item in outgoing.into_inner() {
            match item {
                Outgoing::Message(route, packet) => {
                    connection
                        .commands
                        .send_player_message(&connection.worker, route, &packet)?
                }
                Outgoing::Command(descriptor) => connection.commands.submit_auction(
                    &connection.worker,
                    descriptor,
                    0,
                    &mut self.live_commands,
                    &mut self.world.city.categories,
                    self.world.clock.timing[0],
                )?,
            }
        }
        Ok(())
    }
}
