//! Original4750b0 liquidation score and its business/rent inputs.
use crate::{
    ai_developed_targets,
    ai_strategy_queries::visit_chain,
    player_manager::{Player, word},
    x87::Sum,
};
pub struct World<'a> {
    pub developed: ai_developed_targets::World<'a>,
    pub mode: i32,
    pub cash_statuses: &'a [u8; 6],
    pub auction_active: bool,
}
///462320 selects the preceding four-period ledger slot.
pub fn recent_income(b: &[u8; 544]) -> f32 {
    f32::from_bits(word(
        b,
        0x58 + usize::from(b[0x209].wrapping_add(3) & 3) * 4,
    ))
}
///463160 retains PC64 through history, building capital and expected profit.
pub fn business_base_value(
    b: &[u8; 544],
    buildings: &[[u8; 916]],
    image: &crate::pe::PeImage<'_>,
) -> Result<Sum, String> {
    let id = i16::from_le_bytes(b[8..10].try_into().unwrap());
    let building = buildings
        .get(id as usize)
        .ok_or("liquidation value building missing")?;
    let mut value = Sum::from_f32(recent_income(b))?;
    value.add_f32(f32::from_bits(word(building, 0x38)))?;
    let mut profit = Sum::from_f32(f32::from_bits(word(b, 0x20)))?;
    profit.mul(Sum::from_f64(f64::from_le_bytes(
        image.at(0x5250e8, 8)?.try_into().unwrap(),
    ))?);
    value.add(profit);
    Ok(value)
}
///4643e0 applies its sale multiplier to the unrounded463160 result.
pub fn business_value(
    b: &[u8; 544],
    buildings: &[[u8; 916]],
    image: &crate::pe::PeImage<'_>,
) -> Result<Sum, String> {
    let mut value = business_base_value(b, buildings, image)?;
    value.mul_f32(image.f32_at(0x525c50)?)?;
    Ok(value)
}
///46a5f0/46a670 separately traverse the chain, preserving their different -1 guards.
fn rent(player: &Player, category: u32, owned: bool, w: &World<'_>) -> Result<i32, String> {
    let mut total = 0i32;
    let row = w
        .developed
        .categories
        .get(category as usize)
        .ok_or("liquidation rent category missing")?;
    visit_chain(row, w.developed.buildings, |building| {
        let id = word(building, 0x1c) as i32;
        if !owned && id <= -1 {
            return Ok(());
        }
        let owner = crate::business_pool::owner(id, w.developed.businesses)
            .ok_or("native liquidation rent invalid business")?;
        // The owned traversal reads owner zero for -1. AI owner IDs are
        // positive, so an unfinished building contributes no owned rent.
        if (owner == i32::from(player.bytes[0x1bc]) + 1) == owned {
            let b = &w.developed.businesses[id as usize];
            total = total.wrapping_add(i32::from(u16::from_le_bytes(
                b[0x18..0x1a].try_into().unwrap(),
            )));
        }
        Ok(())
    })?;
    Ok(total)
}
fn all_rent(player: &Player, id: u32, w: &World<'_>) -> Result<i32, String> {
    Ok(rent(player, id, false, w)?.wrapping_add(rent(player, id, true, w)?))
}
fn signed_magic(value: i32, multiplier: i32, shift: u32) -> i32 {
    let high = ((i64::from(value) * i64::from(multiplier)) >> 32) as i32;
    let q = high >> shift;
    q.wrapping_add(((q as u32) >> 31) as i32)
}
pub fn score(player: &mut Player, id: u32, kind: u32, w: &World<'_>) -> Result<i32, String> {
    let image = w.developed.image;
    let mut deficit = Sum::from_f32(image.f32_at(0x525058)?)?;
    deficit.add_f32(-f32::from_bits(word(&player.bytes, 4)))?;
    let deficit = deficit.ftol_low_word();
    let (value, income) = if kind == 1 {
        let c = w
            .developed
            .categories
            .get(id as usize)
            .ok_or("liquidation category missing")?;
        let mut value = crate::ai_messages::offers::execution::category_value(c, image)?;
        value.mul_f32(image.f32_at(0x52502c)?)?;
        (value.ftol_low_word(), all_rent(player, id, w)?)
    } else {
        let b = w
            .developed
            .businesses
            .get(id as usize)
            .ok_or("liquidation business missing")?;
        (
            business_value(b, w.developed.buildings, image)?.ftol_low_word(),
            Sum::from_f32(recent_income(b))?.ftol_low_word(),
        )
    };
    let value_component = if value >= deficit {
        700i32.wrapping_sub(value.wrapping_sub(deficit).min(500))
    } else {
        200i32.wrapping_sub(deficit.wrapping_sub(value).min(200))
    };
    let mut total = value_component.wrapping_sub(income.clamp(-800, 800));
    match kind {
        3 => {
            let category = word(&player.bytes, 0xb68) as i32;
            crate::consumer_poll::start(
                player,
                category,
                w.developed.categories,
                w.developed.buildings,
                w.developed.businesses,
                w.developed.residents,
                w.developed.commodity_active,
            )?;
            crate::consumer_poll::stop(player);
            let houses = u16::from_le_bytes(player.bytes[0x20a..0x20c].try_into().unwrap());
            crate::consumer_poll::clear(player);
            total = total.wrapping_add(
                if matches!(w.mode, 1 | 2) {
                    -i32::from(houses)
                } else {
                    value
                }
                .min(200),
            );
        }
        1 => {
            let hours = super::queries::trouble_hours(player, w.cash_statuses, w.developed.hour)?;
            if w.auction_active || (1..8).contains(&hours) {
                return Ok(0);
            }
            let property = word(&w.developed.categories[id as usize], 0x7c) as i32;
            let component = if property < 10 {
                all_rent(player, id, w)?
            } else if matches!(w.mode, 1 | 2) {
                property.wrapping_neg().wrapping_mul(2)
            } else {
                0
            };
            total = total.wrapping_add(component);
        }
        2 => {
            let count = u16::from_le_bytes(player.bytes[0xb44..0xb46].try_into().unwrap());
            let b = &w.developed.businesses[id as usize];
            let business_kind = word(b, 4);
            let recent = Sum::from_f32(recent_income(b))?.ftol_low_word();
            let mut penalty = if count < 6 {
                i32::from(count) * 10 - 60
            } else {
                0
            };
            let rows = if count == 0 {
                &[][..]
            } else {
                w.developed
                    .ownership
                    .lists
                    .get(&word(&player.bytes, 0xb3c))
                    .and_then(|v| v.get(..count as usize))
                    .ok_or("liquidation ownership list missing")?
            };
            let mut matches = 0;
            for row in rows {
                let id = word(row, 4) as i32;
                if id > -1
                    && word(
                        w.developed
                            .businesses
                            .get(id as usize)
                            .ok_or("liquidation owned business missing")?,
                        4,
                    ) == business_kind
                {
                    matches += 1;
                }
            }
            penalty = penalty.wrapping_add(if matches == 1 {
                signed_magic(recent, 0x99999999u32 as i32, 1)
            } else {
                ai_developed_targets::owned_kind(player, business_kind, &w.developed)?
                    .wrapping_mul(10)
                    .wrapping_sub(signed_magic(recent, 0x66666667, 2))
            });
            total = total.wrapping_add(penalty);
        }
        _ => {}
    }
    Ok(total)
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
    #[ignore = "requires original whole liquidation scoring observations"]
    fn matches_native_liquidation_score() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_LIQUIDATION_SCORE").unwrap())
            .unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTALSC02");
        assert_eq!(r.word(), 0x37f);
        let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let catalog = crate::business_pool::BusinessCatalog::from_pe(&image).unwrap();
        for i in 0..r.word() {
            let mode = r.word() as i32;
            let hour = r.word() as i32;
            let auction_active = r.word() != 0;
            let kind = r.word();
            let expected = r.word();
            let statuses = r.take(6).try_into().unwrap();
            let b: Vec<_> = (0..5).map(|_| r.blob()).collect();
            let a: Vec<_> = (0..5).map(|_| r.blob()).collect();
            let mut p = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![],
            };
            let ownership = crate::ai_ownership::Storage {
                lists: [(
                    0x10030000,
                    b[1].chunks_exact(28)
                        .map(|r| r.try_into().unwrap())
                        .collect(),
                )]
                .into(),
            };
            let businesses: Vec<[u8; 544]> = b[2]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            let buildings: Vec<[u8; 916]> = b[3]
                .chunks_exact(916)
                .map(|r| r.try_into().unwrap())
                .collect();
            let categories = b[4]
                .chunks_exact(380)
                .map(|r| r.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let residents = crate::economy::ResidentAccounts {
                active_count: 0,
                nodes: vec![],
                accounts: vec![],
            };
            let w = World {
                mode,
                cash_statuses: &statuses,
                auction_active,
                developed: ai_developed_targets::World {
                    image: &image,
                    profiles: &[],
                    hour,
                    categories: &categories,
                    buildings: &buildings,
                    businesses: &businesses,
                    catalog: &catalog,
                    ownership: &ownership,
                    residents: &residents,
                    commodity_active: &[false; 32],
                },
            };
            assert_eq!(
                score(&mut p, 0, kind, &w).unwrap() as u32,
                expected,
                "case{i} score"
            );
            assert_eq!(p.bytes, a[0], "case{i} player");
            for j in 1..5 {
                assert_eq!(b[j], a[j]);
            }
            if r.word() != 0 {
                p.bytes = r.take(0x1e50).to_vec();
                let prior = r.take(12).try_into().unwrap();
                let expected_player = r.take(0x1e50);
                let mut expected_rows = std::collections::BTreeMap::<u32, [u8; 12]>::new();
                for _ in 0..r.word() {
                    let address = r.word();
                    for (i, row) in r.blob().chunks_exact(12).enumerate() {
                        expected_rows.insert(address + i as u32 * 12, row.try_into().unwrap());
                    }
                }
                let mut traces = vec![];
                for _ in 0..r.word() {
                    let address = r.word();
                    let arg = r.word();
                    traces.push((address, arg, r.take(0x1e50)));
                }
                let mut rows = std::collections::BTreeMap::new();
                let mut calls = 0;
                let mut next = 0x10060000;
                super::super::liquidation_producer::produce(
                    &mut p,
                    &mut rows,
                    prior,
                    &w,
                    |call, p, _| {
                        use super::super::liquidation_pool::Call;
                        let (address, arg) = match call {
                            Call::Allocate(n) => (0x4e6350, n as u32),
                            Call::Release(a) => (0x4e6060, a),
                        };
                        let expected = traces[calls];
                        calls += 1;
                        assert_eq!(
                            (address, arg),
                            (expected.0, expected.1),
                            "case{i} allocator"
                        );
                        assert_eq!(p.bytes, expected.2, "case{i} allocator player");
                        let address = next;
                        if matches!(call, Call::Allocate(_)) {
                            next += 0x1000;
                        }
                        Ok(address)
                    },
                )
                .unwrap();
                assert_eq!(calls, traces.len());
                assert_eq!(p.bytes, expected_player, "case{i} producer player");
                assert_eq!(rows, expected_rows, "case{i} producer rows");
            }
        }
        assert!(r.0.is_empty());
    }
}
