//! Original fresh category ownership pass (`00445ca0`/`0048a8f0`).
//!
//! The native helper receives the *new* owner as ECX.  The category row still
//! contains the old one-based owner byte when the helper starts, so a general
//! call can transfer accounting from one player to another.  The fresh level
//! caller passes the existing owner pointer and restores the category flag in
//! its instruction immediately after the helper returns.

use crate::economy::transfer_business_owner;

pub const CATEGORY_COUNT: usize = 52;
pub const CATEGORY_RECORD_BYTES: usize = 0x17c;
pub const BUILDING_COUNT: usize = 600;
pub const BUILDING_RECORD_BYTES: usize = 0x394;
pub const BUSINESS_COUNT: usize = 600;
pub const BUSINESS_RECORD_BYTES: usize = 0x220;
pub const PLAYER_RECORD_BYTES: usize = 0x224;

const OWNER_OFFSET: usize = 0x05;
const OWNED_CATEGORIES_OFFSET: usize = 0x178;
const PLAYER_KIND_OFFSET: usize = 0x154;
const PLAYER_SLOT_OFFSET: usize = 0x1bc;
const PLAYER_PERIOD_OFFSET: usize = 0x144;
const PLAYER_CASH_OFFSET: usize = 0x04;
const POSITIVE_LEDGER_BASE: usize = 0x70;
const NEGATIVE_LEDGER_BASE: usize = 0x14;
const LEDGER_PERIOD_STRIDE: usize = 0x98;
const CATEGORY_STATE_OFFSET: usize = 0x78;
const CATEGORY_TICK_OFFSET: usize = 0x7c;
const BUILDING_BUSINESS_OFFSET: usize = 0x1c;
const BUILDING_KIND_OFFSET: usize = 0x24;
const BUILDING_RENDER_KIND_OFFSET: usize = 0x4c;
const BUILDING_OWNER_OFFSET: usize = 0x2d;
const BUILDING_TICK_OFFSET: usize = 0x30;
const BUSINESS_BUILDING_OFFSET: usize = 0x08;

///451546 command19, followed by normal-clock scheduled kind1 at +15000.
pub fn begin_wait(category: &mut [u8; CATEGORY_RECORD_BYTES], normal_tick: u32) -> u32 {
    put_u32(category, 0x78, 1);
    category[0x6d] = 0;
    put_u32(category, 0x80, 24);
    normal_tick.wrapping_add(15000)
}
///480ec0 scheduled kind1; it does not clear the other command19 fields.
pub fn end_wait(category: &mut [u8; CATEGORY_RECORD_BYTES]) {
    put_u32(category, 0x78, 2);
}

#[test]
#[ignore = "requires pinned original saved category wait captures"]
fn matches_native_saved_category_wait_admission() {
    use crate::player_manager::word;
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_SAVED_CATEGORY_WAIT")).unwrap();
    assert_eq!(&bytes[..8], b"MTCSWAIT");
    assert_eq!(word(&bytes, 8), 0x37f);
    assert_eq!(word(&bytes, 12), 312);
    let mut cursor = 16;
    for case in 0..312 {
        let mut take = |n| {
            let result = &bytes[cursor..cursor + n];
            cursor += n;
            result
        };
        let config = take(20);
        let index = word(config, 0);
        let variant = word(config, 4);
        let normal = word(config, 12);
        let mut category: [u8; 380] = take(380).try_into().unwrap();
        if variant == 4 {
            end_wait(&mut category);
        }
        let admission = if variant == 3 {
            [0, word(config, 16), 0, word(config, 16)]
        } else {
            [1, begin_wait(&mut category, normal), 1, index]
        };
        if variant == 5 {
            end_wait(&mut category);
        }
        assert_eq!(
            category.as_slice(),
            take(380),
            "native saved wait case {case}"
        );
        assert_eq!(word(take(4), 0), 1);
        let original = take(16);
        for (i, expected) in admission.into_iter().enumerate() {
            assert_eq!(
                word(original, i * 4),
                expected,
                "case {case} scheduled argument {i}"
            );
        }
    }
    assert_eq!(cursor, bytes.len());
}

#[test]
#[ignore = "requires original category wait lifecycle captures"]
fn matches_native_category_wait() {
    use crate::{
        ai_messages::offers::auction::inbox::wait_descriptor,
        local_commands::{Queue, Schedule},
        player_manager::word,
    };
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_WAIT")).unwrap();
    assert_eq!(&bytes[..8], b"MTCWAIT1");
    assert_eq!(word(&bytes, 8), 0x37f);
    let mut cursor = 16;
    for case in 0..word(&bytes, 12) {
        let mut take = |n| {
            let b = &bytes[cursor..cursor + n];
            cursor += n;
            b
        };
        let config = take(12);
        let total = word(config, 4);
        let normal = word(config, 8);
        let prior = take(24).try_into().unwrap();
        let mut category: [u8; 380] = take(380).try_into().unwrap();
        assert_eq!(
            wait_descriptor(&category, prior).as_slice(),
            take(24),
            "{case} descriptor"
        );
        let mut queue = Queue::default();
        queue
            .submit(false, total, true, |_| panic!("early local command"))
            .unwrap();
        for (step, mut clocks) in [
            [total, normal],
            [total + 24, normal],
            [total + 25, normal],
            [total + 30000, normal + 14999],
            [total + 30000, normal + 15000],
        ]
        .into_iter()
        .enumerate()
        {
            let normal = clocks[1];
            queue
                .drain_scheduling(&mut clocks, |ready, enqueue| {
                    if *ready {
                        end_wait(&mut category);
                        Ok(())
                    } else {
                        let due = begin_wait(&mut category, normal);
                        enqueue(true, Schedule::Normal { due })
                    }
                })
                .unwrap();
            assert_eq!(category.as_slice(), take(380), "{case} step{step}");
        }
    }
    assert_eq!(cursor, bytes.len());
}

/// Synchronous boundaries called while the original helper mutates state.
/// `NativeUiCall` preserves the original address and argument order so the
/// host can attach sound, display, or manager behavior without moving it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seam {
    NativeUiCall {
        address: u32,
        args: [u32; 6],
        argument_count: u8,
    },
    BuildingAppearance {
        building: usize,
    },
    AiOwnershipLists {
        player: usize,
    },
}

/// Authoritative native-sized pools used by the pass. The chain order is the
/// logical equivalent of category `+0x15c` and each building `+0x44` link.
/// The host must keep these links stable throughout a transfer; ownership,
/// accounting and appearance callbacks operate on the live records.
pub struct World<'a> {
    pub categories: &'a mut [[u8; CATEGORY_RECORD_BYTES]; CATEGORY_COUNT],
    pub buildings: &'a mut [[u8; BUILDING_RECORD_BYTES]],
    pub businesses: &'a mut [[u8; BUSINESS_RECORD_BYTES]],
    pub players: &'a mut [crate::player_manager::Player],
    pub chains: &'a [Vec<usize>; CATEGORY_COUNT],
    pub normal_tick: u32,
    pub category_tick: u32,
    pub refresh_counter: u8,
    pub accounting_enabled: bool,
    pub current_player: Option<usize>,
    pub notification_threshold: f32,
}

impl World<'_> {
    /// Original0048c450 compensated sale to the city. Unlike a purchase, this
    /// credits ledger0, leaves +7c intact, and does not reset every building's
    /// timestamp after its appearance callback.
    pub fn release(
        &mut self,
        category: usize,
        amount: f32,
        seams: &mut impl FnMut(&mut Self, Seam) -> Result<(), String>,
    ) -> Result<(), String> {
        let owner = self
            .categories
            .get(category)
            .ok_or("property sale category outside pool")?[5];
        let owner = (owner != 0).then(|| (i32::from(owner as i8) - 1) as usize);
        if let Some(owner) = owner {
            self.players
                .get_mut(owner)
                .ok_or("property sale owner outside roster")?
                .bytes[OWNED_CATEGORIES_OFFSET + category] = 0;
            self.post_player(owner, amount, NEGATIVE_LEDGER_BASE, seams)?;
        }
        self.categories[category][5] = 0;
        put_u32(&mut self.categories[category], CATEGORY_STATE_OFFSET, 2);
        for building in self.chains[category].clone() {
            if word(&self.buildings[building], BUILDING_KIND_OFFSET) == 2 {
                self.buildings[building][BUILDING_OWNER_OFFSET] = 0;
                put_u32(
                    &mut self.buildings[building],
                    BUILDING_TICK_OFFSET,
                    self.normal_tick,
                );
                if self.appearance_needed(building) {
                    seams(self, Seam::BuildingAppearance { building })?;
                }
                let business = word(&self.buildings[building], BUILDING_BUSINESS_OFFSET);
                if business != u32::MAX {
                    self.transfer_linked_business(business as usize, 0)?;
                }
            }
            if self.appearance_needed(building) {
                seams(self, Seam::BuildingAppearance { building })?;
            }
        }
        self.refresh_counter = self.refresh_counter.wrapping_add(100);
        if let Some(player) = owner
            && word(&self.players[player].bytes, PLAYER_KIND_OFFSET) == 1
        {
            seams(self, Seam::AiOwnershipLists { player })?;
        }
        seams(
            self,
            Seam::NativeUiCall {
                address: 0x41f410,
                args: [0; 6],
                argument_count: 0,
            },
        )
    }
    /// Execute one general `0048a8f0` call. `new_owner` is the player pointer
    /// represented by a zero-based runtime player slot; it need not equal the
    /// row's current owner. The fresh caller uses `fresh_loop` below when it
    /// needs the post-return category flag store.
    pub fn apply(
        &mut self,
        category_index: usize,
        new_owner: usize,
        amount: f32,
        seams: &mut impl FnMut(&mut Self, Seam) -> Result<(), String>,
    ) -> Result<(), String> {
        if category_index >= CATEGORY_COUNT {
            return Err("category ownership index exceeds52".into());
        }
        if new_owner >= self.players.len() {
            return Err("new category owner leaves player records".into());
        }
        if self
            .players
            .iter()
            .any(|player| player.bytes.len() < PLAYER_RECORD_BYTES)
        {
            return Err("category ownership player record is truncated".into());
        }
        if self.players.len() > 6 {
            return Err("category ownership exceeds the six native player slots".into());
        }
        let old_owner_byte = self.categories[category_index][OWNER_OFFSET];
        let old_owner = (old_owner_byte != 0).then(|| usize::from(old_owner_byte - 1));
        if let Some(old_owner) = old_owner
            && old_owner >= self.players.len()
        {
            return Err("old category owner leaves player records".into());
        }

        // 0048a902 marks the new owner before the old-owner accounting pass.
        self.players[new_owner].bytes[OWNED_CATEGORIES_OFFSET + category_index] = 1;

        // The first native post resolves the row's old owner. The second post
        // below resolves ECX's new owner after the row owner is normalized.
        if let Some(old_owner) = old_owner {
            self.post_player(old_owner, amount, POSITIVE_LEDGER_BASE, seams)?;
            let owner = self.categories[category_index][OWNER_OFFSET];
            let owner = owner
                .checked_sub(1)
                .map(usize::from)
                .filter(|&index| index < self.players.len())
                .ok_or("category owner changed to an unresolved player during notification")?;
            self.players[owner].bytes[OWNED_CATEGORIES_OFFSET + category_index] = 0;
        }

        let new_owner_byte = self.players[new_owner].bytes[PLAYER_SLOT_OFFSET].wrapping_add(1);
        self.categories[category_index][OWNER_OFFSET] = new_owner_byte;
        self.post_player(new_owner, -amount, NEGATIVE_LEDGER_BASE, seams)?;

        // 0048a965..0048aa75 clears +78 and reloads +7c from the world value.
        put_u32(
            &mut self.categories[category_index],
            CATEGORY_STATE_OFFSET,
            0,
        );
        put_u32(
            &mut self.categories[category_index],
            CATEGORY_TICK_OFFSET,
            self.category_tick,
        );

        let chain = self
            .chains
            .get(category_index)
            .ok_or("category chain leaves52 rows")?
            .clone();
        for building_index in chain {
            self.visit_building(building_index, new_owner, seams)?;
        }

        self.refresh_counter = self.refresh_counter.wrapping_add(100);
        if word(&self.players[new_owner].bytes, PLAYER_KIND_OFFSET) == 1 {
            seams(self, Seam::AiOwnershipLists { player: new_owner })?;
        }
        Ok(())
    }

    /// Execute the fresh `004ba0bf..004ba0de` loop. Native city-owned rows
    /// resolve to null and are skipped. The caller's final category-bit store
    /// is part of this method, after each helper returns.
    pub fn fresh_loop(
        &mut self,
        seams: &mut impl FnMut(&mut Self, Seam) -> Result<(), String>,
    ) -> Result<(), String> {
        for category_index in 0..CATEGORY_COUNT {
            let owner = self.categories[category_index][OWNER_OFFSET];
            if owner == 0 {
                continue;
            }
            let new_owner = usize::from(owner - 1);
            self.apply(category_index, new_owner, 0., seams)?;
            self.players[new_owner].bytes[OWNED_CATEGORIES_OFFSET + category_index] = 1;
        }
        Ok(())
    }

    fn post_player(
        &mut self,
        player: usize,
        amount: f32,
        ledger_base: usize,
        seams: &mut impl FnMut(&mut Self, Seam) -> Result<(), String>,
    ) -> Result<(), String> {
        let record = self
            .players
            .get_mut(player)
            .ok_or("category account leaves player records")?;
        if record.bytes.len() < PLAYER_RECORD_BYTES {
            return Err("category player record is truncated".into());
        }
        let period = usize::from(record.bytes[PLAYER_PERIOD_OFFSET]);
        if period >= 2 {
            return Err("category account period exceeds two native periods".into());
        }
        add_f32(
            &mut record.bytes,
            ledger_base + period * LEDGER_PERIOD_STRIDE,
            amount,
        );
        add_f32(&mut record.bytes, PLAYER_CASH_OFFSET, amount);
        self.notify(player, amount, seams)?;
        Ok(())
    }

    fn notify(
        &mut self,
        player: usize,
        amount: f32,
        seams: &mut impl FnMut(&mut Self, Seam) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.current_player != Some(player) {
            return Ok(());
        }
        let args = |asset| [asset, 0, 0, 0, 1, 0x3c];
        if amount > self.notification_threshold {
            seams(
                self,
                Seam::NativeUiCall {
                    address: 0x004ad760,
                    args: args(0x00541cc8),
                    argument_count: 6,
                },
            )?;
            seams(
                self,
                Seam::NativeUiCall {
                    address: 0x004c6290,
                    args: [0; 6],
                    argument_count: 0,
                },
            )?;
        } else if amount < self.notification_threshold || amount.is_nan() {
            seams(
                self,
                Seam::NativeUiCall {
                    address: 0x004c62d0,
                    args: [0; 6],
                    argument_count: 0,
                },
            )?;
            seams(
                self,
                Seam::NativeUiCall {
                    address: 0x004ad760,
                    args: args(0x00541cb4),
                    argument_count: 6,
                },
            )?;
        }
        Ok(())
    }

    fn visit_building(
        &mut self,
        building_index: usize,
        new_owner_slot: usize,
        seams: &mut impl FnMut(&mut Self, Seam) -> Result<(), String>,
    ) -> Result<(), String> {
        if building_index >= BUILDING_COUNT {
            return Err("category chain building leaves600 records".into());
        }
        if building_index >= self.buildings.len() {
            return Err("category chain building leaves the supplied pool".into());
        }
        let kind = word(&self.buildings[building_index], BUILDING_KIND_OFFSET);
        if kind == 2 {
            self.buildings[building_index][BUILDING_OWNER_OFFSET] =
                self.players[new_owner_slot].bytes[PLAYER_SLOT_OFFSET].wrapping_add(1);
            put_u32(
                &mut self.buildings[building_index],
                BUILDING_TICK_OFFSET,
                self.normal_tick,
            );
            if self.appearance_needed(building_index) {
                seams(
                    self,
                    Seam::BuildingAppearance {
                        building: building_index,
                    },
                )?;
            }
            // Native reloads both the player slot and business binding after
            // the synchronous appearance callback before calling462b00.
            let owner = self.players[new_owner_slot].bytes[PLAYER_SLOT_OFFSET].wrapping_add(1);
            let business = word(&self.buildings[building_index], BUILDING_BUSINESS_OFFSET) as usize;
            self.transfer_linked_business(business, owner)?;
        }
        if self.appearance_needed(building_index) {
            seams(
                self,
                Seam::BuildingAppearance {
                    building: building_index,
                },
            )?;
        }
        // 00445510 runs after 004450a0 for every node, including kind 0.
        put_u32(
            &mut self.buildings[building_index],
            BUILDING_TICK_OFFSET,
            self.normal_tick,
        );
        Ok(())
    }

    fn appearance_needed(&self, building: usize) -> bool {
        let row = &self.buildings[building];
        word(row, BUILDING_RENDER_KIND_OFFSET) == 1
            && (word(row, BUILDING_BUSINESS_OFFSET) == u32::MAX
                || matches!(word(row, BUILDING_KIND_OFFSET), 0 | 1))
    }

    fn transfer_linked_business(
        &mut self,
        business_index: usize,
        new_owner: u8,
    ) -> Result<(), String> {
        if business_index >= BUSINESS_COUNT || business_index >= self.businesses.len() {
            return Err("category building business leaves600 records".into());
        }
        if !self.accounting_enabled {
            return_business_owner_disabled(&mut self.businesses[business_index], new_owner);
            return Ok(());
        }
        let business = self.businesses[business_index];
        let linked_building = i16::from_le_bytes(
            business[BUSINESS_BUILDING_OFFSET..BUSINESS_BUILDING_OFFSET + 2]
                .try_into()
                .unwrap(),
        );
        if linked_building < 0
            || usize::from(linked_building as u16) >= BUILDING_COUNT
            || usize::from(linked_building as u16) >= self.buildings.len()
        {
            return Err("business linked building leaves600 records".into());
        }
        let prefix: [u8; 64] = self.buildings[linked_building as usize][..64]
            .try_into()
            .unwrap();
        let mut accounts = self
            .players
            .iter()
            .map(|player| crate::economy::PlayerAccount::from_runtime(&player.bytes))
            .collect::<Result<Vec<_>, _>>()?;
        let mut business = business;
        transfer_business_owner(&mut business, &prefix, new_owner, true, &mut accounts)?;
        self.businesses[business_index] = business;
        for (player, account) in self.players.iter_mut().zip(accounts.iter()) {
            account.write_runtime(&mut player.bytes)?;
        }
        Ok(())
    }
}

fn return_business_owner_disabled(business: &mut [u8; BUSINESS_RECORD_BYTES], new_owner: u8) {
    business[0x0a] = new_owner;
}

fn word(record: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(record[offset..offset + 4].try_into().unwrap())
}

fn put_u32(record: &mut [u8], offset: usize, value: u32) {
    record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn add_f32(record: &mut [u8], offset: usize, amount: f32) {
    let old = f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
    let next = (f64::from(old) + f64::from(amount)) as f32;
    record[offset..offset + 4].copy_from_slice(&next.to_le_bytes());
}

/// Original48a830 recomputes the player+145 monopoly flag from current category
/// owners. Zero-length groups do not qualify; gaps marked-1 are skipped.
pub fn update_monopoly(
    player: &mut crate::player_manager::Player,
    identities: &[u32],
    categories: &[[u8; 380]; 52],
    image: &crate::pe::PeImage<'_>,
) -> Result<(), String> {
    for group in 0..12 {
        let mut count = 0;
        let mut owned = 0;
        for entry in 0..4 {
            let id = i32::from_le_bytes(
                image
                    .at(0x5397ac + group * 60 + entry * 4, 4)?
                    .try_into()
                    .unwrap(),
            );
            if id == -1 {
                continue;
            }
            count += 1;
            let owner = *categories
                .get(id as usize)
                .ok_or("monopoly category outside pool")?
                .get(5)
                .unwrap() as i8;
            let identity = if owner == 0 {
                0
            } else {
                *identities
                    .get((i32::from(owner) - 1) as usize)
                    .ok_or("monopoly owner outside roster")?
            };
            owned += usize::from(identity == player.reference);
        }
        if count != 0 && count == owned {
            player.bytes[0x145] = 1;
            return Ok(());
        }
    }
    player.bytes[0x145] = 0;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original compensated property capture"]
    fn matches_native_compensated_transfers() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PROPERTY_TRANSFER")).unwrap();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.take(8), b"MTPRTX02");
        assert_eq!(r.u32(), 0x037f);
        let count = r.u32();
        let executable = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        const SIZE: usize = 3 * 548 + 380 * 52 + 3 * 916 + 3 * 544;
        for case in 0..count {
            let config: [u32; 7] = std::array::from_fn(|_| r.u32());
            let mut before = Reader::new(r.take(SIZE));
            let expected = r.take(SIZE);
            let mut players: Vec<_> = (0..3)
                .map(|i| crate::player_manager::Player {
                    reference: 0x10010000 + i * 0x1000,
                    bytes: before.take(548).to_vec(),
                    auxiliaries: vec![],
                })
                .collect();
            let mut categories: [[u8; 380]; 52] = std::array::from_fn(|_| before.bytes());
            let mut buildings: Vec<[u8; 916]> = (0..3).map(|_| before.bytes()).collect();
            let mut businesses: Vec<[u8; 544]> = (0..3).map(|_| before.bytes()).collect();
            let traces = r.u32();
            let expected_trace: Vec<[u32; 3]> = (0..traces)
                .map(|_| std::array::from_fn(|_| r.u32()))
                .collect();
            let mut chains: [Vec<usize>; 52] = std::array::from_fn(|_| vec![]);
            if word(&categories[7], 0x15c) != 0 {
                chains[7] = vec![0, 1, 2];
            }
            let mut world = World {
                categories: &mut categories,
                buildings: &mut buildings,
                businesses: &mut businesses,
                players: &mut players,
                chains: &chains,
                normal_tick: 67890,
                category_tick: 12345,
                refresh_counter: config[4] as u8,
                accounting_enabled: config[6] != 0,
                current_player: Some(config[1] as usize),
                notification_threshold: 0.,
            };
            let mut trace = vec![];
            let mut callback = |world: &mut World<'_>, seam| {
                trace.push(match seam {
                    Seam::NativeUiCall {
                        address: 0x4ad760,
                        args,
                        ..
                    } => {
                        if config[3] != 0 {
                            world.categories[7][5] = 3;
                        }
                        [0, u32::from(args[0] == 0x541cc8), 0]
                    }
                    Seam::NativeUiCall {
                        address: address @ (0x4c6290 | 0x4c62d0),
                        ..
                    } => [1, u32::from(address == 0x4c6290), 0],
                    Seam::NativeUiCall {
                        address: 0x41f410, ..
                    } => [7, 0, 0],
                    Seam::AiOwnershipLists { player } => [6, world.players[player].reference, 0],
                    other => panic!("unexpected transfer seam {other:?}"),
                });
                Ok(())
            };
            if config[0] == 0 {
                world
                    .release(7, f32::from_bits(config[2]), &mut callback)
                    .unwrap();
            } else {
                world
                    .apply(
                        7,
                        config[0] as usize - 1,
                        f32::from_bits(config[2]),
                        &mut callback,
                    )
                    .unwrap();
            }
            let identities: Vec<_> = world.players.iter().map(|p| p.reference).collect();
            for player in world.players.iter_mut() {
                update_monopoly(player, &identities, world.categories, &image).unwrap();
            }
            assert_eq!(u32::from(world.refresh_counter), config[5], "case {case}");
            assert_eq!(trace, expected_trace, "case {case}");
            let actual: Vec<u8> = world
                .players
                .iter()
                .flat_map(|p| p.bytes.iter().copied())
                .chain(world.categories.iter().flatten().copied())
                .chain(world.buildings.iter().flatten().copied())
                .chain(world.businesses.iter().flatten().copied())
                .collect();
            if let Some(at) = actual.iter().zip(expected).position(|(a, b)| a != b) {
                panic!(
                    "case {case}, byte{at:x}: {} != {}",
                    actual[at], expected[at]
                );
            }
        }
        assert_eq!(r.at, bytes.len());
    }

    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
    }

    impl<'a> Reader<'a> {
        fn new(bytes: &'a [u8]) -> Self {
            Self { bytes, at: 0 }
        }
        fn take(&mut self, size: usize) -> &'a [u8] {
            let end = self.at.checked_add(size).unwrap();
            let result = &self.bytes[self.at..end];
            self.at = end;
            result
        }
        fn u32(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn i32(&mut self) -> i32 {
            i32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn f32(&mut self) -> f32 {
            f32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn bytes<const N: usize>(&mut self) -> [u8; N] {
            self.take(N).try_into().unwrap()
        }
        fn length_prefixed(&mut self) -> Vec<u8> {
            let size = self.u32() as usize;
            self.take(size).to_vec()
        }
    }

    fn fixture_path() -> std::path::PathBuf {
        crate::fixtures::optional("NATIVE_POST_ROSTER_OWNERSHIP").unwrap_or_else(|| {
            std::path::PathBuf::from("work/economy/native-post-roster-ownership.bin")
        })
    }

    fn live_players(raw: &[[u8; PLAYER_RECORD_BYTES]]) -> Vec<crate::player_manager::Player> {
        raw.iter()
            .enumerate()
            .map(|(index, bytes)| crate::player_manager::Player {
                reference: (index + 1) as u32,
                bytes: bytes.to_vec(),
                auxiliaries: Vec::new(),
            })
            .collect()
    }

    fn snapshot_players(
        players: &[crate::player_manager::Player],
    ) -> Vec<[u8; PLAYER_RECORD_BYTES]> {
        players
            .iter()
            .map(|player| player.bytes[..PLAYER_RECORD_BYTES].try_into().unwrap())
            .collect()
    }

    #[test]
    #[ignore = "requires the original post-roster ownership oracle corpus"]
    fn matches_native_post_roster_cases() {
        let bytes = std::fs::read(fixture_path()).unwrap();
        let mut read = Reader::new(&bytes);
        assert_eq!(&read.bytes::<8>(), b"MTPOSTO2");
        assert_eq!(read.u32(), 2);
        assert_eq!(read.u32() as usize, CATEGORY_COUNT);
        assert_eq!(read.u32() as usize, CATEGORY_RECORD_BYTES);
        assert_eq!(read.u32() as usize, PLAYER_RECORD_BYTES);
        let cases = read.u32();
        let mut checked = 0;
        for _ in 0..cases {
            let name = String::from_utf8(read.length_prefixed()).unwrap();
            let owner = read.i32();
            let new_owner = read.i32();
            let _owner_kind = read.i32();
            let _period = read.i32();
            let accounting = read.i32() != 0;
            let node_kind = read.i32();
            let _business_discriminator = read.i32();
            let _business_old_owner = read.i32();
            let _business_occupancy = read.i32();
            let amount = read.f32();
            let before_categories_raw = read.length_prefixed();
            assert_eq!(
                before_categories_raw.len(),
                CATEGORY_COUNT * CATEGORY_RECORD_BYTES
            );
            let before_categories: [[u8; CATEGORY_RECORD_BYTES]; CATEGORY_COUNT] =
                std::array::from_fn(|index| {
                    before_categories_raw
                        [index * CATEGORY_RECORD_BYTES..(index + 1) * CATEGORY_RECORD_BYTES]
                        .try_into()
                        .unwrap()
                });
            let player_count = read.u32() as usize;
            assert_eq!(player_count, 4);
            let before_players: Vec<[u8; PLAYER_RECORD_BYTES]> = (0..player_count)
                .map(|_| read.length_prefixed().try_into().unwrap())
                .collect();
            let before_node = read.length_prefixed();
            let before_building = read.length_prefixed();
            let before_business = read.length_prefixed();
            let after_categories_raw = read.length_prefixed();
            assert_eq!(
                after_categories_raw.len(),
                CATEGORY_COUNT * CATEGORY_RECORD_BYTES
            );
            let after_categories: [[u8; CATEGORY_RECORD_BYTES]; CATEGORY_COUNT] =
                std::array::from_fn(|index| {
                    after_categories_raw
                        [index * CATEGORY_RECORD_BYTES..(index + 1) * CATEGORY_RECORD_BYTES]
                        .try_into()
                        .unwrap()
                });
            let after_player_count = read.u32() as usize;
            assert_eq!(after_player_count, 4);
            let after_players: Vec<[u8; PLAYER_RECORD_BYTES]> = (0..after_player_count)
                .map(|_| read.length_prefixed().try_into().unwrap())
                .collect();
            let after_node = read.length_prefixed();
            let after_building = read.length_prefixed();
            let after_business = read.length_prefixed();
            let trace_count = read.u32() as usize;
            let mut native_trace = Vec::new();
            for _ in 0..trace_count {
                let address = read.u32();
                let argument_count = read.u32() as usize;
                let raw_args = read.bytes::<24>();
                let args = std::array::from_fn(|index| {
                    u32::from_le_bytes(raw_args[index * 4..index * 4 + 4].try_into().unwrap())
                });
                native_trace.push((address, argument_count, args));
            }

            let mut categories = before_categories;
            let mut buildings: Vec<[u8; BUILDING_RECORD_BYTES]> =
                vec![before_building.as_slice().try_into().unwrap(); BUILDING_COUNT];
            let mut businesses: Vec<[u8; BUSINESS_RECORD_BYTES]> =
                vec![before_business.as_slice().try_into().unwrap(); BUSINESS_COUNT];
            let mut players = live_players(&before_players);
            let mut chains: [Vec<usize>; CATEGORY_COUNT] = std::array::from_fn(|_| Vec::new());
            if node_kind >= 0 {
                // The oracle uses a compact synthetic chain node at a
                // separate address. Slot one is its image in the 600-record
                // pool; slot zero remains the business-linked source.
                buildings[1][..before_node.len()].copy_from_slice(&before_node);
                chains[7].push(1);
            }
            let current_player = matches!(
                name.as_str(),
                "current_player_positive_notification" | "current_player_negative_notification"
            )
            .then_some(0);
            let mut world = World {
                categories: &mut categories,
                buildings: &mut buildings,
                businesses: &mut businesses,
                players: &mut players,
                chains: &chains,
                normal_tick: 0x1234_5678,
                category_tick: 0x5566_7788,
                refresh_counter: 0x10,
                accounting_enabled: accounting,
                current_player,
                notification_threshold: 0.,
            };
            let mut observed = Vec::new();
            let mut seam = |_: &mut World<'_>, event: Seam| {
                observed.push(event);
                Ok(())
            };
            if name == "all_52_city_and_player_owner_gates" || (owner == 0 && new_owner < 0) {
                world.fresh_loop(&mut seam).unwrap();
            } else {
                let target = if new_owner >= 0 {
                    new_owner as usize - 1
                } else {
                    owner as usize - 1
                };
                world.apply(7, target, amount, &mut seam).unwrap();
                if new_owner < 0 {
                    // This is the caller-side store after 0048a8f0 returns.
                    world.players[target].bytes[OWNED_CATEGORIES_OFFSET + 7] = 1;
                }
            }
            assert_eq!(&*world.categories, &after_categories, "{name} categories");
            assert_eq!(
                snapshot_players(world.players),
                after_players,
                "{name} players"
            );
            assert_eq!(
                &world.buildings[1][..after_node.len()],
                after_node.as_slice(),
                "{name} node"
            );
            assert_eq!(
                world.buildings[0].as_slice(),
                after_building.as_slice(),
                "{name} building"
            );
            assert_eq!(
                world.businesses[0].as_slice(),
                after_business.as_slice(),
                "{name} business"
            );

            let observed_trace: Vec<_> = observed
                .iter()
                .map(|event| match event {
                    Seam::NativeUiCall {
                        address,
                        args,
                        argument_count,
                    } => (*address, usize::from(*argument_count), *args),
                    Seam::BuildingAppearance { .. } => (0x0048_5520, 0, [0; 6]),
                    Seam::AiOwnershipLists { .. } => (0x0046_7a30, 0, [0; 6]),
                })
                .collect();
            let native_seams: Vec<_> = native_trace
                .iter()
                .filter(|(address, _, _)| *address != 0x004b_a0d6)
                .copied()
                .collect();
            assert_eq!(observed_trace, native_seams, "{name} seam trace");
            checked += 1;
        }
        assert_eq!(checked, cases);
    }
}

///405260: announcement visibility gates both the category unlock and panel refresh.
pub fn property_view(category: &mut [u8; 380], announcement_visible: u8, force: u8) -> bool {
    if announcement_visible == 1 && force == 0 {
        return false;
    }
    category[0x6c] = 0;
    true
}
#[test]
#[ignore = "requires original property view fixture"]
fn matches_native_property_view() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_PROPERTY_VIEW")).unwrap();
    assert_eq!(&bytes[..8], b"MTPVIEW1");
    assert_eq!(word(&bytes, 8), 0x37f);
    let count = word(&bytes, 12) as usize;
    assert_eq!(bytes.len(), 16 + count * (16 + 760));
    for row in bytes[16..].chunks_exact(776) {
        let mut category: [u8; 380] = row[16..396].try_into().unwrap();
        let refreshed = property_view(&mut category, row[0], row[4]);
        assert_eq!(
            refreshed,
            u32::from_le_bytes(row[12..16].try_into().unwrap()) != 0
        );
        assert_eq!(category, row[396..]);
    }
}
