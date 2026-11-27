//! Fresh live-business allocation corresponding to original `004634c0`.
//!
//! This module owns only the 600-record allocator and its catalog-field
//! initialization.  Bidirectional building links, capacity, and housing
//! availability belong to the later `00463560` stage.  The two direct native
//! dependencies are explicit in [`BusinessPoolHooks`]: callers must provide
//! the owner-transfer and notification behavior instead of silently dropping
//! either call.

use crate::pe::PeImage;

pub const BUSINESS_COUNT: usize = 600;
pub const BUSINESS_RECORD_SIZE: usize = 0x220;
pub const BUSINESS_INFO_COUNT: usize = 45;
pub const BUSINESS_INFO_SIZE: usize = 0x74;
const BUSINESS_INFO_BASE: u32 = 0x0053_a960;

/// Resolve an owner read through a building's business link. For the normal
/// no-business link (-1), v1.4 reads an initialized zero byte before the pool.
/// Other links must still resolve to an actual business.
pub(crate) fn owner(id: i32, businesses: &[[u8; BUSINESS_RECORD_SIZE]]) -> Option<i32> {
    if id == -1 {
        Some(0)
    } else {
        businesses.get(id as usize).map(|b| i32::from(b[10] as i8))
    }
}

/// The static BusinessInfo rows read by the original allocator.
///
/// `004634c0` consumes row +0x0c, +0x5c, +0x5d and +0x70.  Keeping the full
/// rows makes the input provenance explicit and leaves the remaining fields
/// available to the later business initializer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusinessCatalog {
    rows: [[u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT],
}

impl BusinessCatalog {
    /// Read the original 45-row table from an inert PE image.  The PE is read
    /// as data only; this function never loads or executes it.
    pub fn from_pe(image: &PeImage<'_>) -> Result<Self, String> {
        let mut rows = [[0u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT];
        for (index, row) in rows.iter_mut().enumerate() {
            let address = BUSINESS_INFO_BASE
                .checked_add((index * BUSINESS_INFO_SIZE) as u32)
                .ok_or("business catalog address overflow")?;
            row.copy_from_slice(image.at(address, BUSINESS_INFO_SIZE)?);
        }
        Ok(Self { rows })
    }

    /// Construct a catalog from already validated rows, useful for host-side
    /// tests and for a saved/script settings loader that has copied the PE
    /// table once.
    pub fn from_rows(rows: [[u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT]) -> Self {
        Self { rows }
    }

    pub fn row(&self, type_index: usize) -> Option<&[u8; BUSINESS_INFO_SIZE]> {
        self.rows.get(type_index)
    }

    pub fn row_mut(&mut self, type_index: usize) -> Option<&mut [u8; BUSINESS_INFO_SIZE]> {
        self.rows.get_mut(type_index)
    }

    pub fn rows(&self) -> &[[u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT] {
        &self.rows
    }

    /// Reset the two mutable fields touched by `004641d0` for every catalog
    /// row.  Other BusinessInfo bytes are preserved.
    pub fn reset_runtime_fields(&mut self) {
        for row in &mut self.rows {
            row[0x4c] = 0;
            row[0x70..0x74].fill(0);
        }
    }
}

/// The two direct calls made by `004634c0` after its record stores.
///
/// Original order is owner transfer (`00462b00`), then manager notification
/// (`0041f410`).  A callback may inspect or mutate the just-initialized record;
/// errors are returned after preserving the native mutation order already
/// performed.
pub trait BusinessPoolHooks {
    fn owner_transfer(
        &mut self,
        slot: usize,
        record: &mut [u8; BUSINESS_RECORD_SIZE],
        owner_or_mode: u32,
    ) -> Result<(), String>;

    fn notify(&mut self) -> Result<(), String>;
}

/// The part of `004641d0` that is independent of the catalog and availability
/// list.  The list has no shared representation in this module, so its
/// destruction is an explicit caller dependency below.
pub trait FreshResetHooks {
    fn destroy_availability(&mut self) -> Result<(), String>;
}

/// A caller-owned native-sized live business pool.
pub struct BusinessPool<'a> {
    pub records: &'a mut [[u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT],
}

impl<'a> BusinessPool<'a> {
    pub fn new(records: &'a mut [[u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT]) -> Self {
        Self { records }
    }

    /// Apply the complete 600-record loop used by native `004641d0`.
    ///
    /// This is deliberately records-only: native catalog fields and the
    /// separately allocated availability nodes are reset by `reset_fresh`,
    /// where the node destruction remains an explicit hook.
    pub fn reset_records(&mut self) {
        for (index, record) in self.records.iter_mut().enumerate() {
            reset_record(record, index as u32);
        }
    }

    /// Apply the fresh-world order from `004641d0`: records, catalog mutable
    /// fields, then availability-node destruction.  The callback is required
    /// because this module does not own the native 12-byte list nodes.
    pub fn reset_fresh<H: FreshResetHooks>(
        &mut self,
        catalog: &mut BusinessCatalog,
        hooks: &mut H,
    ) -> Result<(), String> {
        self.reset_records();
        catalog.reset_runtime_fields();
        hooks.destroy_availability()
    }

    /// Apply original `004634c0(catalog_type, owner_or_mode, building_index)`.
    ///
    /// `building_index` is a u16 because the native `MOV CX` stores the low
    /// word verbatim.  The AddBusiness caller supplies a nonnegative building
    /// slot; retaining the low-word contract keeps controlled native cases
    /// reproducible as well.
    pub fn allocate<H: BusinessPoolHooks>(
        &mut self,
        catalog_type: u32,
        owner_or_mode: u32,
        building_index: u16,
        catalog: &BusinessCatalog,
        hooks: &mut H,
    ) -> Result<Option<usize>, String> {
        let slot = self.records.iter().position(|record| record[0x21c] == 0);
        let Some(slot) = slot else {
            return Ok(None);
        };

        // Native 004634c0 scans the pool first. A full pool returns -1 before
        // it consults the catalog row, even for an invalid caller type.
        let type_index = usize::try_from(catalog_type)
            .map_err(|_| "business catalog type does not fit usize".to_string())?;
        let info = catalog
            .row(type_index)
            .ok_or_else(|| "business catalog type leaves original 45-row table".to_string())?;

        let record = &mut self.records[slot];
        record[0x21c] = 1;
        record[0x08..0x0a].copy_from_slice(&building_index.to_le_bytes());
        record[0x04..0x08].copy_from_slice(&catalog_type.to_le_bytes());
        record[0x00..0x04].copy_from_slice(&info[0x0c..0x10]);
        record[0x68] = info[0x5c];
        record[0x69] = info[0x5d];
        record[0x70..0x74].copy_from_slice(&info[0x70..0x74]);

        hooks.owner_transfer(slot, record, owner_or_mode)?;
        hooks.notify()?;
        Ok(Some(slot))
    }
}

/// Original460fc0; preserves unspecified bytes when a live slot is released.
pub fn reset_record(record: &mut [u8; BUSINESS_RECORD_SIZE], index: u32) {
    record[0x21c] = 0;
    record[0x04..0x08].copy_from_slice(&u32::MAX.to_le_bytes());
    record[0x00..0x04].copy_from_slice(&u32::MAX.to_le_bytes());
    record[0x10..0x14].fill(0);
    record[0x0e..0x10].fill(0);
    record[0x14..0x18].fill(0);
    record[0x18..0x1a].fill(0);
    record[0x08..0x0a].copy_from_slice(&u16::MAX.to_le_bytes());
    record[0x1dc..0x1e0].fill(0);
    record[0x204..0x208].fill(0);
    record[0x20..0x24].fill(0);
    record[0x24..0x68].fill(0);
    record[0x0b] = 1;
    record[0x0c] = 1;
    record[0x20c..0x210].copy_from_slice(&u32::MAX.to_le_bytes());
    record[0x210..0x214].copy_from_slice(&index.to_le_bytes());
    record[0x0a] = 0xff;
    record[0x74] = 0;
    record[0x209] = 0;
    for index in 0..6 {
        let base = 0x80 + index * 0x34;
        record[base..base + 0x12].fill(0);
        record[base + 0x14..base + 0x24].fill(0);
    }
    for offset in [0x1b8, 0x1ba, 0x1bc, 0x1be, 0x1c0, 0x1c2] {
        record[offset..offset + 2].fill(0);
    }
    record[0x1c4..0x1dc].fill(0);
    record[0x1e0..0x1ec].fill(0);
    record[0x1ec..0x204].fill(0);
    record[0x214..0x218].fill(0);
    record[0x208] = 0;
    record[0x21d] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Hooks {
        owner_calls: Vec<(usize, u32)>,
        notify_calls: usize,
    }

    impl BusinessPoolHooks for Hooks {
        fn owner_transfer(
            &mut self,
            slot: usize,
            record: &mut [u8; BUSINESS_RECORD_SIZE],
            owner_or_mode: u32,
        ) -> Result<(), String> {
            self.owner_calls.push((slot, owner_or_mode));
            record[0x0a] = owner_or_mode as u8;
            Ok(())
        }

        fn notify(&mut self) -> Result<(), String> {
            self.notify_calls += 1;
            Ok(())
        }
    }

    #[test]
    fn allocates_first_free_and_preserves_unrelated_bytes() {
        let mut rows = [[0u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT];
        rows[42][0x0c..0x10].copy_from_slice(&1u32.to_le_bytes());
        rows[42][0x5c] = 7;
        rows[42][0x5d] = 19;
        rows[42][0x70..0x74].copy_from_slice(&0x0102_0304u32.to_le_bytes());
        let catalog = BusinessCatalog::from_rows(rows);
        let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
        records[0].fill(0xa5);
        records[0][0x21c] = 1;
        records[1].fill(0x5a);
        records[1][0x21c] = 0;
        let preserved = records[1];
        let mut pool = BusinessPool::new(&mut records);
        let mut hooks = Hooks {
            owner_calls: Vec::new(),
            notify_calls: 0,
        };
        assert_eq!(
            pool.allocate(42, 0xff, 17, &catalog, &mut hooks).unwrap(),
            Some(1)
        );
        assert_eq!(pool.records[1][0x00..0x04], 1u32.to_le_bytes());
        assert_eq!(pool.records[1][0x04..0x08], 42u32.to_le_bytes());
        assert_eq!(pool.records[1][0x08..0x0a], 17u16.to_le_bytes());
        assert_eq!(pool.records[1][0x0a], 0xff);
        assert_eq!(pool.records[1][0x68], 7);
        assert_eq!(pool.records[1][0x69], 19);
        assert_eq!(pool.records[1][0x70..0x74], 0x0102_0304u32.to_le_bytes());
        assert_eq!(pool.records[1][0x21c], 1);
        for (offset, (&actual, &expected)) in
            pool.records[1].iter().zip(preserved.iter()).enumerate()
        {
            if (0..=0x0a).contains(&offset)
                || (0x68..=0x69).contains(&offset)
                || (0x70..=0x73).contains(&offset)
                || offset == 0x21c
            {
                continue;
            }
            assert_eq!(actual, expected, "offset {offset:#x}");
        }
        assert_eq!(hooks.owner_calls, vec![(1, 0xff)]);
        assert_eq!(hooks.notify_calls, 1);
    }

    #[test]
    fn full_pool_does_not_call_hooks() {
        let catalog = BusinessCatalog::from_rows([[0u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT]);
        let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
        for record in &mut records {
            record[0x21c] = 1;
        }
        let mut pool = BusinessPool::new(&mut records);
        let mut hooks = Hooks {
            owner_calls: Vec::new(),
            notify_calls: 0,
        };
        assert_eq!(pool.allocate(0, 1, 2, &catalog, &mut hooks).unwrap(), None);
        assert!(hooks.owner_calls.is_empty());
        assert_eq!(hooks.notify_calls, 0);
    }

    #[test]
    fn full_pool_does_not_read_invalid_catalog() {
        let catalog = BusinessCatalog::from_rows([[0u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT]);
        let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
        for record in &mut records {
            record[0x21c] = 1;
        }
        let mut pool = BusinessPool::new(&mut records);
        let mut hooks = Hooks {
            owner_calls: Vec::new(),
            notify_calls: 0,
        };
        assert_eq!(
            pool.allocate(u32::MAX, 1, 2, &catalog, &mut hooks).unwrap(),
            None
        );
    }

    struct ResetHooks {
        destroy_calls: usize,
    }

    impl FreshResetHooks for ResetHooks {
        fn destroy_availability(&mut self) -> Result<(), String> {
            self.destroy_calls += 1;
            Ok(())
        }
    }

    #[test]
    fn reset_records_matches_native_field_contract() {
        let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
        for (index, record) in records.iter_mut().enumerate() {
            record.fill(0xa5);
            record[0x21c] = 1;
            reset_record(record, index as u32);
            assert_eq!(record[0x21c], 0);
            assert_eq!(record[0x00..0x08], [0xff; 8]);
            assert_eq!(record[0x08..0x0a], u16::MAX.to_le_bytes());
            assert_eq!(record[0x0b..0x0d], [1, 1]);
            assert_eq!(record[0x210..0x214], (index as u32).to_le_bytes());
            assert_eq!(record[0x20c..0x210], u32::MAX.to_le_bytes());
            assert_eq!(record[0x0a], 0xff);
            assert_eq!(record[0x21d], 0);
        }
    }

    #[test]
    fn fresh_reset_clears_catalog_fields_then_destroys_availability() {
        let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
        let mut rows = [[0u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT];
        rows[7][0x4c] = 0xa5;
        rows[7][0x70..0x74].copy_from_slice(&0x1020_3040u32.to_le_bytes());
        rows[7][0x4b] = 0x5a;
        let mut catalog = BusinessCatalog::from_rows(rows);
        let mut hooks = ResetHooks { destroy_calls: 0 };
        BusinessPool::new(&mut records)
            .reset_fresh(&mut catalog, &mut hooks)
            .unwrap();
        assert_eq!(catalog.rows()[7][0x4c], 0);
        assert_eq!(catalog.rows()[7][0x70..0x74], [0; 4]);
        assert_eq!(catalog.rows()[7][0x4b], 0x5a);
        assert_eq!(hooks.destroy_calls, 1);
    }

    #[test]
    #[ignore = "requires original PE and native reset fixture"]
    fn matches_native_business_pool_reset() {
        let fixture = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_RESET")).unwrap();
        assert_eq!(&fixture[..8], b"MTBRST01");
        let u32_at = |at: &mut usize| {
            let value = u32::from_le_bytes(fixture[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        let mut at = 8usize;
        assert_eq!(u32_at(&mut at), 1);
        let cases = u32_at(&mut at) as usize;
        assert_eq!(u32_at(&mut at) as usize, BUSINESS_COUNT);
        assert_eq!(u32_at(&mut at) as usize, BUSINESS_RECORD_SIZE);
        for _ in 0..cases {
            let name_len = u32_at(&mut at) as usize;
            at += name_len;
            let freed_count = u32_at(&mut at) as usize;
            at += freed_count * 4;

            let record_before_len = u32_at(&mut at) as usize;
            assert_eq!(record_before_len, BUSINESS_COUNT * BUSINESS_RECORD_SIZE);
            let record_before = &fixture[at..at + record_before_len];
            at += record_before_len;
            let record_after_len = u32_at(&mut at) as usize;
            assert_eq!(record_after_len, record_before_len);
            let record_after = &fixture[at..at + record_after_len];
            at += record_after_len;

            let catalog_before_len = u32_at(&mut at) as usize;
            assert_eq!(catalog_before_len, BUSINESS_INFO_COUNT * BUSINESS_INFO_SIZE);
            let catalog_before = &fixture[at..at + catalog_before_len];
            at += catalog_before_len;
            let catalog_after_len = u32_at(&mut at) as usize;
            assert_eq!(catalog_after_len, catalog_before_len);
            let catalog_after = &fixture[at..at + catalog_after_len];
            at += catalog_after_len;

            let list_before_len = u32_at(&mut at) as usize;
            assert_eq!(list_before_len, 16);
            at += list_before_len;
            let list_after_len = u32_at(&mut at) as usize;
            assert_eq!(list_after_len, 16);
            assert_eq!(&fixture[at..at + list_after_len], &[0; 16]);
            at += list_after_len;

            let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
            for (slot, record) in records.iter_mut().enumerate() {
                let start = slot * BUSINESS_RECORD_SIZE;
                record.copy_from_slice(&record_before[start..start + BUSINESS_RECORD_SIZE]);
            }
            let mut rows = [[0u8; BUSINESS_INFO_SIZE]; BUSINESS_INFO_COUNT];
            for (row, dst) in rows.iter_mut().enumerate() {
                let start = row * BUSINESS_INFO_SIZE;
                dst.copy_from_slice(&catalog_before[start..start + BUSINESS_INFO_SIZE]);
            }
            let mut catalog = BusinessCatalog::from_rows(rows);
            let mut hooks = ResetHooks { destroy_calls: 0 };
            BusinessPool::new(&mut records)
                .reset_fresh(&mut catalog, &mut hooks)
                .unwrap();
            assert_eq!(records.as_flattened(), record_after);
            assert_eq!(catalog.rows().as_flattened(), catalog_after);
            assert_eq!(hooks.destroy_calls, 1);
        }
        assert_eq!(at, fixture.len());
    }

    #[test]
    #[ignore = "requires original PE and native allocator fixture"]
    fn matches_native_business_allocator() {
        let image_bytes = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&image_bytes).unwrap();
        let catalog = BusinessCatalog::from_pe(&image).unwrap();
        let fixture = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_ALLOCATOR")).unwrap();
        assert_eq!(&fixture[..8], b"MTBIZ01\0");
        let u32_at = |at: &mut usize| {
            let value = u32::from_le_bytes(fixture[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        let version = u32_at(&mut 8usize);
        assert_eq!(version, 1);
        let cases = u32_at(&mut 12usize) as usize;
        assert_eq!(u32_at(&mut 16usize) as usize, BUSINESS_COUNT);
        assert_eq!(u32_at(&mut 20usize) as usize, BUSINESS_RECORD_SIZE);
        let mut at = 24usize;
        for _ in 0..cases {
            let name_len = u32_at(&mut at) as usize;
            at += name_len;
            let calls = u32_at(&mut at) as usize;
            let mut inputs = Vec::with_capacity(calls);
            for _ in 0..calls {
                let catalog_type = u32_at(&mut at);
                let owner = u32_at(&mut at);
                let building = u32_at(&mut at) as u16;
                let returned = i32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
                at += 4;
                let _reserved = i32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
                at += 4;
                inputs.push((catalog_type, owner, building, returned));
            }
            let before_len = u32_at(&mut at) as usize;
            assert_eq!(before_len, BUSINESS_COUNT * BUSINESS_RECORD_SIZE);
            let before = &fixture[at..at + before_len];
            at += before_len;
            let after_len = u32_at(&mut at) as usize;
            assert_eq!(after_len, before_len);
            let expected = &fixture[at..at + after_len];
            at += after_len;

            let mut records = [[0u8; BUSINESS_RECORD_SIZE]; BUSINESS_COUNT];
            for (slot, record) in records.iter_mut().enumerate() {
                let lo = slot * BUSINESS_RECORD_SIZE;
                record.copy_from_slice(&before[lo..lo + BUSINESS_RECORD_SIZE]);
            }
            let mut hooks = Hooks {
                owner_calls: Vec::new(),
                notify_calls: 0,
            };
            let mut pool = BusinessPool::new(&mut records);
            for (catalog_type, owner, building, returned) in inputs {
                let actual = pool
                    .allocate(catalog_type, owner, building, &catalog, &mut hooks)
                    .unwrap()
                    .map_or(-1, |slot| slot as i32);
                assert_eq!(actual, returned);
            }
            let actual = records.as_flattened();
            assert_eq!(actual, expected);
        }
        assert_eq!(at, fixture.len());
    }
}
