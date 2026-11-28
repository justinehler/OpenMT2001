//! The build-panel logic moved into the library (`openmt2001::build_panel`, #99).
//! `Context` reads the Session and moves with it.
pub use openmt2001::build_panel::*;

/// Read-only worker snapshot. Raw retained records preserve the distinction
/// between actual scenario settings and executable defaults.
#[derive(Clone)]
pub struct Context {
    pub category: usize,
    pub category_record: [u8; 0x17c],
    pub catalog: openmt2001::business_pool::BusinessCatalog,
    pub products: [[[u32; 2]; 6]; 45],
    pub commodities: [[u8; 0x1c]; 32],
    pub current_owner: u8,
    pub player_active: bool,
    pub category_owners: [u8; 52],
    pub players: Vec<Vec<u8>>,
    pub era: u32,
}
impl Context {
    pub fn from_session(
        session: &crate::fresh_city::Session<'_>,
        category: usize,
    ) -> Result<Self, String> {
        let player = session
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == session.roster.status.current_player)
            .ok_or("planning current player missing")?;
        Ok(Self {
            category,
            category_record: *session
                .world
                .categories
                .get(category)
                .ok_or("planning category outside city")?,
            catalog: session.world.catalog.info.clone(),
            products: session.world.catalog.products,
            commodities: session.world.commodities.rows,
            current_owner: player.bytes[0x1bc].wrapping_add(1),
            player_active: player.bytes[0x10] != 0,
            category_owners: session.world.categories.each_ref().map(|row| row[5]),
            players: session
                .roster
                .manager
                .players
                .iter()
                .map(|p| p.bytes.to_vec())
                .collect(),
            era: session.world.clock.calendar[2],
        })
    }
    pub fn entry(&self) -> Entry {
        if controller_exits(self.category_record[0x6c], self.player_active) {
            return Entry::Exit;
        }
        entry(
            self.category_record[0x6d],
            self.category_record[5] == self.current_owner,
            u32::from_le_bytes(self.category_record[0x78..0x7c].try_into().unwrap()),
        )
    }
    /// Original commodity rows at PE 0x53be98 carry the localized full
    /// text ID at +4. The +8 ID is the shorter poll/UI label.
    pub fn provides_text_ids(&self, business: u32) -> Result<Vec<u32>, String> {
        let row = self
            .catalog
            .row(business as usize)
            .ok_or("planning business outside catalog")?;
        let count = usize::from(row[0x4c]);
        if count > 6 {
            return Err("planning product count exceeds original six slots".into());
        }
        self.products[business as usize][..count]
            .iter()
            .map(|product| {
                let id = product[0];
                let commodity = self
                    .commodities
                    .get(id as usize)
                    .ok_or("planning product leaves commodity table")?;
                if u32::from_le_bytes(commodity[..4].try_into().unwrap()) != id {
                    return Err("planning commodity identity mismatch".into());
                }
                Ok(u32::from_le_bytes(commodity[4..8].try_into().unwrap()))
            })
            .collect()
    }
}
