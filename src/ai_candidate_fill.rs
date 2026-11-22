//! Original 472890/472b90 candidate production, including native sorting.
use crate::{
    ai_selection::{Time, windows},
    ai_strategy_queries::occupied_area,
    ai_targets::{Candidate, sort_candidates},
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 2],
    pub count: usize,
    pub member: bool,
}

pub struct World<'a> {
    pub image: &'a PeImage<'a>,
    pub profiles: &'a [[u8; 0x650]],
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    /// The original reverse scan reads resident +0x3c, at absolute 6fd824.
    pub resident_kinds: &'a [u32],
    pub time: Time,
}

fn random(player: &mut Player) -> i32 {
    let value = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, value);
    ((value >> 16) & 0x7fff) as i32 % 101
}

fn append(
    rows: &mut [Candidate; 52],
    count: &mut i32,
    category: i32,
    score: i32,
) -> Result<(), String> {
    *rows
        .get_mut(*count as usize)
        .ok_or("AI candidate fill exceeds native table")? = Candidate { category, score };
    *count = count.wrapping_add(1);
    Ok(())
}

/// Candidate rows beyond count remain part of the original fixed-size sort.
pub fn fill(
    existing: bool,
    player: &mut Player,
    world: &World<'_>,
    rows: &mut [Candidate; 52],
    count: &mut i32,
    mut callback: impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<(), String> {
    let mut call = |address, arguments, count, member, player: &mut Player| {
        callback(
            Call {
                address,
                arguments,
                count,
                member,
            },
            player,
        )
    };
    let (restrict, window) = if existing {
        let restrict = random(player) <= 80;
        let (build, buy) = windows(player, world.profiles, world.time.hour)?;
        (
            restrict,
            if build {
                1
            } else if buy {
                0
            } else {
                -1
            },
        )
    } else {
        *count = 0;
        let constant =
            |at| -> Result<f32, String> { Ok(f32::from_bits(word(world.image.at(at, 4)?, 0))) };
        let mut sum = Sum::from_f32(constant(0x525040)?)?;
        for &kind in world.resident_kinds.iter().rev() {
            if kind == 2 {
                sum.add_f32(constant(0x525034)?)?;
            }
        }
        sum.mul_f32(constant(0x525058)?)?;
        // The original masked x87 divide by zero reaches FTOL's indefinite
        // integer. Preserve its low word, which is zero, without nonfinite Sum.
        let percent = if world.resident_kinds.is_empty() {
            0
        } else {
            sum.div(Sum::from_i32(
                i32::try_from(world.resident_kinds.len())
                    .map_err(|_| "AI resident count overflow")?,
            ))?;
            sum.mul_f32(constant(0x525048)?)?;
            sum.ftol_low_word()
        };
        (world.time.first_day() && random(player) > percent, -1)
    };
    for category in 0..52u32 {
        let data = &world.categories[category as usize];
        let mode = word(data, 0x78);
        if mode != 2 && (mode != 1 || player.bytes[0x178 + category as usize] == 0) {
            continue;
        }
        let kind = world.image.at(0x5385a4 + category * 88, 1)?[0];
        if matches!(kind, 0 | 10 | 11) {
            continue;
        }
        if existing {
            if occupied_area(data, world.buildings)? == 0 {
                continue;
            }
            let target = if window == 0 { 0x46c9f0 } else { 0x46c900 };
            if call(target, [category, 0], 1, true, player)? as u8 == 0 {
                continue;
            }
            if restrict {
                let allowed =
                    call(0x47d110, [word(&player.bytes, 0x150), 0], 1, false, player)? as i32;
                let prestige =
                    crate::economy::ftol_low_word(f64::from(f32::from_bits(word(data, 0x84))));
                if allowed < prestige {
                    continue;
                }
            }
        } else {
            if restrict
                && f32::from_bits(word(data, 0x84))
                    > f32::from_bits(word(world.image.at(0x525cb0, 4)?, 0))
            {
                continue;
            }
            let (build, buy) = windows(player, world.profiles, world.time.hour)?;
            let target = if build {
                Some(0x46c900)
            } else if buy {
                Some(0x46c9f0)
            } else {
                None
            };
            if let Some(target) = target
                && call(target, [category, 0], 1, true, player)? as u8 == 0
            {
                continue;
            }
        }
        if call(
            if existing { 0x473420 } else { 0x473230 },
            [category, player.reference],
            2,
            false,
            player,
        )? as u8
            == 0
        {
            continue;
        }
        call(0x48b030, [category, 0], 1, true, player)?;
        call(0x48b130, [0, 0], 0, true, player)?;
        let base = i32::from(u16::from_le_bytes(
            player.bytes[0x20a..0x20c].try_into().unwrap(),
        ));
        call(0x48b140, [0, 0], 0, true, player)?;
        let reserved = call(0x473ec0, [category, 0], 1, false, player)? as i32;
        if reserved == 0 {
            continue;
        }
        let free = 49i32
            .wrapping_sub(occupied_area(data, world.buildings)?)
            .wrapping_sub(reserved);
        if (existing && free == 0) || (!existing && free <= 0) {
            continue;
        }
        let mut score = if existing {
            let value = call(0x473740, [player.reference, category], 2, false, player)? as i32;
            if value == 0 || value == 100 {
                continue;
            }
            let mut score = base
                .wrapping_add(value)
                .wrapping_add(free)
                .wrapping_add(reserved.wrapping_mul(2));
            let first = call(0x472ed0, [player.reference, category], 2, false, player)? as i32;
            if first > 0 {
                score = score.wrapping_add(first.wrapping_mul(25));
            }
            let second = call(0x473030, [player.reference, category], 2, false, player)? as i32;
            if second > 0 {
                score = score.wrapping_add(second.wrapping_mul(5));
            }
            score
        } else {
            base.wrapping_add(free.wrapping_mul(2))
                .wrapping_add(reserved.wrapping_mul(3))
        };
        if player.bytes[0x178 + category as usize] != 0 {
            let owned = call(
                0x46e540,
                [category, u32::from(player.bytes[0x1bc])],
                2,
                false,
                player,
            )? as i32;
            score = score.wrapping_add(owned.wrapping_add(5).wrapping_mul(20));
        }
        if score >= 60 {
            append(rows, count, category as i32, score)?;
        }
    }
    if *count > 0 {
        sort_candidates(rows);
    }
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
    }
    #[test]
    #[ignore = "requires original candidate production observations"]
    fn matches_native_candidate_fill() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_CANDIDATE_FILL").unwrap()).unwrap();
        let original = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let mut reader = Reader(&bytes);
        assert_eq!(reader.take(8), b"MTCFILL1");
        let h: [u32; 8] = std::array::from_fn(|_| reader.word());
        assert_eq!(
            h,
            [
                0x037f,
                h[1],
                0x1e50,
                0x650,
                52 * 380,
                600 * 916,
                80 * 88,
                52 * 8
            ]
        );
        for _ in 0..h[1] {
            let name = std::str::from_utf8(reader.blob()).unwrap();
            let c: [u32; 5] = std::array::from_fn(|_| reader.word());
            assert_eq!(c[4], 0, "{name} native fault");
            let pairs: Vec<_> = (0..9).map(|_| (reader.blob(), reader.blob())).collect();
            let mut player = Player {
                reference: 0x10010000,
                bytes: pairs[0].0.to_vec(),
                auxiliaries: vec![],
            };
            let profiles = vec![pairs[1].0.try_into().unwrap()];
            let categories: Box<[[u8; 380]; 52]> = pairs[2]
                .0
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .into_boxed_slice()
                .try_into()
                .unwrap();
            let buildings: Vec<[u8; 916]> = pairs[3]
                .0
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            // The fixture snapshots exactly the native active prefix.
            let resident_kinds: Vec<u32> =
                pairs[4].0.chunks_exact(336).map(|r| word(r, 0)).collect();
            let mut exe = original.clone();
            let image = PeImage::parse(&exe).unwrap();
            let offset = image.at(0x5385a4, pairs[5].0.len()).unwrap().as_ptr() as usize
                - exe.as_ptr() as usize;
            exe[offset..offset + pairs[5].0.len()].copy_from_slice(pairs[5].0);
            let image = PeImage::parse(&exe).unwrap();
            let mut count = word(pairs[6].0, 0) as i32;
            let mut rows = std::array::from_fn(|i| Candidate {
                category: word(pairs[6].0, 4 + i * 8) as i32,
                score: word(pairs[6].0, 8 + i * 8) as i32,
            });
            let time = Time {
                hour: word(pairs[7].0, 0) as i32,
                year: word(pairs[7].0, 4) as i32,
            };
            let world = World {
                image: &image,
                profiles: &profiles,
                categories: &categories,
                buildings: &buildings,
                resident_kinds: &resident_kinds,
                time,
            };
            let mut cursor = 0;
            fill(
                c[0] == 0x472b90,
                &mut player,
                &world,
                &mut rows,
                &mut count,
                |call, player| {
                    assert!(cursor < c[3], "{name}: unexpected {call:?}");
                    cursor += 1;
                    let e: [u32; 5] = std::array::from_fn(|_| reader.word());
                    let args: Vec<_> = (0..e[4]).map(|_| reader.word()).collect();
                    let before = reader.blob();
                    let after = reader.blob();
                    assert_eq!(call.address, e[0], "{name}: callback {cursor}");
                    assert_eq!(&call.arguments[..call.count], args, "{name}: args");
                    if call.member {
                        assert_eq!(e[1], player.reference, "{name}: receiver");
                    }
                    assert_eq!(player.bytes, before, "{name}: player before callback");
                    player.bytes.copy_from_slice(after);
                    Ok(e[3])
                },
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(cursor, c[3], "{name}: callbacks consumed");
            assert_eq!(count, c[2] as i32, "{name}: candidate count");
            assert_eq!(player.bytes, pairs[0].1, "{name}: player after");
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(
                    (row.category, row.score),
                    (
                        word(pairs[6].1, 4 + i * 8) as i32,
                        word(pairs[6].1, 8 + i * 8) as i32
                    ),
                    "{name}: row {i}"
                );
            }
        }
        assert!(reader.0.is_empty());
    }
}
