//! Original46a8d0 liquidation target deletion and462f30 category lookup.
use crate::player_manager::{Player, put, word};
use std::collections::BTreeMap;
pub fn remove(
    player: &mut Player,
    reference: u32,
    records: &mut BTreeMap<u32, [u8; 12]>,
) -> Result<(), String> {
    let count = word(&player.bytes, 0x13ec) as i32;
    if count <= 0 {
        return Ok(());
    }
    let base = word(&player.bytes, 0x13e8);
    let target = *records
        .get(&reference)
        .ok_or("AI liquidation target missing")?;
    let mut rows: Vec<_> = (0..count as u32)
        .map(|i| {
            records
                .get(&base.wrapping_add(i.wrapping_mul(12)))
                .copied()
                .ok_or("AI liquidation row missing")
        })
        .collect::<Result<_, _>>()?;
    if let Some(index) = rows
        .iter()
        .position(|r| word(r, 4) == word(&target, 4) && word(r, 8) == word(&target, 8))
    {
        put(&mut player.bytes, 0x13ec, (count - 1) as u32);
        put(&mut rows[index], 4, 0);
        put(&mut rows[index], 0, 0);
        put(&mut rows[index], 8, u32::MAX);
        // The native routine sorts the decremented prefix, without copying the last
        // record into the vacated slot. Preserve that observable order and tail.
        crate::native_sort::sort(
            &mut rows[..count as usize - 1],
            &|a, b| match (word(&a, 0) as i32).cmp(&(word(&b, 0) as i32)) {
                std::cmp::Ordering::Less => 1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => -1,
            },
        );
        for (i, row) in rows.into_iter().enumerate() {
            records.insert(base.wrapping_add((i as u32).wrapping_mul(12)), row);
        }
    }
    Ok(())
}
pub fn category(
    business: u32,
    businesses: &[[u8; 544]],
    buildings: &[[u8; 916]],
    categories: &[[u8; 380]; 52],
) -> Result<i16, String> {
    let row = businesses
        .get(business as usize)
        .ok_or("AI sale business missing")?;
    let building = i16::from_le_bytes(row[8..10].try_into().unwrap());
    if building == -1 {
        return Ok(-1);
    }
    let row = buildings
        .get(building as usize)
        .ok_or("AI sale building missing")?;
    let offset = word(row, 0x40)
        .checked_sub(0x617628)
        .ok_or("AI sale category precedes pool")?;
    if offset % 380 != 0 {
        return Err("AI sale category reference unaligned".into());
    }
    let row = categories
        .get(offset as usize / 380)
        .ok_or("AI sale category missing")?;
    Ok(i16::from_le_bytes(row[..2].try_into().unwrap()))
}
#[cfg(test)]
mod tests {
    use super::*;
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
    #[ignore = "requires native liquidation removal observations"]
    fn matches_native_liquidation() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_LIQUIDATION")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAILIQ1");
        assert_eq!(r.word(), 0x37f);
        for i in 0..r.word() {
            let entry = r.word();
            let ptr = r.word();
            let value = r.word();
            let b: Vec<_> = (0..5).map(|_| r.blob()).collect();
            let a: Vec<_> = (0..5).map(|_| r.blob()).collect();
            let mut player = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![],
            };
            let mut rows: BTreeMap<_, [u8; 12]> = b[1]
                .chunks_exact(12)
                .enumerate()
                .map(|(i, b)| (0x10030000 + i as u32 * 12, b.try_into().unwrap()))
                .collect();
            if entry == 0x46a8d0 {
                remove(&mut player, ptr, &mut rows).unwrap();
            } else {
                let business: Vec<[u8; 544]> = b[2]
                    .chunks_exact(544)
                    .map(|b| b.try_into().unwrap())
                    .collect();
                let buildings: Vec<[u8; 916]> = b[3]
                    .chunks_exact(916)
                    .map(|b| b.try_into().unwrap())
                    .collect();
                let cats = b[4]
                    .chunks_exact(380)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
                assert_eq!(
                    category(0, &business, &buildings, &cats).unwrap() as u16 as u32,
                    value,
                    "case{i} category"
                );
            }
            assert_eq!(player.bytes, a[0], "case{i} player");
            assert_eq!(
                rows.values()
                    .flat_map(|r| r.iter().copied())
                    .collect::<Vec<_>>(),
                a[1],
                "case{i} rows"
            );
            for j in 2..5 {
                assert_eq!(a[j], b[j]);
            }
        }
        assert!(r.0.is_empty());
    }
}
