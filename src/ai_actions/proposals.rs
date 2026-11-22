//! Original478650 construction proposal and478240/478b10 retained admission.
use crate::{
    player_manager::{Player, put, word},
    x87::Sum,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Diagnostic(u32),
    UnknownKind(u32),
    CategoryValue(u32),
    CategoryActive(u32),
}
///478b10 deliberately stops at the first non-ready record; later slots are
/// not searched across that gap. Traversal follows retained next pointers.
pub fn duplicate(player: &Player, proposal: &[u8; 0x68]) -> Result<bool, String> {
    let mut reference = player.reference.wrapping_add(0x248);
    let mut seen = 0;
    while reference != 0 {
        let at = reference
            .checked_sub(player.reference)
            .ok_or("AI proposal list precedes player")? as usize;
        let row = player
            .bytes
            .get(at..at + 0x68)
            .ok_or("AI proposal list leaves player")?;
        if word(row, 8) != 1 {
            return Ok(false);
        }
        let kind = word(proposal, 0x10);
        if word(row, 0x10) == kind {
            let equal = match kind {
                1 | 2 => word(row, 0x14) == word(proposal, 0x14),
                3 => word(row, 0x2c) == word(proposal, 0x2c),
                5 | 7 => row[0x44..0x46] == proposal[0x44..0x46],
                8 => [0x14, 0x58, 0x28]
                    .iter()
                    .all(|&a| word(row, a) == word(proposal, a)),
                _ => false,
            };
            if equal {
                return Ok(true);
            }
        }
        reference = word(row, 0x20);
        seen += 1;
        if seen > 10 {
            return Err("AI proposal list cycle".into());
        }
    }
    Ok(false)
}
///478240 returns after all stores and increments the shared identity sequence.
/// No-free-slot native writes before the offer bank are explicitly unsupported.
pub fn admit(
    player: &mut Player,
    proposal: &mut [u8; 0x68],
    identity: &mut u32,
    mut callback: impl FnMut(Call, &mut Player, &mut [u8; 0x68]) -> Result<Sum, String>,
) -> Result<bool, String> {
    if duplicate(player, proposal)? {
        return Ok(false);
    }
    let mut reserve = None;
    match word(proposal, 0x10) {
        1 => {
            let value = callback(Call::CategoryValue(word(proposal, 0x14)), player, proposal)?
                .ftol_low_word();
            let amount = (word(&player.bytes, 0xb6c) as i32).max(value);
            let float = amount as f32;
            put(proposal, 0x60, float.to_bits());
            // FST32 stores the rounded reservation but retains the exact FILD
            // integer in ST0 for the following FTOL.
            reserve = Some(amount);
        }
        2 | 4 => {}
        5..=8 => {
            let amount = f32::from_bits(word(proposal, 0x4c));
            put(proposal, 0x60, amount.to_bits());
            reserve = Some(crate::economy::ftol_low_word(f64::from(amount)));
        }
        other => {
            callback(Call::UnknownKind(other), player, proposal)?;
        }
    }
    if let Some(amount) = reserve {
        let mut total = Sum::from_i32(amount);
        total.add_f32(f32::from_bits(word(&player.bytes, 0x1430)))?;
        put(&mut player.bytes, 0x1430, total.to_f32().to_bits());
    }
    let index = (0..10)
        .find(|&i| {
            let at = 0x248 + i * 0x68;
            [8, 0x10, 0xc]
                .iter()
                .all(|&a| word(&player.bytes, at + a) == 0)
                && [0x14, 0x18]
                    .iter()
                    .all(|&a| word(&player.bytes, at + a) == u32::MAX)
        })
        .ok_or("AI proposal native no-free-slot overwrite is unsupported")?;
    let at = 0x248 + index * 0x68;
    let next = word(&player.bytes, at + 0x20);
    let previous = word(&player.bytes, at + 0x24);
    player.bytes[at..at + 0x68].copy_from_slice(proposal);
    put(&mut player.bytes, at + 0x20, next);
    put(&mut player.bytes, at + 0x24, previous);
    player.bytes[at + 4] = index as u8;
    let owner = u32::from(player.bytes[0x1bc]);
    put(&mut player.bytes, at + 0x18, owner);
    *identity = identity.wrapping_add(1);
    if *identity as i32 > 9000 {
        *identity = 1;
    }
    put(&mut player.bytes, at, *identity);
    player.bytes[0x1404] = player.bytes[0x1404].wrapping_add(1);
    Ok(true)
}
///478650's ten argument words: category, building kind, business type, style,
/// quote bits, x, y, width, height, floors. Undefined stack fields are supplied
/// explicitly by the caller instead of fabricated as recovered native stores.
pub fn construction(
    player: &mut Player,
    arguments: [u32; 10],
    mut prior: [u8; 0x68],
    identity: &mut u32,
    mut callback: impl FnMut(Call, &mut Player, &mut [u8; 0x68]) -> Result<Sum, String>,
) -> Result<bool, String> {
    if player.bytes[0x1404] > 9 {
        callback(Call::Diagnostic(0x53da84), player, &mut prior)?;
    }
    for (at, value) in [
        (0, 0),
        (8, 1),
        (0xc, 0),
        (0x10, 8),
        (0x14, arguments[0]),
        (0x18, u32::MAX),
        (0x20, 0),
        (0x24, 0),
        (0x28, arguments[2]),
        (0x34, arguments[5]),
        (0x38, arguments[6]),
        (0x3c, arguments[7]),
        (0x40, arguments[8]),
        (0x48, 0),
        (0x4c, arguments[4]),
        (0x50, arguments[9]),
        (0x54, arguments[3]),
        (0x58, arguments[1]),
        (0x5c, u32::MAX),
        (0x60, arguments[4]),
    ] {
        put(&mut prior, at, value);
    }
    prior[4] = 99;
    prior[0x44..0x46].fill(255);
    admit(player, &mut prior, identity, callback)
}
///478410 sells a business,4784c0 starts category acquisition,478570 sells land.
pub fn transaction(
    address: u32,
    player: &mut Player,
    arguments: [u32; 2],
    mut prior: [u8; 104],
    identity: &mut u32,
    mut callback: impl FnMut(Call, &mut Player, &mut [u8; 104]) -> Result<Sum, String>,
) -> Result<bool, String> {
    if player.bytes[0x1404] > 9 {
        callback(Call::Diagnostic(0x53da84), player, &mut prior)?;
    }
    for (at, value) in [
        (0, 0),
        (8, 1),
        (0xc, 0),
        (0x18, u32::MAX),
        (0x20, 0),
        (0x24, 0),
        (0x28, u32::MAX),
        (0x34, 0),
        (0x38, 0),
        (0x3c, 0),
        (0x40, 0),
        (0x48, 0),
        (0x4c, 0),
        (0x50, 0),
        (0x58, u32::MAX),
        (0x5c, u32::MAX),
        (0x60, 0),
    ] {
        put(&mut prior, at, value);
    }
    prior[4] = 99;
    prior[0x44..0x46].fill(255);
    match address {
        0x478410 => {
            put(&mut prior, 0x10, 4);
            put(&mut prior, 0x14, arguments[1]);
            put(&mut prior, 0x2c, arguments[0]);
        }
        0x4784c0 => {
            put(&mut prior, 0x10, 1);
            put(&mut prior, 0x14, arguments[0]);
            put(&mut prior, 0x1c, arguments[1]);
        }
        0x478570 => {
            put(&mut prior, 8, 0);
            put(&mut prior, 0x10, 0);
            let category = arguments[0] & 255;
            if callback(Call::CategoryActive(category), player, &mut prior)?.ftol_low_word() as u8
                == 0
            {
                callback(Call::Diagnostic(0x53daa4), player, &mut prior)?;
            }
            put(&mut prior, 0x14, category);
            put(&mut prior, 0x10, 2);
            put(&mut prior, 8, 1);
        }
        _ => {
            return Err(format!(
                "AI transaction constructor {address:08x} unsupported"
            ));
        }
    }
    admit(player, &mut prior, identity, callback)
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
    #[ignore = "requires original retained construction proposal observations"]
    fn matches_native_proposals() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PROPOSALS").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIPRO1");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let mut identity = r.word();
            let expected_identity = r.word();
            let expected = r.word() != 0;
            let args = std::array::from_fn(|_| r.word());
            let count = r.word();
            let mut prior = r.blob().try_into().unwrap();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let after = r.blob();
            let proposal_after = r.blob();
            let mut trace = vec![];
            for _ in 0..count {
                trace.push((r.word(), r.word(), r.blob(), r.blob()));
            }
            let mut cursor = 0;
            let mut cb = |call, player: &mut Player, _proposal: &mut [u8; 0x68]| {
                let (at, value, b, a) = &trace[cursor];
                cursor += 1;
                let (address, arg) = match call {
                    Call::CategoryValue(id) => (0x4477e0, id),
                    Call::CategoryActive(id) => (0x470550, id),
                    Call::Diagnostic(text) => (0x4e6960, text),
                    Call::UnknownKind(_) => (0x4e6960, *value),
                };
                assert_eq!((address, arg), (*at, *value), "{name} callback");
                assert_eq!(player.bytes, *b, "{name} callbackplayer");
                player.bytes.copy_from_slice(a);
                Ok(if matches!(call, Call::CategoryActive(_)) {
                    Sum::from_i32(1)
                } else {
                    Sum::from_f32(1000.75).unwrap()
                })
            };
            let actual = if entry == 0x478650 {
                construction(&mut player, args, prior, &mut identity, &mut cb)
            } else if entry != 0x478240 {
                transaction(
                    entry,
                    &mut player,
                    [args[0], args[1]],
                    prior,
                    &mut identity,
                    &mut cb,
                )
            } else {
                admit(&mut player, &mut prior, &mut identity, &mut cb)
            }
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(actual, expected, "{name} result");
            assert_eq!(identity, expected_identity, "{name} identity");
            assert_eq!(cursor, trace.len(), "{name} callbacks");
            for (i, (actual, expected)) in player.bytes.iter().zip(after).enumerate() {
                assert_eq!(actual, expected, "{name} player+{i:x}");
            }
            if entry == 0x478240 {
                assert_eq!(prior.as_slice(), proposal_after, "{name} proposal");
            }
        }
        assert!(r.0.is_empty());
    }
}
