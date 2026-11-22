//! Direct world/profile queries used by continuation actions.
use crate::player_manager::{put, word, Player};
///4727d0: positive cash-trouble status gives the remaining hours to24.
pub fn trouble_hours(player: &Player, statuses: &[u8; 6], hour: i32) -> Result<i32, String> {
    let status = *statuses
        .get(player.bytes[0x1bc] as usize)
        .ok_or("AI trouble owner missing")?;
    Ok(if status == 0 {
        0
    } else {
        let value = 24i32.wrapping_sub(hour);
        if value == 0 {
            1
        } else {
            value
        }
    })
}
///47d700/47d990 read adjacent bytes in the live profile's difficulty group.
pub fn limit(player: &Player, profiles: &[[u8; 0x650]], cash: bool) -> Result<u32, String> {
    let profile = profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("AI continuation profile missing")?;
    let at = 0x5a8usize
        .checked_add(
            (word(&player.bytes, 0x13f4) as usize)
                .checked_mul(36)
                .ok_or("AI continuation difficulty overflow")?,
        )
        .and_then(|a| a.checked_add(usize::from(cash)))
        .ok_or("AI continuation profile offset overflow")?;
    Ok(u32::from(
        *profile
            .get(at)
            .ok_or("AI continuation difficulty leaves profile")?,
    ))
}
///47b950 enables liquidation when projected cash is below the profile limit.
pub fn liquidation_needed(
    player: &Player,
    profiles: &[[u8; 0x650]],
    statuses: &[u8; 6],
    hour: i32,
    image: &crate::pe::PeImage<'_>,
) -> Result<bool, String> {
    if player.bytes[0x1402] == 0 {
        return Ok(false);
    }
    let status = *statuses
        .get(player.bytes[0x1bc] as usize)
        .ok_or("AI liquidation owner missing")?;
    let remaining = (if status != 0 { 24i32 } else { 48i32 }).wrapping_sub(hour);
    let income = f32::from_bits(word(&player.bytes, 0xca4));
    let estimate =
        crate::economy::ftol_low_word(f64::from(income) * f64::from(image.f32_at(0x525cec)?))
            .wrapping_mul(remaining);
    let threshold = limit(player, profiles, true)? as i32;
    let cash = f32::from_bits(word(&player.bytes, 4));
    if !cash.is_finite() {
        return Ok(cash == f32::NEG_INFINITY);
    }
    let mut projected = crate::x87::Sum::from_f32(cash)?;
    projected.add(crate::x87::Sum::from_i32(estimate));
    Ok(crate::x87::Sum::from_i32(threshold)
        .compare(projected)
        .is_gt())
}
///46b6c0 sets one category bit using the original64-bit shift helper.
pub fn mark_category(player: &mut Player, category: u32) {
    let shift = category as u8;
    let mask = if shift < 64 { 1u64 << shift } else { 0 };
    let bits = u64::from(word(&player.bytes, 0x1e28))
        | (u64::from(word(&player.bytes, 0x1e2c)) << 32)
        | mask;
    put(&mut player.bytes, 0x1e28, bits as u32);
    put(&mut player.bytes, 0x1e2c, (bits >> 32) as u32);
}
///4698c0 scans six pending-offer rows; preserves both caller outputs on no match.
pub fn pending_offer(
    player: &Player,
    category: u32,
    owner: &mut u32,
    amount: &mut u32,
) -> Result<bool, String> {
    for slot in 0..6 {
        let at = 0x1438usize
            .checked_add(
                (category as usize)
                    .checked_mul(8)
                    .ok_or("AI pending category overflow")?,
            )
            .and_then(|a| a.checked_add(slot * 0x1a0))
            .ok_or("AI pending row overflow")?;
        let row = player
            .bytes
            .get(at..at + 8)
            .ok_or("AI pending offer row missing")?;
        if word(row, 4) as i32 > 0 {
            *owner = slot as u32;
            break;
        }
    }
    if *owner as i32 <= -1 {
        return Ok(false);
    }
    let at = 0x1438usize
        .checked_add(
            (category as usize)
                .checked_mul(8)
                .ok_or("AI pending category overflow")?,
        )
        .and_then(|a| a.checked_add((*owner as usize).checked_mul(0x1a0)?))
        .ok_or("AI pending owner overflow")?;
    let row = player
        .bytes
        .get(at..at + 4)
        .ok_or("AI pending owner row missing")?;
    let value = word(row, 0);
    if f32::from_bits(value) > 0.0 {
        *amount = value;
        Ok(true)
    } else {
        Ok(false)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original continuation query observations"]
    fn matches_native_queries() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_CONTINUATION_QUERIES").unwrap())
                .unwrap();
        assert_eq!(&data[..8], b"MTACNQ01");
        let w = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(w(&data, 8), 0x37f);
        let size = 28 + 0x1e50 * 2 + 0x650 * 10 + 8;
        assert_eq!(data.len(), 16 + w(&data, 12) as usize * size);
        for (i, row) in data[16..].chunks_exact(size).enumerate() {
            let entry = w(row, 0);
            let cat = w(row, 4);
            let hour = w(row, 8) as i32;
            let status = w(row, 12) as u8;
            let mut owner = w(row, 20);
            let mut amount = w(row, 24);
            let mut player = Player {
                reference: 0x10010000,
                bytes: row[28..28 + 0x1e50].to_vec(),
                auxiliaries: vec![],
            };
            let at = 28 + 0x1e50;
            let profiles: Vec<[u8; 0x650]> = row[at..at + 0x650 * 10]
                .chunks_exact(0x650)
                .map(|b| b.try_into().unwrap())
                .collect();
            let at = at + 0x650 * 10;
            let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
            let image = crate::pe::PeImage::parse(&exe).unwrap();
            let value = match entry {
                0x47b950 => u32::from(
                    liquidation_needed(&player, &profiles, &[status; 6], hour, &image).unwrap(),
                ),
                0x4727d0 => trouble_hours(&player, &[status; 6], hour).unwrap() as u32,
                0x47d700 | 0x47d990 => limit(&player, &profiles, entry == 0x47d990).unwrap(),
                0x46b6c0 => {
                    mark_category(&mut player, cat);
                    w(row, 16)
                }
                0x4698c0 => {
                    u32::from(pending_offer(&player, cat, &mut owner, &mut amount).unwrap())
                }
                _ => unreachable!(),
            };
            assert_eq!(value, w(row, 16), "case{i} return");
            assert_eq!(player.bytes, &row[at..at + 0x1e50], "case{i} player");
            assert_eq!(
                [owner, amount],
                [w(row, at + 0x1e50), w(row, at + 0x1e50 + 4)],
                "case{i} outputs"
            );
        }
    }
}
