//! Original46a6f0/46a890 retained liquidation allocation lifecycle.
use crate::player_manager::{put, word, Player};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Allocate(usize),
    Release(u32),
}
fn rows(
    player: &Player,
    records: &BTreeMap<u32, [u8; 12]>,
    count: usize,
) -> Result<Vec<[u8; 12]>, String> {
    let base = word(&player.bytes, 0x13e8);
    (0..count)
        .map(|i| {
            records
                .get(&base.wrapping_add(i as u32 * 12))
                .copied()
                .ok_or("AI liquidation allocation truncated".into())
        })
        .collect()
}
pub fn sort(player: &Player, records: &mut BTreeMap<u32, [u8; 12]>) -> Result<(), String> {
    let count = word(&player.bytes, 0x13ec) as i32;
    if count <= 0 {
        return Ok(());
    }
    let mut rows = rows(player, records, count as usize)?;
    crate::native_sort::sort(&mut rows, &|a, b| match (word(&a, 0) as i32)
        .cmp(&(word(&b, 0) as i32))
    {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => -1,
    });
    let base = word(&player.bytes, 0x13e8);
    for (i, row) in rows.into_iter().enumerate() {
        records.insert(base.wrapping_add(i as u32 * 12), row);
    }
    Ok(())
}
///Input is the original caller's stack descriptor, not an alias into the list.
pub fn append(
    player: &mut Player,
    proposal: Option<[u8; 12]>,
    records: &mut BTreeMap<u32, [u8; 12]>,
    mut call: impl FnMut(Call, &Player, &BTreeMap<u32, [u8; 12]>) -> Result<u32, String>,
) -> Result<(), String> {
    let Some(proposal) = proposal else {
        return Ok(());
    };
    sort(player, records)?;
    let count = word(&player.bytes, 0x13ec) as i32;
    let capacity = word(&player.bytes, 0x13f0) as i32;
    let old = word(&player.bytes, 0x13e8);
    if count < 0 || capacity < 0 {
        return Err("negative native liquidation allocation size".into());
    }
    if count == 0 {
        let base = if old == 0 {
            let base = call(Call::Allocate(12), player, records)?;
            if base == 0 {
                return Err("native liquidation allocator returned null".into());
            }
            put(&mut player.bytes, 0x13e8, base);
            base
        } else {
            old
        };
        records.insert(base, proposal);
    } else if count < capacity {
        records.insert(old.wrapping_add(count as u32 * 12), proposal);
    } else {
        let new = call(Call::Allocate((count as usize + 1) * 12), player, records)?;
        if new == 0 {
            return Err("native liquidation allocator returned null".into());
        }
        let previous = rows(player, records, count as usize)?;
        for (i, row) in previous.into_iter().enumerate() {
            records.insert(new.wrapping_add(i as u32 * 12), row);
        }
        records.insert(new.wrapping_add(count as u32 * 12), proposal);
        call(Call::Release(old), player, records)?;
        for i in 0..capacity {
            records.remove(&old.wrapping_add(i as u32 * 12));
        }
        put(&mut player.bytes, 0x13e8, new);
    }
    let count = count.wrapping_add(1);
    put(&mut player.bytes, 0x13ec, count as u32);
    if count > capacity {
        put(&mut player.bytes, 0x13f0, count as u32);
    }
    Ok(())
}
pub fn clear(
    player: &mut Player,
    records: &mut BTreeMap<u32, [u8; 12]>,
    mut call: impl FnMut(Call, &Player, &BTreeMap<u32, [u8; 12]>) -> Result<u32, String>,
) -> Result<(), String> {
    let base = word(&player.bytes, 0x13e8);
    if base != 0 {
        call(Call::Release(base), player, records)?;
        for i in 0..word(&player.bytes, 0x13f0) {
            records.remove(&base.wrapping_add(i.wrapping_mul(12)));
        }
        put(&mut player.bytes, 0x13e8, 0);
    }
    put(&mut player.bytes, 0x13e8, 0);
    put(&mut player.bytes, 0x13ec, 0);
    put(&mut player.bytes, 0x13f0, 0);
    Ok(())
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
        fn snapshot(&mut self) -> (Vec<u8>, BTreeMap<u32, [u8; 12]>) {
            let p = self.blob().to_vec();
            let mut rows = BTreeMap::new();
            for _ in 0..self.word() {
                let a = self.word();
                for (i, b) in self.blob().chunks_exact(12).enumerate() {
                    rows.insert(a + i as u32 * 12, b.try_into().unwrap());
                }
            }
            (p, rows)
        }
    }
    #[test]
    #[ignore = "requires original allocation lifecycle observations"]
    fn matches_native_liquidation_pool() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_LIQUIDATION_POOL").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAILPL1");
        assert_eq!(r.word(), 0x37f);
        for i in 0..r.word() {
            let entry = r.word();
            let ptr = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let mut records: BTreeMap<u32, [u8; 12]> = r
                .blob()
                .chunks_exact(12)
                .enumerate()
                .map(|(i, b)| (0x10030000 + i as u32 * 12, b.try_into().unwrap()))
                .collect();
            let proposal = r.take(12).try_into().unwrap();
            let n = r.word();
            let mut traces = Vec::new();
            for _ in 0..n {
                let a = r.word();
                let arg = r.word();
                traces.push((a, arg, r.snapshot()));
            }
            let after = r.snapshot();
            let mut calls = 0;
            let mut callback = |c: Call, p: &Player, rows: &BTreeMap<u32, [u8; 12]>| {
                let (a, arg, (expected_p, expected_rows)) = &traces[calls];
                calls += 1;
                assert_eq!(
                    (
                        match c {
                            Call::Allocate(_) => 0x4e6350,
                            Call::Release(_) => 0x4e6060,
                        },
                        match c {
                            Call::Allocate(n) => n as u32,
                            Call::Release(a) => a,
                        }
                    ),
                    (*a, *arg),
                    "case{i} callback"
                );
                assert_eq!(&p.bytes, expected_p, "case{i} callback player");
                assert_eq!(rows, expected_rows, "case{i} callback rows");
                Ok(0x10040000)
            };
            if entry == 0x46a6f0 {
                append(
                    &mut player,
                    (ptr != 0).then_some(proposal),
                    &mut records,
                    &mut callback,
                )
                .unwrap();
            } else {
                clear(&mut player, &mut records, &mut callback).unwrap();
            }
            assert_eq!(calls, n as usize);
            assert_eq!(player.bytes, after.0, "case{i} player");
            assert_eq!(records, after.1, "case{i} rows");
        }
        assert!(r.0.is_empty());
    }
}
