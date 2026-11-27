//! Shared original player metrics used by comparison, objectives and history.
//! Player records are the saved/common prefix (runtime bytes after the vtable).

/// Original0047ff20/0047ff80 business-count cache. Its age is an explicit
/// runtime input; reading a clean cache must not silently recount businesses.
pub struct BusinessCounts {
    pub age: u8,
    pub values: [i32; 6],
}
impl BusinessCounts {
    pub fn get(
        &mut self,
        slot: usize,
        players: usize,
        businesses: &[[u8; 544]],
    ) -> Result<i32, String> {
        if slot >= players || players > 6 {
            return Err("business-count player leaves roster".into());
        }
        if self.age > 10 {
            self.age = 0;
            self.refresh(players, businesses)?;
        }
        Ok(self.values[slot])
    }
    ///47ff80 refreshes values without changing the lazy age byte.
    pub fn refresh(&mut self, players: usize, businesses: &[[u8; 544]]) -> Result<(), String> {
        if players > 6 {
            return Err("business-count roster exceeds six slots".into());
        }
        self.values[..players].fill(0);
        if businesses.len() != 600 {
            return Err("business-count pool must have 600 records".into());
        }
        for business in businesses {
            let owner = i32::from(business[0xa] as i8) - 1;
            if business[0x21c] != 0
                && owner >= 0
                && (owner as usize) < players
                && i32::from_le_bytes(business[..4].try_into().unwrap()) != 2
            {
                self.values[owner as usize] = self.values[owner as usize].wrapping_add(1);
            }
        }
        Ok(())
    }
}

/// Retained portions of47f8e0, called at fresh-start4ba148 before demand setup.
#[allow(clippy::too_many_arguments)]
pub fn reset(
    neighbors: &mut crate::ai_neighbors::Cache,
    ownership_age: &mut u8,
    counts: &mut BusinessCounts,
    market: &mut crate::market::Cache,
    players: &[&crate::player_manager::Player],
    categories: &[[u8; 380]; 52],
    businesses: &[[u8; 544]],
    residents: &crate::economy::ResidentAccounts,
    image: &crate::pe::PeImage<'_>,
) -> Result<(), String> {
    *ownership_age = 100;
    counts.age = 100;
    market.dirty = true;
    neighbors.owned_properties.fill(0);
    neighbors.completed_groups.fill(0);
    counts.values.fill(0);
    market.prices.fill(0.);
    // Native reset does not overwrite the cached rent at object+168.
    neighbors.refresh_population(residents)?;
    neighbors.refresh_owned(players, categories, residents, image)?;
    counts.refresh(players.len(), businesses)
}

#[cfg(feature = "lua4")]
pub fn from_runtime(
    city: &crate::level_startup::City,
    players: &[[u8; 440]],
    image: &crate::pe::PeImage<'_>,
    hour: u32,
    building_base: u32,
) -> Result<Vec<(usize, [f32; 9])>, String> {
    let chains =
        crate::building_pool::category_chains(&city.categories, &city.buildings, building_base)?;
    let buildings: Vec<_> = chains
        .iter()
        .enumerate()
        .flat_map(|(category, chain)| {
            chain.iter().map(move |&slot| Building {
                category,
                slot,
                bytes: &city.buildings[slot],
            })
        })
        .collect();
    let businesses: Vec<_> = city
        .ledgers
        .businesses
        .iter()
        .map(|row| row.as_slice())
        .collect();
    let categories: Vec<_> = city
        .categories
        .iter()
        .map(|row| {
            (
                row[5],
                f32::from_le_bytes(row[0x84..0x88].try_into().unwrap()),
            )
        })
        .collect();
    comparison(players, &businesses, &buildings, &categories, image, hour)
}
pub struct Building<'a> {
    pub category: usize,
    pub slot: usize,
    pub bytes: &'a [u8],
}
pub fn comparison(
    players: &[[u8; 440]],
    businesses: &[&[u8]],
    buildings: &[Building<'_>],
    categories: &[(u8, f32)],
    image: &crate::pe::PeImage<'_>,
    hour: u32,
) -> Result<Vec<(usize, [f32; 9])>, String> {
    use crate::x87::Sum;
    if players.len() > 6 || categories.len() != 52 {
        return Err("invalid comparison roster/category count".into());
    }
    let int = |b: &[u8], at| i32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let float = |b: &[u8], at| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let mut counts = vec![[0u32; 2]; players.len()];
    for b in businesses {
        if b[0x21c] != 0
            && int(b, 0) != 2
            && let Some(owner) = b[0x0a].checked_sub(1).map(usize::from)
        {
            let row = counts
                .get_mut(owner)
                .ok_or("comparison business owner leaves roster")?;
            row[0] += 1;
        }
    }
    for category in 0..52 {
        if image.at(0x5385a4 + category as u32 * 88, 1)?[0] != 0
            && let Some(owner) = players.iter().position(|p| p[0x174 + category] != 0)
        {
            counts[owner][1] += 1;
        }
    }
    let mut rows = Vec::new();
    for (index, player) in players.iter().enumerate().rev() {
        let p = player;
        if p[0xc] == 0 {
            continue;
        }
        let current = usize::from(p[0x140]);
        if current > 1 {
            return Err("comparison ledger selector exceeds banks".into());
        }
        let sum = |offsets: &[usize], bank: usize| -> Result<f32, String> {
            let get = |offset: usize| float(p, offset - 4 + bank * 152);
            let mut v = Sum::from_f32(get(offsets[0]))?;
            for &offset in &offsets[1..] {
                v.add_f32(get(offset))?;
            }
            Ok(v.to_f32())
        };
        let empire = empire_value(index, p, businesses, buildings, categories, image)?.to_f32();
        rows.push((
            index,
            [
                float(p, 0),
                counts[index][0] as f32,
                counts[index][1] as f32,
                current_income(p)?.to_f32(),
                empire,
                sum(
                    &[
                        0x58, 0x4c, 0x24, 0x28, 0x2c, 0x30, 0x78, 0x7c, 0x80, 0x84, 0x88, 0x8c,
                        0x90, 0x94, 0x9c, 0x98,
                    ],
                    if hour < 6 { 1 - current } else { current },
                )?,
                int(p, 0x148) as f32,
                int(p, 0x1ac).wrapping_add(int(p, 0x1a8)).max(0) as f32,
                int(p, 0x1b0) as f32,
            ],
        ));
    }
    Ok(rows)
}

/// Original47fbb0: intermediate property totals store to f32, but the final
/// cash addition returns in ST(0). Objectives must not round that return early.
pub fn empire_value(
    index: usize,
    p: &[u8; 440],
    businesses: &[&[u8]],
    buildings: &[Building<'_>],
    categories: &[(u8, f32)],
    image: &crate::pe::PeImage<'_>,
) -> Result<crate::x87::Sum, String> {
    use crate::x87::Sum;
    let building_record = |slot: usize| {
        buildings
            .iter()
            .find(|b| b.slot == slot)
            .ok_or("comparison business building is absent from category links")
    };
    let int = |b: &[u8], at| i32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let float = |b: &[u8], at| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let category_value = |category: usize| -> Result<Sum, String> {
        let mut v = Sum::from_f32(categories[category].1)?;
        v.mul_f32(image.f32_at(0x52502c)?)?;
        v.add_f32(image.f32_at(0x525034)?)?;
        v.mul_f32(image.f32_at(0x525b20)?)?;
        v.mul_f32(image.f32_at(0x525b1c)?)?;
        v.add_f32(0.5)?;
        let mut v = Sum::from_i32(v.ftol_low_word());
        v.mul_f32(image.f32_at(0x525248)?)?;
        Ok(v)
    };
    let mut empire = 0f32;
    for (category, &(owner, _)) in categories.iter().enumerate() {
        if image.at(0x5385a4 + category as u32 * 88, 1)?[0] == 0 {
            continue;
        }
        if usize::from(owner) == index + 1 {
            let mut value = category_value(category)?;
            value.add_f32(empire)?;
            empire = value.to_f32();
        }
        for building in buildings.iter().filter(|b| b.category == category) {
            let slot = int(building.bytes, 0x1c);
            if slot == -1 {
                continue;
            }
            let business = businesses
                .get(slot as usize)
                .ok_or("comparison building business slot")?;
            if business[0x0a].checked_sub(1).map(usize::from) != Some(index) {
                continue;
            }
            let record = *business;
            let prior = (usize::from(record[0x209]) + 3) % 4;
            let mut value = Sum::from_f32(float(record, 0x58 + prior * 4))?;
            let slot = i16::from_le_bytes(record[8..10].try_into().unwrap());
            let model = building_record(slot as usize)?;
            value.add_f32(float(model.bytes, 0x38))?;
            let mut half = Sum::from_f32(float(record, 0x20))?;
            half.mul_f32(0.5)?;
            value.add(half);
            value.add_f32(empire)?;
            empire = value.to_f32();
        }
    }
    let mut value = Sum::from_f32(float(p, 0))?;
    value.add_f32(empire)?;
    Ok(value)
}

/// Original47fca0 current ledger income, before the caller's f32 store.
pub fn current_income(p: &[u8; 440]) -> Result<crate::x87::Sum, String> {
    let bank = usize::from(p[0x140]);
    if bank > 1 {
        return Err("income ledger selector exceeds banks".into());
    }
    let offsets = [
        0x98, 0x9c, 0x78, 0x7c, 0x80, 0x84, 0x88, 0x8c, 0x90, 0x94, 0xa0, 0x4c, 0x24, 0x28, 0x2c,
        0x30, 0x58,
    ];
    // Native 0047fcf6 loads the first field before the 16 ordered adds.
    // Seeding with +0 instead loses an all-negative-zero ledger's sign.
    let first = offsets[0] - 4 + bank * 152;
    let mut result =
        crate::x87::Sum::from_f32(f32::from_le_bytes(p[first..first + 4].try_into().unwrap()))?;
    for offset in &offsets[1..] {
        let at = offset - 4 + bank * 152;
        result.add_f32(f32::from_le_bytes(p[at..at + 4].try_into().unwrap()))?;
    }
    Ok(result)
}

#[cfg(feature = "lua4")]
pub fn runtime_metric(
    city: &crate::level_startup::City,
    prefix: &[u8; 440],
    slot: usize,
    address: u32,
    image: &crate::pe::PeImage<'_>,
    building_base: u32,
) -> Result<crate::x87::Sum, String> {
    if address == 0x47fca0 {
        return current_income(prefix);
    }
    if address != 0x47fbb0 {
        return Err("unknown objective metric".into());
    }
    let chains =
        crate::building_pool::category_chains(&city.categories, &city.buildings, building_base)?;
    let buildings: Vec<_> = chains
        .iter()
        .enumerate()
        .flat_map(|(category, chain)| {
            chain.iter().map(move |&slot| Building {
                category,
                slot,
                bytes: &city.buildings[slot],
            })
        })
        .collect();
    let businesses: Vec<_> = city
        .ledgers
        .businesses
        .iter()
        .map(|row| row.as_slice())
        .collect();
    let categories: Vec<_> = city
        .categories
        .iter()
        .map(|row| {
            (
                row[5],
                f32::from_le_bytes(row[0x84..0x88].try_into().unwrap()),
            )
        })
        .collect();
    empire_value(slot, prefix, &businesses, &buildings, &categories, image)
}

#[cfg(test)]
mod reset_tests {
    use super::*;
    use crate::player_manager::{Player, word};
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    fn rows<const N: usize>(b: &[u8]) -> Vec<[u8; N]> {
        b.chunks_exact(N).map(|v| v.try_into().unwrap()).collect()
    }
    #[test]
    #[ignore = "requires original whole metric reset fixture"]
    fn matches_native_reset() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_METRIC_RESET")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTMETRS1");
        assert_eq!(r.word(), 0x37f);
        let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        for case in 0..r.word() {
            let count = r.word() as usize;
            let _entry = r.word();
            let active_count = r.word();
            assert_eq!(r.word(), 0);
            let b: Vec<_> = (0..6).map(|_| r.blob()).collect();
            let expected = r.blob();
            let expected_rows = r.blob();
            let mut neighbors = crate::ai_neighbors::Cache {
                rows: rows(b[1]).try_into().unwrap(),
                owned_properties: std::array::from_fn(|i| word(b[0], 0x8c + i * 16) as i32),
                completed_groups: std::array::from_fn(|i| word(b[0], 0x94 + i * 16) as i32),
            };
            let mut age = b[0][6];
            let mut counts = BusinessCounts {
                age: b[0][3],
                values: std::array::from_fn(|i| word(b[0], 0x90 + i * 16) as i32),
            };
            let mut market = crate::market::Cache {
                dirty: b[0][7] != 0,
                prices: std::array::from_fn(|i| f32::from_bits(word(b[0], 0xe8 + i * 4))),
                rent: f32::from_bits(word(b[0], 0x168)),
            };
            let players: Vec<_> = b[3]
                .chunks_exact(0x1e50)
                .enumerate()
                .map(|(i, b)| Player {
                    reference: 0x10010000 + i as u32 * 0x1e50,
                    bytes: b.to_vec(),
                    auxiliaries: vec![],
                })
                .collect();
            let players: Vec<_> = players[..count].iter().collect();
            let residents = crate::economy::ResidentAccounts {
                active_count,
                nodes: b[4]
                    .chunks_exact(336)
                    .map(|b| crate::resident::Resident::from_bytes(b[..330].try_into().unwrap()))
                    .collect(),
                accounts: vec![],
            };
            reset(
                &mut neighbors,
                &mut age,
                &mut counts,
                &mut market,
                &players,
                &rows(b[2]).try_into().unwrap(),
                &rows(b[5]),
                &residents,
                &image,
            )
            .unwrap();
            assert_eq!(age, expected[6], "case{case} ownership age");
            assert_eq!(counts.age, expected[3]);
            assert_eq!(market.dirty, expected[7] != 0);
            assert_eq!(
                neighbors.rows.as_flattened(),
                expected_rows,
                "case{case} rows"
            );
            for i in 0..6 {
                assert_eq!(
                    neighbors.owned_properties[i] as u32,
                    word(expected, 0x8c + i * 16)
                );
                assert_eq!(
                    neighbors.completed_groups[i] as u32,
                    word(expected, 0x94 + i * 16)
                );
                assert_eq!(counts.values[i] as u32, word(expected, 0x90 + i * 16));
            }
            for i in 0..32 {
                assert_eq!(market.prices[i].to_bits(), word(expected, 0xe8 + i * 4));
            }
            assert_eq!(market.rent.to_bits(), word(expected, 0x168));
        }
        assert!(r.0.is_empty());
    }
}

#[cfg(test)]
mod income_tests {
    use super::current_income;
    const OFFSETS: [usize; 17] = [
        0x98, 0x9c, 0x78, 0x7c, 0x80, 0x84, 0x88, 0x8c, 0x90, 0x94, 0xa0, 0x4c, 0x24, 0x28, 0x2c,
        0x30, 0x58,
    ];

    #[test]
    fn current_income_preserves_negative_zero() {
        for bank in 0..=1 {
            let mut player = [0u8; 440];
            player[0x140] = bank;
            for offset in OFFSETS {
                let at = offset - 4 + usize::from(bank) * 152;
                player[at..at + 4].copy_from_slice(&(-0.0f32).to_le_bytes());
            }
            assert_eq!(
                current_income(&player).unwrap().to_f32().to_bits(),
                (-0.0f32).to_bits(),
                "bank {bank}"
            );
            let at = OFFSETS[8] - 4 + usize::from(bank) * 152;
            player[at..at + 4].copy_from_slice(&0.0f32.to_le_bytes());
            assert_eq!(
                current_income(&player).unwrap().to_f32().to_bits(),
                0,
                "mixed zero signs, bank {bank}"
            );
        }
    }

    #[test]
    fn current_income_keeps_ledger_bank_and_addition_order() {
        for bank in 0..=1 {
            let mut player = [0u8; 440];
            player[0x140] = bank;
            // Large cancellation makes reordering observable even at PC64.
            for (offset, value) in OFFSETS.into_iter().zip(
                [2.0f32.powi(100), -2.0f32.powi(100), 1.25]
                    .into_iter()
                    .chain(std::iter::repeat(0.0)),
            ) {
                let at = offset - 4 + usize::from(bank) * 152;
                player[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(
                current_income(&player).unwrap().to_f32().to_bits(),
                1.25f32.to_bits(),
                "bank {bank}"
            );
        }
    }
}
