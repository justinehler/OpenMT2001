//! 0x481340 additive summaries and 0x481520 matching-tick diagnostics.
use super::super::{Error, Result, SLOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub sender: u8,
    pub tick: u32,
    /// People, road actors, buildings, businesses, players, in native order.
    pub sums: [u32; 5],
    /// Bits of the modeled x87 control word: nearest rounding, PC64 precision.
    /// These describe arithmetic, never an RNG state or host FPU register.
    pub round: u16,
    pub precision: u16,
    /// Named unavailable suppliers must clear their corresponding bit. Values
    /// in those placeholder fields are never evidence of synchronization.
    pub available: u8,
}
impl Sample {
    pub fn validate(&self) -> Result<()> {
        if usize::from(self.sender) >= SLOTS
            || !self.tick.is_multiple_of(200)
            || self.available & !0x1f != 0
        {
            return Err(Error::Row);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Round,
    Precision,
    People,
    Traffic,
    Buildings,
    Businesses,
    Players,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mismatch {
    pub sender: u8,
    pub tick: u32,
    pub field: Field,
    pub local: u32,
    pub remote: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub mismatch_count: u64,
    /// Every individual difference from the most recent mismatched sample.
    pub last_mismatches: Vec<Mismatch>,
}
#[derive(Default)]
pub struct Samples {
    ring: [Option<Sample>; 10],
    pub status: Status,
}
fn index(tick: u32) -> usize {
    ((tick % 2000) / 200) as usize
}
impl Samples {
    pub fn retain(&mut self, sample: Sample) {
        self.ring[index(sample.tick)] = Some(sample);
    }
    pub fn retained(&self, tick: u32) -> Option<Sample> {
        self.ring[index(tick)].filter(|s| s.tick == tick)
    }
    pub fn compare(&mut self, remote: Sample) {
        if remote.validate().is_err() {
            return;
        }
        let Some(local) = self.retained(remote.tick) else {
            return;
        };
        let mut differences = Vec::new();
        let mut compare = |field, a, b| {
            if a != b {
                differences.push(Mismatch {
                    sender: remote.sender,
                    tick: remote.tick,
                    field,
                    local: a,
                    remote: b,
                });
            }
        };
        // Native +30 contains CW[9:8] (precision), +32 CW[11:10]
        // (rounding), despite the historical dialog string names.
        compare(
            Field::Precision,
            u32::from(local.precision),
            u32::from(remote.precision),
        );
        compare(
            Field::Round,
            u32::from(local.round),
            u32::from(remote.round),
        );
        for (i, field) in [
            Field::People,
            Field::Traffic,
            Field::Buildings,
            Field::Businesses,
            Field::Players,
        ]
        .into_iter()
        .enumerate()
        {
            if local.available & remote.available & (1 << i) != 0 {
                compare(field, local.sums[i], remote.sums[i]);
            }
        }
        if !differences.is_empty() {
            for mismatch in &differences {
                eprintln!(
                    "[network desync] tick {} player {} {:?}: local {} remote {}",
                    mismatch.tick, mismatch.sender, mismatch.field, mismatch.local, mismatch.remote
                );
            }
            self.status.mismatch_count = self
                .status
                .mismatch_count
                .saturating_add(differences.len() as u64);
            self.status.last_mismatches = differences;
        }
    }
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}
fn truncate(value: f32) -> u32 {
    crate::economy::ftol_low_word(f64::from(value)) as u32
}

/// Active bytes and native field offsets, rather than visible models or derived
/// account totals. The sample loops start at building+0xc and business+0xa.
pub fn buildings(rows: &[[u8; 0x394]]) -> u32 {
    rows.iter().filter(|r| r[0x3f] != 0).fold(0u32, |sum, r| {
        sum.wrapping_add(truncate(float(r, 0xc)))
            .wrapping_add(truncate(float(r, 0x10)))
            .wrapping_add(word(r, 0x1c))
            .wrapping_add(word(r, 0x24))
    })
}
pub fn businesses(
    rows: &[[u8; 0x220]],
    buildings: &[[u8; 0x394]],
    categories: &[[u8; 0x17c]; 52],
) -> std::result::Result<u32, String> {
    let mut sum = 0u32;
    for row in rows.iter().filter(|r| r[0x21c] != 0) {
        let building = i16::from_le_bytes([row[8], row[9]]);
        let key = if building == -1 {
            -1
        } else {
            let row = buildings
                .get(building as usize)
                .ok_or("sample business building outside pool")?;
            let offset = word(row, 0x40)
                .checked_sub(0x617628)
                .ok_or("sample category before pool")? as usize;
            if !offset.is_multiple_of(0x17c) {
                return Err("sample category is not a row".into());
            }
            let category = categories
                .get(offset / 0x17c)
                .ok_or("sample category outside pool")?;
            i16::from_le_bytes([category[0], category[1]])
        };
        sum = sum
            .wrapping_add(row[0xa] as i8 as i32 as u32)
            .wrapping_add(key as i32 as u32)
            .wrapping_add(truncate(float(row, 0x1c)))
            .wrapping_add(truncate(float(row, 0x1c4)));
    }
    Ok(sum)
}
/// The accumulator is reloaded as signed i32 on each iteration. No f32 spill
/// occurs between adding cash and FTOL (0x4814d6..0x4814fb). Network menu
/// launches have already crossed 4b9c3c even though sampling precedes this
/// iteration's 4816e9 reset.
pub fn players<'a>(
    rows: impl IntoIterator<Item = (bool, &'a crate::economy::PlayerAccount)>,
) -> u32 {
    players_with_precision(rows, crate::x87::Precision::Pc64)
}

/// Explicit incoming control for a pre-reset sampler invocation. Individual
/// f32-to-FTOL conversions are exact loads and independent of PC; FIADD is not.
pub fn players_with_precision<'a>(
    rows: impl IntoIterator<Item = (bool, &'a crate::economy::PlayerAccount)>,
    precision: crate::x87::Precision,
) -> u32 {
    let mut sum = 0i32;
    for (active, account) in rows {
        if !active {
            continue;
        }
        sum = if let Ok(cash) = crate::x87::Sum::from_f32(account.cash) {
            precision
                .add(crate::x87::Sum::from_i32(sum), cash)
                .ftol_low_word()
        } else {
            0
        };
        sum = sum.wrapping_add(
            account.retail_counters[0]
                .wrapping_add(account.housing_counter)
                .max(0),
        );
    }
    sum as u32
}
#[cfg(feature = "lua4")]
pub fn capture(
    sender: u8,
    tick: u32,
    city: &mut crate::level_startup::City,
    roads: &crate::traffic::road::runtime::RoadTraffic,
    roster: &[crate::player_manager::Player],
) -> std::result::Result<Sample, String> {
    let count = city.ledgers.residents.active_count as usize;
    let residents = city
        .ledgers
        .residents
        .nodes
        .get(..count)
        .ok_or("sample resident count outside pool")?;
    // 0x481340 reads each counted resident's auxiliary through its pointer
    // with no null test. Creation binds an auxiliary before the resident is
    // live (0x45bb80 faults at 0x45c300 otherwise) and residents are never
    // removed, so every counted resident has one. The engine's handle is
    // slot + 1, and the slot need not equal the resident's index after a load.
    let mut people = 0u32;
    for resident in residents {
        let slot = resident
            .auxiliary_slot()
            .ok_or("sample resident has no auxiliary")?;
        if slot >= city.creation.auxiliary.used() {
            return Err("sample resident auxiliary unallocated".into());
        }
        let row = &city.creation.auxiliary.slots()[slot];
        people = people
            .wrapping_add(truncate(float(row, 4)))
            .wrapping_add(truncate(float(row, 0xc)));
    }
    let traffic = roads.position_checksum(&mut city.road_network)?;
    if roster.len() != city.ledgers.players.len() {
        return Err("sample player/account count mismatch".into());
    }
    let sample = Sample {
        sender,
        tick,
        sums: [
            people,
            traffic,
            buildings(&city.buildings),
            businesses(&city.ledgers.businesses, &city.buildings, &city.categories)?,
            players(
                roster
                    .iter()
                    .zip(&city.ledgers.players)
                    .map(|(p, a)| (p.bytes.get(0x10).is_some_and(|b| *b != 0), a)),
            ),
        ],
        round: 0,
        precision: 3,
        available: 0x1f,
    };
    sample.validate().map_err(|e| e.to_string())?;
    Ok(sample)
}
