//! Original group-based valuation classification (4790c0 and46d990..46df40).
use crate::{
    ai_finance, ai_profiles, ai_strategy_queries as q, ai_valuation_queries as v,
    pe::PeImage,
    player_manager::{Player, put, word},
    player_messages::Players,
    x87::Sum,
};
pub struct World<'a, P: Players> {
    pub image: &'a PeImage<'a>,
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub players: &'a mut P,
    pub loan_status: &'a [u8; 6],
}
fn player<'w, P: Players>(world: &'w World<'_, P>, slot: u32) -> Result<&'w Player, String> {
    world
        .players
        .get(slot as usize)
        .ok_or_else(|| format!("valuation player slot {slot} unresolved"))
}
fn loan<P: Players>(world: &World<'_, P>, slot: u32) -> Result<bool, String> {
    let p = player(world, slot)?;
    Ok(*world
        .loan_status
        .get(p.bytes[0x1bc] as usize)
        .ok_or("valuation loan slot unresolved")?
        != 0)
}
fn cash<P: Players>(world: &World<'_, P>, slot: u32) -> Result<i32, String> {
    Ok(Sum::from_f32(f32::from_bits(word(&player(world, slot)?.bytes, 4)))?.ftol_low_word())
}
fn set<P: Players>(
    world: &mut World<'_, P>,
    slot: u32,
    at: usize,
    value: u32,
) -> Result<(), String> {
    put(
        &mut world
            .players
            .get_mut(slot as usize)
            .ok_or("valuation player missing")?
            .bytes,
        at,
        value,
    );
    Ok(())
}
///479d40. All valid codes are fixed original PE constants; unsupported codes
///would diagnose in the original and remain an explicit error here.
pub fn score(image: &PeImage<'_>, code: u32, size: i32) -> Result<Sum, String> {
    let at = if code == 10 {
        0x525ce0
    } else if code > 10 {
        return Err("valuation classification code outside original switch".into());
    } else {
        match (code, size) {
            (0, 4) => 0x525248,
            (1, 2 | 3) => 0x52505c,
            (1, 4) => 0x525058,
            (2, 2 | 3) => 0x52506c,
            (2, 4) => 0x5251dc,
            (3, 3) => 0x525248,
            (3, 4) => 0x525078,
            (4, 2 | 3) => 0x5250a0,
            (4, 4) => 0x525248,
            (6, 2 | 3) => 0x5250a4,
            (6, 4) => 0x5250c0,
            (7, 4) => 0x525094,
            (8, 2 | 3) => 0x525ce8,
            (8, 4) => 0x525ce4,
            (9, 2..=4) => 0x525ce8,
            _ => 0x525040,
        }
    };
    Sum::from_f32(image.f32_at(at)?).map_err(Into::into)
}
///46df40 compares the two players' original truncated cash/income and profile
///allowance, retaining the classification store on successful competition.
fn contest<P: Players>(
    world: &mut World<'_, P>,
    category: u32,
    other: u32,
    slot: u32,
    size: i32,
) -> Result<Sum, String> {
    let other_cash = cash(world, other)?;
    let other_income = ai_finance::income_extended(player(world, other)?, false)?.ftol_low_word();
    let other_loan = loan(world, other)?;
    let own_cash = cash(world, slot)?;
    let own_income = ai_finance::income_extended(player(world, slot)?, false)?.ftol_low_word();
    let character = word(&player(world, slot)?.bytes, 0x150);
    let penalty = ai_profiles::loan_multiplier(character);
    let own_loan = loan(world, slot)?;
    let adjustment = ai_profiles::adjustment(character);
    let mut adjusted = adjustment / 2;
    if own_loan {
        let mut value = super::super::super::execution::category_value(
            world
                .categories
                .get(category as usize)
                .ok_or("valuation category missing")?,
            world.image,
        )?;
        value.add(Sum::from_i32(adjusted));
        value.add(Sum::from_i32(penalty));
        adjusted = value.ftol_low_word();
    }
    let own = own_income.wrapping_add(own_cash);
    let other_total = other_income.wrapping_add(other_cash);
    let succeeds = own > other_total.wrapping_add(adjusted)
        || (!own_loan
            && other_loan
            && own_cash > adjustment.wrapping_add(1000)
            && own.wrapping_sub(adjustment.wrapping_add(1000)) > other_total);
    let result = score(world.image, if succeeds { 2 } else { 8 }, size)?.ftol_low_word();
    if succeeds {
        set(world, slot, 0x1e30, 2)?;
    }
    Ok(Sum::from_i32(result))
}
///4790c0 executes the size-specific classifier then FTOL/FILD before returning.
pub fn evaluate<P: Players>(
    world: &mut World<'_, P>,
    slot: u32,
    auction: bool,
    category: u32,
) -> Result<Sum, String> {
    let size = v::group_size(world.image, category)?;
    if !(2..=4).contains(&size) {
        return Ok(Sum::default());
    }
    let ids = v::category_group(world.image, category)?;
    let city = v::city_peers(category, &ids, world.categories)?;
    let (leader, holding) = v::largest_owner(&ids, world.categories, world.players)?;
    let own_count = if size == 4 {
        let mut n = 0;
        for id in ids {
            n += i32::from(
                i32::from(q::category_owner(id, world.categories, world.buildings)?)
                    == slot.wrapping_add(1) as i32,
            );
        }
        n
    } else {
        0
    };
    let active_loan = loan(world, slot)?;
    let money = cash(world, slot)?;
    let character = word(&player(world, slot)?.bytes, 0x150);
    let adjustment = ai_profiles::adjustment(character);
    let reserve = ai_profiles::reserve(character);
    let (_, richest) = v::richest_other(slot, world.players);
    let strong = auction
        || (money.wrapping_add(reserve) > richest
            && (!active_loan
                || (money > adjustment.wrapping_add(2000)
                    && money > richest.wrapping_add(adjustment).wrapping_add(1000))));
    let code = if leader == slot as i32 {
        match size {
            2 => {
                if strong {
                    set(world, slot, 0x1e30, 0)?;
                    1
                } else {
                    9
                }
            }
            3 => {
                if holding == 2 {
                    if strong {
                        set(world, slot, 0x1e30, 0)?;
                        1
                    } else {
                        9
                    }
                } else if holding == 1 && city == 1 {
                    3
                } else {
                    return Ok(Sum::default());
                }
            }
            4 => {
                if holding >= 2 {
                    if strong {
                        if holding == 3 {
                            set(world, slot, 0x1e30, 0)?;
                            1
                        } else {
                            set(world, slot, 0x1e30, 1)?;
                            3
                        }
                    } else if holding == 3 {
                        9
                    } else {
                        10
                    }
                } else if holding == 1 {
                    4
                } else {
                    return Ok(Sum::default());
                }
            }
            _ => unreachable!(),
        }
    } else if leader > -1 {
        if size == 3 && holding != 2 {
            if holding == 1 && city == 1 {
                5
            } else {
                return Ok(Sum::default());
            }
        } else if auction {
            set(world, slot, 0x1e30, 2)?;
            set(world, slot, 0x1e38, leader as u32)?;
            2
        } else {
            return contest(world, category, leader as u32, slot, size);
        }
    } else {
        match size {
            2 if city == 1 => 4,
            3 if city == 2 => 4,
            4 if holding == 1 && city == 2 => {
                if money > richest {
                    0
                } else {
                    5
                }
            }
            4 if city == 3 || own_count > 0 => 7,
            _ => return Ok(Sum::default()),
        }
    };
    Ok(Sum::from_i32(
        score(world.image, code, size)?.ftol_low_word(),
    ))
}
#[test]
#[ignore = "requires original auction base valuation capture and executable"]
fn matches_native_base() {
    fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, b) = data.split_at(n);
        *data = b;
        a
    }
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_VALUATION_BASE")).unwrap();
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut data = &bytes[..];
    assert_eq!(take(&mut data, 8), b"MTAVBAS1");
    assert_eq!(word(take(&mut data, 4), 0), 0x37f);
    let count = word(take(&mut data, 4), 0);
    for case in 0..count {
        let category = word(take(&mut data, 4), 0);
        let slot = word(take(&mut data, 4), 0);
        let auction = word(take(&mut data, 4), 0) != 0;
        let status: &[u8; 6] = take(&mut data, 6).try_into().unwrap();
        let mut buildings = vec![[0u8; 916]; 600];
        buildings[599][0x24d] = take(&mut data, 1)[0];
        let categories: Box<[[u8; 380]; 52]> = take(&mut data, 52 * 380)
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        let mut players: Vec<Player> = (0..6)
            .map(|i| Player {
                reference: 0x10010000 + i * 0x2000,
                bytes: take(&mut data, 0x1e50).to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let mut world = World {
            image: &image,
            categories: &categories,
            buildings: &buildings,
            players: &mut players,
            loan_status: status,
        };
        let result = evaluate(&mut world, slot, auction, category).unwrap();
        for (i, p) in players.iter().enumerate() {
            assert_eq!(
                &p.bytes[..],
                take(&mut data, 0x1e50),
                "case {case}, player {i}"
            );
        }
        assert_eq!(
            result.to_f32().to_bits(),
            word(take(&mut data, 4), 0),
            "case {case} return"
        );
    }
    assert!(data.is_empty());
}
