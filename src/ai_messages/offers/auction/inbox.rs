//! Original auction offer lookup, admission and4699c0 activation.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};

///445cc0 builds a 24-byte delayed category command from category+0 identity.
pub fn wait_descriptor(category: &[u8; 380], mut prior: [u8; 24]) -> [u8; 24] {
    for (at, value) in [(0, 0x525b14), (4, 0x19), (12, 0), (20, word(category, 0))] {
        put(&mut prior, at, value);
    }
    prior[16] = 1;
    prior
}

fn row(player: &Player, reference: u32) -> Result<usize, String> {
    super::super::offset(player, reference)
}
fn find(
    player: &Player,
    base: usize,
    category: u32,
    kind: Option<u32>,
) -> Result<Option<usize>, String> {
    let mut reference = player.reference.wrapping_add(base as u32);
    let mut seen = std::collections::BTreeSet::new();
    while reference != 0 {
        if !seen.insert(reference) {
            return Err("auction offer list cycle".into());
        }
        let at = row(player, reference)?;
        if word(&player.bytes, at + 0x14) == category
            && kind.is_none_or(|k| word(&player.bytes, at + 0x10) == k)
        {
            return Ok(Some(at));
        }
        reference = word(&player.bytes, at + 0x20);
    }
    Ok(None)
}
fn canonical(player: &Player, base: u32, at: usize) -> u32 {
    player
        .reference
        .wrapping_add(base)
        .wrapping_add(i32::from(player.bytes[at + 4] as i8).wrapping_mul(104) as u32)
}
///478e00 returns the signed slot's canonical ordinary address, not the traversed address.
pub fn ordinary(player: &Player, category: u32) -> Result<u32, String> {
    Ok(find(player, 0x248, category, Some(9))?.map_or(0, |at| canonical(player, 0x248, at)))
}
///4789b0 first checks the message's recipient slot, then scans urgent categories.
pub fn winning(player: &Player, message: &[u8; 48]) -> Result<Option<u32>, String> {
    if word(message, 0x1c) != u32::from(player.bytes[0x1bc]) {
        return Ok(None);
    }
    Ok(find(player, 0x658, word(message, 0x18), None)?.map(|at| canonical(player, 0x658, at)))
}
///478950 reads each next link after cancellation.
pub fn cleanup(player: &mut Player) -> Result<(), String> {
    let mut reference = player.reference.wrapping_add(0x658);
    let mut seen = std::collections::BTreeSet::new();
    while reference != 0 {
        if !seen.insert(reference) {
            return Err("auction cleanup list cycle".into());
        }
        let at = row(player, reference)?;
        if word(&player.bytes, at + 8) == 3 {
            super::super::cancel(player, reference)?;
        }
        reference = word(&player.bytes, at + 0x20);
    }
    Ok(())
}
///478a00 copies into the first zero-state urgent slot and rewrites adjacent links.
/// The native full-bank index -1 aliases ordinary slot9; that in-allocation write is retained.
pub fn admit(player: &mut Player, offer: &[u8; 104]) -> Result<(), String> {
    if crate::ai_actions::proposals::duplicate(player, offer)? {
        return Err("original478a00 duplicate diagnostic53dac8".into());
    }
    let index = (0..10)
        .find(|&i| word(&player.bytes, 0x658 + i * 104 + 8) == 0)
        .map_or(-1, |i| i as i32);
    let at = (0x658 + index * 104) as usize;
    player.bytes[at..at + 104].copy_from_slice(offer);
    player.bytes[at + 4] = index as u8;
    let reference = player.reference.wrapping_add(at as u32);
    put(
        &mut player.bytes,
        at + 0x20,
        if index == 9 {
            0
        } else {
            reference.wrapping_add(104)
        },
    );
    put(
        &mut player.bytes,
        at + 0x24,
        if index == 0 {
            0
        } else {
            reference.wrapping_sub(104)
        },
    );
    if index != 0 {
        put(&mut player.bytes, at - 104 + 0x20, reference);
    }
    if index != 9 {
        put(&mut player.bytes, at + 104 + 0x24, reference);
    }
    for off in [8, 12] {
        put(&mut player.bytes, at + off, 1);
    }
    let owner = u32::from(player.bytes[0x1bc]);
    put(&mut player.bytes, at + 0x18, owner);
    player.bytes[0x1405] = player.bytes[0x1405].wrapping_add(1);
    Ok(())
}
fn random(player: &mut Player) -> u32 {
    let next = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, next);
    (next >> 16) & 0x7fff
}
///47d730 reads the retained character's difficulty byte before advancing RNG.
pub fn history_age(player: &mut Player, profiles: &[[u8; 0x650]]) -> Result<u32, String> {
    let profile = profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("auction history character unresolved")?;
    let at = (word(&player.bytes, 0x13f4) as usize)
        .checked_mul(36)
        .and_then(|n| n.checked_add(0x5bb))
        .ok_or("auction history difficulty overflow")?;
    let cap = *profile
        .get(at)
        .ok_or("auction history difficulty leaves profile")?;
    Ok(random(player) % (u32::from(cap) + 1) + 20)
}
///477db0's three conditional draws and speculative bid limit.
pub fn retry(
    player: &mut Player,
    auction: &[u8; 128],
    categories: &[[u8; 380]; 52],
    loan: bool,
    image: &PeImage<'_>,
) -> Result<i32, String> {
    let cash = f32::from_bits(word(&player.bytes, 4));
    if loan && (cash.is_nan() || cash < image.f32_at(0x525cc0)?) {
        return Ok(0);
    }
    let mut chance = Sum::from_i32(crate::ai_profiles::reserve(word(&player.bytes, 0x150)));
    chance.mul_f32(image.f32_at(0x525210)?)?;
    if (random(player) % 101) as i32 > chance.ftol_low_word() {
        return Ok(0);
    }
    let count = random(player) % 4;
    if count == 0 {
        return Ok(0);
    }
    let factor = random(player) % 31 + 10;
    let category = categories
        .get(word(auction, 4) as usize)
        .ok_or("auction retry category unresolved")?;
    let value = super::super::execution::category_value(category, image)?;
    let mut bonus = value;
    bonus.mul(Sum::from_i32(factor as i32));
    bonus.mul_f32(image.f32_at(0x525250)?)?;
    let mut total = value;
    total.add_f32(bonus.to_f32())?;
    let amount = total.ftol_low_word();
    if (word(auction, 0x68) as i32).wrapping_add(50) > amount {
        return Ok(0);
    }
    for (off, value) in [(0xccc, factor), (0xcc4, count), (0xcc8, amount as u32)] {
        put(&mut player.bytes, off, value);
    }
    player.bytes[0xcd0] = 1;
    Ok(amount)
}
fn fresh(player: &Player, category: u32, identity: &mut u32, mut prior: [u8; 104]) -> [u8; 104] {
    for off in [
        0, 8, 12, 16, 0x20, 0x24, 0x34, 0x38, 0x3c, 0x40, 0x48, 0x4c, 0x50, 0x60,
    ] {
        put(&mut prior, off, 0);
    }
    for off in [0x18, 0x28, 0x58, 0x5c] {
        put(&mut prior, off, u32::MAX);
    }
    prior[4] = 99;
    prior[0x44..0x46].fill(255);
    prior[4] = (0..10)
        .find(|&i| {
            let at = 0x248 + i * 104;
            [8, 12, 16]
                .iter()
                .all(|&o| word(&player.bytes, at + o) == 0)
                && [20, 24]
                    .iter()
                    .all(|&o| word(&player.bytes, at + o) == u32::MAX)
        })
        .map_or(255, |i| i as u8);
    *identity = identity.wrapping_add(1);
    if *identity as i32 > 9000 {
        *identity = 1;
    }
    for (off, value) in [
        (0, *identity),
        (8, 1),
        (16, 13),
        (20, category),
        (24, u32::from(player.bytes[0x1bc])),
    ] {
        put(&mut prior, off, value);
    }
    prior
}
pub struct World<'a> {
    pub auction: &'a [u8; 128],
    pub categories: &'a [[u8; 380]; 52],
    pub loan: bool,
    pub image: &'a PeImage<'a>,
    pub identity: &'a mut u32,
}
///4699c0. Caller supplies the actual local stack prior policy explicitly.
pub fn activate(
    player: &mut Player,
    offer: u32,
    message: &[u8; 48],
    prior: [u8; 104],
    world: &mut World<'_>,
) -> Result<(), String> {
    let category = word(message, 0x18);
    if player.bytes[0xcd1] != 0 {
        player.bytes[0xcd1] = 0;
        let amount = retry(
            player,
            world.auction,
            world.categories,
            world.loan,
            world.image,
        )?;
        put(&mut player.bytes, 0xcb8, amount as u32);
    }
    let eligible = word(&player.bytes, 0xcb8) as i32 > word(world.auction, 0x68) as i32;
    if eligible && offer != 0 {
        let at = row(player, offer)?;
        let owner = u32::from(player.bytes[0x1bc]);
        put(&mut player.bytes, at + 0x18, owner);
        put(&mut player.bytes, at + 0x10, 13);
        let copy = player.bytes[at..at + 104].try_into().unwrap();
        admit(player, &copy)?;
        super::super::cancel(player, offer)?;
    } else {
        if !eligible {
            put(&mut player.bytes, 0xb64, 0);
        }
        let copy = fresh(player, category, world.identity, prior);
        admit(player, &copy)?;
    }
    if eligible {
        let amount = word(
            &player.bytes,
            if player.bytes[0xcd0] != 0 {
                0xcc8
            } else {
                0xcb8
            },
        ) as i32;
        put(&mut player.bytes, 0x1dfc, (amount as f32).to_bits());
    } else {
        put(&mut player.bytes, 8, 5);
        put(&mut player.bytes, 0x1dfc, 0);
    }
    Ok(())
}

#[test]
#[ignore = "requires original auction inbox captures"]
fn matches_native_inbox() {
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_INBOX").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    assert_eq!(&data[..8], b"MTAOIN01");
    assert_eq!(word(&data, 8), 0x37f);
    let mut cursor = 16;
    for case in 0..word(&data, 12) {
        let mut take = |n| {
            let b = &data[cursor..cursor + n];
            cursor += n;
            b
        };
        let config = take(24);
        let entry = word(config, 0);
        let category = word(config, 4);
        let mut identity = word(config, 8);
        let loan = word(config, 12) != 0;
        let offer = word(config, 16);
        let auction = take(128).try_into().unwrap();
        let message = take(48).try_into().unwrap();
        let categories: Box<[[u8; 380]; 52]> = take(52 * 380)
            .chunks_exact(380)
            .map(|b| b.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        let prior = take(104).try_into().unwrap();
        let mut player = Player {
            reference: 0x10010000,
            bytes: take(0x1e50).to_vec(),
            auxiliaries: vec![],
        };
        let expected = take(0x1e50);
        let results = take(12);
        let value = match entry {
            0x4699c0 => {
                activate(
                    &mut player,
                    offer,
                    message,
                    prior,
                    &mut World {
                        auction,
                        categories: &categories,
                        loan,
                        image: &image,
                        identity: &mut identity,
                    },
                )
                .unwrap();
                None
            }
            0x477db0 => {
                Some(retry(&mut player, auction, &categories, loan, &image).unwrap() as u32)
            }
            0x478e00 => Some(ordinary(&player, category).unwrap()),
            0x4789b0 => {
                let actual = winning(&player, message).unwrap();
                assert_eq!(
                    actual,
                    if word(results, 4) == 0 {
                        None
                    } else {
                        Some(word(results, 8))
                    },
                    "{case} winner"
                );
                None
            }
            0x478950 => {
                cleanup(&mut player).unwrap();
                None
            }
            0x47d730 => {
                let mut profiles = vec![[0; 0x650]; 14];
                profiles[word(&player.bytes, 0x150) as usize]
                    [0x5bb + word(&player.bytes, 0x13f4) as usize * 36] = word(config, 20) as u8;
                Some(history_age(&mut player, &profiles).unwrap())
            }
            _ => unreachable!(),
        };
        if let Some(v) = value {
            assert_eq!(v, word(results, 4), "{case} result");
        }
        let differences: Vec<_> = player
            .bytes
            .iter()
            .zip(expected)
            .enumerate()
            .filter_map(|(at, (a, b))| (a != b).then_some((at, *a, *b)))
            .collect();
        assert!(
            differences.is_empty(),
            "{case} player differences {differences:x?}"
        );
        assert_eq!(identity, word(results, 0), "{case} identity");
    }
    assert_eq!(cursor, data.len());
}
