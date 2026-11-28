//! Auction panel state: the title contents (original 413de0: the lot's
//! category and colour, the high bidder's character and name, the price) and
//! the live panel state Host publishes. The binary's `auction_panel` module
//! draws it.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub category: u32,
    pub category_color: u32,
    pub character: u32,
    pub actor: Vec<u8>,
    pub price: u32,
}
impl Info {
    pub fn from_session(session: &crate::session::Session<'_>) -> Result<Self, String> {
        let record = &session.world.city.auction.controller.record;
        let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let category = word(4);
        let row = session
            .world
            .categories
            .get(category as usize)
            .ok_or("Auction title category outside pool")?;
        let player = session
            .roster
            .manager
            .players
            .get(word(0x5c) as usize)
            .ok_or("Auction high bidder outside roster")?;
        Ok(Self {
            category,
            category_color: u32::from_le_bytes(row[0x94..0x98].try_into().unwrap()),
            character: u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()),
            actor: player.bytes[0x158..0x1b8].to_vec(),
            price: word(0x6c),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Live auction panel state that Host publishes to the frontend each frame.
pub struct Auction {
    pub controls: crate::auction_controls::State,
    pub visible: bool,
    /// 414160 disables the textureless title-right child in network mode.
    pub hidden_bid_toggle_enabled: bool,
    /// Authoritative details858 voice/AI gate published with the live panel.
    pub bid_voice_gate: bool,
    pub info: Option<crate::auction_panel::Info>,
    pub participants: Vec<crate::auction_labels::Participant>,
}

impl Auction {
    pub fn refresh_participants(
        &mut self,
        session: &crate::session::Session<'_>,
    ) -> Result<(), String> {
        self.participants = crate::auction_labels::participants(session)?;
        Ok(())
    }
}

/// Applies one auction presentation call from the AI offer script to the live
/// panel state (moved from the game Host, #99).
pub fn action(
    call: crate::ai_messages::offers::auction::Call,
    auction: &mut Option<Auction>,
    omitted: &mut std::collections::BTreeSet<u32>,
    tutorial_state: u8,
    actor_events: &Option<std::sync::mpsc::Sender<crate::render_requests::auction::ActorEvent>>,
    bid_gate: &std::cell::Cell<bool>,
) -> Result<u32, String> {
    use crate::ai_messages::offers::auction::Call;
    let auction = auction
        .as_mut()
        .ok_or("Auction presentation action arrived without a prepared scene")?;
    match call {
        Call::ShowPanel(0x565b9c) => {
            auction.visible = true;
            // 4140b0 resets the hidden bid-voice/AI gate on panel reopen.
            bid_gate.set(false);
            auction.bid_voice_gate = false;
            if let Some(sender) = actor_events {
                sender
                    .send(crate::render_requests::auction::ActorEvent::BidVoiceGate(
                        false,
                    ))
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            }
        }
        Call::ShowPanel(0x565b94) => (), // Original top panel is part of the renderer.
        Call::ShowPanel(address @ (0x565ba4 | 0x565ba0)) => {
            omitted.insert(address);
        }
        Call::ShowPanel(address) => return Err(format!("Unmapped auction panel{address:08x}")),
        Call::ParticipantEffect { participant, kind } => {
            if let Some(sender) = actor_events {
                sender
                    .send(crate::render_requests::auction::ActorEvent::Effect { participant, kind })
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            } else {
                omitted.insert(0x465980);
            }
        }
        Call::Tutorial(_) if tutorial_state == 1 => {
            return Err("Auction tutorial event needs its tutorial session".into());
        }
        Call::Tutorial(_) => (),
        Call::RefreshTurn(_) => {
            // Invalidate the old text; the host refreshes from the completed
            // controller after the simulation releases its mutable borrow.
            auction.participants.clear();
        }
        Call::WithdrawNotice(participant) => {
            if let Some(sender) = actor_events {
                sender
                    .send(crate::render_requests::auction::ActorEvent::WithdrawNotice(
                        participant,
                    ))
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            } else {
                omitted.insert(0x464769);
            }
        }
    }
    Ok(0)
}
