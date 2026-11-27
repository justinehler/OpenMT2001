//! Original41bdf0 event command financial effects. Structural type2 effects
//! and utility rebates retain synchronous boundaries for their world helpers.
use super::{
    Deck,
    lifecycle::{Command, card_row},
};
use crate::{
    economy::{AccountPosting, PlayerAccount},
    pe::PeImage,
    player_manager::word,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notice {
    pub player: usize,
    pub card: u32,
    pub enabled: bool,
    pub affected: bool,
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// An account mutation already applied, before the next effect.
    AccountPosted(AccountPosting),
    ///41bf6c..41c231: subtype0..8, target identity from41ca60.
    Target {
        kind: u32,
        target: i32,
        player: usize,
    },
    ///41ada0. Its AL result controls notification suppression.
    Utility {
        kind: u32,
        amount: f32,
        player: usize,
    },
}

pub struct World<'a> {
    pub accounts: &'a mut [PlayerAccount],
    pub active: &'a [u8],
    pub businesses: &'a mut [[u8; 544]],
    pub buildings: &'a mut [[u8; 0x394]],
    pub categories: &'a [[u8; 0x17c]],
    pub counts: &'a mut crate::player_metrics::BusinessCounts,
    pub current: usize,
}

fn product(count: i32, amount: f32) -> Result<f32, String> {
    let mut value = crate::x87::Sum::from_i32(count);
    value.mul_f32(amount)?;
    Ok(value.to_f32())
}
fn volume(business: &[u8; 544], buildings: &[[u8; 0x394]]) -> Result<i32, String> {
    let slot = i16::from_le_bytes(business[8..10].try_into().unwrap());
    let b = buildings
        .get(slot as usize)
        .ok_or("event business building outside pool")?;
    Ok((i32::from(b[7]) - i32::from(b[5]) + 1)
        .wrapping_mul(i32::from(b[6]) - i32::from(b[4]) + 1)
        .wrapping_mul(i32::from(b[8])))
}

/// Original41ada0: compensate each rival according to its count of businesses
/// consuming the specified utility. Counts ignore owner activity. The utility
/// owner is charged once, after all six rival slots have been visited.
/// Reports each completed credit and the final debit synchronously.
#[allow(clippy::too_many_arguments)]
pub fn utility_rebate(
    kind: u32,
    amount: f32,
    actor: usize,
    world: &mut World<'_>,
    utilities: &mut crate::economy::Utilities,
    catalog: &crate::business_pool::BusinessCatalog,
    notice: &mut Notice,
    mut callback: impl FnMut(Call, &mut World<'_>, &mut Notice) -> Result<bool, String>,
) -> Result<bool, String> {
    let kind = kind as usize;
    let category = *([39, 42, 35, 2]
        .get(kind)
        .ok_or("event utility kind outside table")?);
    let owner = world
        .categories
        .get(category)
        .ok_or("event utility category outside world")?[5] as i8;
    if i32::from(owner) != actor as i32 + 1 {
        return Ok(false);
    }
    // FCOM equality also sets C3 for unordered operands in the original.
    if amount == 0. || amount.is_nan() {
        utilities.enabled[[2, 3, 1, 0][kind]] = false;
        return Ok(true);
    }
    let mut counts = [0i32; 7];
    for business in world.businesses.iter() {
        if business[0x21c] == 0 {
            continue;
        }
        let info = catalog
            .row(word(business, 4) as usize)
            .ok_or("event utility business type outside catalog")?;
        let rate = f32::from_bits(word(info, [0x68, 0x6c, 0x64, 0x60][kind]));
        if rate > 0. {
            let owner = business[0xa] as i8;
            let count = counts
                .get_mut(owner as usize)
                .ok_or("event utility business owner leaves count scratch")?;
            *count = count.wrapping_add(1);
        }
    }
    let mut total = 0f32;
    for slot in 0..6 {
        let count = counts[slot + 1];
        if count == 0 || slot == actor {
            continue;
        }
        let value = product(count, amount)?;
        post(world, slot, value, 0x24, notice, &mut callback)?;
        if slot == world.current {
            notice.affected = true;
            notice.amount = value;
        }
        let mut value = crate::x87::Sum::from_i32(count);
        value.mul_f32(amount)?;
        value.add_f32(total)?;
        total = value.to_f32();
    }
    post(world, actor, -total, 0x12, notice, &mut callback)?;
    Ok(true)
}

/// Report each successful post synchronously; observers must not post it again.
fn post(
    world: &mut World<'_>,
    player: usize,
    amount: f32,
    category: usize,
    notice: &mut Notice,
    callback: &mut impl FnMut(Call, &mut World<'_>, &mut Notice) -> Result<bool, String>,
) -> Result<(), String> {
    world
        .accounts
        .get_mut(player)
        .ok_or("event recipient leaves roster")?
        .post(amount, category)?;
    callback(
        Call::AccountPosted(AccountPosting { player, amount }),
        world,
        notice,
    )?;
    Ok(())
}

/// Release the deck flag first, then execute the effect. The returned notice
/// represents the original notification gate after effects; UI presentation
/// remains host-owned. Errors preserve preceding stores.
pub fn apply(
    command: &Command,
    deck: &mut Deck,
    world: &mut World<'_>,
    image: &PeImage<'_>,
    mut callback: impl FnMut(Call, &mut World<'_>, &mut Notice) -> Result<bool, String>,
) -> Result<Notice, String> {
    deck.release(command.card)?;
    let actor = command.player as usize;
    if actor >= world.accounts.len() || world.active.len() != world.accounts.len() {
        return Err("event player leaves roster".into());
    }
    let row = card_row(image, command.card)?;
    let raw = word(row, 8) as i32;
    let amount = product(raw, image.f32_at(0x534200)?)?;
    let mut notice = Notice {
        player: actor,
        card: command.card,
        enabled: row[0x10] != 0,
        affected: false,
        amount: 0.,
    };
    let kind = word(row, 12);
    match kind {
        0 => post(world, actor, amount, 0x24, &mut notice, &mut callback)?,
        7 => post(world, actor, -amount, 0x12, &mut notice, &mut callback)?,
        1 | 8 => {
            for slot in 0..world.active.len() {
                if slot == actor || world.active[slot] == 0 {
                    continue;
                }
                let other_amount = if kind == 1 { -amount } else { amount };
                post(
                    world,
                    slot,
                    other_amount,
                    if kind == 1 { 0x12 } else { 0x24 },
                    &mut notice,
                    &mut callback,
                )?;
                post(
                    world,
                    actor,
                    -other_amount,
                    if kind == 1 { 0x24 } else { 0x12 },
                    &mut notice,
                    &mut callback,
                )?;
                if slot == world.current {
                    notice.affected = true;
                    notice.amount = other_amount;
                }
            }
        }
        2 | 10 => {
            let mut best = if kind == 2 {
                image.f32_at(0x525154)?
            } else {
                image.f32_at(0x525158)?
            };
            let mut chosen = None;
            for (slot, account) in world.accounts.iter().enumerate() {
                let cash = account.cash;
                if world.active[slot] != 0
                    && if kind == 2 {
                        cash.is_nan() || best.is_nan() || cash < best
                    } else {
                        cash > best
                    }
                {
                    best = cash;
                    chosen = Some(slot);
                }
            }
            if chosen != Some(actor) {
                notice.enabled = false;
            } else if kind == 2 {
                post(world, actor, amount, 0x24, &mut notice, &mut callback)?;
            } else {
                post(world, actor, -amount, 0x12, &mut notice, &mut callback)?;
                let mut best = image.f32_at(0x525154)?;
                let mut poorest = None;
                for (slot, account) in world.accounts.iter().enumerate() {
                    let cash = account.cash;
                    if world.active[slot] != 0 && (cash.is_nan() || best.is_nan() || cash < best) {
                        best = cash;
                        poorest = Some(slot);
                    }
                }
                let slot = poorest.ok_or("event poorest-player helper returned null")?;
                post(world, slot, amount, 0x24, &mut notice, &mut callback)?;
                if slot == world.current {
                    notice.affected = true;
                    notice.amount = amount;
                }
            }
        }
        3 | 4 | 13 => {
            let business_type = if kind == 3 { 42 } else { 44 };
            let mut count = 0i32;
            for b in world.businesses.iter() {
                if b[0x21c] != 0
                    && i32::from(b[0xa] as i8) == actor as i32 + 1
                    && word(b, 4) == business_type
                {
                    count = count.wrapping_add(if kind == 3 {
                        i32::from(u16::from_le_bytes(b[0x10..0x12].try_into().unwrap()))
                    } else {
                        volume(b, world.buildings)?
                    });
                }
            }
            if kind != 13 {
                // Kind3 posts zero too; kind4 suppresses without posting.
                if kind == 3 || count != 0 {
                    post(
                        world,
                        actor,
                        product(count, amount)?,
                        0x24,
                        &mut notice,
                        &mut callback,
                    )?;
                }
                if count == 0 {
                    notice.enabled = false;
                }
            } else {
                let owner = world
                    .categories
                    .get(39)
                    .ok_or("event utility outside categories")?[5]
                    as i8;
                if count == 0 || i32::from(owner) == actor as i32 + 1 {
                    notice.enabled = false;
                } else {
                    post(
                        world,
                        actor,
                        product(count.wrapping_neg(), amount)?,
                        0x12,
                        &mut notice,
                        &mut callback,
                    )?;
                    if owner != 0 {
                        post(
                            world,
                            (i32::from(owner) - 1) as usize,
                            product(count, amount)?,
                            0x24,
                            &mut notice,
                            &mut callback,
                        )?;
                    }
                    if i32::from(owner) == world.current as i32 + 1 {
                        notice.affected = true;
                        notice.amount = product(count, amount)?;
                    }
                }
            }
        }
        5 | 23 => world.accounts[actor].replenishment_modifier = if kind == 5 { 1 } else { 2 },
        6 => {
            if !callback(
                Call::Target {
                    kind: raw as u32,
                    target: command.target,
                    player: actor,
                },
                world,
                &mut notice,
            )? {
                notice.enabled = false;
            }
        }
        9 | 14 | 15 | 16 => {
            let owner = if kind == 9 {
                0
            } else {
                let category = match kind {
                    14 => 42,
                    15 => 35,
                    _ => 2,
                };
                world
                    .categories
                    .get(category)
                    .ok_or("event utility outside categories")?[5] as i8
            };
            if kind != 9 && i32::from(owner) == actor as i32 + 1 {
                notice.enabled = false;
            } else {
                let count = world
                    .counts
                    .get(actor, world.accounts.len(), world.businesses)?;
                post(
                    world,
                    actor,
                    product(count.wrapping_neg(), amount)?,
                    0x12,
                    &mut notice,
                    &mut callback,
                )?;
                if owner != 0 {
                    let count = world
                        .counts
                        .get(actor, world.accounts.len(), world.businesses)?;
                    post(
                        world,
                        (i32::from(owner) - 1) as usize,
                        product(count, amount)?,
                        0x24,
                        &mut notice,
                        &mut callback,
                    )?;
                }
                if kind != 9 && i32::from(owner) == world.current as i32 + 1 {
                    notice.affected = true;
                    notice.amount = product(
                        world
                            .counts
                            .get(actor, world.accounts.len(), world.businesses)?,
                        amount,
                    )?;
                }
            }
        }
        11 | 12 | 21 | 22 => {
            let mut count = 0i32;
            if matches!(kind, 11 | 12) {
                let table = if kind == 11 { 0x541ec8 } else { 0x541eb8 };
                for offset in 0..4 {
                    let category = word(image.at(table - offset * 4, 4)?, 0) as usize;
                    let owner = world
                        .categories
                        .get(category)
                        .ok_or("event count category outside pool")?[5]
                        as i8;
                    if i32::from(owner) == actor as i32 + 1 {
                        count = count.wrapping_add(1);
                    }
                }
            } else {
                for b in world.buildings.iter() {
                    if b[0x3f] != 0
                        && i32::from(b[0x2d] as i8) == actor as i32 + 1
                        && b[0x2c] == u8::from(kind == 21)
                    {
                        count = count.wrapping_add(1);
                    }
                }
            }
            if count == 0 {
                notice.enabled = false;
            } else {
                post(
                    world,
                    actor,
                    -product(count, amount)?,
                    0x12,
                    &mut notice,
                    &mut callback,
                )?;
            }
        }
        17..=20 => {
            if !callback(
                Call::Utility {
                    kind: kind - 17,
                    amount,
                    player: actor,
                },
                world,
                &mut notice,
            )? {
                notice.enabled = false;
            }
        }
        _ => {
            return Err(format!(
                "original event effect kind{kind} reaches diagnostic"
            ));
        }
    }
    notice.enabled &= actor != world.current;
    Ok(notice)
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
    #[ignore = "requires original41bdf0 event effect observations"]
    fn matches_native_financial_effects() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_EVENT_EFFECTS")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTECEF01");
        assert_eq!(r.word(), 3);
        assert_eq!(r.word(), 0x037f);
        for _ in 0..r.word() {
            let [card, variant, current, ui] = std::array::from_fn::<_, 4, _>(|_| r.word());
            let mut players: Vec<Vec<u8>> =
                r.blob().chunks_exact(0x600).map(|b| b.to_vec()).collect();
            let mut buildings = vec![[0; 0x394]; 600];
            for (out, b) in buildings.iter_mut().zip(r.blob().chunks_exact(0x394)) {
                out.copy_from_slice(b);
            }
            let mut businesses = vec![[0; 544]; 600];
            for (out, b) in businesses.iter_mut().zip(r.blob().chunks_exact(544)) {
                out.copy_from_slice(b);
            }
            let categories: Vec<[u8; 0x17c]> = r
                .blob()
                .chunks_exact(0x17c)
                .map(|b| b.try_into().unwrap())
                .collect();
            let cache = r.blob();
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(0x74)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let mut utilities = crate::economy::Utilities::new([0; 4]);
            let mut counts = crate::player_metrics::BusinessCounts {
                age: cache[3],
                values: std::array::from_fn(|i| word(cache, 0x90 + i * 16) as i32),
            };
            let expected_players = r.blob();
            let expected_cache = r.blob();
            let expected_flags = r.blob();
            let expected_notice = r.blob();
            let expected_utilities = r.blob();
            let expected_trace: Vec<[u32; 4]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            for _ in 0..r.word() {
                r.take(12);
            }
            let mut deck = Deck::new([1, 0x343fd, 0x269ec3]);
            deck.used.fill(1);
            let mut accounts: Vec<_> = players
                .iter()
                .map(|p| PlayerAccount::from_runtime(p).unwrap())
                .collect();
            let active: Vec<_> = players.iter().map(|p| p[0x10]).collect();
            let mut trace = vec![];
            let notice = apply(
                &Command {
                    card,
                    player: 0,
                    target: 0,
                },
                &mut deck,
                &mut World {
                    accounts: &mut accounts,
                    active: &active,
                    businesses: &mut businesses,
                    buildings: &mut buildings,
                    categories: &categories,
                    counts: &mut counts,
                    current: current as usize,
                },
                &image,
                |call, world, notice| match call {
                    Call::Utility {
                        kind,
                        amount,
                        player,
                    } => {
                        trace.push([
                            0x41ada0,
                            kind,
                            amount.to_bits(),
                            0x10010000 + player as u32 * 0x600,
                        ]);
                        utility_rebate(
                            kind,
                            amount,
                            player,
                            world,
                            &mut utilities,
                            &catalog,
                            notice,
                            |_, _, _| Ok(true),
                        )
                    }
                    Call::AccountPosted(_) => Ok(true),
                    _ => panic!("unexpected target"),
                },
            )
            .unwrap_or_else(|e| panic!("card{card} variant{variant}: {e}"));
            for (p, a) in players.iter_mut().zip(accounts) {
                a.write_runtime(p).unwrap();
            }
            let actual: Vec<_> = players.into_iter().flatten().collect();
            let differences: Vec<_> = actual
                .chunks_exact(4)
                .zip(expected_players.chunks_exact(4))
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (i * 4, word(a, 0), word(b, 0)))
                .collect();
            assert!(
                differences.is_empty(),
                "players card{card} variant{variant}: {differences:?}"
            );
            assert_eq!(counts.age, expected_cache[3], "cache age card{card}");
            assert_eq!(
                counts.values,
                std::array::from_fn(|i| word(expected_cache, 0x90 + i * 16) as i32),
                "cache card{card}"
            );
            assert_eq!(deck.used, expected_flags, "flags card{card}");
            assert_eq!(
                notice.enabled,
                ui != 0,
                "notification card{card} variant{variant}"
            );
            assert_eq!(
                notice.affected,
                expected_notice[0] != 0,
                "affected card{card} variant{variant}"
            );
            assert_eq!(
                notice.amount.to_bits(),
                word(expected_notice, 4),
                "notice amount card{card} variant{variant}"
            );
            assert_eq!(trace, expected_trace, "calls card{card} variant{variant}");
            assert_eq!(
                utilities.enabled.map(u8::from),
                [
                    expected_utilities[3],
                    expected_utilities[2],
                    expected_utilities[0],
                    expected_utilities[1]
                ],
                "utilities card{card} variant{variant}"
            );
        }
        assert!(r.0.is_empty());
    }
}
