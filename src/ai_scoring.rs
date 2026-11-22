//! Original strategic scores (47a140..47a490) and weighted stage choice467840.
//! Every score is signed wrapping32-bit; RNG state advances only on the native
//! weighted branch. Query callbacks remain ordered, with extended income ST0.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    FirstDay,
    GroupSize(u32),
    OwnedInGroup(u32),
    UtilityOrTransport(u32),
    Income,
    OccupiedArea(u32),
    OwnsGroup(u32),
    LandscapeCount(u32),
    OwnedCategories,
    BusinessCount(u32),
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub value: u32,
    pub extended: Sum,
}
impl From<u32> for Reply {
    fn from(value: u32) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }
}

/// Original callbacks471f50/47ff20 can inspect the current player. Peer bytes
/// are read in roster order; the current player is represented by None.
pub fn contribute(
    entry: u32,
    player: &Player,
    peers: &[Option<&Player>],
    image: &PeImage<'_>,
    scores: &mut [i32; 2],
    mut query: impl FnMut(Query) -> Result<Reply, String>,
) -> Result<(), String> {
    let mut total = 0i32;
    match entry {
        0x47a140 => {
            for index in 0..12 {
                let size = query(Query::GroupSize(index))?.value as i32;
                let owned = query(Query::OwnedInGroup(index))?.value as i32;
                if size.wrapping_sub(owned) == 1 {
                    total = total.wrapping_add(100);
                }
            }
        }
        0x47a1c0 => {
            for index in 0..12 {
                let category = crate::ai_strategy_queries::group(image, index)?[0] as u32;
                if query(Query::UtilityOrTransport(category))?.value as u8 == 0 {
                    continue;
                }
                let size = query(Query::GroupSize(index))?.value;
                let owned = query(Query::OwnedInGroup(index))?.value;
                if matches!((size, owned), (3, 1) | (4, 2)) {
                    total = total.wrapping_add(60);
                }
            }
        }
        0x47a280 => {
            if query(Query::FirstDay)?.value as u8 != 0 {
                return Ok(());
            }
            let income = query(Query::Income)?.extended;
            let mut shortfall = Sum::from_f32(image.f32_at(0x525bb0)?)?;
            let mut minus_income = income;
            minus_income.mul_f32(-1.0)?;
            shortfall.add(minus_income);
            scores[1] = scores[1].wrapping_add(income_weight(shortfall, image)?);
            let mut surplus = income;
            surplus.add_f32(-image.f32_at(0x525ca0)?)?;
            scores[0] = scores[0].wrapping_add(income_weight(surplus, image)?);
            return Ok(());
        }
        0x47a360 => {
            for category in 0..52 {
                if query(Query::UtilityOrTransport(category))?.value as u8 == 0
                    && player.bytes[0x178 + category as usize] != 0
                {
                    let area = query(Query::OccupiedArea(category))?.value as i32;
                    total = total.wrapping_add(49i32.wrapping_sub(area));
                }
            }
        }
        0x47a3b0 | 0x47a3f0 => {
            let (wanted, weight) = if entry == 0x47a3b0 {
                (0, 200)
            } else {
                (1, 100)
            };
            for group in 0..12 {
                if query(Query::OwnsGroup(group))?.value as u8 != 0
                    && query(Query::LandscapeCount(group))?.value == wanted
                {
                    total = total.wrapping_add(weight);
                }
            }
        }
        0x47a430 => {
            let cash =
                crate::economy::ftol_low_word(f64::from(f32::from_bits(word(&player.bytes, 4))));
            if query(Query::OwnedCategories)?.value == 0 && cash > 2000 {
                total = (cash - 2000) / 1000 + 50;
            }
        }
        0x47a490 => {
            let owned = i32::from(u16::from_le_bytes(
                player.bytes[0xb44..0xb46].try_into().unwrap(),
            ));
            for (slot, peer) in peers.iter().enumerate() {
                let peer = peer.unwrap_or(player);
                if peer.bytes[0x10] != 0
                    && peer.bytes[0x1bc] != player.bytes[0x1bc]
                    && owned < (query(Query::BusinessCount(slot as u32))?.value as i32)
                {
                    total = total.wrapping_add(40);
                }
            }
        }
        _ => return Err(format!("unrecovered strategic score {entry:08x}")),
    }
    scores[0] = scores[0].wrapping_add(total);
    Ok(())
}

fn income_weight(mut amount: Sum, image: &PeImage<'_>) -> Result<i32, String> {
    amount.mul_f32(image.f32_at(0x5250e0)?)?;
    let floor = Sum::from_f32(image.f32_at(0x525040)?)?;
    if amount.compare(floor).is_lt() {
        amount = floor;
    }
    // The original ceilings the FTOL low word, even when that word wraps.
    let mut value = amount.ftol_low_word();
    if amount.compare(Sum::from_i32(value)).is_gt() {
        value = value.wrapping_add(1);
    }
    Ok(value.wrapping_mul(50).min(300))
}

///467820/467840. Stage2 weights use the first score; stage1 the second.
pub fn choose(
    player: &mut Player,
    peers: &[Option<&Player>],
    image: &PeImage<'_>,
    mode: i32,
    mut query: impl FnMut(Query, &Player) -> Result<Reply, String>,
) -> Result<u32, String> {
    choose_with(player, mode, |entry, player, scores| {
        if let Some(entry) = entry {
            contribute(entry, player, peers, image, scores, |call| {
                query(call, player)
            })?;
            Ok(0)
        } else {
            Ok(query(Query::FirstDay, player)?.value)
        }
    })
}

fn choose_with(
    player: &mut Player,
    mode: i32,
    mut evaluate: impl FnMut(Option<u32>, &Player, &mut [i32; 2]) -> Result<u32, String>,
) -> Result<u32, String> {
    if player.bytes[0x1e44] > 1 {
        return Ok(1);
    }
    let first_day = evaluate(None, player, &mut [0; 2])? as u8 != 0;
    let mut stage2 = if first_day { -50i32 } else { 0 };
    let mut stage1 = if first_day { 30i32 } else { 0 };
    for (entry, target_stage2) in [
        (0x47a140, true),
        (0x47a1c0, true),
        (0x47a280, true),
        (0x47a360, false),
        (0x47a3b0, false),
        (0x47a3f0, false),
        (0x47a430, true),
        (0x47a490, false),
    ] {
        let mut pair = if target_stage2 {
            [stage2, stage1]
        } else {
            [stage1, stage2]
        };
        evaluate(Some(entry), player, &mut pair)?;
        if target_stage2 {
            [stage2, stage1] = pair;
        } else {
            [stage1, stage2] = pair;
        }
    }
    if mode == 6 {
        stage1 = stage1.wrapping_add(100);
    }
    if stage2 >= 0 && stage1 >= 0 {
        let random = word(&player.bytes, 0x228)
            .wrapping_mul(word(&player.bytes, 0x224))
            .wrapping_add(word(&player.bytes, 0x22c));
        put(&mut player.bytes, 0x224, random);
        let divisor = stage1.wrapping_add(stage2).wrapping_add(1);
        if divisor == 0 {
            return Err("native AI weighted score division by zero".into());
        }
        let draw = ((random >> 16) & 0x7fff) as i32 % divisor;
        Ok(1 + u32::from(draw <= stage2))
    } else if stage2 > 0 {
        Ok(2)
    } else if stage1 > 0 && stage2 < 0 {
        Ok(1)
    } else {
        Ok(0)
    }
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
    #[ignore = "requires original strategy score observations"]
    fn matches_native_contributors() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_STRATEGY_SCORES").unwrap()).unwrap();
        let original = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTSCOR02");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            assert_eq!(r.word(), 0x037f);
            let mut scores = [r.word() as i32, r.word() as i32];
            let expected = [r.word() as i32, r.word() as i32];
            let mut trace = vec![];
            for _ in 0..r.word() {
                let h: [u32; 7] = std::array::from_fn(|_| r.word());
                let args: Vec<_> = (0..h[5]).map(|_| r.word()).collect();
                trace.push((h, args));
            }
            let player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let after = r.blob();
            let groups = r.blob();
            let pointers = r.blob();
            let records = r.blob();
            let players: Vec<_> = records
                .chunks_exact(0x300)
                .enumerate()
                .map(|(i, b)| Player {
                    reference: word(pointers, i * 4),
                    bytes: b.to_vec(),
                    auxiliaries: vec![],
                })
                .collect();
            let peers: Vec<_> = players.iter().map(Some).collect();
            let mut exe = original.clone();
            let image = PeImage::parse(&exe).unwrap();
            let at =
                image.at(0x5397ac, groups.len()).unwrap().as_ptr() as usize - exe.as_ptr() as usize;
            exe[at..at + groups.len()].copy_from_slice(groups);
            let image = PeImage::parse(&exe).unwrap();
            let mut cursor = 0;
            contribute(entry, &player, &peers, &image, &mut scores, |query| {
                let (h, args) = &trace[cursor];
                cursor += 1;
                let (address, expected_args) = match query {
                    Query::FirstDay => (0x470ad0, vec![]),
                    Query::GroupSize(i) => (0x46e380, vec![i]),
                    Query::OwnedInGroup(i) => (0x46e490, vec![player.reference, i]),
                    Query::UtilityOrTransport(i) => (0x476920, vec![i]),
                    Query::Income => (0x471f50, vec![0, player.reference]),
                    Query::OccupiedArea(i) => (0x473c90, vec![i]),
                    Query::OwnsGroup(i) => (0x48a770, vec![i]),
                    Query::LandscapeCount(i) => (0x477780, vec![i]),
                    Query::OwnedCategories => (0x46afe0, vec![]),
                    Query::BusinessCount(i) => (0x47ff20, vec![i]),
                };
                assert_eq!(address, h[0], "{name} target");
                assert_eq!(expected_args, *args, "{name} args");
                Ok(Reply {
                    value: h[3],
                    extended: Sum::from_f32(f32::from_bits(h[4])).unwrap(),
                })
            })
            .unwrap();
            assert_eq!(cursor, trace.len(), "{name} full trace");
            assert_eq!(scores, expected, "{name} scores");
            assert_eq!(player.bytes, after, "{name} full player");
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original weighted strategy observations"]
    fn matches_native_weighted_choice() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_CHOICE").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(7), b"MTCHO02");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let config: [u32; 11] = std::array::from_fn(|_| r.word());
            assert_eq!(config[1], 0x037f);
            let mut trace = vec![];
            for _ in 0..config[3] {
                let h: [u32; 8] = std::array::from_fn(|_| r.word());
                let args: Vec<_> = (0..h[5]).map(|_| r.word()).collect();
                let pointers: Vec<_> = (0..h[7]).map(|_| r.word()).collect();
                let deltas: Vec<_> = (0..h[6]).map(|_| r.word()).collect();
                let before: Vec<_> = (0..h[7]).map(|_| r.word()).collect();
                let after: Vec<_> = (0..h[7]).map(|_| r.word()).collect();
                trace.push((h, args, pointers, deltas, before, after));
            }
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let after = r.blob();
            let mut cursor = 0;
            let result = choose_with(&mut player, config[10] as i32, |entry, player, scores| {
                let (h, args, _pointers, deltas, before, after) = &trace[cursor];
                cursor += 1;
                assert_eq!(entry.unwrap_or(0x470ad0), h[0], "{name} target");
                if entry.is_some() {
                    assert_eq!(args[0], player.reference);
                    assert_eq!(
                        scores[..before.len()]
                            .iter()
                            .map(|&v| v as u32)
                            .collect::<Vec<_>>(),
                        *before,
                        "{name} accumulator before"
                    );
                    for (score, delta) in scores.iter_mut().zip(deltas) {
                        *score = score.wrapping_add(*delta as i32);
                    }
                    assert_eq!(
                        scores[..after.len()]
                            .iter()
                            .map(|&v| v as u32)
                            .collect::<Vec<_>>(),
                        *after,
                        "{name} accumulator after"
                    );
                }
                Ok(h[3])
            })
            .unwrap();
            assert_eq!(cursor, trace.len(), "{name} full trace");
            assert_eq!(result, config[2], "{name} result");
            assert_eq!(player.bytes, after, "{name} full player/RNG");
        }
        assert!(r.0.is_empty());
    }
}
