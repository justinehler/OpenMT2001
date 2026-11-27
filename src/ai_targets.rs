//! Original46c870 landscape target scan and4776c0 perimeter preference.
use crate::{
    ai_strategy_queries::visit_chain,
    pe::PeImage,
    player_manager::{Player, word},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Active(u32),
    Undevelopable(u32),
    LandscapeLimit(u32),
    OwnerCount { owner: u32, category: u32 },
    Allowed(u32),
    Probe { category: u32, kind: u32, mode: u32 },
    Fallback(u32),
}

/// Return the first eligible category, preserving native short-circuit order.
pub fn landscape(
    player: &mut Player,
    mut call: impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<i32, String> {
    for category in 0..52 {
        if call(Call::Active(category), player)? as u8 == 0
            || call(Call::Undevelopable(category), player)? as u8 != 0
            || call(Call::LandscapeLimit(category), player)? as u8 != 0
        {
            continue;
        }
        let count = call(
            Call::OwnerCount {
                owner: u32::from(player.bytes[0x1bc]),
                category,
            },
            player,
        )? as i32;
        if count < 4
            || player.bytes[0x178 + category as usize] == 0
            || call(Call::Allowed(category), player)? as u8 == 0
        {
            continue;
        }
        if call(
            Call::Probe {
                category,
                kind: 44,
                mode: 0,
            },
            player,
        )? as u8
            != 0
        {
            return Ok(category as i32);
        }
    }
    Ok(-1)
}

///4768f0: types0,10 and11 cannot be developed through this scan.
pub fn undevelopable(image: &PeImage<'_>, category: u32) -> Result<bool, String> {
    let at = category
        .checked_mul(88)
        .and_then(|v| v.checked_add(0x5385a4))
        .ok_or("target category overflow")?;
    Ok(matches!(image.at(at, 1)?[0], 0 | 10 | 11))
}

///477860: kind2 linked buildings occupy at least20 ground squares.
pub fn landscape_limit(category: &[u8; 380], buildings: &[[u8; 916]]) -> Result<bool, String> {
    let mut area = 0i32;
    visit_chain(category, buildings, |row| {
        if word(row, 0x24) == 2 {
            let width = i32::from(row[6]) - i32::from(row[4]) + 1;
            let height = i32::from(row[7]) - i32::from(row[5]) + 1;
            area = area.wrapping_add(width.wrapping_mul(height));
        }
        Ok(())
    })?;
    Ok(area >= 20)
}

///477920: count business owners, without a business-active or type filter.
pub fn owner_count(
    owner: u32,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<i32, String> {
    let mut count = 0i32;
    visit_chain(category, buildings, |row| {
        let id = word(row, 0x1c) as i32;
        if id >= 0 {
            let business = businesses
                .get(id as usize)
                .ok_or("target business leaves pool")?;
            if i32::from(business[0xa] as i8) == owner.wrapping_add(1) as i32 {
                count = count.wrapping_add(1);
            }
        }
        Ok(())
    })?;
    Ok(count)
}

///46b700 and the original64-bit shift helper. Only the low8 shift bits are
/// read; counts64..255 produce zero instead of a host-language modulo shift.
pub fn allowed(player: &Player, category: u32) -> bool {
    let shift = category as u8;
    let mask =
        u64::from(word(&player.bytes, 0x1e28)) | (u64::from(word(&player.bytes, 0x1e2c)) << 32);
    mask & 1u64.checked_shl(u32::from(shift)).unwrap_or(0) != 0
}

///4776c0 uses the raw player byte when comparing business owners (unlike
///477920's +1). Any matching perimeter edge returns4; otherwise473f50 runs.
pub fn perimeter_preference(
    player: &mut Player,
    category_id: u32,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    mut fallback: impl FnMut(u32, &mut Player) -> Result<i32, String>,
) -> Result<i32, String> {
    let mut edge = false;
    visit_chain(category, buildings, |row| {
        let id = word(row, 0x1c) as i32;
        // 4776fd..47770a unconditionally indexes +0x1c. A pending building
        // has -1, so the native read is from 0x7a3f92, 0x220 bytes before the
        // business pool's owner byte at 0x7a41b2. That initialized .data
        // byte is zero in v1.4; the original does not skip this building.
        let owner = if id == -1 {
            0
        } else {
            i32::from(
                businesses
                    .get(id as usize)
                    .ok_or("target perimeter business reference unresolved")?[0xa]
                    as i8,
            )
        };
        if owner == i32::from(player.bytes[0x1bc])
            && (row[7] == 6 || row[6] == 6 || row[5] == 0 || row[4] == 0)
        {
            edge = true;
        }
        Ok(())
    })?;
    if edge {
        Ok(4)
    } else {
        let value = fallback(category_id, player)?;
        Ok(if value == -1 { 0 } else { value })
    }
}

///473f50 probes the four edges in native order. For multiple available edges
/// it returns the random INDEX, although the one-edge path returns its code.
pub fn vacant_edge(player: &mut Player, category: &[u8; 380]) -> i32 {
    use crate::{ai_messages::offers::execution::placement, player_manager::put};
    let mut codes = [0i32; 4];
    let mut count = 0;
    for (code, vertical, far, end) in [
        (3, false, 0, 7),
        (1, false, 6, 6),
        (4, true, 0, 6),
        (2, true, 6, 6),
    ] {
        // These calls have no side effects. Retain the original extra x=7
        // probe on the first edge; placement itself owns native bounds checks.
        let any = (0..=end).any(|i| {
            placement(
                category,
                if vertical {
                    [far, i, 1, 1]
                } else {
                    [i, far, 1, 1]
                },
            )
        });
        if any {
            codes[count] = code;
            count += 1;
        }
    }
    if count <= 1 {
        return codes[0];
    }
    let random = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, random);
    (((random >> 16) & 0x7fff) % count as u32) as i32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub category: i32,
    pub score: i32,
}
pub fn sort_candidates(rows: &mut [Candidate]) {
    crate::native_sort::sort(rows, &|a, b| match b.score.cmp(&a.score) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    });
}

///4739f0 chooses among original candidate scores, with native tie order and
/// cutoff. The two candidate producers remain synchronous until reconstructed.
pub fn initial(
    player: &mut Player,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    image: &PeImage<'_>,
    kind: &mut u32,
    mut fill: impl FnMut(u32, &mut Player, &mut [Candidate; 52], &mut i32) -> Result<(), String>,
) -> Result<i32, String> {
    use crate::{player_manager::put, x87::Sum};
    let mut rows = [Candidate {
        category: -1,
        score: 0,
    }; 52];
    // The original traverses all chains before filling, despite discarding
    // their computed areas. Preserve malformed-input failure at this boundary.
    for category in categories {
        crate::ai_strategy_queries::occupied_area(category, buildings)?;
    }
    let mut count = 0i32;
    let mut choice_kind = 3;
    let initial_done = player.bytes[0x13dc] != 0;
    if initial_done {
        choice_kind = 4;
        fill(0x472b90, player, &mut rows, &mut count)?;
    }
    if !initial_done || count == 0 {
        choice_kind = 3;
        fill(0x472890, player, &mut rows, &mut count)?;
    }
    if count <= 0 {
        return Ok(-1);
    }
    let n = usize::try_from(count).map_err(|_| "AI candidate count negative")?;
    if n > rows.len() {
        return Err("AI candidate count exceeds52".into());
    }
    sort_candidates(&mut rows);
    for i in 0..n {
        let mut cutoff = Sum::from_i32(rows[0].score);
        cutoff.mul_f32(image.f32_at(0x525044)?)?;
        if rows[i].score < cutoff.ftol_low_word() {
            rows[i].score = 0;
            count = count.wrapping_sub(1);
        }
    }
    sort_candidates(&mut rows);
    let random = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, random);
    if count == 0 {
        return Err("native AI candidate selection divides by zero".into());
    }
    let index = ((random >> 16) & 0x7fff) as i32 % count;
    if index <= -1 {
        return Ok(index);
    }
    *kind = choice_kind;
    Ok(rows
        .get(index as usize)
        .ok_or("AI selected candidate leaves52rows")?
        .category)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_perimeter_reads_native_pre_pool_zero() {
        let mut category = [0; 380];
        category[0x15c..0x160].copy_from_slice(&0x591318u32.to_le_bytes());
        let mut building = [0; 916];
        building[0x1c..0x20].copy_from_slice(&u32::MAX.to_le_bytes());
        building[7] = 6;
        let buildings = [building];
        let businesses = [];
        // Direct v1.4 4776c0 oracle with this pending edge returns 4 for
        // player owner byte 0 and 0 for owner bytes 1 and 2 (fallback -1).
        for (owner, expected) in [(0, 4), (1, 0), (2, 0)] {
            let mut player = Player {
                reference: 0,
                bytes: vec![0; 0x1e50],
                auxiliaries: vec![],
            };
            player.bytes[0x1bc] = owner;
            assert_eq!(
                perimeter_preference(
                    &mut player,
                    0,
                    &category,
                    &buildings,
                    &businesses,
                    |_, _| Ok(-1)
                )
                .unwrap(),
                expected
            );
        }
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
    #[ignore = "requires original target scan and edge observations"]
    fn matches_native_targets() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_TARGETS").unwrap()).unwrap();
        let original = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTTARG02");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let h: [u32; 13] = std::array::from_fn(|_| r.word());
            assert_eq!(h[2], 0x037f);
            let mut trace = vec![];
            for _ in 0..h[5] {
                let header: [u32; 7] = std::array::from_fn(|_| r.word());
                let args: Vec<_> = (0..header[5]).map(|_| r.word()).collect();
                trace.push((header, args, r.blob(), r.blob()));
            }
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let after = r.blob();
            let categories: Vec<[u8; 380]> = r
                .blob()
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect();
            let buildings: Vec<[u8; 916]> = r
                .blob()
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            let types = r.blob();
            let businesses: Vec<[u8; 544]> = r
                .blob()
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            let _groups = r.blob();
            let mut exe = original.clone();
            let image = PeImage::parse(&exe).unwrap();
            let offset =
                image.at(0x5385a4, types.len()).unwrap().as_ptr() as usize - exe.as_ptr() as usize;
            exe[offset..offset + types.len()].copy_from_slice(types);
            let image = PeImage::parse(&exe).unwrap();
            let mut cursor = 0;
            let mut replay = |call: Call, player: &mut Player| {
                let (header, args, pb, pa) = &trace[cursor];
                cursor += 1;
                let (address, expected) = match call {
                    Call::Active(i) => (0x470550, vec![i]),
                    Call::Undevelopable(i) => (0x4768f0, vec![i]),
                    Call::LandscapeLimit(i) => (0x477860, vec![i]),
                    Call::OwnerCount { owner, category } => (0x477920, vec![owner, category]),
                    Call::Allowed(i) => (0x46b700, vec![i]),
                    Call::Probe {
                        category,
                        kind,
                        mode,
                    } => (0x46bef0, vec![player.reference, category, kind, mode]),
                    Call::Fallback(i) => (0x473f50, vec![i, player.reference]),
                };
                assert_eq!(address, header[0], "{name} calltarget");
                assert_eq!(expected, *args, "{name} arguments");
                assert_eq!(player.bytes, *pb, "{name} beforecall");
                if header[6] != 0 {
                    let value = match call {
                        Call::Active(i) => u32::from(categories[i as usize][0x74]),
                        Call::Undevelopable(i) => u32::from(undevelopable(&image, i).unwrap()),
                        Call::LandscapeLimit(i) => {
                            u32::from(landscape_limit(&categories[i as usize], &buildings).unwrap())
                        }
                        Call::OwnerCount { owner, category } => owner_count(
                            owner,
                            &categories[category as usize],
                            &buildings,
                            &businesses,
                        )
                        .unwrap()
                            as u32,
                        other => panic!("unexpected native scan helper{other:?}"),
                    };
                    assert_eq!(
                        value,
                        if matches!(call, Call::OwnerCount { .. }) {
                            header[3]
                        } else {
                            header[3] & 255
                        },
                        "{name} actualhelper"
                    );
                    assert_eq!(player.bytes, *pa);
                } else {
                    player.bytes.copy_from_slice(pa);
                }
                Ok(header[3])
            };
            let value = match h[0] {
                0x46c870 => landscape(&mut player, &mut replay).unwrap() as u32,
                0x4776c0 => perimeter_preference(
                    &mut player,
                    h[1],
                    &categories[h[1] as usize],
                    &buildings,
                    &businesses,
                    |id, p| replay(Call::Fallback(id), p).map(|v| v as i32),
                )
                .unwrap() as u32,
                0x473f50 => {
                    cursor = trace.len();
                    vacant_edge(&mut player, &categories[h[1] as usize]) as u32
                }
                0x46b700 => {
                    cursor = trace.len();
                    u32::from(allowed(&player, h[1]))
                }
                _ => panic!("unexpected targetentry"),
            };
            assert_eq!(
                value,
                if h[0] == 0x46b700 { h[4] } else { h[3] },
                "{name} result"
            );
            assert_eq!(cursor, trace.len(), "{name} complete scan trace");
            assert_eq!(player.bytes, after, "{name} fullplayer/RNG");
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original4739f0 candidate-selection observations"]
    fn matches_native_candidates() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_CANDIDATES").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTCAND01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let h: [u32; 5] = std::array::from_fn(|_| r.word());
            assert_eq!(h[0], 0x037f);
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let after = r.blob();
            let mut trace = vec![];
            for _ in 0..h[4] {
                let header = [r.word(), r.word(), r.word()];
                trace.push((header, r.blob(), r.blob(), r.blob(), r.blob()));
            }
            let decode = |raw: &[u8]| -> Vec<Candidate> {
                raw.chunks_exact(8)
                    .map(|row| Candidate {
                        category: word(row, 0) as i32,
                        score: word(row, 4) as i32,
                    })
                    .collect()
            };
            let mut cursor = 0;
            let mut kind = 73;
            let result = initial(
                &mut player,
                &[[0; 380]; 52],
                &[],
                &image,
                &mut kind,
                |address, player, rows, count| {
                    let (header, rb, pb, ra, pa) = &trace[cursor];
                    cursor += 1;
                    assert_eq!(address, header[0], "{name} target");
                    assert_eq!(*count as u32, header[1], "{name} countbefore");
                    assert_eq!(rows.as_slice(), decode(rb), "{name} rowsbefore");
                    assert_eq!(player.bytes, *pb, "{name} playerbefore");
                    rows.copy_from_slice(&decode(ra));
                    *count = header[2] as i32;
                    player.bytes.copy_from_slice(pa);
                    Ok(())
                },
            );
            if h[3] != 0 {
                assert!(result.is_err(), "{name} native fault");
            } else {
                assert_eq!(result.unwrap() as u32, h[1], "{name} target result");
            }
            assert_eq!(cursor, trace.len(), "{name} full trace");
            assert_eq!(kind, h[2], "{name} kind");
            assert_eq!(player.bytes, after, "{name} fullplayer/RNG");
        }
        assert!(r.0.is_empty());
    }
}
