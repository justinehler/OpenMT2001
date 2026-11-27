//! Original00467a30 ownership bookkeeping. This is not the AI decision loop.

#[derive(Debug, PartialEq, Eq)]
pub struct State {
    pub completed_groups: u8,
    /// Original player+aac, twelve 12-byte records, including opaque bytes.
    pub groups: [[u8; 12]; 12],
    /// Original player+b3c/b40. Entries contain business slot IDs, not land IDs.
    pub lists: [BusinessList; 2],
}

#[derive(Debug, PartialEq, Eq)]
pub struct BusinessList {
    pub entries: Vec<[u8; 28]>,
    pub active_count: u16,
}

/// Retained native allocations. Player b3c/b40 references and b44..b4a counts
/// remain authoritative; groups live only in the player allocation.
#[derive(Default)]
pub struct Storage {
    pub lists: std::collections::BTreeMap<u32, Vec<[u8; 28]>>,
}
impl Storage {
    pub fn state(&self, player: &crate::player_manager::Player) -> Result<State, String> {
        use crate::player_manager::word;
        if player.bytes.len() < 0xb50 {
            return Err("truncated AI ownership player".into());
        }
        let mut lists = Vec::new();
        for mode in 0..2 {
            let at = 0xb44 + mode * 4;
            let active_count = u16::from_le_bytes(player.bytes[at..at + 2].try_into().unwrap());
            let allocated = usize::from(u16::from_le_bytes(
                player.bytes[at + 2..at + 4].try_into().unwrap(),
            ));
            let reference = word(&player.bytes, 0xb3c + mode * 4);
            let entries = if allocated == 0 {
                vec![]
            } else {
                self.lists
                    .get(&reference)
                    .ok_or("AI ownership allocation unresolved")?
                    .get(..allocated)
                    .ok_or("AI ownership allocation truncated")?
                    .to_vec()
            };
            lists.push(BusinessList {
                entries,
                active_count,
            });
        }
        Ok(State {
            completed_groups: player.bytes[0xaa8],
            groups: std::array::from_fn(|i| {
                player.bytes[0xaac + i * 12..0xaac + (i + 1) * 12]
                    .try_into()
                    .unwrap()
            }),
            lists: lists.try_into().unwrap(),
        })
    }

    /// Publish a reviewed list operation back into the same player. Newly grown
    /// allocations receive caller-owned logical identities; old identities die.
    pub fn mutate(
        &mut self,
        player: &mut crate::player_manager::Player,
        mut allocate: impl FnMut(usize) -> Result<u32, String>,
        operation: impl FnOnce(&mut State) -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::player_manager::{put, word};
        let mut state = self.state(player)?;
        let result = operation(&mut state);
        player.bytes[0xaa8] = state.completed_groups;
        for (i, group) in state.groups.iter().enumerate() {
            player.bytes[0xaac + i * 12..0xaac + (i + 1) * 12].copy_from_slice(group);
        }
        for (mode, list) in state.lists.into_iter().enumerate() {
            let at = 0xb44 + mode * 4;
            let old_len = usize::from(u16::from_le_bytes(
                player.bytes[at + 2..at + 4].try_into().unwrap(),
            ));
            let old = word(&player.bytes, 0xb3c + mode * 4);
            let len = u16::try_from(list.entries.len())
                .map_err(|_| "AI ownership allocation exceeds WORD")?;
            let reference = if list.entries.len() != old_len && len != 0 {
                allocate(list.entries.len() * 28)?
            } else {
                old
            };
            if old != reference {
                self.lists.remove(&old);
            }
            if reference != 0 {
                self.lists.insert(reference, list.entries);
            } else if len != 0 {
                return Err("AI ownership allocation returned null".into());
            }
            put(&mut player.bytes, 0xb3c + mode * 4, reference);
            player.bytes[at..at + 2].copy_from_slice(&list.active_count.to_le_bytes());
            player.bytes[at + 2..at + 4].copy_from_slice(&len.to_le_bytes());
        }
        result
    }
}

/// Resolve4708a0's inputs from the actual business/building/category pools.
pub fn live_entry_value(
    business: i32,
    businesses: &[[u8; 544]],
    buildings: &[[u8; 916]],
    categories: &[[u8; 380]; 52],
    image: &crate::pe::PeImage<'_>,
) -> Result<i32, String> {
    live_entry_value_from_catalog(
        business,
        businesses,
        buildings,
        categories,
        &crate::business_pool::BusinessCatalog::from_pe(image)?,
    )
}

///4708a0 reads the script-mutable refurbishment cost at BusinessInfo+54.
pub fn live_entry_value_from_catalog(
    business: i32,
    businesses: &[[u8; 544]],
    buildings: &[[u8; 916]],
    categories: &[[u8; 380]; 52],
    catalog: &crate::business_pool::BusinessCatalog,
) -> Result<i32, String> {
    use crate::player_manager::word;
    let business = businesses
        .get(business as usize)
        .ok_or("AI ownership business leaves pool")?;
    let building_index = i16::from_le_bytes(business[8..10].try_into().unwrap());
    let building = buildings
        .get(building_index as usize)
        .ok_or("AI ownership building leaves pool")?;
    let reference = word(building, 0x40);
    let offset = reference
        .checked_sub(0x617628)
        .ok_or("AI ownership category reference precedes pool")?;
    if offset % 380 != 0 {
        return Err("unaligned AI ownership category reference".into());
    }
    let category = categories
        .get(offset as usize / 380)
        .ok_or("AI ownership category leaves pool")?;
    let category = categories
        .get(word(category, 0) as usize)
        .ok_or("AI ownership category identity leaves pool")?;
    let kind = word(business, 4);
    let cost = word(
        catalog
            .row(kind as usize)
            .ok_or("AI ownership kind leaves catalog")?,
        0x54,
    ) as i32;
    Ok(entry_value_extended(
        building[..64].try_into().unwrap(),
        f32::from_bits(word(category, 0x84)),
        cost,
        f32::from_bits(word(business, 0x20)),
    ))
}

pub fn group_mapping(image: &crate::pe::PeImage<'_>) -> Result<[u8; 52], String> {
    let mut mapping = [0; 52];
    for (index, value) in mapping.iter_mut().enumerate() {
        *value = image.at(0x5385a4 + index as u32 * 88, 1)?[0];
        if *value >= 12 {
            return Err("ownership group exceeds original player records".into());
        }
    }
    Ok(mapping)
}

impl State {
    /// Fresh Rust storage supplies zero for the opaque byte7 which the original
    /// initializer leaves untouched. Membership/counter fields come from v1.4.
    pub fn from_v14(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let mut state = Self {
            completed_groups: 0,
            groups: [[0; 12]; 12],
            lists: std::array::from_fn(|_| BusinessList {
                entries: Vec::new(),
                active_count: 0,
            }),
        };
        state.reset_from_v14(image)?;
        Ok(state)
    }

    /// Original0046b0e0 inside00466870, followed by its ownership-list reset.
    /// Resetting an existing record preserves opaque byte7, as does470900.
    pub fn reset_from_v14(&mut self, image: &crate::pe::PeImage<'_>) -> Result<(), String> {
        self.completed_groups = 0;
        for (index, group) in self.groups.iter_mut().enumerate() {
            group[0] = 0;
            group[1..5].fill(255);
            group[5] = 0;
            group[6] = 0;
            group[8..12].fill(0);
            for slot in 0..4 {
                let value = i32::from_le_bytes(
                    image
                        .at(0x5397ac + index as u32 * 60 + slot * 4, 4)?
                        .try_into()
                        .unwrap(),
                );
                if value > -1 {
                    group[1 + slot as usize] = value as u8;
                    group[5] += 1;
                }
            }
            group[6] = group[5];
        }
        for list in &mut self.lists {
            list.entries.clear();
            list.active_count = 0;
        }
        Ok(())
    }

    /// Complete00467a30 bookkeeping over current shared ownership/business data.
    pub fn maintain(
        &mut self,
        player_index: u8,
        owned: &[u8; 52],
        mapping: &[u8; 52],
        businesses: &[[u8; 544]],
        categories: &[i16; 600],
        initialize: impl FnMut(i32) -> Result<i32, String>,
    ) -> Result<(), String> {
        self.reconcile_groups(owned, mapping)?;
        self.reconcile_businesses(player_index, owned, businesses, categories, initialize)
    }

    /// Original00467a30 first pass, including x86 modulo32 bit shifts.
    pub fn reconcile_groups(&mut self, owned: &[u8; 52], mapping: &[u8; 52]) -> Result<(), String> {
        for (index, &owner) in owned.iter().enumerate() {
            let group = self
                .groups
                .get_mut(usize::from(mapping[index]))
                .ok_or("ownership group leaves original player records")?;
            let mask = 1u32 << (index & 31);
            let mut bits = u32::from_le_bytes(group[8..12].try_into().unwrap());
            if owner != 0 && bits & mask == 0 {
                bits |= mask;
                group[0] = group[0].wrapping_add(1);
                group[6] = group[6].wrapping_sub(1);
                if group[6] == 0 {
                    self.completed_groups = self.completed_groups.wrapping_add(1);
                }
            } else if owner == 0 && bits & mask != 0 {
                bits &= !mask;
                group[0] = group[0].wrapping_sub(1);
                group[6] = group[6].wrapping_add(1);
                if group[6] == 1 {
                    self.completed_groups = self.completed_groups.wrapping_sub(1);
                }
            }
            group[8..12].copy_from_slice(&bits.to_le_bytes());
        }
        Ok(())
    }

    /// Original0046aa40 checks list0 for duplicates even when adding to list1.
    /// The initializer supplies4708a0's recovered numeric value on demand.
    pub fn insert(
        &mut self,
        mode: usize,
        business: i32,
        initialize: impl FnOnce(i32) -> Result<i32, String>,
    ) -> Result<(), String> {
        if self.lists[0].contains(business) {
            return Ok(());
        }
        let list = self
            .lists
            .get_mut(mode)
            .ok_or("ownership list mode exceeds0/1")?;
        let allocated =
            u16::try_from(list.entries.len()).map_err(|_| "ownership list exceeds u16 slots")?;
        let slot = if allocated == 0 {
            list.entries.push([0; 28]);
            0
        } else if allocated > list.active_count {
            let Some(slot) = list.entries.iter().position(|entry| entry[0] == 0) else {
                return Ok(());
            };
            slot
        } else {
            // Valid game lists grow by one. Preserve the native count update
            // before initialization; incoherent counters become explicit errors.
            list.active_count = list.active_count.wrapping_add(1);
            if allocated == u16::MAX && list.active_count == 0 {
                // Native increments both u16 counters to0, allocates zero
                // bytes, then returns without rebinding or initializing. The
                // old allocation is no longer visible through either count.
                list.entries.clear();
                return Ok(());
            }
            if usize::from(list.active_count) != list.entries.len() + 1 {
                return Err("ownership list growth counters do not describe its allocation".into());
            }
            list.entries.push([0; 28]);
            let slot = list.entries.len() - 1;
            initialize_slot(&mut list.entries[slot], business, initialize)?;
            return Ok(());
        };
        initialize_slot(&mut list.entries[slot], business, initialize)?;
        list.active_count = list.active_count.wrapping_add(1);
        Ok(())
    }

    /// Original00467a30 business pass. Inactive businesses leave old list
    /// entries untouched; owned businesses update only list0 in this pass.
    /// `categories` holds00462f30's result per business slot, not building slot.
    pub fn reconcile_businesses(
        &mut self,
        player_index: u8,
        owned: &[u8; 52],
        businesses: &[[u8; 544]],
        categories: &[i16; 600],
        mut initialize: impl FnMut(i32) -> Result<i32, String>,
    ) -> Result<(), String> {
        if businesses.len() != 600 {
            return Err("ownership pass requires the original600 businesses".into());
        }
        for (slot, business) in businesses.iter().enumerate() {
            if business[0x21c] == 0 {
                continue;
            }
            if i32::from(business[0xa] as i8) == i32::from(player_index) + 1 {
                self.insert(0, slot as i32, &mut initialize)?;
            } else {
                if self.lists[0].contains(slot as i32) {
                    self.lists[0].remove(slot as i32);
                }
                let held = *owned
                    .get(categories[slot] as usize)
                    .ok_or("business category leaves original ownership bytes")?;
                if held != 0 {
                    self.insert(1, slot as i32, &mut initialize)?;
                } else {
                    self.lists[1].remove(slot as i32);
                }
            }
        }
        Ok(())
    }
}

impl BusinessList {
    fn contains(&self, business: i32) -> bool {
        self.entries
            .iter()
            .any(|entry| entry[0] != 0 && entry[4..8] == business.to_le_bytes())
    }

    /// Original0046ae20 matches the ID without testing the active byte.
    pub fn remove(&mut self, business: i32) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry[4..8] == business.to_le_bytes())
        {
            clear_slot(entry);
            self.active_count = self.active_count.wrapping_sub(1);
        }
    }
}

fn clear_slot(entry: &mut [u8; 28]) {
    entry[0] = 0;
    entry[4..28].fill(0);
    entry[4..8].copy_from_slice(&(-1i32).to_le_bytes());
    entry[24..28].copy_from_slice(&(-1i32).to_le_bytes());
}

fn initialize_slot(
    entry: &mut [u8; 28],
    business: i32,
    initialize: impl FnOnce(i32) -> Result<i32, String>,
) -> Result<(), String> {
    clear_slot(entry);
    entry[0] = 1;
    entry[4..8].copy_from_slice(&business.to_le_bytes());
    entry[20..24].copy_from_slice(&initialize(business)?.to_le_bytes());
    Ok(())
}

/// PC53 observations of004453b0 followed by004708a0. Live gameplay uses PC64.
pub fn entry_value(building: &[u8; 64], prestige: f32, base_cost: i32, denominator: f32) -> i32 {
    let factor = match building[0x2c] as i8 {
        0 => 1.,
        1 => f64::from(1.2f32),
        _ => 1.5,
    };
    let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
        .wrapping_mul(i32::from(building[8]));
    let size = f64::from(volume) * 0.6 + 0.8;
    let location = f64::from(prestige) * 0.5 + 1.;
    let value = ((size * location) * f64::from(base_cost)) * factor;
    crate::economy::ftol_low_word(value / f64::from(denominator))
}

/// Live PC64 arithmetic, including the original binary64 size constants and
/// the single-precision quality factor. Invalid FTOL returns indefinite's low0.
pub fn entry_value_extended(
    building: &[u8; 64],
    prestige: f32,
    base_cost: i32,
    denominator: f32,
) -> i32 {
    use crate::x87::Sum;
    if !prestige.is_finite() || !denominator.is_finite() || denominator == 0.0 {
        return 0;
    }
    let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
        .wrapping_mul(i32::from(building[8]));
    let mut size = Sum::from_i32(volume);
    size.mul(Sum::from_f64(0.6).unwrap());
    size.add(Sum::from_f64(0.8).unwrap());
    let mut location = Sum::from_f32(prestige).unwrap();
    location.mul_f32(0.5).unwrap();
    location.add_f32(1.0).unwrap();
    size.mul(location);
    size.mul(Sum::from_i32(base_cost));
    size.mul_f32(match building[0x2c] as i8 {
        0 => 1.0,
        1 => 1.2,
        _ => 1.5,
    })
    .unwrap();
    size.div_f32(denominator).unwrap();
    size.ftol_low_word()
}

/// Bind467a30 to the authoritative retained allocations and live pools.
#[allow(clippy::too_many_arguments)]
pub fn maintain_player(
    storage: &mut Storage,
    player: &mut crate::player_manager::Player,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    image: &crate::pe::PeImage<'_>,
    catalog: &crate::business_pool::BusinessCatalog,
    allocate: impl FnMut(usize) -> Result<u32, String>,
) -> Result<(), String> {
    use crate::player_manager::word;
    let mapping = crate::ai_ownership::group_mapping(image)?;
    let owned: [u8; 52] = player.bytes[0x178..0x1ac].try_into().unwrap();
    let slot = player.bytes[0x1bc];
    let mut business_categories = [-1i16; 600];
    for (i, business) in businesses.iter().enumerate() {
        if business[0x21c] == 0 || i32::from(business[10] as i8) == i32::from(slot) + 1 {
            continue;
        }
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
        let building = buildings
            .get(building as usize)
            .ok_or("AI ownership building leaves pool")?;
        let reference = word(building, 0x40);
        let offset = reference
            .checked_sub(0x617628)
            .ok_or("AI ownership category precedes pool")?;
        if offset % 380 != 0 || offset / 380 >= 52 {
            return Err("AI ownership category reference invalid".into());
        }
        *business_categories
            .get_mut(i)
            .ok_or("AI ownership business pool exceeds600")? =
            i16::from_le_bytes(categories[offset as usize / 380][..2].try_into().unwrap());
    }
    storage.mutate(player, allocate, |state| {
        state.maintain(
            slot,
            &owned,
            &mapping,
            businesses,
            &business_categories,
            |business| {
                crate::ai_ownership::live_entry_value_from_catalog(
                    business, businesses, buildings, categories, catalog,
                )
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original AI initialization observations"]
    fn matches_native_initialization() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_RESET").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAIGR01");
        assert_eq!([word(8), word(12), word(16), word(20)], [1, 12, 60, 0x1e50]);
        assert_eq!(image.at(0x5397ac, 720).unwrap(), &bytes[32..752]);
        let mut at = 752;
        for _ in 0..word(24) {
            let before_len = word(at + 4) as usize;
            let after_len = word(at + 8) as usize;
            at += 12;
            let mut actual = bytes[at..at + before_len].to_vec();
            at += before_len;
            let mut state = State::from_v14(&image).unwrap();
            state.completed_groups = actual[0xaa8];
            state.groups =
                std::array::from_fn(|i| actual[0xaac + i * 12..0xab8 + i * 12].try_into().unwrap());
            state.reset_from_v14(&image).unwrap();
            actual[0xaa8] = state.completed_groups;
            for (i, group) in state.groups.iter().enumerate() {
                actual[0xaac + i * 12..0xab8 + i * 12].copy_from_slice(group);
            }
            assert_eq!(
                actual,
                bytes[at..at + after_len],
                "whole native group initialization"
            );
            at += after_len;
        }
        for _ in 0..word(28) {
            let pointers = [word(at + 4), word(at + 8)];
            let frees = word(at + 12) as usize;
            at += 16;
            let expected: Vec<u32> = pointers
                .iter()
                .zip([0x10080000, 0x10088000])
                .filter_map(|(&present, address)| (present != 0).then_some(address))
                .collect();
            assert_eq!(frees, expected.len());
            for pointer in expected {
                assert_eq!(
                    [word(at), word(at + 4), word(at + 8)],
                    [pointer, 0x53c9f0, 0xbeb]
                );
                at += 12;
            }
            let before_len = word(at) as usize;
            let after_len = word(at + 4) as usize;
            at += 8;
            let mut actual = bytes[at..at + before_len].to_vec();
            at += before_len;
            let mut state = State::from_v14(&image).unwrap();
            for (mode, list) in state.lists.iter_mut().enumerate() {
                let count = 0xb44 + mode * 4;
                list.active_count =
                    u16::from_le_bytes(actual[count..count + 2].try_into().unwrap());
                list.entries.resize(
                    usize::from(u16::from_le_bytes(
                        actual[count + 2..count + 4].try_into().unwrap(),
                    )),
                    [0xa5; 28],
                );
            }
            state.reset_from_v14(&image).unwrap();
            actual[0xb3c..0xb44].fill(0);
            for (mode, list) in state.lists.iter().enumerate() {
                let count = 0xb44 + mode * 4;
                actual[count..count + 2].copy_from_slice(&list.active_count.to_le_bytes());
                actual[count + 2..count + 4]
                    .copy_from_slice(&(list.entries.len() as u16).to_le_bytes());
            }
            assert_eq!(
                actual,
                bytes[at..at + after_len],
                "whole native list reset block"
            );
            at += after_len;
        }
        // This additional seam isolates the serialized boundary; the separate
        // saved-player fixture executes the actual manager/slot creator too.
        let before_len = word(at) as usize;
        let after_len = word(at + 4) as usize;
        at += 8;
        let before = &bytes[at..at + before_len];
        at += before_len;
        let after = &bytes[at..at + after_len];
        assert_eq!(&before[0x1bc..], &after[0x1bc..]);
        at += after_len;
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original saved-player loader observations"]
    fn matches_native_saved_player_initialization() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_LOAD").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MT48C800");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let name_len = word(at) as usize;
            let count = word(at + 4);
            at += 8 + name_len;
            let mut players = Vec::new();
            for slot in 0..count {
                let kind = word(at);
                assert_eq!(word(at + 8), slot);
                assert_eq!(word(at + 12), if kind == 0 { 0x224 } else { 0x1e50 });
                assert_eq!(word(at + 16), kind);
                let length = word(at + 20) as usize;
                at += 24;
                assert_eq!(length, 440);
                let payload = &bytes[at..at + length];
                at += length;
                let prefix_len = word(at) as usize;
                at += 4 + prefix_len;
                let after = &bytes[at..at + prefix_len];
                at += prefix_len;
                assert_eq!(&after[4..0x1bc], payload, "case{case} slot{slot} prefix");
                assert_eq!(after[0x1bc], slot as u8);
                players.push(
                    crate::economy::PlayerAccount::from_saved(&crate::save::SavedPlayer {
                        metadata: &[0; 48],
                        bytes: payload.try_into().unwrap(),
                    })
                    .unwrap(),
                );
                let trace_len = word(at) as usize;
                at += 4 + trace_len;
            }
            let ledgers = crate::economy::Ledgers {
                businesses: Vec::new(),
                residents: crate::economy::ResidentAccounts {
                    active_count: 0,
                    nodes: Vec::new(),
                    accounts: Vec::new(),
                },
                players,
            };
            let states = ledgers.initialize_ai_ownership(&image).unwrap();
            for (player, state) in ledgers.players.iter().zip(states) {
                match player.controller_kind {
                    0 => assert!(state.is_none()),
                    1 | 2 => assert_eq!(state.unwrap(), State::from_v14(&image).unwrap()),
                    _ => panic!("unexpected fixture player kind"),
                }
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original AI entry arithmetic observations"]
    fn matches_native_entry_value() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_ENTRY").unwrap()).unwrap();
        check_entry_value(&bytes, entry_value);
    }

    #[test]
    #[ignore = "requires original PC64 AI entry observations"]
    fn matches_native_extended_entry_value() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_ENTRY_PC64").unwrap()).unwrap();
        check_entry_value(&bytes, entry_value_extended);
    }

    fn check_entry_value(bytes: &[u8], calculate: fn(&[u8; 64], f32, i32, f32) -> i32) {
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MT4708A0");
        assert_eq!(word(8), 2);
        let mut at = 16;
        for _ in 0..word(12) {
            let length = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            at += length;
            let inputs: [u32; 12] = std::array::from_fn(|i| word(at + i * 4));
            at += 48;
            assert_eq!(word(at), 28);
            at += 4;
            let mut building = [0; 64];
            for i in 0..5 {
                building[4 + i] = inputs[1 + i] as u8;
            }
            building[0x2c] = inputs[6] as u8;
            let value = calculate(
                &building,
                f32::from_bits(inputs[7]),
                inputs[8] as i32,
                f32::from_bits(inputs[9]),
            );
            at += 28; // Sentinel input entry; this test checks recovered arithmetic.
            assert_eq!(value as u32, word(at + 20), "{name} native_ftol");
            at += 28;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original complete ownership maintenance observations"]
    fn matches_native_lists_and_expiry() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_OWNERSHIP").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let mapping = group_mapping(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let field = |raw: &[u8], at| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        let word = |at| field(&bytes, at);
        assert_eq!(&bytes[..8], b"MTAIO001");
        assert_eq!(word(8), 8);
        let mut at = 12;
        let businesses: Vec<[u8; 544]> = bytes[at..at + 600 * 544]
            .chunks_exact(544)
            .map(|r| r.try_into().unwrap())
            .collect();
        at += 600 * 544;
        let buildings: Vec<[u8; 64]> = bytes[at..at + 600 * 64]
            .chunks_exact(64)
            .map(|r| r.try_into().unwrap())
            .collect();
        at += 600 * 64;
        let categories: [i16; 600] = std::array::from_fn(|i| {
            i16::from_le_bytes(bytes[at + i * 2..at + i * 2 + 2].try_into().unwrap())
        });
        at += 1200;
        let prestige: [f32; 52] = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
        at += 208;
        let cost: [i32; 45] = std::array::from_fn(|i| word(at + i * 4) as i32);
        at += 180;
        let initialize = |slot: i32| {
            let record = &businesses[slot as usize];
            let building = i16::from_le_bytes(record[8..10].try_into().unwrap()) as usize;
            Ok(entry_value(
                &buildings[building],
                prestige[categories[slot as usize] as usize],
                cost[field(record, 4) as usize],
                f32::from_bits(field(record, 0x20)),
            ))
        };
        let decode = |records: &[Vec<u8>]| {
            let raw = &records[0];
            assert_eq!(raw.len(), 0xb50);
            State {
                completed_groups: raw[0xaa8],
                groups: std::array::from_fn(|i| {
                    raw[0xaac + i * 12..0xab8 + i * 12].try_into().unwrap()
                }),
                lists: std::array::from_fn(|mode| {
                    let offset = 0xb44 + mode * 4;
                    let active_count =
                        u16::from_le_bytes(raw[offset..offset + 2].try_into().unwrap());
                    let count = u16::from_le_bytes(raw[offset + 2..offset + 4].try_into().unwrap());
                    assert_eq!(records[mode + 1].len(), usize::from(count) * 28);
                    BusinessList {
                        active_count,
                        entries: records[mode + 1]
                            .chunks_exact(28)
                            .map(|r| r.try_into().unwrap())
                            .collect(),
                    }
                }),
            }
        };
        let mut retained = None;
        for case in 0..8 {
            let operation = word(at);
            let business = word(at + 4) as i32;
            let mode = word(at + 8) as usize;
            at += 12;
            let mut phases = Vec::new();
            for _ in 0..2 {
                let mut records = Vec::new();
                for _ in 0..3 {
                    let length = word(at) as usize;
                    at += 4;
                    records.push(bytes[at..at + length].to_vec());
                    at += length;
                }
                phases.push(records);
            }
            let mut state = if let Some(previous) = retained.take() {
                assert_eq!(previous, decode(&phases[0]), "case{case} preceding state");
                previous
            } else {
                decode(&phases[0])
            };
            let mut raw = phases[0][0].clone();
            let apply = |state: &mut State, raw: &mut Vec<u8>| {
                let owned = raw[0x178..0x1ac].try_into().unwrap();
                match operation {
                    0x467a30 => {
                        state
                            .maintain(
                                raw[0x1bc],
                                &owned,
                                &mapping,
                                &businesses,
                                &categories,
                                initialize,
                            )
                            .unwrap();
                    }
                    0x46aa40 => state.insert(mode, business, initialize).unwrap(),
                    0x46ae20 => state.lists[mode].remove(business),
                    0x48c450 => {
                        let player =
                            crate::economy::PlayerAccount::from_saved(&crate::save::SavedPlayer {
                                metadata: &[0; 48],
                                bytes: raw[4..444].try_into().unwrap(),
                            })
                            .unwrap();
                        let mut ledgers = crate::economy::Ledgers {
                            businesses: businesses.clone(),
                            players: vec![player],
                            residents: crate::economy::ResidentAccounts {
                                active_count: 0,
                                nodes: Vec::new(),
                                accounts: Vec::new(),
                            },
                        };
                        let mut prefixes = buildings.clone();
                        let mut expiry = crate::category::Expiry {
                            buildings: &mut prefixes,
                            render_kinds: &[0; 600],
                            chains: &std::array::from_fn(|_| Vec::new()),
                            ledgers: &mut ledgers,
                            normal_tick: 0,
                            accounting_enabled: false,
                            refresh_counter: 0,
                        };
                        let mut category = crate::category::State {
                            owner: 1,
                            ..Default::default()
                        };
                        expiry
                            .release(business as usize, &mut category, |event, expiry| {
                                if let crate::category::ExpiryEvent::OwnerLists(owner) = event {
                                    let held = &expiry.ledgers.players[owner].owned_categories;
                                    state.maintain(
                                        raw[0x1bc],
                                        held,
                                        &mapping,
                                        &expiry.ledgers.businesses,
                                        &categories,
                                        initialize,
                                    )?;
                                }
                                Ok(())
                            })
                            .unwrap();
                        assert_eq!(category.owner, 0);
                        raw[0x178..0x1ac].copy_from_slice(&ledgers.players[0].owned_categories);
                    }
                    _ => panic!("unknown original operation"),
                }
            };
            let mut stored_raw = raw.clone();
            let mut runtime = crate::player_manager::Player {
                reference: 0x10010000,
                bytes: raw.clone(),
                auxiliaries: vec![],
            };
            let mut storage = Storage::default();
            for mode in 0..2 {
                if !phases[0][mode + 1].is_empty() {
                    let reference = 0x10020000 + mode as u32 * 0x10000;
                    crate::player_manager::put(&mut runtime.bytes, 0xb3c + mode * 4, reference);
                    storage.lists.insert(
                        reference,
                        phases[0][mode + 1]
                            .chunks_exact(28)
                            .map(|r| r.try_into().unwrap())
                            .collect(),
                    );
                }
            }
            let mut next = 0x10040000;
            storage
                .mutate(
                    &mut runtime,
                    |size| {
                        let r = next;
                        next += size as u32;
                        Ok(r)
                    },
                    |live| {
                        apply(live, &mut stored_raw);
                        Ok(())
                    },
                )
                .unwrap();
            // The expiry callback changes owned categories outside list storage.
            runtime.bytes[0x178..0x1ac].copy_from_slice(&stored_raw[0x178..0x1ac]);
            assert_eq!(
                storage.state(&runtime).unwrap(),
                decode(&phases[1]),
                "case{case} retained allocations"
            );
            runtime.bytes[0xb3c..0xb44].fill(0); // opaque host addresses are normalized in this native fixture
            assert_eq!(runtime.bytes, phases[1][0], "case{case} retained player");
            apply(&mut state, &mut raw);
            assert_eq!(
                state,
                decode(&phases[1]),
                "case{case} complete ownership state"
            );
            raw[0xaa8] = state.completed_groups;
            for (i, group) in state.groups.iter().enumerate() {
                raw[0xaac + i * 12..0xab8 + i * 12].copy_from_slice(group);
            }
            for (mode, list) in state.lists.iter().enumerate() {
                let offset = 0xb44 + mode * 4;
                raw[offset..offset + 2].copy_from_slice(&list.active_count.to_le_bytes());
                raw[offset + 2..offset + 4]
                    .copy_from_slice(&(list.entries.len() as u16).to_le_bytes());
            }
            assert_eq!(raw, phases[1][0], "case{case} complete player image");
            retained = Some(state);
        }
        assert_eq!(&bytes[at..at + 8], b"MTWRAP01");
        at += 8;
        let mut phases = Vec::new();
        for _ in 0..2 {
            let mut records = Vec::new();
            for _ in 0..3 {
                let length = word(at) as usize;
                at += 4;
                records.push(bytes[at..at + length].to_vec());
                at += length;
            }
            phases.push(records);
        }
        let mut state = decode(&phases[0]);
        state
            .insert(0, 0, |_| {
                panic!("wrapped native allocation does not initialize")
            })
            .unwrap();
        assert_eq!(state, decode(&phases[1]), "u16 allocation/count wrap");
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original ownership group observations"]
    fn matches_native_groups() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_OWNERSHIP_GROUPS").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MT467P01");
        assert_eq!([word(8), word(12), word(16), word(20)], [8, 0xb50, 52, 88]);
        let mapping = group_mapping(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        assert_eq!(mapping, &bytes[24..76]);
        let mut at = 76;
        for case in 0..8 {
            assert_eq!(word(at), case);
            let completed_groups = word(at + 4) as u8;
            at += 8;
            let owned = bytes[at..at + 52].try_into().unwrap();
            at += 52;
            let length = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            at += length;
            let mut actual = bytes[at..at + 0xb50].to_vec();
            at += 0xb50;
            let mut state = State {
                completed_groups,
                groups: std::array::from_fn(|i| {
                    actual[0xaac + i * 12..0xab8 + i * 12].try_into().unwrap()
                }),
                lists: std::array::from_fn(|_| BusinessList {
                    entries: Vec::new(),
                    active_count: 0,
                }),
            };
            state.reconcile_groups(&owned, &mapping).unwrap();
            actual[0xaa8] = state.completed_groups;
            for (i, group) in state.groups.iter().enumerate() {
                actual[0xaac + i * 12..0xab8 + i * 12].copy_from_slice(group);
            }
            assert_eq!(
                actual,
                &bytes[at..at + 0xb50],
                "{name} complete player image"
            );
            at += 0xb50;
        }
        assert_eq!(at, bytes.len());
    }
}
