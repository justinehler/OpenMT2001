//! Original world queries shared by strategic scoring and target selection.
//! Group tables, category identities and linked-building order stay explicit.
use crate::{
    pe::PeImage,
    player_manager::{Player, word},
};

/// Original5397ac, four signed category IDs within each60-byte group row.
pub fn group(image: &PeImage<'_>, index: u32) -> Result<[i32; 4], String> {
    let at = index
        .checked_mul(60)
        .and_then(|v| v.checked_add(0x5397ac))
        .ok_or("AI group address overflow")?;
    let row = image.at(at, 16)?;
    Ok(std::array::from_fn(|i| word(row, i * 4) as i32))
}

///476920 classifies the original property definition, not current ownership.
pub fn utility_or_transport(image: &PeImage<'_>, category: u32) -> Result<bool, String> {
    // Scoring passes the -1 group sentinel too. Original LEA arithmetic wraps
    // into the preceding PE bytes (53854c), with no range-check callback.
    let at = category.wrapping_mul(88).wrapping_add(0x5385a4);
    Ok(matches!(image.at(at, 1)?[0], 10 | 11))
}

///46e380 counts nonnegative IDs;48a770 instead excludes only literal -1.
pub fn group_size(ids: &[i32; 4]) -> i32 {
    ids.iter().filter(|&&id| id >= 0).count() as i32
}

///46e490 performs all four owner reads, even for the group's -1 sentinel.
/// The caller must explicitly resolve that original pre-pool byte; skipping it
/// would change the query if its value matches this player's owner ID.
pub fn owned_in_group(
    player: &Player,
    ids: &[i32; 4],
    mut owner: impl FnMut(i32) -> Result<i8, String>,
) -> Result<i32, String> {
    let wanted = i32::from(player.bytes[0x1bc]) + 1;
    let mut count = 0;
    for &id in ids {
        count += i32::from(i32::from(owner(id)?) == wanted);
    }
    Ok(count)
}

/// Original61762d+id*380. The common -1 group sentinel aliases byte24d of
/// building599 in the preceding pool; it is not a synthetic city-owner zero.
pub fn category_owner(
    id: i32,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
) -> Result<i8, String> {
    if id == -1 {
        Ok(*buildings
            .get(599)
            .ok_or("AI sentinel owner requires building599")?
            .get(0x24d)
            .unwrap() as i8)
    } else {
        Ok(categories
            .get(id as usize)
            .ok_or("AI group category leaves pool")?[5] as i8)
    }
}

///48a770 compares resolved player object references. City ownership resolves
/// to null; a null receiver can therefore own an entire city-owned group.
pub fn owns_group(
    receiver: u32,
    ids: &[i32; 4],
    mut owner_reference: impl FnMut(i32) -> Result<u32, String>,
) -> Result<bool, String> {
    let mut count = 0;
    let mut owned = 0;
    for &id in ids {
        if id != -1 {
            count += 1;
            owned += i32::from(owner_reference(id)? == receiver);
        }
    }
    Ok(count != 0 && count == owned)
}

pub(crate) fn visit_chain(
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    mut visit: impl FnMut(&[u8; 916]) -> Result<(), String>,
) -> Result<(), String> {
    let mut next = word(category, 0x15c);
    let mut visited = 0;
    while next != 0 {
        let offset = next
            .checked_sub(0x591318)
            .ok_or("AI building chain precedes pool")?;
        if offset % 916 != 0 {
            return Err("AI building chain unaligned".into());
        }
        let row = buildings
            .get(offset as usize / 916)
            .ok_or("AI building chain leaves pool")?;
        if visited >= buildings.len() {
            return Err("AI building chain cycles".into());
        }
        visited += 1;
        visit(row)?;
        next = word(row, 0x44);
    }
    Ok(())
}

///473c90 adds inclusive ground areas in linked-list order; no active/type,
/// occupancy or height filter participates in this query.
pub fn occupied_area(category: &[u8; 380], buildings: &[[u8; 916]]) -> Result<i32, String> {
    let mut area = 0i32;
    visit_chain(category, buildings, |row| {
        let width = i32::from(row[6]) - i32::from(row[4]) + 1;
        let height = i32::from(row[7]) - i32::from(row[5]) + 1;
        area = area.wrapping_add(width.wrapping_mul(height));
        Ok(())
    })?;
    Ok(area)
}

///477780 intentionally reloads the FIRST group's category for each of the
/// first three nonnegative IDs. Preserve this native repeated-chain behavior.
pub fn landscape_count(
    ids: &[i32; 4],
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<i32, String> {
    let mut count = 0i32;
    for &gate in &ids[..3] {
        if gate < 0 {
            continue;
        }
        let category = categories
            .get(ids[0] as usize)
            .ok_or("AI landscape first category leaves pool")?;
        visit_chain(category, buildings, |building| {
            let index = word(building, 0x1c) as i32;
            if index >= 0 {
                let business = businesses
                    .get(index as usize)
                    .ok_or("AI landscape business leaves pool")?;
                if word(business, 4) == 43 {
                    count = count.wrapping_add(1);
                }
            }
            Ok(())
        })?;
    }
    Ok(count)
}

///46afe0/470a00 count signed IDs selected by the native modulo32 bit mask.
pub fn owned_categories(player: &Player) -> Result<i32, String> {
    let mut count = 0;
    for row in player
        .bytes
        .get(0xaac..0xb3c)
        .ok_or("AI player lacks ownership groups")?
        .chunks_exact(12)
    {
        let mask = word(row, 8);
        for &id in &row[1..5] {
            if id as i8 >= 0 && mask & (1u32 << (id & 31)) != 0 {
                count += 1;
            }
        }
    }
    Ok(count)
}

///476a70 scans category+74, independent of its enabled/owner/auction fields.
pub fn any_category_flag(categories: &[[u8; 380]; 52]) -> bool {
    categories.iter().any(|row| row[0x74] != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (head, tail) = self.0.split_at(n);
            self.0 = tail;
            head
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
    #[ignore = "requires original strategic world-query captures"]
    fn matches_native_queries() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_STRATEGY_QUERIES").unwrap()).unwrap();
        let original = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(7), b"MTQRY01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let receiver = r.word();
            assert_eq!(r.word(), 0x037f);
            let eax = r.word();
            let al = r.word();
            let argc = r.word();
            assert_eq!(r.word(), 0, "{name} diagnostics");
            let args: Vec<_> = (0..argc).map(|_| r.word()).collect();
            let before: [&[u8]; 8] = std::array::from_fn(|_| r.blob());
            let after: [&[u8]; 8] = std::array::from_fn(|_| r.blob());
            assert_eq!(before, after, "{name} native query must not mutate tables");
            let mut exe = original.clone();
            for (address, bytes) in [(0x5397ac, before[6]), (0x5385a4, before[7])] {
                let image = PeImage::parse(&exe).unwrap();
                let offset = image.at(address, bytes.len()).unwrap().as_ptr() as usize
                    - exe.as_ptr() as usize;
                exe[offset..offset + bytes.len()].copy_from_slice(bytes);
            }
            let image = PeImage::parse(&exe).unwrap();
            let player = Player {
                reference: receiver,
                bytes: before[0].to_vec(),
                auxiliaries: vec![],
            };
            let categories: Box<[[u8; 380]; 52]> = before[2]
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .into_boxed_slice()
                .try_into()
                .unwrap();
            let buildings: Vec<[u8; 916]> = before[3]
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            let businesses: Vec<[u8; 544]> = before[4]
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            let result = match entry {
                0x476920 => u32::from(utility_or_transport(&image, args[0]).unwrap()),
                0x46e380 => group_size(&group(&image, args[0]).unwrap()) as u32,
                0x46e490 => owned_in_group(&player, &group(&image, args[1]).unwrap(), |id| {
                    category_owner(id, &categories, &buildings)
                })
                .unwrap() as u32,
                0x48a770 => u32::from(
                    owns_group(receiver, &group(&image, args[0]).unwrap(), |id| {
                        let owner = categories[id as usize][5];
                        Ok(if owner == 0 {
                            0
                        } else {
                            word(before[5], usize::from(owner) * 4)
                        })
                    })
                    .unwrap(),
                ),
                0x477780 => landscape_count(
                    &group(&image, args[0]).unwrap(),
                    &categories,
                    &buildings,
                    &businesses,
                )
                .unwrap() as u32,
                0x473c90 => {
                    occupied_area(&categories[args[0] as usize], &buildings).unwrap() as u32
                }
                0x46afe0 => owned_categories(&player).unwrap() as u32,
                0x476a70 => u32::from(any_category_flag(&categories)),
                _ => panic!("unexpected query {entry:08x}"),
            };
            let expected = if matches!(entry, 0x476920 | 0x48a770 | 0x476a70) {
                al
            } else {
                eax
            };
            assert_eq!(result, expected, "{name}");
            assert_eq!(player.bytes, before[0]);
        }
        assert!(r.0.is_empty());
    }
}
