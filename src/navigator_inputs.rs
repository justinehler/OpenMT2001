//! Inputs of the navigator's upper strip (F 43d010/43d130): the category rows,
//! owners and colours copied from the actual World and Manager at one worker
//! boundary, and the PE tables they are read with. The binary's
//! `navigator::upper` lays them out and draws them.
use crate::{level_startup::World, pe::PeImage, player_manager::Manager};
use std::sync::Arc;
type Result<T> = std::result::Result<T, String>;
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Current actual World/Manager data, copied while Host is suspended at the
/// acknowledged boundary. No saved/Scene/Status category or roster provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inputs {
    pub source_epoch: crate::level_startup::PropertyEpoch,
    pub rows: [[u8; 380]; 52],
    pub owners: [Option<Owned>; 52],
    pub count: i32,
    pub phase: u32,
    pub presentation_mode: u32,
    pub property_labels: [u32; 52],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owned {
    pub color: u32,
    pub remaining: i32,
    pub current: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tables {
    pub palette: [u32; 10],
    pub property_labels: [u32; 52],
}
///The separate process presentation global accompanies immutable PE tables.
///The category count is read from the actual World topology at each capture.
///No Default, backend-kind guess or saved/current Scene supplies these cells.
#[derive(Clone, Debug)]
pub struct Producer {
    pub tables: Tables,
    pub mode8b2318: u32,
}
impl Producer {
    pub fn capture(&self, session: &crate::session::Session<'_>) -> Result<Arc<Inputs>> {
        Ok(Arc::new(Inputs::capture(
            &session.world,
            &session.roster.manager,
            session.roster.status.current_player,
            self.mode8b2318,
            &self.tables,
        )?))
    }
}
impl Tables {
    pub fn capture(image: &PeImage<'_>) -> Result<Self> {
        let mut palette = [0; 10];
        for (i, v) in palette.iter_mut().enumerate() {
            *v = word(image.at(0x541af0 + i as u32 * 40, 4)?, 0);
        }
        let mut property_labels = [0; 52];
        for (i, v) in property_labels.iter_mut().enumerate() {
            *v = word(image.at(0x5385a0 + i as u32 * 88, 4)?, 0);
        }
        Ok(Self {
            palette,
            property_labels,
        })
    }
}
impl Inputs {
    /// Count comes from this SAME World's completed topology writer. Only
    /// presentationmode comes from the separate process graphics policy.
    /// Neither vector length nor a backend supplies the native count.
    pub fn capture(
        world: &World,
        manager: &Manager,
        current_reference: u32,
        mode8b2318: u32,
        tables: &Tables,
    ) -> Result<Self> {
        let count617624 = world.topology.count617624();
        if count617624 > 52 {
            return Err("F actual category count leaves52-row owner".into());
        }
        let mut owners = [None; 52];
        // The native loop runs to the World's own category count, not 52.
        #[allow(clippy::needless_range_loop)]
        for i in 0..count617624.max(0) as usize {
            let row = &world.categories[i];
            if row[4] == 0 || row[5] == 0 {
                continue;
            }
            //43d691 sign-extends owner5, then indexes86d1fc+owner*4.
            //No negative-index zero/None is fabricated for an unmapped owner.
            let owner = usize::try_from(i32::from(row[5] as i8) - 1)
                .map_err(|_| "F negative owner requires actual before-roster mapping")?;
            let at = 0x10 + owner * 4;
            let reference = u32::from_le_bytes(
                manager
                    .bytes
                    .get(at..at + 4)
                    .ok_or("F owner outside actual six-slot Manager table")?
                    .try_into()
                    .unwrap(),
            );
            let p = manager
                .players
                .iter()
                .find(|p| p.reference == reference && reference != 0)
                .ok_or("F actual roster-table reference has no live player allocation")?;
            let style = u32::from_le_bytes(
                p.bytes
                    .get(0x150..0x154)
                    .ok_or("F player style record truncated")?
                    .try_into()
                    .unwrap(),
            ) as usize;
            let color = *tables
                .palette
                .get(style)
                .ok_or("F style outside supplied ten-row palette")?;
            owners[i] = Some(Owned {
                color,
                remaining: word(row, 0x7c) as i32,
                current: p.reference == current_reference,
            });
        }
        Ok(Self {
            source_epoch: crate::level_startup::PropertyEpoch::World(world.topology.epoch()),
            rows: world.categories,
            owners,
            count: count617624,
            phase: world.clock.timing[0],
            presentation_mode: mode8b2318,
            property_labels: tables.property_labels,
        })
    }
}
