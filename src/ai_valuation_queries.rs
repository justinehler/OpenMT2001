//! World queries used by original auction valuation.
use crate::{
    ai_strategy_queries as q,
    pe::PeImage,
    player_manager::{Player, word},
    player_messages::Players,
    x87::Sum,
};
pub fn category_group(image: &PeImage<'_>, category: u32) -> Result<[i32; 4], String> {
    let address = category
        .checked_mul(88)
        .and_then(|n| n.checked_add(0x5385a4))
        .ok_or("valuation category definition overflow")?;
    q::group(image, u32::from(image.at(address, 1)?[0]))
}
///46e320 counts the nonnegative category IDs in this category's group.
pub fn group_size(image: &PeImage<'_>, category: u32) -> Result<i32, String> {
    Ok(q::group_size(&category_group(image, category)?))
}
///476c30 counts city-owned peers, excluding the requested category itself.
pub fn city_peers(
    category: u32,
    ids: &[i32; 4],
    categories: &[[u8; 380]; 52],
) -> Result<i32, String> {
    let mut count = 0;
    for &id in ids {
        if id >= 0 && id as u32 != category {
            count += i32::from(
                categories
                    .get(id as usize)
                    .ok_or("valuation peer category leaves pool")?[5]
                    == 0,
            );
        }
    }
    Ok(count)
}
///476d60 keeps the first active manager slot with a strictly larger holding.
pub fn largest_owner(
    ids: &[i32; 4],
    categories: &[[u8; 380]; 52],
    players: &impl Players,
) -> Result<(i32, i32), String> {
    let mut best = (-1, 0);
    for slot in 0..6 {
        let Some(player) = players.get(slot) else {
            continue;
        };
        if player.bytes[0x10] == 0 {
            continue;
        }
        let mut count = 0;
        for &id in ids {
            if id >= 0 {
                count += i32::from(
                    i32::from(
                        categories
                            .get(id as usize)
                            .ok_or("valuation group category leaves pool")?[5]
                            as i8,
                    ) == slot as i32 + 1,
                );
            }
        }
        if count > best.1 {
            best = (slot as i32, count);
        }
    }
    Ok(best)
}
///476e70 compares each cash float with the previously truncated integer. Two
///fractional values can therefore replace one another despite descending cash.
pub fn richest_other(exclude: u32, players: &impl Players) -> (i32, i32) {
    let mut best = (-1, 0);
    for slot in 0..6 {
        let Some(player) = players.get(slot) else {
            continue;
        };
        if player.bytes[0x10] == 0 || u32::from(player.bytes[0x1bc]) == exclude {
            continue;
        }
        let cash = f32::from_bits(word(&player.bytes, 4));
        if f64::from(best.1) < f64::from(cash) || cash.is_nan() {
            best = (
                slot as i32,
                Sum::from_f32(cash)
                    .map(|v| v.ftol_low_word())
                    // Native FTOL stores signed64 indefinite then returns its
                    // low DWORD for NaN/infinity: zero, not signed32 indefinite.
                    .unwrap_or(0),
            );
        }
    }
    best
}
///473e80 sums WORD18 of every business reachable from this category's chain.
pub fn business_sum(
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<i32, String> {
    let mut total = 0i32;
    q::visit_chain(category, buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id > -1 {
            let business = businesses
                .get(id as usize)
                .ok_or("valuation business leaves pool")?;
            total = total.wrapping_add(i32::from(u16::from_le_bytes(
                business[0x18..0x1a].try_into().unwrap(),
            )));
        }
        Ok(())
    })?;
    Ok(total)
}
///4770a0 counts linked business owner bytes equal to the player's one-based ID.
pub fn owned_businesses(
    slot: u32,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<i32, String> {
    let mut count = 0i32;
    q::visit_chain(category, buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id > -1 {
            let business = businesses
                .get(id as usize)
                .ok_or("valuation business leaves pool")?;
            if i32::from(business[0xa] as i8) == slot.wrapping_add(1) as i32 {
                count = count.wrapping_add(1);
            }
        }
        Ok(())
    })?;
    Ok(count)
}
///476b60 retains the intermediate f32 store before its signed FTOL difference.
pub fn income_margin(player: &Player) -> Result<i32, String> {
    let first = crate::ai_finance::income_extended(player, false)?.to_f32();
    let second = crate::ai_finance::income_extended(player, true)?;
    let mut difference = Sum::from_f32(first)?;
    difference.add(second.negated());
    let margin = difference.ftol_low_word();
    let compare = Sum::from_f32(Sum::from_i32(margin).to_f32())?;
    let current = crate::ai_finance::income_extended(player, true)?;
    Ok(if current.compare(compare).is_gt() {
        crate::ai_finance::income_extended(player, true)?.ftol_low_word()
    } else {
        margin
    })
}
#[test]
#[ignore = "requires original auction world queries capture and executable"]
fn matches_native_world_queries() {
    struct Slots(Vec<Option<Player>>);
    impl Players for Slots {
        fn get(&self, slot: usize) -> Option<&Player> {
            self.0.get(slot).and_then(Option::as_ref)
        }
        fn get_mut(&mut self, slot: usize) -> Option<&mut Player> {
            self.0.get_mut(slot).and_then(Option::as_mut)
        }
    }
    fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, b) = data.split_at(n);
        *data = b;
        a
    }
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_WORLD_QUERIES").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut data = &bytes[..];
    assert_eq!(take(&mut data, 8), b"MTAVWQ01");
    assert_eq!(word(take(&mut data, 4), 0), 0x37f);
    let count = word(take(&mut data, 4), 0);
    for case in 0..count {
        let category = word(take(&mut data, 4), 0);
        let slot = word(take(&mut data, 4), 0);
        let present = take(&mut data, 6);
        let players = Slots(
            (0..6)
                .map(|i| {
                    let bytes = take(&mut data, 0x1e50).to_vec();
                    (present[i] != 0).then_some(Player {
                        reference: 0x10010000 + i as u32 * 0x2000,
                        bytes,
                        auxiliaries: vec![],
                    })
                })
                .collect(),
        );
        let categories: Box<[[u8; 380]; 52]> = take(&mut data, 52 * 380)
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        let buildings: Vec<[u8; 916]> = take(&mut data, 16 * 916)
            .chunks_exact(916)
            .map(|r| r.try_into().unwrap())
            .collect();
        let businesses: Vec<[u8; 544]> = take(&mut data, 16 * 544)
            .chunks_exact(544)
            .map(|r| r.try_into().unwrap())
            .collect();
        let ids = category_group(&image, category).unwrap();
        let cat = &categories[category as usize];
        let leader = largest_owner(&ids, &categories, &players).unwrap();
        let rich = richest_other(slot, &players);
        let results = [
            (group_size(&image, category).unwrap(), 0xdeadbeefu32 as i32),
            (
                city_peers(category, &ids, &categories).unwrap(),
                0xdeadbeefu32 as i32,
            ),
            leader,
            rich,
            (
                business_sum(cat, &buildings, &businesses).unwrap(),
                0xdeadbeefu32 as i32,
            ),
            (
                owned_businesses(slot, cat, &buildings, &businesses).unwrap(),
                0xdeadbeefu32 as i32,
            ),
            (
                income_margin(players.get(slot as usize).unwrap()).unwrap(),
                0xdeadbeefu32 as i32,
            ),
        ];
        for (index, (a, b)) in results.into_iter().enumerate() {
            assert_eq!(
                (a as u32, b as u32),
                (word(take(&mut data, 4), 0), word(take(&mut data, 4), 0)),
                "case {case} query {index}"
            );
        }
    }
    assert!(data.is_empty());
}
///473d50 includes ground area only for this player's linked businesses.
pub fn owned_area(
    player: &Player,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<i32, String> {
    let wanted = i32::from(player.bytes[0x1bc]) + 1;
    let mut total = 0i32;
    q::visit_chain(category, buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id >= 0
            && i32::from(
                businesses
                    .get(id as usize)
                    .ok_or("owned area business leaves pool")?[0xa] as i8,
            ) == wanted
        {
            let area = (i32::from(b[6]) - i32::from(b[4]) + 1)
                .wrapping_mul(i32::from(b[7]) - i32::from(b[5]) + 1);
            total = total.wrapping_add(area);
        }
        Ok(())
    })?;
    Ok(total)
}
///46e630 calls462350's13-cell extended sum then truncates after EACH business.
pub fn history_sum(
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    image: &PeImage<'_>,
) -> Result<i32, String> {
    let mut total = 0i32;
    q::visit_chain(category, buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id >= 0 {
            let business = businesses
                .get(id as usize)
                .ok_or("history sum business leaves pool")?;
            let mut sum = Sum::from_f32(image.f32_at(0x525040)?)?;
            for at in (0x24..0x58).step_by(4) {
                sum.add_f32(f32::from_bits(word(business, at)))?;
            }
            sum.add(Sum::from_i32(total));
            total = sum.ftol_low_word();
        }
        Ok(())
    })?;
    Ok(total)
}
///473dd0 scans every original category for an active development with vacant
///perimeter and a positive linked-business WORD18 total.
pub fn any_development(
    image: &PeImage<'_>,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<bool, String> {
    for (id, c) in categories.iter().enumerate() {
        let group = image.at(0x5385a4 + id as u32 * 88, 1)?[0];
        if group == 0
            || matches!(group, 10 | 11)
            || c[0x74] == 0
            || crate::ai_candidate_queries::vacant_perimeter(c) <= 0
        {
            continue;
        }
        if business_sum(c, buildings, businesses)? > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}
///471140 uses live business catalog demand factors. Unlike the ordinary
///utility accumulator, every individual business contribution passes FTOL.
#[allow(clippy::too_many_arguments)]
pub fn utility_investment(
    player: &Player,
    category: u32,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    catalog: &crate::business_pool::BusinessCatalog,
    native_utilities: &[f32; 4],
    image: &PeImage<'_>,
) -> Result<i32, String> {
    let mut totals = [0i32; 4];
    for business in businesses
        .get(..600)
        .ok_or("utility investment needs original600business slots")?
    {
        if business[0x21c] == 0 {
            continue;
        }
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
        let b = buildings
            .get(building as usize)
            .ok_or("utility investment building leaves pool")?;
        let area = (i32::from(b[6]) - i32::from(b[4]) + 1)
            .wrapping_mul(i32::from(b[7]) - i32::from(b[5]) + 1)
            .wrapping_mul(i32::from(b[8]));
        let info = catalog
            .row(word(business, 4) as usize)
            .ok_or("utility investment business catalog missing")?;
        for (index, offset) in [(2, 0x64), (0, 0x68), (3, 0x60), (1, 0x6c)] {
            let mut value = Sum::from_i32(area);
            value.mul_f32(f32::from_bits(word(info, offset)))?;
            value.add(Sum::from_i32(totals[index]));
            totals[index] = value.ftol_low_word();
        }
    }
    let group = image.at(0x5385a4 + category * 88, 1)?[0];
    let table = if group == 10 { 0x541ec8 } else { 0x541eb8 };
    let mut subtotal = 0.0f32;
    for index in 0..4 {
        let id = word(image.at(table - index as u32 * 4, 4)?, 0);
        let c = categories
            .get(id as usize)
            .ok_or("utility investment category table leaves pool")?;
        if i32::from(c[5] as i8) == i32::from(player.bytes[0x1bc]) + 1 {
            let mut value = if image.at(0x5385a4 + id * 88, 1)?[0] == 10 {
                Sum::from_f32(native_utilities[3 - index])?
            } else {
                Sum::from_i32(totals[3 - index])
            };
            value.mul_f32(image.f32_at(0x525048)?)?;
            value.add_f32(subtotal)?;
            subtotal = value.to_f32();
        }
    }
    for index in 0..4 {
        if word(image.at(table - index as u32 * 4, 4)?, 0) == category {
            let mut value = if group == 10 {
                let mut value = Sum::from_f32(native_utilities[3 - index])?;
                value.mul_f32(image.f32_at(0x525048)?)?;
                value
            } else {
                Sum::from_i32(totals[3 - index])
            };
            value.add_f32(subtotal)?;
            subtotal = value.to_f32();
        }
    }
    Ok(Sum::from_f32(subtotal)?.ftol_low_word())
}
#[test]
#[ignore = "requires original auction valuation extra capture and executable"]
fn matches_native_extra_queries() {
    fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, b) = data.split_at(n);
        *data = b;
        a
    }
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_VALUATION_EXTRA").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut data = &bytes[..];
    assert_eq!(take(&mut data, 8), b"MTAVEXT1");
    assert_eq!(word(take(&mut data, 4), 0), 0x37f);
    let count = word(take(&mut data, 4), 0);
    for case in 0..count {
        let category = word(take(&mut data, 4), 0);
        let player = Player {
            reference: 0x10010000,
            bytes: take(&mut data, 0x1e50).to_vec(),
            auxiliaries: vec![],
        };
        let categories: Box<[[u8; 380]; 52]> = take(&mut data, 52 * 380)
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        let buildings: Vec<[u8; 916]> = take(&mut data, 16 * 916)
            .chunks_exact(916)
            .map(|r| r.try_into().unwrap())
            .collect();
        let mut businesses = vec![[0u8; 544]; 600];
        for (out, bytes) in businesses
            .iter_mut()
            .zip(take(&mut data, 16 * 544).chunks_exact(544))
        {
            out.copy_from_slice(bytes);
        }
        let rows = take(&mut data, 45 * 116);
        let catalog = crate::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|i| {
            rows[i * 116..(i + 1) * 116].try_into().unwrap()
        }));
        let utilities = std::array::from_fn(|_| f32::from_bits(word(take(&mut data, 4), 0)));
        let cat = &categories[category as usize];
        let results = [
            utility_investment(
                &player,
                category,
                &categories,
                &buildings,
                &businesses,
                &catalog,
                &utilities,
                &image,
            )
            .unwrap() as u32,
            owned_area(&player, cat, &buildings, &businesses).unwrap() as u32,
            history_sum(cat, &buildings, &businesses, &image).unwrap() as u32,
            u32::from(any_development(&image, &categories, &buildings, &businesses).unwrap()),
        ];
        for (index, result) in results.into_iter().enumerate() {
            assert_eq!(
                result,
                word(take(&mut data, 4), 0),
                "case {case} query {index}"
            );
        }
    }
    assert!(data.is_empty());
}
