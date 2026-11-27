//! Original41bf6c targeted-card effects over retained business/building pools.
use super::effects::{Notice, World};
use crate::{
    pe::PeImage,
    player_manager::{put, word},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    AccountPosted(crate::economy::AccountPosting),
    Sound {
        actor_is_current: bool,
    },
    BuildingAppearance {
        building: usize,
    },
    RefreshInterface,
    Message {
        from: usize,
        to: usize,
        building: i32,
    },
}
pub struct Context<'a, 'image> {
    pub image: &'a PeImage<'image>,
    pub catalog: &'a crate::business_pool::BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 0x1c]; 32],
    pub residents: &'a mut crate::economy::ResidentAccounts,
    pub tick: u32,
    pub accounting: bool,
    pub network: bool,
    pub network_override: bool,
}

fn business_at(world: &World<'_>, target: i32) -> Result<Option<usize>, String> {
    let b = world
        .buildings
        .get(target as usize)
        .ok_or("event target building outside pool")?;
    let slot = word(b, 0x1c);
    if slot == u32::MAX {
        return Ok(None);
    }
    if slot as usize >= world.businesses.len() {
        return Err("event target business outside pool".into());
    }
    Ok(Some(slot as usize))
}
fn promote(business: &mut [u8; 544], argument: u32) {
    put(business, 0x78, argument);
    business[0x74] = 24;
    put(
        business,
        0x7c,
        match argument {
            1 => 0x3f99999a,
            2 => 0x3fcccccd,
            _ => 0x40000000,
        },
    );
}
fn chain(world: &World<'_>, target: i32) -> Result<Vec<usize>, String> {
    let category = world
        .categories
        .get(target as usize)
        .ok_or("event target category outside pool")?;
    let mut reference = word(category, 0x15c);
    let mut result = vec![];
    while reference != 0 {
        let offset = reference.wrapping_sub(0x591318);
        if !offset.is_multiple_of(0x394) {
            return Err("event category building unaligned".into());
        }
        let slot = (offset / 0x394) as usize;
        let building = world
            .buildings
            .get(slot)
            .ok_or("event category building outside pool")?;
        if result.len() >= world.buildings.len() {
            return Err("event category building cycle".into());
        }
        result.push(slot);
        reference = word(building, 0x44);
    }
    Ok(result)
}
/// Returns false only for subtype8's missing-business early return, which
/// bypasses the original notification tail entirely.
pub fn apply(
    kind: u32,
    target: i32,
    actor: usize,
    world: &mut World<'_>,
    context: &mut Context<'_, '_>,
    notice: &mut Notice,
    mut callback: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<bool, String> {
    callback(
        Call::Sound {
            actor_is_current: actor == world.current,
        },
        world,
    )?;
    match kind {
        0 => {
            let b = world
                .businesses
                .get_mut(target as usize)
                .ok_or("event shutdown business outside pool")?;
            let state = if word(b, 0) == 1 { 0 } else { 2 };
            put(b, 0x6c, state);
            if i32::from(b[0xa] as i8) == world.current as i32 + 1 {
                notice.affected = true;
            }
        }
        1 | 5 => {
            for slot in chain(world, target)? {
                if let Some(business) = business_at(world, slot as i32)? {
                    promote(
                        &mut world.businesses[business],
                        if kind == 1 { 1 } else { 3 },
                    );
                }
            }
        }
        2 => {
            if let Some(business) = business_at(world, target)? {
                promote(&mut world.businesses[business], 2);
            }
        }
        3 | 4 => {
            if let Some(slot) = business_at(world, target)? {
                let b = &mut world.businesses[slot];
                let business_type = word(b, 4) as usize;
                let info = context
                    .catalog
                    .row(business_type)
                    .ok_or("event stock business type outside catalog")?;
                let count = usize::from(info[0x4c]);
                if count > 6 {
                    return Err("event stock catalog exceeds six slots".into());
                }
                if kind == 4 {
                    for index in 0..count {
                        b[0x80 + index * 0x34..0x82 + index * 0x34].fill(0);
                    }
                    put(b, 0x218, 0);
                    if i32::from(b[0xa] as i8) == world.current as i32 + 1 {
                        notice.affected = true;
                    }
                } else {
                    let building = i16::from_le_bytes(b[8..10].try_into().unwrap());
                    let building = world
                        .buildings
                        .get(building as usize)
                        .ok_or("event restock building outside pool")?;
                    let area = (i32::from(building[6]) - i32::from(building[4]) + 1)
                        .wrapping_mul(i32::from(building[7]) - i32::from(building[5]) + 1);
                    let capacity = crate::economy::stock_capacity(
                        area,
                        building[8],
                        f32::from_bits(word(info, 0x58)),
                    );
                    let mut prices = vec![];
                    for product in &context.products[business_type][..count] {
                        let commodity = context
                            .commodities
                            .get(product[0] as usize)
                            .ok_or("event restock commodity outside table")?;
                        prices.push(f32::from_bits(word(commodity, 0x14)));
                    }
                    let owner = if b[0xa] == 0 {
                        None
                    } else {
                        Some(
                            world
                                .accounts
                                .get_mut((i32::from(b[0xa] as i8) - 1) as usize)
                                .ok_or("event restock owner outside roster")?,
                        )
                    };
                    crate::economy::replenish_business_record(
                        b,
                        &prices,
                        capacity,
                        word(info, 0x70),
                        true,
                        owner,
                    )?;
                }
            }
        }
        6 => {
            let count = (context.residents.active_count as i32).max(0) as usize;
            for resident in context
                .residents
                .nodes
                .get_mut(..count)
                .ok_or("event resident count outside pool")?
            {
                resident.reset_category_counter(target)?;
            }
        }
        7 => {
            for slot in chain(world, target)? {
                put(&mut world.buildings[slot], 0x30, 0xfff0bdc1);
            }
        }
        8 => {
            let Some(slot) = business_at(world, target)? else {
                return Ok(false);
            };
            let business = &world.businesses[slot];
            let owner = business[0xa] as i8;
            let prior = (usize::from(business[0x209]) + 3) % 4;
            let mut value =
                crate::x87::Sum::from_f32(f32::from_bits(word(business, 0x58 + prior * 4)))?;
            let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
            value.add_f32(f32::from_bits(word(
                world
                    .buildings
                    .get(building as usize)
                    .ok_or("event valuation building outside pool")?,
                0x38,
            )))?;
            let half = f64::from_le_bytes(context.image.at(0x5250e8, 8)?.try_into().unwrap());
            let mut extra = crate::x87::Sum::from_f32(f32::from_bits(word(business, 0x20)))?;
            extra.mul(crate::x87::Sum::from_f64(half)?);
            value.add(extra);
            value.mul(crate::x87::Sum::from_f64(half)?);
            let price = value.to_f32();
            if owner != 0 {
                world
                    .accounts
                    .get_mut((i32::from(owner) - 1) as usize)
                    .ok_or("event buyout owner outside roster")?
                    .post(price, 0x16)?;
                callback(
                    Call::AccountPosted(crate::economy::AccountPosting {
                        player: (i32::from(owner) - 1) as usize,
                        amount: price,
                    }),
                    world,
                )?;
            }
            world.accounts[actor].post(-price, 3)?;
            callback(
                Call::AccountPosted(crate::economy::AccountPosting {
                    player: actor,
                    amount: -price,
                }),
                world,
            )?;
            let prefix = world.buildings[building as usize][..64].try_into().unwrap();
            crate::economy::transfer_business_owner(
                &mut world.businesses[slot],
                prefix,
                (actor as u8).wrapping_add(1),
                context.accounting,
                world.accounts,
            )?;
            let b = &mut world.buildings[target as usize];
            b[0x2d] = (actor as u8).wrapping_add(1);
            put(b, 0x30, context.tick);
            if word(b, 0x4c) == 1 && (word(b, 0x1c) == u32::MAX || matches!(word(b, 0x24), 0 | 1)) {
                callback(
                    Call::BuildingAppearance {
                        building: target as usize,
                    },
                    world,
                )?;
            }
            world.counts.age = world.counts.age.wrapping_add(100);
            callback(Call::RefreshInterface, world)?;
            if (!context.network || context.network_override) && owner != 0 {
                callback(
                    Call::Message {
                        from: (i32::from(owner) - 1) as usize,
                        to: actor,
                        building: target,
                    },
                    world,
                )?;
            }
        }
        _ => (),
    }
    Ok(true)
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
    #[ignore = "requires original41bdf0 targeted effect observations"]
    fn matches_native_targeted_effects() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_EVENT_TARGETED")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTECTG01");
        assert_eq!(r.word(), 2);
        assert_eq!(r.word(), 0x037f);
        for _ in 0..r.word() {
            let [kind, variant, ui] = std::array::from_fn::<_, 3, _>(|_| r.word());
            let mut players: Vec<Vec<u8>> =
                r.blob().chunks_exact(0x600).map(|b| b.to_vec()).collect();
            let mut buildings: Vec<[u8; 0x394]> = r
                .blob()
                .chunks_exact(0x394)
                .map(|b| b.try_into().unwrap())
                .collect();
            let mut businesses: Vec<[u8; 544]> = r
                .blob()
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            let categories: Vec<[u8; 0x17c]> = r
                .blob()
                .chunks_exact(0x17c)
                .map(|b| b.try_into().unwrap())
                .collect();
            let mut resident_records = r.blob().to_vec();
            let mut residents = crate::economy::ResidentAccounts {
                active_count: 4,
                nodes: resident_records
                    .chunks_exact(0x150)
                    .map(|b| {
                        crate::resident::Resident::from_runtime_bytes(
                            b[..0x14a].try_into().unwrap(),
                            b[0x14a],
                            word(b, 0x14c),
                        )
                    })
                    .collect(),
                accounts: vec![],
            };
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(0x74)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let raw_products = r.blob();
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [
                        word(raw_products, i * 48 + j * 8),
                        word(raw_products, i * 48 + j * 8 + 4),
                    ]
                })
            });
            let commodities = r
                .blob()
                .chunks_exact(0x1c)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<[u8; 0x1c]>>()
                .try_into()
                .unwrap();
            let expected_players = r.blob();
            let expected_buildings = r.blob();
            let expected_businesses = r.blob();
            let expected_categories = r.blob();
            let expected_residents = r.blob();
            let expected_flags = r.blob();
            let expected_notice = r.blob();
            let expected_age = r.blob()[0];
            let expected_trace: Vec<[u32; 10]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut deck = super::super::Deck::new([1, 0x343fd, 0x269ec3]);
            deck.used.fill(1);
            let mut accounts: Vec<_> = players
                .iter()
                .map(|p| crate::economy::PlayerAccount::from_runtime(p).unwrap())
                .collect();
            let active: Vec<_> = players.iter().map(|p| p[0x10]).collect();
            let mut counts = crate::player_metrics::BusinessCounts {
                age: 240,
                values: [0; 6],
            };
            let mut trace = vec![];
            let notice = super::super::effects::apply(
                &super::super::lifecycle::Command {
                    card: 65 + kind,
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
                    current: (variant % 3) as usize,
                },
                &image,
                |call, world, notice| {
                    let super::super::effects::Call::Target {
                        kind,
                        target,
                        player,
                    } = call
                    else {
                        panic!("unexpected utility")
                    };
                    apply(
                        kind,
                        target,
                        player,
                        world,
                        &mut Context {
                            image: &image,
                            catalog: &catalog,
                            products: &products,
                            commodities: &commodities,
                            residents: &mut residents,
                            tick: 0xffffffe7,
                            accounting: variant != 4,
                            network: variant == 4,
                            network_override: false,
                        },
                        notice,
                        |call, _| {
                            trace.push(match call {
                                Call::AccountPosted(_) => return Ok(()),
                                Call::Sound { actor_is_current } => [
                                    0x4ad760,
                                    0x8af288,
                                    if actor_is_current { 0x5349ec } else { 0x5349fc },
                                    1,
                                    0,
                                    0,
                                    0,
                                    100,
                                    0,
                                    0,
                                ],
                                Call::BuildingAppearance { building } => [
                                    0x485520,
                                    0x591318 + building as u32 * 0x394,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                ],
                                Call::RefreshInterface => [0x41eb00, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                                Call::Message { from, to, building } => [
                                    0x48cab0,
                                    0x86d1f0,
                                    from as u32,
                                    to as u32,
                                    0x2e,
                                    building as u32,
                                    0,
                                    0,
                                    0,
                                    (-999i32) as u32,
                                ],
                            });
                            Ok(())
                        },
                    )
                },
            )
            .unwrap_or_else(|e| panic!("kind{kind} variant{variant}: {e}"));
            for (p, a) in players.iter_mut().zip(accounts) {
                a.write_runtime(p).unwrap();
            }
            for (record, resident) in resident_records
                .chunks_exact_mut(0x150)
                .zip(&residents.nodes)
            {
                record[..0x14a].copy_from_slice(resident.as_bytes());
            }
            assert_eq!(
                players.into_iter().flatten().collect::<Vec<_>>(),
                expected_players,
                "players kind{kind} variant{variant}"
            );
            assert_eq!(
                buildings.into_iter().flatten().collect::<Vec<_>>(),
                expected_buildings,
                "buildings kind{kind} variant{variant}"
            );
            assert_eq!(
                businesses.into_iter().flatten().collect::<Vec<_>>(),
                expected_businesses,
                "businesses kind{kind} variant{variant}"
            );
            assert_eq!(
                categories.into_iter().flatten().collect::<Vec<_>>(),
                expected_categories,
                "categories kind{kind} variant{variant}"
            );
            assert_eq!(
                resident_records, expected_residents,
                "residents kind{kind} variant{variant}"
            );
            assert_eq!(deck.used, expected_flags);
            assert_eq!(counts.age, expected_age);
            assert_eq!(notice.enabled, ui != 0, "UI kind{kind} variant{variant}");
            assert_eq!(
                notice.affected,
                expected_notice[0] != 0,
                "affected kind{kind} variant{variant}"
            );
            assert_eq!(notice.amount.to_bits(), word(expected_notice, 4));
            assert_eq!(trace, expected_trace, "calls kind{kind} variant{variant}");
        }
        assert!(r.0.is_empty());
    }
}
