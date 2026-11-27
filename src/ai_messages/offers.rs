//! Original4779b0/477be0 auction admission and4675f0 offer servicing.
//! Embedded offer references remain addresses relative to their retained owner.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
};
pub mod auction;
pub mod execution;
pub mod negotiation;
pub mod producers;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    Eligible {
        target: u32,
        amount: u32,
    },
    Submit([u32; 8]),
    Reply {
        packet: u32,
        kind: u32,
    },
    AuctionBudget,
    CategoryValue(u32),
    Complete {
        category: u32,
        amount: u32,
        recipient: i32,
    },
    Execute(u32),
    Diagnostic(u32),
}
#[derive(Clone, Copy, Debug)]
pub struct Reply {
    pub value: u32,
    pub amount: i32,
    pub extended: crate::x87::Sum,
}
impl Default for Reply {
    fn default() -> Self {
        Self {
            value: 0,
            amount: 0,
            extended: crate::x87::Sum::from_i32(0),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Auction {
    pub category: u32,
    pub active: u8,
}

fn offset(player: &Player, reference: u32) -> Result<usize, String> {
    let at = reference
        .checked_sub(player.reference)
        .ok_or("offer reference precedes player")? as usize;
    if at
        .checked_add(0x68)
        .is_none_or(|end| end > player.bytes.len())
    {
        return Err("offer reference leaves player".into());
    }
    Ok(at)
}
pub(crate) fn packet(player: &Player, reference: u32) -> Result<[u8; 48], String> {
    player
        .auxiliaries
        .iter()
        .find(|a| a.reference == reference)
        .map(|a| a.bytes)
        .ok_or("offer message unresolved".into())
}

///478d40 for an offer belonging to this player. Original malformed/foreign
/// owner dereferences remain explicit errors instead of clearing another slot.
pub fn cancel(player: &mut Player, reference: u32) -> Result<(), String> {
    let at = offset(player, reference)?;
    if word(&player.bytes, at + 0x18) != u32::from(player.bytes[0x1bc])
        || word(&player.bytes, 0x154) != 1
    {
        return Err("offer cancellation owner is not this AI player".into());
    }
    match word(&player.bytes, at + 0xc) {
        0 => player.bytes[0x1404] = player.bytes[0x1404].wrapping_sub(1),
        1 => player.bytes[0x1405] = player.bytes[0x1405].wrapping_sub(1),
        _ => (),
    }
    for field in [0, 8, 0x10, 0xc, 0x60, 0x1c] {
        put(&mut player.bytes, at + field, 0);
    }
    for field in [0x14, 0x18] {
        put(&mut player.bytes, at + field, u32::MAX);
    }
    Ok(())
}

///4675f0 handles at most one urgent offer, otherwise scans ordinary offers.
/// Next links are read after synchronous execution/cancellation mutations.
pub fn service(
    player: &mut Player,
    mut callback: impl FnMut(Call, &mut Player) -> Result<Reply, String>,
) -> Result<bool, String> {
    if player.bytes.len() < 0x1e50 {
        return Err("offer service AI record truncated".into());
    }
    if player.bytes[0x1405] == 0 && player.bytes[0x1404] == 0 {
        return Ok(false);
    }
    if player.bytes[0x1405] > 0 {
        let mut reference = player.reference.wrapping_add(0x658);
        let mut visited = 0;
        while reference != 0 {
            let at = offset(player, reference)?;
            if word(&player.bytes, at + 8) == 1 {
                callback(Call::Execute(reference), player)?;
                return Ok(true);
            }
            reference = word(&player.bytes, at + 0x20);
            visited += 1;
            if visited > 20 {
                return Err("urgent offer list cycle".into());
            }
        }
    }
    if player.bytes[0x1404] > 0 && player.bytes[0x1df8] == 0 {
        let mut reference = player.reference.wrapping_add(0x248);
        let mut visited = 0;
        while reference != 0 {
            let at = offset(player, reference)?;
            let state = word(&player.bytes, at + 8);
            if state != 2 {
                if state == 3 {
                    cancel(player, reference)?;
                }
                if word(&player.bytes, at + 8) == 1 {
                    callback(Call::Execute(reference), player)?;
                }
            }
            reference = word(&player.bytes, at + 0x20);
            visited += 1;
            if visited > 20 {
                return Err("ordinary offer list cycle".into());
            }
        }
    }
    Ok(true)
}

fn request(
    player: &mut Player,
    reference: u32,
    auction: Option<Auction>,
    statuses: &[u8; 6],
    image: &PeImage<'_>,
    callback: &mut impl FnMut(Call, &mut Player) -> Result<Reply, String>,
) -> Result<(), String> {
    if reference != 0 {
        let p = packet(player, reference)?;
        if callback(
            Call::Eligible {
                target: word(&p, 0x18),
                amount: word(&p, 0x1c),
            },
            player,
        )?
        .value as u8
            != 0
        {
            let p = packet(player, reference)?;
            put(&mut player.bytes, 0xc, word(&p, 0x28));
            let target = word(&p, 0x18);
            let amount = word(&p, 0x1c);
            callback(
                Call::Submit([
                    target,
                    i32::from(p[0x14] as i8) as u32,
                    amount,
                    target,
                    amount,
                    word(&p, 0x20),
                    word(&p, 0x24),
                    word(&p, 0x28),
                ]),
                player,
            )?;
        } else {
            callback(
                Call::Reply {
                    packet: reference,
                    kind: 0x26,
                },
                player,
            )?;
        }
        return Ok(());
    }
    let Some(auction) = auction else {
        return Ok(());
    };
    if auction.active == 0 || player.bytes[0xb86] != 0 {
        return Ok(());
    }
    let status = *statuses
        .get(player.bytes[0x1bc] as usize)
        .ok_or("auction cash status leaves table")?;
    let cash = f32::from_bits(word(&player.bytes, 4));
    if status != 0
        && cash.partial_cmp(&image.f32_at(0x525040)?) != Some(std::cmp::Ordering::Greater)
    {
        return Ok(());
    }
    let previous = word(&player.bytes, 0xb68);
    put(&mut player.bytes, 0xb68, auction.category);
    let budget = callback(Call::AuctionBudget, player)?.amount;
    put(&mut player.bytes, 0xb68, previous);
    let mut value = callback(Call::CategoryValue(auction.category), player)?.extended;
    value.add_f32(image.f32_at(0x525178)?)?;
    let budget_float = budget as f32;
    if value.compare(crate::x87::Sum::from_f32(budget_float)?) != std::cmp::Ordering::Greater {
        put(&mut player.bytes, 0xcb8, budget as u32);
    } else {
        player.bytes[0xcd1] = 1;
        put(&mut player.bytes, 0xcb8, 0);
    }
    for at in [0xcb4, 0x13c4, 0x13c5] {
        player.bytes[at] = 1;
    }
    Ok(())
}

///4779b0. Kind2 locates an ordinary offer by packet identity, then uses its
/// signed stored slot byte to recover the canonical embedded row.
pub fn predecision(
    player: &mut Player,
    kind: u32,
    reference: u32,
    auction: Option<Auction>,
    statuses: &[u8; 6],
    image: &PeImage<'_>,
    mut callback: impl FnMut(Call, &mut Player) -> Result<Reply, String>,
) -> Result<(), String> {
    if player.bytes.len() < 0x1e50 {
        return Err("offer predecision AI record truncated".into());
    }
    match kind {
        0 => {
            if player.bytes[0x13c5] == 0 {
                request(player, 0, auction, statuses, image, &mut callback)?;
            }
        }
        1 => request(player, reference, auction, statuses, image, &mut callback)?,
        2 => {
            let mut next = player.reference.wrapping_add(0x248);
            let mut found = None;
            let mut visited = 0;
            while next != 0 {
                let at = offset(player, next)?;
                let p = packet(player, reference)?;
                if word(&player.bytes, at) == word(&p, 0x28) {
                    found = Some(
                        (player.reference as i64
                            + 0x248
                            + i64::from(player.bytes[at + 4] as i8) * 0x68)
                            as u32,
                    );
                    break;
                }
                next = word(&player.bytes, at + 0x20);
                visited += 1;
                if visited > 20 {
                    return Err("offer identity list cycle".into());
                }
            }
            if let Some(offer) = found {
                let at = offset(player, offer)?;
                if word(&player.bytes, at + 0x10) != 9 {
                    callback(Call::Diagnostic(0x53d9fc), player)?;
                }
                let amount =
                    crate::x87::Sum::from_f32(f32::from_bits(word(&player.bytes, at + 0x60)))?
                        .ftol_low_word();
                let mut remaining =
                    crate::x87::Sum::from_f32(f32::from_bits(word(&player.bytes, 0x1430)))?;
                remaining.add(crate::x87::Sum::from_i32(amount).negated());
                put(&mut player.bytes, 0x1430, remaining.to_f32().to_bits());
                let p = packet(player, reference)?;
                callback(
                    Call::Complete {
                        category: word(&player.bytes, at + 0x14),
                        amount: word(&player.bytes, at + 0x60),
                        recipient: i32::from(p[0x14] as i8),
                    },
                    player,
                )?;
                cancel(player, offer)?;
                player.bytes[0xb85] = 0;
                player.bytes[0xb86] = 0;
                put(&mut player.bytes, 0xb70, 10);
            } else {
                callback(Call::Diagnostic(0x53da30), player)?;
            }
            put(&mut player.bytes, 0xc, u32::MAX);
        }
        3 => (),
        _ => {
            callback(Call::Diagnostic(0x53d9e0), player)?;
        }
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
    #[ignore = "requires original4779b0/4675f0 offer-service observations"]
    fn matches_native_offer_service() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_OFFER_SERVICE").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAOFS01");
        assert_eq!(r.word(), 2);
        assert_eq!(r.word(), 0x037f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let [
                op,
                kind,
                reference,
                auction,
                active,
                status,
                budget,
                accepted,
                mutation,
                returned,
            ] = std::array::from_fn::<_, 10, _>(|_| r.word());
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![crate::player_manager::Auxiliary {
                    reference: 0x10014000,
                    bytes: r.blob().try_into().unwrap(),
                }],
            };
            let expected_player = r.blob();
            let expected_packet = r.blob();
            let expected_trace: Vec<[u32; 9]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut trace = vec![];
            let mut callback = |call, player: &mut Player| {
                let mut result = Reply::default();
                let (address, args): (u32, Vec<u32>) = match call {
                    Call::Eligible { target, amount } => {
                        result.value = accepted;
                        (0x47c5f0, vec![target, amount])
                    }
                    Call::Submit(words) => (0x450490, words.to_vec()),
                    Call::Reply { packet, kind } => (0x48cb70, vec![packet, kind]),
                    Call::AuctionBudget => {
                        result.amount = budget as i32;
                        (0x46d7a0, vec![])
                    }
                    Call::CategoryValue(category) => (0x4477e0, vec![category]),
                    Call::Complete {
                        category,
                        amount,
                        recipient,
                    } => (0x469850, vec![category, amount, recipient as u32]),
                    Call::Execute(reference) => {
                        if mutation != 0 {
                            let at = offset(player, reference)?;
                            put(&mut player.bytes, at + 0x20, 0);
                        }
                        (0x469bf0, vec![reference])
                    }
                    Call::Diagnostic(text) => (0x4e6960, vec![text]),
                };
                let mut row = [0; 9];
                row[0] = address;
                row[1..1 + args.len()].copy_from_slice(&args);
                trace.push(row);
                Ok(result)
            };
            if op == 0 {
                predecision(
                    &mut player,
                    kind,
                    reference,
                    (auction != 0).then_some(Auction {
                        category: 7,
                        active: active as u8,
                    }),
                    &[status as u8, 0, 0, 0, 0, 0],
                    &image,
                    &mut callback,
                )
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            } else {
                assert_eq!(
                    u8::from(service(&mut player, &mut callback).unwrap()),
                    returned as u8,
                    "return {name}"
                );
            }
            let diffs: Vec<_> = player
                .bytes
                .iter()
                .zip(expected_player)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (i, *a, *b))
                .collect();
            assert!(diffs.is_empty(), "player {name}: {diffs:?}");
            assert_eq!(
                player.auxiliaries[0].bytes, expected_packet,
                "packet {name}"
            );
            assert_eq!(trace, expected_trace, "trace {name}");
        }
        assert!(r.0.is_empty());
    }
}
