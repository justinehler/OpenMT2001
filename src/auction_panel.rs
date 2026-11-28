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
