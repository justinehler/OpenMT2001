//! Auction announcement contents (original 41d3d0/41d700): the category, the
//! announcing player's character and name, and the amount. The binary's
//! `auction_notice` module draws it.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub category: u32,
    pub character: u32,
    pub actor: [u8; 96],
    pub amount: u32,
    /// The original text starts empty and changes only at whole-second callbacks.
    pub seconds: Option<i32>,
}
impl Notice {
    pub fn from_session(
        session: &crate::session::Session<'_>,
        seconds: Option<i32>,
    ) -> Result<Option<Self>, String> {
        let presentation = &session.world.city.auction.presentation;
        if !presentation.visible {
            return Ok(None);
        }
        let [category, participant, amount] = presentation.announcement;
        let player = session
            .roster
            .manager
            .players
            .get(participant as usize)
            .ok_or("Auction announcement participant outside roster")?;
        Ok(Some(Self {
            category,
            character: u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()),
            actor: player.bytes[0x158..0x1b8].try_into().unwrap(),
            amount,
            seconds,
        }))
    }
}
