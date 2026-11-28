//! Human event-card controls against the same retained roster, deck and command queue.
use super::{CommandAssets, Session};
use crate::{
    event_cards::{self, human, lifecycle},
    live_commands, player_messages,
};
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Open,
    Button { event: u32, button: u32 },
    Select { category: i32, building: i32 },
    Request(Option<u32>),
}

impl Session<'_> {
    pub fn human_card(
        &mut self,
        action: Action,
        assets: &CommandAssets<'_, '_>,
        network: [bool; 2],
        mut presentation: impl FnMut(human::Call, &crate::player_manager::Player) -> Result<(), String>,
    ) -> Result<(), String> {
        let current = self.roster.status.current_player;
        let slot = self
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == current)
            .ok_or("human card current player unresolved")?;
        let references: Vec<_> = self
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.reference)
            .collect();
        let nodes: Vec<_> = assets
            .board
            .blocks
            .iter()
            .map(|b| lifecycle::Node {
                reference: 0xe0000000 + b.id as u32 * 284,
                kind: 4,
                category: b.id as u32,
            })
            .collect();
        let targets = lifecycle::Targets {
            buildings: &self.world.city.buildings,
            businesses: &self.world.city.ledgers.businesses,
            categories: &self.world.city.categories,
            players: &references,
            nodes: &nodes,
        };
        let admitted = word(&self.roster.manager.bytes, 12);
        let (before, tail) = self.roster.manager.players.split_at_mut(slot);
        let (player, after) = tail.split_first_mut().unwrap();
        let reference = word(&player.bytes, 0x21c);
        let mut arena = player_messages::Arena {
            next: self.allocation_next,
        };
        let routing = player_messages::Routing {
            current_player: current,
            network,
            prior: [0; 48],
        };
        let mut outgoing_cards = Vec::new();
        // A client's card requests and releases go to the authority's deck.
        let mut outgoing_messages = Vec::new();
        let mut call = |call, player: &mut crate::player_manager::Player| match call {
            human::Call::Submit(command) => {
                if self.connected.is_some() {
                    outgoing_cards.push(command);
                    return Ok(());
                }
                if network[0] && !network[1] {
                    return Err("human card command requires network transport".into());
                }
                self.live_commands.submit(
                    live_commands::Command::EventCard(command),
                    self.world.clock.timing[0],
                    true,
                    |_| Err("wrapped human card command requires synchronous execution".into()),
                )
            }
            human::Call::Send(args) => {
                let mut players = player_messages::SplitPlayers {
                    before,
                    current: player,
                    after,
                };
                let mut broadcast = None;
                player_messages::produce_in(
                    args,
                    admitted,
                    &mut players,
                    &mut arena,
                    routing,
                    |route, packet| {
                        if route == player_messages::Route::Broadcast {
                            broadcast = Some(*packet);
                            Ok(())
                        } else if route == player_messages::Route::NetworkSend
                            && self.connected.is_some()
                        {
                            outgoing_messages.push(*packet);
                            Ok(())
                        } else {
                            Err(format!(
                                "human card route {route:?} requires host transport"
                            ))
                        }
                    },
                )?;
                if let Some(packet) = broadcast
                    && !self.event_cards.broadcast(
                        &packet,
                        self.world.clock.calendar[4],
                        assets.image,
                        |card, _| {
                            player_messages::reply_in(
                                &packet,
                                0x33,
                                [card, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff],
                                &mut players,
                                &mut arena,
                                routing,
                                |route, _| {
                                    Err(format!(
                                        "human card reply {route:?} requires host transport"
                                    ))
                                },
                            )
                            .map(|_| ())
                        },
                    )?
                {
                    return Err("unrecognized human card broadcast".into());
                }
                Ok(())
            }
            _ => presentation(call, player),
        };
        let validate =
            |p: &mut crate::player_manager::Player, a| targets.validate(p, a, assets.image);
        let result = match action {
            Action::Open => human::open(player, reference, assets.image, validate, &mut call),
            Action::Button { event, button } => human::button(
                player,
                reference,
                assets.image,
                event,
                button,
                validate,
                &mut call,
            ),
            Action::Request(card) => human::request(player, reference, card, &mut call),
            Action::Select { category, building } => event_cards::ai::select(
                player,
                reference,
                category,
                building,
                |id| {
                    nodes
                        .iter()
                        .find(|n| n.category == id as u32)
                        .map(|n| n.reference)
                        .ok_or("human card category node unresolved".into())
                },
                validate,
            )
            .map(|_| ()),
        };
        self.allocation_next = arena.next;
        result?;
        if let Some(connection) = &mut self.connected {
            for packet in outgoing_messages {
                connection.commands.send_player_message(
                    &connection.worker,
                    player_messages::Route::NetworkSend,
                    &packet,
                )?;
            }
            for command in outgoing_cards {
                let targets = crate::network::commands::Targets::new(
                    &self.world.city.buildings,
                    &self.world.city.ledgers.businesses,
                    &self.world.city.categories,
                );
                connection.commands.submit(
                    &connection.worker,
                    live_commands::Command::EventCard(command),
                    &mut self.live_commands,
                    &mut self.world.city.categories,
                    targets,
                    self.world.clock.timing[0],
                )?;
            }
        }
        Ok(())
    }
}

/// Test driver (the binary's Session tests); not used by the game.
#[cfg(any(test, feature = "test-support"))]
pub fn exercise(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    policy: &mut super::tick::Policy,
) {
    use super::tick;
    let slot = session
        .roster
        .manager
        .players
        .iter()
        .position(|p| p.reference == session.roster.status.current_player)
        .unwrap();
    let state = |session: &Session<'_>| {
        let p = &session.roster.manager.players[slot];
        let a = p
            .auxiliaries
            .iter()
            .find(|a| a.reference == word(&p.bytes, 0x21c))
            .unwrap();
        (word(&p.bytes, 0x1b8), word(&a.bytes, 0x18), a.bytes[8])
    };
    let mut advance = |session: &mut Session<'_>| {
        session
            .advance_session(assets, policy, |event, _| {
                if matches!(event, tick::Event::Ai(_)) {
                    return Err(
                        "human card regression unexpectedly entered strategy boundary".into(),
                    );
                }
                Ok(0)
            })
            .unwrap();
    };
    // Force an available deck, then use the actual32 broadcast and33 inbox.
    session.event_cards.release(65).unwrap();
    session.event_cards.rebuild();
    session
        .human_card(Action::Request(Some(65)), assets, [false, false], |_, _| {
            Ok(())
        })
        .unwrap();
    assert_eq!(session.event_cards.used[65], 1);
    for _ in 0..20 {
        if state(session).0 == 65 {
            break;
        }
        advance(session);
    }
    assert_eq!(
        state(session).0,
        65,
        "specific-card request reaches normal human inbox"
    );
    let mut calls = vec![];
    session
        .human_card(Action::Open, assets, [false, false], |c, _| {
            calls.push(c);
            Ok(())
        })
        .unwrap();
    assert_eq!(calls, [human::Call::Present(65), human::Call::Activate]);
    session
        .human_card(
            Action::Button {
                event: 12,
                button: 2,
            },
            assets,
            [false, false],
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(state(session).1, 2);
    session
        .human_card(
            Action::Select {
                category: -1,
                building: -1,
            },
            assets,
            [false, false],
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(state(session).2, 0);
    calls.clear();
    session
        .human_card(
            Action::Button {
                event: 12,
                button: 1,
            },
            assets,
            [false, false],
            |c, _| {
                calls.push(c);
                Ok(())
            },
        )
        .unwrap();
    assert!(
        calls.is_empty(),
        "invalid target preserves held card and open dialog"
    );
    assert_eq!(state(session).0, 65);
    session
        .human_card(
            Action::Button {
                event: 12,
                button: 3,
            },
            assets,
            [false, false],
            |c, _| {
                calls.push(c);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(calls, [human::Call::Close]);
    assert_eq!(state(session), (0, 0, 0));
    assert_eq!(session.event_cards.used[65], 0);

    session.event_cards.release(1).unwrap();
    session.event_cards.rebuild();
    session
        .human_card(Action::Request(Some(1)), assets, [false, false], |_, _| {
            Ok(())
        })
        .unwrap();
    for _ in 0..20 {
        if state(session).0 == 1 {
            break;
        }
        advance(session);
    }
    assert_eq!(state(session).0, 1);
    let ledger = session.world.ledgers.players[slot].ledger(0x24).unwrap();
    let due = session.world.clock.timing[0] + 25;
    session
        .human_card(Action::Open, assets, [false, false], |_, _| Ok(()))
        .unwrap();
    assert_eq!(state(session), (0, 0, 0));
    assert_eq!(session.event_cards.used[1], 1);
    while session.world.clock.timing[0] < due {
        advance(session);
        if session.world.clock.timing[0] < due {
            assert_eq!(session.event_cards.used[1], 1);
        }
    }
    assert_eq!(session.event_cards.used[1], 0);
    assert_eq!(
        session.world.ledgers.players[slot].ledger(0x24).unwrap(),
        ledger + 200.
    );
    println!(
        "Human card controls: actual32/33 request/receipt, hold, invalid target, discard34, command1a25-tick credit passed"
    );
}
