//! Inputs the strategic Locator (40f550 / 411140 / 411860) reads at one
//! worker boundary, and the poll and colour writes it hands back. The binary's
//! `locator::strategic` runs the Locator over them.
use crate::{business_pool::BusinessCatalog, pe::PeImage};
use std::sync::Arc;

fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tables {
    pub business_order: [u32; 45],
    pub commodity_order: [u32; 32],
    pub business_count: u32,
    pub commodity_count: u32,
    pub property_labels: [u32; 52],
    pub default_colors: [u32; 4],
    pub owner_palette: [u32; 10],
}
impl Tables {
    pub fn capture(image: &PeImage<'_>) -> Result<Self, String> {
        fn words<const N: usize>(
            image: &PeImage<'_>,
            at: u32,
            stride: u32,
        ) -> Result<[u32; N], String> {
            (0..N)
                .map(|i| image.at(at + i as u32 * stride, 4).map(|b| word(b, 0)))
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| "UI table extent".into())
        }
        Ok(Self {
            business_order: words(image, 0x53bdc4, 4)?,
            commodity_order: words(image, 0x53c218, 4)?,
            business_count: word(image.at(0x525c30, 4)?, 0),
            commodity_count: word(image.at(0x525c54, 4)?, 0),
            property_labels: words(image, 0x5385a0, 88)?,
            default_colors: words(image, 0x535920, 4)?,
            owner_palette: words(image, 0x541af0, 40)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollScratch {
    pub active: u8,
    pub collecting: u8,
    pub category: u32,
    pub counters: [u16; 34],
}
impl PollScratch {
    pub fn capture(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 0x20c {
            return Err("C human poll record truncated".into());
        }
        Ok(Self {
            active: bytes[0x11],
            collecting: bytes[0x12],
            category: word(bytes, 0x1c4),
            counters: std::array::from_fn(|i| {
                u16::from_le_bytes(bytes[0x1c8 + i * 2..0x1ca + i * 2].try_into().unwrap())
            }),
        })
    }
    pub fn clear(&mut self) {
        self.active = 0;
        self.collecting = 0;
        self.category = 0;
        self.counters.fill(0);
    }
    pub fn stop(&mut self) {
        self.collecting = 0;
    }
    pub fn start(&mut self, category: u32, input: &Inputs) {
        //48b030 invalid category returns before ANY flag/category store.
        let Some(query) = input.poll_queries.get(category as usize) else {
            return;
        };
        self.active = 1;
        self.collecting = 1;
        self.category = category;
        for (i, n) in self.counters.iter_mut().enumerate() {
            *n = n.wrapping_add(query[i]);
        }
        self.collecting = 0;
    }
    pub fn write(&self, b: &mut [u8]) {
        b[0x11] = self.active;
        b[0x12] = self.collecting;
        b[0x1c4..0x1c8].copy_from_slice(&self.category.to_le_bytes());
        for (i, n) in self.counters.iter().enumerate() {
            b[0x1c8 + i * 2..0x1ca + i * 2].copy_from_slice(&n.to_le_bytes());
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    pub category: usize,
    pub label: u32,
    pub building_slot: usize,
    pub building_reference: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inputs {
    pub tables: Arc<Tables>,
    pub financial: crate::player_info::Snapshot,
    pub current_reference: u32,
    pub human_enabled: u8,
    pub roster_slots: Vec<u8>,
    pub roster_references: Vec<u32>,
    pub roster_table: [u32; 6],
    pub scan_enable: i32,
    pub poll: PollScratch,
    pub poll_queries: [[u16; 34]; 52],
    pub catalog: BusinessCatalog,
    pub commodities: [[u8; 28]; 32],
    pub memberships: [u32; 5],
    pub categories: [[u8; 380]; 52],
    pub building_active: [u8; 600],
    pub before_colors: [u32; 600],
    pub owner_colors: [u32; 600],
    pub people_rows: Vec<crate::city_people::Row>,
    pub owned: Vec<Owned>,
}
impl Inputs {
    pub fn capture(
        session: &crate::session::Session<'_>,
        tables: Arc<Tables>,
    ) -> Result<Self, String> {
        let world = &session.world;
        let manager = &session.roster.manager;
        let current_reference = session.roster.status.current_player;
        let current = manager
            .players
            .iter()
            .position(|p| p.reference == current_reference)
            .ok_or("C supplier requires actual86cfc0 human")?;
        if manager.players.len() > 6 || word(&manager.bytes, 8) as usize != manager.players.len() {
            return Err("C actual86d1f8 roster extent mismatch".into());
        }
        if world.buildings.len() != 600 {
            return Err("C requires actual600 building slots".into());
        }
        let mut building_active = [0; 600];
        let mut before_colors = [0; 600];
        let mut owner_colors = [0; 600];
        for (i, b) in world.buildings.iter().enumerate() {
            building_active[i] = b[0x3f];
            before_colors[i] = word(b, 0x90);
            if b[0x3f] == 0 {
                owner_colors[i] = before_colors[i];
                continue;
            }
            let owner = b[0x2d] as i8;
            owner_colors[i] = if owner == 0 {
                0xff808080
            } else {
                let player = manager
                    .players
                    .get(
                        usize::try_from(i32::from(owner) - 1)
                            .map_err(|_| "C negative land-owner byte")?,
                    )
                    .ok_or("C owner outside actual roster")?;
                *tables
                    .owner_palette
                    .get(word(&player.bytes, 0x150) as usize)
                    .ok_or("C owner character leaves palette")?
            };
        }
        let polls = crate::consumer_poll::Poll::from_runtime(world)?;
        let poll_queries = std::array::from_fn(|i| {
            let mut counts = [0; 34];
            counts[..32].copy_from_slice(&polls[i].counts);
            counts[32] = polls[i].residents;
            counts[33] = polls[i].residents;
            counts
        });
        let mut owned = Vec::new();
        //4138b0 uses category linked order, not inspection vector order.
        for (category, row) in world.categories.iter().enumerate() {
            if row[4] != 1 || row[0x90] & 1 != 0 || word(row, 0x8c) as i32 > 45 {
                continue;
            }
            if (word(row, 0x8c) as i32) < 0 {
                return Err("C owned title before supplied table".into());
            }
            let mut next = word(row, 0x15c);
            let mut visits = 0;
            while next != 0 {
                let offset = next
                    .checked_sub(0x591318)
                    .ok_or("C building link before arena")?;
                if offset % 916 != 0 {
                    return Err("C building link unaligned".into());
                }
                let slot = offset as usize / 916;
                let b = world
                    .buildings
                    .get(slot)
                    .ok_or("C building link outside arena")?;
                if visits >= 600 {
                    return Err("C building link cycle".into());
                }
                visits += 1;
                let business = word(b, 0x1c) as i32;
                if business != -1 {
                    let business = world
                        .ledgers
                        .businesses
                        .get(usize::try_from(business).map_err(|_| "C negative business link")?)
                        .ok_or("C business link outside arena")?;
                    //462ae0 resolves original proprietor byte, not building land owner.
                    if usize::from(business[0xa])
                        == usize::from(manager.players[current].bytes[0x1bc]) + 1
                    {
                        let kind = word(business, 4) as usize;
                        let info = world
                            .catalog
                            .info
                            .rows()
                            .get(kind)
                            .ok_or("C business type outside actual catalog")?;
                        owned.push(Owned {
                            category,
                            label: word(info, 8),
                            building_slot: slot,
                            building_reference: word(business, 0x210),
                        });
                    }
                }
                next = word(b, 0x44);
            }
        }
        Ok(Self {
            tables,
            financial: crate::player_info::Snapshot::from_runtime(
                manager,
                &world.ledgers.players,
                current_reference,
            )?,
            current_reference,
            human_enabled: manager.players[current].bytes[0x10],
            roster_slots: manager.players.iter().map(|p| p.bytes[0x1bc]).collect(),
            roster_references: manager.players.iter().map(|p| p.reference).collect(),
            roster_table: std::array::from_fn(|i| word(&manager.bytes, 0x10 + i * 4)),
            scan_enable: word(&manager.bytes, 0xc) as i32,
            poll: PollScratch::capture(&manager.players[current].bytes)?,
            poll_queries,
            catalog: world.catalog.info.clone(),
            commodities: world.commodities.rows,
            memberships: world.commodities.memberships,
            categories: world.categories,
            building_active,
            before_colors,
            owner_colors,
            people_rows: crate::city_people::Report::live_rows(world)?,
            owned,
        })
    }
}
///Exact non-scheduler write set, returned in the CURRENT callback ack. Worker
///applies this before resuming logic or accepting another command/supplier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Writeback {
    pub current_reference: u32,
    pub before_poll: PollScratch,
    pub poll: PollScratch,
    pub before_colors: [u32; 600],
    pub colors: [u32; 600],
    pub active: [u8; 600],
}
impl Writeback {
    pub fn commit(self, session: &mut crate::session::Session<'_>) -> Result<(), String> {
        let current = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == self.current_reference)
            .ok_or("C writeback human no longer live")?;
        if session.roster.status.current_player != self.current_reference
            || session.world.buildings.len() != 600
        {
            return Err("C writeback authority changed across synchronous boundary".into());
        }
        let p = &session.roster.manager.players[current];
        if PollScratch::capture(&p.bytes)? != self.before_poll {
            return Err("C poll scratch changed while Host was suspended".into());
        }
        //Validate ALL cells before the first write; never bypass a mismatch.
        for (i, b) in session.world.buildings.iter().enumerate() {
            if b[0x3f] != self.active[i] || word(b, 0x90) != self.before_colors[i] {
                return Err("C colors changed while Host was suspended".into());
            }
        }
        self.poll
            .write(&mut session.roster.manager.players[current].bytes);
        for (i, b) in session.world.buildings.iter_mut().enumerate() {
            if self.active[i] != 0 {
                b[0x90..0x94].copy_from_slice(&self.colors[i].to_le_bytes());
            }
        }
        Ok(())
    }
}
