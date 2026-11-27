//! Original landscape area queries and47de90 scored perimeter placement.
use crate::{
    ai_strategy_queries::visit_chain,
    player_manager::{Player, put, word},
};
fn area(category: &[u8; 380], buildings: &[[u8; 916]], landscape: bool) -> Result<i32, String> {
    let mut total = 0i32;
    visit_chain(category, buildings, |row| {
        if (word(row, 0x24) == 2) == landscape {
            let w = i32::from(row[6]) - i32::from(row[4]) + 1;
            let h = i32::from(row[7]) - i32::from(row[5]) + 1;
            total = total.wrapping_add(w.wrapping_mul(h));
        }
        Ok(())
    })?;
    Ok(total)
}
///4764a0 and476530 return percentage of49 ground squares, truncating signed.
pub fn percentage(
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    landscape: bool,
) -> Result<i32, String> {
    Ok(area(category, buildings, landscape)?.wrapping_mul(100) / 49)
}
///4778c0 returns the remaining20-square landscaping allowance.
pub fn remaining(category: &[u8; 380], buildings: &[[u8; 916]]) -> Result<i32, String> {
    Ok(20i32.wrapping_sub(area(category, buildings, true)?).max(0))
}
///47de90 takes one RNG draw even when the occupancy difference is zero.
///Scans the original exclusive edge ranges and keeps the first equal-score fit.
pub fn place(
    player: &mut Player,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    position: &mut [u32; 2],
    width: &mut u32,
    height: &mut u32,
    mut available: impl FnMut([i32; 4]) -> Result<bool, String>,
) -> Result<bool, String> {
    let chance = percentage(category, buildings, false)?
        .wrapping_sub(percentage(category, buildings, true)?)
        .clamp(0, 100);
    let seed = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, seed);
    if (((seed >> 16) & 0x7fff) % 101) as i32 > chance {
        return Ok(false);
    }
    let mut best = 0;
    let mut chosen = [0; 4];
    for w in (1..=6).rev() {
        for h in (1..=6).rev() {
            let nx = 7 - w;
            let ny = 7 - h;
            for edge in 0..4 {
                let length = if edge < 2 { nx } else { ny };
                for along in 0..length {
                    let (x, y) = match edge {
                        0 => (along, 0),
                        1 => (along, ny),
                        2 => (0, along),
                        _ => (nx, along),
                    };
                    if !available([x, y, w, h])? {
                        continue;
                    }
                    let perimeter = if along == 0 {
                        w + h - 1
                    } else if edge < 2 {
                        w
                    } else {
                        h
                    };
                    let score = (w * h + 2 * (5 - perimeter)) * 10;
                    if score > best {
                        best = score;
                        chosen = [x, y, w, h];
                    }
                }
            }
        }
    }
    if best <= 0 {
        return Ok(false);
    }
    *position = [chosen[0] as u32, chosen[1] as u32];
    *width = chosen[2] as u32;
    *height = chosen[3] as u32;
    Ok(true)
}
pub fn place_in_category(
    player: &mut Player,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    position: &mut [u32; 2],
    width: &mut u32,
    height: &mut u32,
) -> Result<bool, String> {
    let grid = category[0x9c..0xcd].try_into().unwrap();
    place(
        player,
        category,
        buildings,
        position,
        width,
        height,
        |[x, y, w, h]| {
            Ok(crate::building_pool::can_place(
                grid,
                [x, y, x + w - 1, y + h - 1],
            ))
        },
    )
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
    #[ignore = "requires original landscape placement observations"]
    fn matches_native_landscape() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_LANDSCAPE")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAILND1");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let expected = r.word();
            let count = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let category: &[u8; 380] = r.blob().try_into().unwrap();
            let buildings: Vec<[u8; 916]> = r
                .blob()
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            let mut out = r.blob().to_vec();
            let pa = r.blob();
            let ca = r.blob();
            let ba = r.blob();
            let oa = r.blob();
            let expected_trace: Vec<[u32; 4]> = (0..count)
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut trace = vec![];
            let value = match entry {
                0x4764a0 | 0x476530 => {
                    percentage(category, &buildings, entry == 0x476530).unwrap() as u32
                }
                0x4778c0 => remaining(category, &buildings).unwrap() as u32,
                0x47de90 => {
                    let mut pos = [word(&out, 0), word(&out, 4)];
                    let mut w = word(&out, 8);
                    let mut h = word(&out, 12);
                    let grid = category[0x9c..0xcd].try_into().unwrap();
                    let result = place(
                        &mut player,
                        category,
                        &buildings,
                        &mut pos,
                        &mut w,
                        &mut h,
                        |a| {
                            trace.push(a.map(|v| v as u32));
                            let [x, y, w, h] = a;
                            Ok(crate::building_pool::can_place(
                                grid,
                                [x, y, x + w - 1, y + h - 1],
                            ))
                        },
                    )
                    .unwrap();
                    for (i, v) in [pos[0], pos[1], w, h].iter().enumerate() {
                        put(&mut out, i * 4, *v);
                    }
                    u32::from(result)
                }
                _ => panic!(),
            };
            assert_eq!(
                value,
                if entry == 0x47de90 {
                    expected & 255
                } else {
                    expected
                },
                "{name} result"
            );
            assert_eq!(trace, expected_trace, "{name} trace");
            assert_eq!(player.bytes, pa, "{name} player");
            assert_eq!(category.as_slice(), ca, "{name} category");
            assert_eq!(buildings.as_flattened(), ba, "{name} buildings");
            assert_eq!(out, oa, "{name} outputs");
        }
        assert!(r.0.is_empty());
    }
}
