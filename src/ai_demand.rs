//! Original471760/4717c0 commodity-demand cache and471a20 business changes.
//! Keep complete264-byte records: CRT sorting also moves names and opaque tails.
use crate::{
    pe::PeImage,
    player_manager::{put, word},
};
pub struct State {
    pub rows: [[u8; 264]; 33],
    /// Original801dec, cleared by AI reset and enabled only after rebuild.
    pub ready: u8,
}
impl Default for State {
    fn default() -> Self {
        Self {
            rows: [[0; 264]; 33],
            ready: 0,
        }
    }
}
impl State {
    ///471c30: only bit0 is consumed; mode0 adds one homeless resident,
    /// mode1 subtracts one. Inactive source slots never change the cache.
    pub fn resident_changed(
        &mut self,
        resident: &crate::resident::Resident,
        flags: u32,
        mode: u32,
    ) -> Result<(), String> {
        if !resident.is_active() || self.ready == 0 {
            return Ok(());
        }
        if flags & 1 != 0 {
            self.adjust(32, 1, mode)?;
        }
        self.sort();
        Ok(())
    }
    ///471760 only resets identities/counts and copies terminated debug names.
    pub fn initialize(&mut self, image: &PeImage<'_>) -> Result<(), String> {
        for (id, row) in self.rows.iter_mut().enumerate() {
            put(row, 0, id as u32);
            put(row, 4, 0);
            let target = word(image.at(0x4706e0 + id as u32 * 4, 4)?, 0);
            let stub = image.at(target, 6)?;
            if stub[0] != 0xb8 || stub[5] != 0xc3 {
                return Err("unsupported original commodity-name table".into());
            }
            let name = image.at(word(stub, 1), 256)?;
            let len = name
                .iter()
                .position(|&b| b == 0)
                .ok_or("AI demand name unterminated")?
                + 1;
            row[8..8 + len].copy_from_slice(&name[..len]);
        }
        Ok(())
    }
    fn sort(&mut self) {
        crate::native_sort::sort(&mut self.rows, &|a, b| match (word(&a, 4) as i32)
            .cmp(&(word(&b, 4) as i32))
        {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -1,
        });
    }
    fn adjust(&mut self, commodity: u32, capacity: i32, mode: u32) -> Result<(), String> {
        let row = self
            .rows
            .iter_mut()
            .find(|r| word(*r, 0) == commodity)
            .ok_or("AI demand commodity missing; original indexes before cache")?;
        let old = word(row, 4) as i32;
        let next = match mode {
            1 => old.wrapping_sub(capacity),
            0 => old.wrapping_add(capacity),
            _ => old,
        };
        put(row, 4, next.max(0) as u32);
        Ok(())
    }
    ///471a20 uses business discriminator (0 retail,1 residential), not kind42.
    pub fn business_changed(
        &mut self,
        business: usize,
        mode: u32,
        businesses: &[[u8; 544]],
        buildings: &[[u8; 916]],
        catalog: &crate::business_pool::BusinessCatalog,
        products: &[[[u32; 2]; 6]; 45],
    ) -> Result<(), String> {
        let row = businesses
            .get(business)
            .ok_or("demand business leaves pool")?;
        if row[0x21c] == 0 {
            return Ok(());
        }
        match word(row, 0) {
            0 => {
                let kind = word(row, 4) as usize;
                let info = catalog
                    .row(kind)
                    .ok_or("demand business kind leaves catalog")?;
                let count = usize::from(info[0x4c]);
                if count > 6 {
                    return Err("AI demand product count exceeds catalog".into());
                }
                if count != 0 {
                    let capacity = capacity(row, buildings, info)?;
                    for entry in &products[kind][..count] {
                        self.adjust(entry[0], capacity, mode)?;
                    }
                }
            }
            1 => self.adjust(
                32,
                i32::from(u16::from_le_bytes(row[14..16].try_into().unwrap())),
                mode,
            )?,
            _ => (),
        }
        self.sort();
        Ok(())
    }
    ///4717c0 rebuilds demand from every active resident then subtracts all
    /// active business capacities. Rebuild tests kind42, unlike471a20.
    #[allow(clippy::too_many_arguments)]
    pub fn rebuild(
        &mut self,
        residents: &crate::economy::ResidentAccounts,
        businesses: &[[u8; 544]],
        buildings: &[[u8; 916]],
        catalog: &crate::business_pool::BusinessCatalog,
        products: &[[[u32; 2]; 6]; 45],
        commodities: &[[u8; 28]; 32],
        image: &PeImage<'_>,
    ) -> Result<(), String> {
        self.initialize(image)?;
        for resident in &residents.nodes {
            if !resident.is_active() {
                continue;
            }
            let account = residents
                .accounts
                .get(resident.account_index() as usize)
                .ok_or("demand resident account unresolved")?
                .as_bytes();
            for (i, commodity) in commodities.iter().enumerate() {
                let amount = f32::from_bits(word(account, 4 + i * 4));
                let price = f32::from_bits(word(commodity, 0x14));
                let value = if amount.is_finite() && price.is_finite() && price != 0.0 {
                    let mut value = crate::x87::Sum::from_f32(amount)?;
                    value.div_f32(price)?;
                    value.ftol_low_word()
                } else {
                    crate::economy::ftol_low_word(f64::from(amount) / f64::from(price))
                }
                .max(0);
                let old = word(&self.rows[i], 4);
                put(&mut self.rows[i], 4, old.wrapping_add(value as u32));
            }
        }
        let count = residents
            .nodes
            .get(..residents.active_count as usize)
            .ok_or("demand resident prefix leaves pool")?
            .iter()
            .filter(|r| {
                word(r.as_bytes(), 0) != 4
                    && i16::from_le_bytes(r.as_bytes()[14..16].try_into().unwrap()) == -1
            })
            .count();
        put(&mut self.rows[32], 4, count as u32);
        for row in businesses {
            if row[0x21c] == 0 {
                continue;
            }
            let kind = word(row, 4) as usize;
            if kind == 42 {
                self.adjust(
                    32,
                    i32::from(u16::from_le_bytes(row[14..16].try_into().unwrap())),
                    1,
                )?;
                continue;
            }
            let info = catalog
                .row(kind)
                .ok_or("demand rebuild business kind leaves catalog")?;
            let count = usize::from(info[0x4c]);
            if count > 6 {
                return Err("demand rebuild product count exceeds catalog".into());
            }
            if count == 0 {
                continue;
            }
            let capacity = capacity(row, buildings, info)?;
            for entry in &products[kind][..count] {
                self.adjust(entry[0], capacity, 1)?;
            }
        }
        self.sort();
        self.ready = 1;
        Ok(())
    }
}
pub(crate) fn capacity(
    row: &[u8; 544],
    buildings: &[[u8; 916]],
    info: &[u8; 116],
) -> Result<i32, String> {
    let building = buildings
        .get(i16::from_le_bytes(row[8..10].try_into().unwrap()) as usize)
        .ok_or("demand building leaves pool")?;
    let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
        .wrapping_mul(i32::from(building[8]));
    let factor = f32::from_bits(word(info, 0x58));
    if !factor.is_finite() {
        return Ok(0);
    }
    let mut value = crate::x87::Sum::from_i32(volume);
    value.mul_f32(factor)?;
    Ok(value.ftol_low_word())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original471c30 housing demand observations"]
    fn matches_native_housing_changes() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_HOUSING_DEMAND").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTHDMD01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let index = r.word() as usize;
            r.word();
            r.word();
            let flags = r.word();
            let mode = r.word();
            r.word();
            r.word();
            let ready = r.word() as u8;
            let after_ready = r.word();
            assert_eq!(r.word(), 0x037f);
            let residents = r.take(8 * 336);
            assert_eq!(residents, r.take(8 * 336), "{name} residents preserved");
            let before = r.take(33 * 264);
            let after = r.take(33 * 264);
            let resident = &residents[index * 336..(index + 1) * 336];
            let resident = crate::resident::Resident::from_pool_bytes(
                resident[..0x14a].try_into().unwrap(),
                resident[0x14a],
            );
            let mut state = State {
                ready,
                rows: std::array::from_fn(|i| before[i * 264..(i + 1) * 264].try_into().unwrap()),
            };
            state.resident_changed(&resident, flags, mode).unwrap();
            assert_eq!(
                state.rows.as_flattened(),
                after,
                "{name} sorted complete cache"
            );
            assert_eq!(u32::from(state.ready), after_ready, "{name} ready");
        }
        assert!(r.0.is_empty());
    }
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
    #[test]
    #[ignore = "requires original471a20/471760/4717c0 demand observations"]
    fn matches_native_demand() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_DEMAND").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTDMD03\0");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let op = r.word();
            let index = r.word();
            let direction = r.word();
            let mut phases = Vec::new();
            for _ in 0..2 {
                let regions: [&[u8]; 8] = [
                    8 * 544,
                    8 * 916,
                    45 * 116,
                    45 * 6 * 8,
                    33 * 264,
                    8 * 336,
                    8 * 144,
                    32 * 28,
                ]
                .map(|n| r.take(n));
                let count = r.word();
                let gate = r.word();
                phases.push((regions, count, gate));
            }
            r.word();
            let (before, count, gate) = phases[0];
            let (after, after_count, after_gate) = phases[1];
            let businesses: Vec<[u8; 544]> = before[0]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            let buildings: Vec<[u8; 916]> = before[1]
                .chunks_exact(916)
                .map(|r| r.try_into().unwrap())
                .collect();
            let catalog =
                crate::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|i| {
                    before[2][i * 116..(i + 1) * 116].try_into().unwrap()
                }));
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [
                        word(before[3], i * 48 + j * 8),
                        word(before[3], i * 48 + j * 8 + 4),
                    ]
                })
            });
            let mut state = State {
                ready: gate as u8,
                rows: std::array::from_fn(|i| {
                    before[4][i * 264..(i + 1) * 264].try_into().unwrap()
                }),
            };
            let residents = crate::economy::ResidentAccounts {
                active_count: count,
                nodes: before[5]
                    .chunks_exact(336)
                    .map(|r| {
                        crate::resident::Resident::from_pool_bytes(
                            r[..0x14a].try_into().unwrap(),
                            r[0x14a],
                        )
                    })
                    .collect(),
                accounts: before[6]
                    .chunks_exact(144)
                    .map(|r| crate::resident::SpendingAccount::from_bytes(r.try_into().unwrap()))
                    .collect(),
            };
            let commodities =
                std::array::from_fn(|i| before[7][i * 28..(i + 1) * 28].try_into().unwrap());

            match op {
                0 => state
                    .business_changed(
                        index as usize,
                        direction,
                        &businesses,
                        &buildings,
                        &catalog,
                        &products,
                    )
                    .unwrap(),
                1 => state.initialize(&image).unwrap(),
                2 => state
                    .rebuild(
                        &residents,
                        &businesses,
                        &buildings,
                        &catalog,
                        &products,
                        &commodities,
                        &image,
                    )
                    .unwrap(),
                _ => panic!("unknown demand operation"),
            }
            for i in [0, 1, 2, 3, 5, 6, 7] {
                assert_eq!(
                    before[i], after[i],
                    "{name} native input region{i} unchanged"
                );
            }
            assert_eq!(count, after_count, "{name} count");
            assert_eq!(u32::from(state.ready), after_gate, "{name} ready");
            assert_eq!(
                state.rows.as_flattened(),
                after[4],
                "{name} complete demand cache"
            );
        }
        assert!(r.0.is_empty());
    }

    #[test]
    #[ignore = "requires original v1.4 and supplied Sandbox save demand rebuild"]
    fn matches_native_saved_population_rebuild() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SAVED_DEMAND").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let save = std::fs::read(std::env::var("OPENMT2001_SAVE_TWO").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTSDMD01");
        assert_eq!(r.word(), 0x037f);
        let resident_count = r.word() as usize;
        let account_count = r.word() as usize;
        assert_eq!((resident_count, account_count), (500, 2000));
        let resident_bytes = r.take(resident_count * 336);
        let account_bytes = r.take(account_count * 144);
        let commodity_bytes = r.take(32 * 28);
        let business_bytes = r.take(600 * 544);
        let before = r.take(33 * 264);
        let expected = r.take(33 * 264);
        assert!(r.0.is_empty());

        // Verify that the large oracle actually uses the supplied save's
        // prefixes and account/commodity bytes, rather than generated rows.
        let mut at = 0;
        let mut seen_residents = 0;
        let mut seen_accounts = false;
        let mut seen_commodities = false;
        let mut seen_businesses = false;
        loop {
            let tag = word(&save, at);
            let size = word(&save, at + 4) as usize;
            at += 8;
            if tag == u32::MAX {
                break;
            }
            let payload = &save[at..at + size];
            at += size;
            match tag {
                0x10 => {
                    assert_eq!(size, 330);
                    assert_eq!(
                        payload,
                        &resident_bytes[seen_residents * 336..seen_residents * 336 + 330]
                    );
                    assert_eq!(resident_bytes[seen_residents * 336 + 330], 1);
                    seen_residents += 1;
                }
                0x1000 => {
                    assert_eq!(payload, account_bytes);
                    seen_accounts = true;
                }
                0x200000 => {
                    assert_eq!(payload, commodity_bytes);
                    seen_commodities = true;
                }
                0x20 => {
                    assert_eq!(payload, business_bytes);
                    seen_businesses = true;
                }
                _ => (),
            }
        }
        assert_eq!(at, save.len());
        assert_eq!(seen_residents, resident_count);
        assert!(seen_accounts && seen_commodities && seen_businesses);

        let residents = crate::economy::ResidentAccounts {
            active_count: resident_count as u32,
            nodes: resident_bytes
                .chunks_exact(336)
                .map(|raw| {
                    crate::resident::Resident::from_pool_bytes(
                        raw[..330].try_into().unwrap(),
                        raw[330],
                    )
                })
                .collect(),
            accounts: account_bytes
                .chunks_exact(144)
                .map(|raw| crate::resident::SpendingAccount::from_bytes(raw.try_into().unwrap()))
                .collect(),
        };
        let commodities =
            std::array::from_fn(|i| commodity_bytes[i * 28..(i + 1) * 28].try_into().unwrap());
        let catalog = crate::business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
        let businesses: Vec<[u8; 544]> = business_bytes
            .chunks_exact(544)
            .map(|raw| raw.try_into().unwrap())
            .collect();
        assert!(businesses.iter().all(|row| row[0x21c] == 0));
        let mut state = State {
            rows: std::array::from_fn(|i| before[i * 264..(i + 1) * 264].try_into().unwrap()),
            ready: 0,
        };
        state
            .rebuild(
                &residents,
                &businesses,
                &[],
                &catalog,
                &[[[0; 2]; 6]; 45],
                &commodities,
                &image,
            )
            .unwrap();
        assert_eq!(state.ready, 1);
        assert_eq!(
            state.rows.as_flattened(),
            expected,
            "full 500-resident original demand cache"
        );
    }

    #[test]
    #[ignore = "requires original v1.4 and supplied Scenario 3 active-business demand rebuild"]
    fn matches_native_saved_active_business_rebuild() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SAVED_ACTIVE_DEMAND").unwrap())
                .unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let save = std::fs::read(std::env::var("OPENMT2001_SAVE_ONE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTADMD01");
        assert_eq!(r.word(), 0x037f);
        let resident_count = r.word() as usize;
        let account_count = r.word() as usize;
        assert_eq!((resident_count, account_count), (340, 2000));
        let resident_bytes = r.take(resident_count * 336);
        let account_bytes = r.take(account_count * 144);
        let business_bytes = r.take(600 * 544);
        let building_bytes = r.take(600 * 916);
        let catalog_bytes = r.take(45 * 116);
        let product_bytes = r.take(45 * 6 * 8);
        let commodity_bytes = r.take(32 * 28);
        let before = r.take(33 * 264);
        let expected = r.take(33 * 264);
        assert!(r.0.is_empty());

        let mut saved_buildings = None;
        let mut saved_accounts = None;
        let mut saved_businesses = None;
        let mut saved_catalog = None;
        let mut saved_commodities = None;
        let mut seen_residents = 0;
        let mut at = 0;
        loop {
            let tag = word(&save, at);
            let size = word(&save, at + 4) as usize;
            at += 8;
            if tag == u32::MAX {
                break;
            }
            let payload = &save[at..at + size];
            at += size;
            match tag {
                0x10 => {
                    assert_eq!(
                        payload,
                        &resident_bytes[seen_residents * 336..seen_residents * 336 + 330]
                    );
                    assert_eq!(resident_bytes[seen_residents * 336 + 330], 1);
                    seen_residents += 1;
                }
                4 => saved_buildings = Some(payload),
                0x20 => saved_businesses = Some(payload),
                0x1000 => saved_accounts = Some(payload),
                0x40000 => saved_catalog = Some(payload),
                0x200000 => saved_commodities = Some(payload),
                _ => (),
            }
        }
        assert_eq!(at, save.len());
        assert_eq!(seen_residents, resident_count);
        assert_eq!(saved_accounts.unwrap(), account_bytes);
        assert_eq!(saved_businesses.unwrap(), business_bytes);
        assert_eq!(saved_catalog.unwrap(), catalog_bytes);
        assert_eq!(saved_commodities.unwrap(), commodity_bytes);
        let prefixes = saved_buildings.unwrap();
        assert_eq!(prefixes.len(), 0xb270);
        for (i, runtime) in building_bytes.chunks_exact(916).enumerate() {
            assert_eq!(&runtime[..64], &prefixes[i * 64..(i + 1) * 64]);
            assert!(runtime[64..].iter().all(|&byte| byte == 0));
        }

        let businesses: Vec<[u8; 544]> = business_bytes
            .chunks_exact(544)
            .map(|raw| raw.try_into().unwrap())
            .collect();
        assert_eq!(businesses.iter().filter(|row| row[0x21c] != 0).count(), 15);
        for row in businesses.iter().filter(|row| row[0x21c] != 0) {
            let kind = word(row, 4) as usize;
            if kind == 42 {
                continue;
            }
            assert_eq!(catalog_bytes[kind * 116 + 0x4c], 1);
            let mask = word(catalog_bytes, kind * 116 + 0x70);
            assert_eq!(mask.count_ones(), 1);
            assert_eq!(word(product_bytes, kind * 48), mask.trailing_zeros());
        }
        let residents = crate::economy::ResidentAccounts {
            active_count: resident_count as u32,
            nodes: resident_bytes
                .chunks_exact(336)
                .map(|raw| {
                    crate::resident::Resident::from_pool_bytes(
                        raw[..330].try_into().unwrap(),
                        raw[330],
                    )
                })
                .collect(),
            accounts: account_bytes
                .chunks_exact(144)
                .map(|raw| crate::resident::SpendingAccount::from_bytes(raw.try_into().unwrap()))
                .collect(),
        };
        let buildings: Vec<[u8; 916]> = building_bytes
            .chunks_exact(916)
            .map(|raw| raw.try_into().unwrap())
            .collect();
        let catalog = crate::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|i| {
            catalog_bytes[i * 116..(i + 1) * 116].try_into().unwrap()
        }));
        let products = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                [
                    word(product_bytes, i * 48 + j * 8),
                    word(product_bytes, i * 48 + j * 8 + 4),
                ]
            })
        });
        let commodities =
            std::array::from_fn(|i| commodity_bytes[i * 28..(i + 1) * 28].try_into().unwrap());
        let mut state = State {
            rows: std::array::from_fn(|i| before[i * 264..(i + 1) * 264].try_into().unwrap()),
            ready: 0,
        };
        state
            .rebuild(
                &residents,
                &businesses,
                &buildings,
                &catalog,
                &products,
                &commodities,
                &image,
            )
            .unwrap();
        assert_eq!(state.ready, 1);
        assert_eq!(
            state.rows.as_flattened(),
            expected,
            "full supplied Scenario 3 demand cache after capacity subtraction"
        );
    }
}
