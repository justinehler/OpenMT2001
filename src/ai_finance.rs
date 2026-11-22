//! Original466fa0 financial snapshots used by the AI controller.
use crate::{
    player_manager::{Player, put, word},
    x87::Sum,
};

///471320 traverses every category, retaining the accumulated owned count but
/// clearing the subtotal for each utility/transport category encountered.
pub fn projected_income(
    owner: u8,
    categories: &[[u8; 0x17c]; 52],
    native_utilities: &[f32; 4],
    properties: &[f32; 4],
    image: &crate::pe::PeImage<'_>,
) -> Result<i32, String> {
    let mut count = 0i32;
    let mut subtotal = 0.0;
    for category in 0..52 {
        let group = image.at(0x5385a4 + category * 88, 1)?[0];
        if !matches!(group, 10 | 11) {
            continue;
        }
        let table = if group == 10 { 0x541ec8 } else { 0x541eb8 };
        let mut owned = [false; 4];
        for (index, owned) in owned.iter_mut().enumerate() {
            let id = u32::from_le_bytes(image.at(table - index as u32 * 4, 4)?.try_into().unwrap())
                as usize;
            let record = categories
                .get(id)
                .ok_or("AI projected category leaves pool")?;
            *owned = i32::from(record[5] as i8) == i32::from(owner) + 1;
            count = count.wrapping_add(i32::from(*owned));
        }
        subtotal = 0.0;
        for (index, owned) in owned.into_iter().enumerate() {
            if !owned {
                continue;
            }
            let value = if group == 10 {
                native_utilities[3 - index]
            } else {
                properties[3 - index]
            };
            let mut amount = Sum::from_i32(count);
            amount.mul_f32(image.f32_at(0x525048)?)?;
            amount.mul_f32(value)?;
            amount.add_f32(subtotal)?;
            subtotal = amount.to_f32();
        }
    }
    Ok(Sum::from_f32(subtotal)?.ftol_low_word())
}

///472090 counts unordered cash comparisons as well as strictly richer rivals.
pub fn cash_rank(player: &Player, players: &[Player]) -> u32 {
    let cash = f32::from_bits(word(&player.bytes, 4));
    1 + players
        .iter()
        .filter(|other| {
            let other_cash = f32::from_bits(word(&other.bytes, 4));
            other.bytes[0x10] != 0
                && other.bytes[0x1bc] != player.bytes[0x1bc]
                && (cash < other_cash || cash.is_nan() || other_cash.is_nan())
        })
        .count() as u32
}

///471f50: the opposite ledger bank, with slots21 and37 deliberately omitted.
pub fn income(player: &Player, current: bool) -> Result<f32, String> {
    Ok(income_extended(player, current)?.to_f32())
}

/// Preserve ST0 for callers such as strategic scoring which do not store f32.
pub fn income_extended(player: &Player, current: bool) -> Result<Sum, String> {
    let bank = if current {
        usize::from(player.bytes[0x144])
    } else {
        usize::from(player.bytes[0x144] == 0)
    };
    let base = 0x14 + bank * 152;
    if base + 37 * 4 > player.bytes.len() {
        return Err("AI income bank leaves player".into());
    }
    let mut sums = [Sum::default(); 2];
    for (sum, indices) in sums.iter_mut().zip([0..21, 22..37]) {
        for index in indices {
            let value = f32::from_bits(word(&player.bytes, base + index * 4));
            if value.is_nan() {
                return Ok(Sum::default());
            }
            sum.add_f32(value)?;
        }
    }
    let [first, mut second] = sums;
    second.add(first);
    Ok(if second.compare(Sum::default()).is_gt() {
        second
    } else {
        Sum::default()
    })
}

///47d790: income multiplied by the active profile row, capped by its WORD.
pub fn budget(player: &Player, profiles: &[[u8; 0x650]]) -> Result<f32, String> {
    let income = f32::from_bits(word(&player.bytes, 0xca4));
    if income <= 0.0 || income.is_nan() {
        return Ok(0.0);
    }
    let profile = profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("AI budget character leaves profiles")?;
    let at = (word(&player.bytes, 0x13f4) as usize)
        .checked_mul(36)
        .and_then(|v| v.checked_add(0x5b4))
        .ok_or("AI budget row overflow")?;
    let row = profile
        .get(at..at + 6)
        .ok_or("AI budget row leaves profile")?;
    let multiplier = f32::from_le_bytes(row[..4].try_into().unwrap());
    let cap = Sum::from_i32(i32::from(u16::from_le_bytes(row[4..6].try_into().unwrap())));
    let mut product = Sum::from_f32(income)?;
    product.mul_f32(multiplier)?;
    Ok(if product.compare(cap).is_gt() {
        cap
    } else {
        product
    }
    .to_f32())
}

/// Source list rows are retained in their original order. The original sums all
/// b44 rows, including rows whose active byte is clear. `projected` is471320.
pub fn update(
    slot: usize,
    players: &mut [Player],
    profiles: &[[u8; 0x650]],
    normal_tick: u32,
    periodic: bool,
    balances: &[f32],
    projected: i32,
) -> Result<(), String> {
    if slot >= players.len() {
        return Err("AI finance player leaves roster".into());
    }
    let (before, rest) = players.split_at_mut(slot);
    let (player, after) = rest.split_first_mut().unwrap();
    let peers: Vec<_> = before
        .iter()
        .map(Some)
        .chain(std::iter::once(None))
        .chain(after.iter().map(Some))
        .collect();
    update_selected(
        player,
        &peers,
        profiles,
        normal_tick,
        periodic,
        balances,
        projected,
    )
}

/// `None` denotes the selected player in manager order. This permits the normal
/// controller to retain exclusive access to its allocation while reading rivals.
pub fn update_selected(
    player: &mut Player,
    peers: &[Option<&Player>],
    profiles: &[[u8; 0x650]],
    normal_tick: u32,
    periodic: bool,
    balances: &[f32],
    projected: i32,
) -> Result<(), String> {
    if player.bytes.len() < 0x1e50 {
        return Err("truncated AI finance player".into());
    }
    if periodic {
        let due = u16::from_le_bytes(player.bytes[0xb4e..0xb50].try_into().unwrap());
        if normal_tick < u32::from(due) {
            return Ok(());
        }
        player.bytes[0xb4e..0xb50]
            .copy_from_slice(&(normal_tick.wrapping_add(250) as u16).to_le_bytes());
    }
    let rank = |subject: &Player, current: &Player| {
        let cash = f32::from_bits(word(&subject.bytes, 4));
        1 + peers
            .iter()
            .filter(|p| {
                let other = p.unwrap_or(current);
                let value = f32::from_bits(word(&other.bytes, 4));
                other.bytes[0x10] != 0
                    && other.bytes[0x1bc] != subject.bytes[0x1bc]
                    && (cash < value || cash.is_nan() || value.is_nan())
            })
            .count() as u32
    };
    let cash = word(&player.bytes, 4);
    put(&mut player.bytes, 0xc9c, cash);
    let cash_rank = rank(player, player);
    put(&mut player.bytes, 0xca0, cash_rank);
    let own_income = income(player, false)?;
    put(&mut player.bytes, 0xca4, own_income.to_bits());
    let mut balance = Sum::default();
    for &value in balances {
        balance.add_f32(value)?;
    }
    let mut total = Sum::from_i32(projected);
    total.add_f32(balance.to_f32())?;
    let total = Sum::from_i32(total.ftol_low_word()).to_f32();
    put(&mut player.bytes, 0xca8, total.to_bits());
    let allocation = budget(player, profiles)?;
    put(&mut player.bytes, 0xcac, allocation.to_bits());
    for (index, peer) in peers.iter().enumerate() {
        let peer = peer.unwrap_or(player);
        if peer.bytes[0x10] == 0 {
            continue;
        }
        let cash = word(&peer.bytes, 4);
        let rank = rank(peer, player);
        let income = income(peer, false)?;
        let row = 0xcd8 + index * 60;
        if row + 24 > player.bytes.len() {
            return Err("AI finance rival row leaves player".into());
        }
        put(&mut player.bytes, row, cash);
        put(&mut player.bytes, row + 4, rank);
        put(&mut player.bytes, row + 8, income.to_bits());
    }
    let rank = |value: f32, current: &Player| {
        1 + peers
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                p.unwrap_or(current).bytes[0x10] != 0
                    && f32::from_bits(word(&current.bytes, 0xce0 + i * 60)) > value
            })
            .count() as u32
    };
    let own_rank = rank(own_income, player);
    put(&mut player.bytes, 0xcb0, own_rank);
    for (index, peer) in peers.iter().enumerate() {
        if peer.unwrap_or(player).bytes[0x10] == 0 {
            continue;
        }
        let income = f32::from_bits(word(&player.bytes, 0xce0 + index * 60));
        let rank = rank(income, player);
        put(&mut player.bytes, 0xcec + index * 60, rank);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original471320 observations"]
    fn matches_native_projected_income() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PROJECTED").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIPR01");
        for _ in 0..r.word() {
            let owners = r.take(52);
            let values: [f32; 8] = std::array::from_fn(|_| f32::from_bits(r.word()));
            let expected = r.word() as i32;
            let mut categories = [[0; 0x17c]; 52];
            for (row, &owner) in categories.iter_mut().zip(owners) {
                row[5] = owner;
            }
            assert_eq!(
                projected_income(
                    1,
                    &categories,
                    values[4..].try_into().unwrap(),
                    values[..4].try_into().unwrap(),
                    &image
                )
                .unwrap(),
                expected
            );
        }
        assert!(r.0.is_empty());
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
    #[ignore = "requires original466fa0 observations"]
    fn matches_native_finance() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_FINANCE").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAFIN03");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let tick = r.word();
            let periodic = r.word() != 0;
            let projected = r.word() as i32;
            let balances: Vec<_> = (0..r.word()).map(|_| f32::from_bits(r.word())).collect();
            let profiles = vec![r.blob().try_into().unwrap()];
            let mut players: Vec<_> = (0..6)
                .map(|i| Player {
                    reference: 0x10010000 + i * 0x3000,
                    bytes: r.blob().to_vec(),
                    auxiliaries: vec![],
                })
                .collect();
            let expected: Vec<_> = (0..6).map(|_| r.blob()).collect();
            update(
                0,
                &mut players,
                &profiles,
                tick,
                periodic,
                &balances,
                projected,
            )
            .unwrap();
            for (i, (p, e)) in players.iter().zip(expected).enumerate() {
                for (at, (&a, &b)) in p.bytes.iter().zip(e).enumerate() {
                    assert_eq!(a, b, "{name}: player{i}+{at:x}");
                }
            }
        }
        assert!(r.0.is_empty());
    }
}
