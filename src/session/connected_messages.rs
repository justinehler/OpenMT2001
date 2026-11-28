//! Player inbox messages between connected peers (0x450490 -> 0x451fe0 ->
//! 0x451720 on the authority, 0x450a30 on a client).
use super::{Session, tick::Event};
use crate::{
    live_commands,
    player_messages::{self, Packet, Route},
};

/// Broadcast kinds the original broadcaster (0x48c1c0) answers from the deck:
/// draw a card, draw a named card, release a card.
fn is_card_broadcast(packet: &Packet) -> bool {
    matches!(
        u32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]),
        0x31 | 0x32 | 0x34
    )
}

impl Session<'_> {
    /// Route every player message the peers sent since the last frame. The
    /// authority runs the broadcaster for card requests and sends each reply
    /// for a remote human to that human's peer; a client queues a message only
    /// for its own current player. Presentation effects go to `boundary`.
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
        let mut arena = player_messages::Arena {
            next: self.allocation_next,
        };
        let mut outgoing = Vec::new();
        let world = &mut self.world;
        let players = &mut self.roster.manager.players;
        let deck = &mut self.event_cards;
        let result = (|| {
            while let Some(packet) = connection.commands.next_player_message() {
                let card_request = std::cell::Cell::new(None);
                let mut deliver = |route: Route, packet: &Packet| -> Result<(), String> {
                    match route {
                        Route::NetworkRecipient { .. } => outgoing.push((route, *packet)),
                        Route::Broadcast if is_card_broadcast(packet) => {
                            card_request.set(Some(*packet));
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
                let Some(request) = card_request.get() else {
                    continue;
                };
                // 0x48c1c0 answers a card request with a kind-0x33 reply (0x48cb70)
                // to its sender while the deck update is still in progress.
                deck.broadcast(&request, world.clock.calendar[4], image, |card, _| {
                    player_messages::reply_in(
                        &request,
                        0x33,
                        [card, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff],
                        players,
                        &mut arena,
                        player_messages::Routing {
                            current_player: current,
                            network: [true, true],
                            prior,
                        },
                        |route, packet| deliver(route, packet),
                    )
                    .map(|_| ())
                })?;
            }
            Ok::<(), String>(())
        })();
        self.allocation_next = arena.next;
        result?;
        for (route, packet) in outgoing {
            connection
                .commands
                .send_player_message(&connection.worker, route, &packet)?;
        }
        Ok(())
    }
}
