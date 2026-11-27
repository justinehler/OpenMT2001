//! Original474f40 liquidation target production and threshold trimming.
use super::{liquidation_pool, liquidation_score};
use crate::{
    player_manager::{Player, put, word},
    x87::Sum,
};
use std::collections::BTreeMap;
pub fn produce(
    player: &mut Player,
    records: &mut BTreeMap<u32, [u8; 12]>,
    prior: [u8; 12],
    w: &liquidation_score::World<'_>,
    mut allocation: impl FnMut(
        liquidation_pool::Call,
        &Player,
        &BTreeMap<u32, [u8; 12]>,
    ) -> Result<u32, String>,
) -> Result<(), String> {
    let old = word(&player.bytes, 0xb68);
    put(&mut player.bytes, 0xb68, u32::MAX);
    let mut descriptor = prior;
    for category in 0..52u32 {
        let mut next = word(&w.developed.categories[category as usize], 0x15c);
        put(&mut player.bytes, 0xb68, category);
        if player.bytes[0x178 + category as usize] != 0 {
            let score = liquidation_score::score(player, category, 1, w)?;
            if score > 0 {
                put(&mut descriptor, 0, score as u32);
                put(&mut descriptor, 4, 1);
                put(&mut descriptor, 8, category);
                liquidation_pool::append(player, Some(descriptor), records, &mut allocation)?;
            }
        }
        let mut visited = 0;
        while next != 0 {
            if visited >= w.developed.buildings.len() {
                return Err("native liquidation building chain cycles".into());
            }
            visited += 1;
            let offset = next
                .checked_sub(0x591318)
                .ok_or("liquidation building before pool")?;
            if offset % 916 != 0 {
                return Err("liquidation building pointer unaligned".into());
            }
            let building = w
                .developed
                .buildings
                .get(offset as usize / 916)
                .ok_or("liquidation building missing")?;
            let id = word(building, 0x1c) as i32;
            if id > -1 {
                let b = w
                    .developed
                    .businesses
                    .get(id as usize)
                    .ok_or("liquidation producer business missing")?;
                if i32::from(b[10] as i8) == i32::from(player.bytes[0x1bc]) + 1 {
                    match word(b, 0) {
                        0 => put(&mut descriptor, 4, 2),
                        1 => put(&mut descriptor, 4, 3),
                        _ => {}
                    }
                    let score =
                        liquidation_score::score(player, id as u32, word(&descriptor, 4), w)?;
                    if score > 0 {
                        put(&mut descriptor, 0, score as u32);
                        put(&mut descriptor, 8, id as u32);
                        liquidation_pool::append(
                            player,
                            Some(descriptor),
                            records,
                            &mut allocation,
                        )?;
                    }
                }
            }
            next = word(building, 0x44);
        }
    }
    put(&mut player.bytes, 0xb68, old);
    liquidation_pool::sort(player, records)?;
    let base = word(&player.bytes, 0x13e8);
    let count = word(&player.bytes, 0x13ec) as i32;
    for i in 0..count {
        let top = records.get(&base).ok_or("liquidation top row missing")?;
        let mut threshold = Sum::from_i32(word(top, 0) as i32);
        threshold.mul_f32(w.developed.image.f32_at(0x525b64)?)?;
        let row = records
            .get_mut(&base.wrapping_add(i as u32 * 12))
            .ok_or("liquidation trim row missing")?;
        if (word(row, 0) as i32) < threshold.ftol_low_word() {
            put(row, 4, 0);
            put(row, 0, 0);
            put(row, 8, u32::MAX);
        }
    }
    liquidation_pool::sort(player, records)
}
