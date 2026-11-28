//! Original468fc0: rank competitors' businesses on the player's property.
use crate::{
    player_manager::{Player, word},
    x87::Sum,
};
pub struct World<'a> {
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub players: &'a [&'a Player],
    pub cash_statuses: &'a [u8; 6],
    pub time: crate::ai_selection::Time,
    pub image: &'a crate::pe::PeImage<'a>,
}
fn troubled(p: &Player, w: &World<'_>) -> Result<bool, String> {
    Ok(*w
        .cash_statuses
        .get(p.bytes[0x1bc] as usize)
        .ok_or("takeover cash status missing")?
        != 0)
}
/// The last retail score deliberately survives a non-retail business, and
/// ties use the original CRT sort. Both affect which business is purchased.
pub fn select(
    player: &Player,
    w: &World<'_>,
    mut quote: impl FnMut(usize) -> Result<Sum, String>,
) -> Result<Option<u32>, String> {
    if troubled(player, w)?
        || w.time.first_day()
        || crate::ai_strategy_queries::owned_categories(player)? == 0
    {
        return Ok(None);
    }
    let cash = f32::from_bits(word(&player.bytes, 4));
    if cash.is_nan() || cash < w.image.f32_at(0x525040)? {
        return Ok(None);
    }
    let mut candidates = vec![];
    let mut last_score = 0i32;
    for (id, category) in w.categories.iter().enumerate() {
        if player.bytes[0x178 + id] == 0 {
            continue;
        }
        crate::ai_strategy_queries::visit_chain(category, w.buildings, |building| {
            let business = word(building, 0x1c) as i32;
            if business < 0 {
                return Ok(());
            }
            let row = w
                .businesses
                .get(business as usize)
                .ok_or("takeover business missing")?;
            let owner = i32::from(row[10] as i8);
            if owner == i32::from(player.bytes[0x1bc]) + 1 {
                return Ok(());
            }
            let mut income =
                Sum::from_f32(crate::ai_actions::liquidation_score::recent_income(row))?
                    .ftol_low_word();
            let slot = w
                .buildings
                .iter()
                .position(|row| std::ptr::eq(row, building))
                .ok_or("takeover chain building outside pool")?;
            let price = quote(slot)?.ftol_low_word();
            let available = if owner > 0 {
                !troubled(
                    w.players
                        .get((owner - 1) as usize)
                        .ok_or("takeover owner missing")?,
                    w,
                )?
            } else {
                true
            };
            if word(row, 0) == 0 {
                let monopoly =
                    crate::ai_actions::expansion::complete_group(id as u32, w.categories, w.image)?;
                if monopoly {
                    income = income.wrapping_add(income);
                }
                last_score = (10i32 + i32::from(available) + i32::from(monopoly))
                    .wrapping_mul(100)
                    .wrapping_sub(price)
                    .wrapping_add(income);
            }
            if last_score >= 800 && candidates.len() < 255 {
                candidates.push((business as u32, Sum::from_i32(last_score).to_f32()));
            }
            Ok(())
        })?;
    }
    crate::native_sort::sort(&mut candidates, &|a, b| {
        if a.1 < b.1 {
            1
        } else if a.1 > b.1 {
            -1
        } else {
            0
        }
    });
    Ok(candidates.first().map(|r| r.0))
}

#[test]
#[ignore = "requires whole original468fc0 takeover decisions"]
fn matches_native_takeovers() {
    struct R<'a>(&'a [u8]);
    impl<'a> R<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            word(self.take(4), 0)
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word();
            self.take(n as usize)
        }
    }
    fn rows<const N: usize>(b: &[u8]) -> Vec<[u8; N]> {
        b.chunks_exact(N).map(|r| r.try_into().unwrap()).collect()
    }
    let data = std::fs::read(crate::fixtures::path("NATIVE_AI_TAKEOVER")).unwrap();
    let mut r = R(&data);
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = crate::pe::PeImage::parse(&exe).unwrap();
    assert_eq!(r.take(8), b"MTAITAK1");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 6] = std::array::from_fn(|_| r.word());
        let statuses = r.take(6).try_into().unwrap();
        let b: Vec<_> = (0..9).map(|_| r.blob()).collect();
        let packets: Vec<_> = (0..h[4]).map(|_| r.word()).collect();
        let calls: Vec<_> = (0..h[5]).map(|_| r.word()).collect();
        let players: Vec<_> = b[0]
            .chunks_exact(0x2000)
            .enumerate()
            .map(|(i, b)| Player {
                reference: 0x10010000 + i as u32 * 0x2000,
                bytes: b.to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let buildings = rows::<916>(b[4]);
        let businesses = rows::<544>(b[3]);
        let categories = rows::<380>(b[5]);
        let references: Vec<_> = players.iter().map(|p| p.reference).collect();
        let peers: Vec<_> = players.iter().collect();
        let w = World {
            categories: categories.as_slice().try_into().unwrap(),
            buildings: &buildings,
            businesses: &businesses,
            players: &peers,
            cash_statuses: statuses,
            time: crate::ai_selection::Time {
                year: h[1] as i32,
                hour: h[2] as i32,
            },
            image: &image,
        };
        let price = crate::business_takeover::PriceWorld {
            categories: w.categories,
            buildings: &buildings,
            businesses: &businesses,
            player_references: &references,
        };
        let mut actual_calls = vec![];
        let result = select(&players[h[0] as usize], &w, |id| {
            actual_calls.push(0x591318 + id as u32 * 916);
            crate::business_takeover::price(id, &price, h[3], &image)
        })
        .unwrap();
        assert_eq!(actual_calls, calls, "case{case} price calls");
        let selected: Vec<_> = result
            .map(|id| {
                u32::from(u16::from_le_bytes(
                    businesses[id as usize][8..10].try_into().unwrap(),
                ))
            })
            .into_iter()
            .collect();
        assert_eq!(selected, packets, "case{case} purchase target");
        let mut player = Player {
            reference: players[h[0] as usize].reference,
            bytes: players[h[0] as usize].bytes.clone(),
            auxiliaries: vec![],
        };
        let mut storage = crate::ai_ownership::Storage::default();
        storage.lists.insert(0x10030000, rows::<28>(b[1]));
        storage.lists.insert(0x10031000, rows::<28>(b[2]));
        if let Some(business) = result {
            storage
                .mutate(
                    &mut player,
                    |_| Err("unexpected takeover allocation".into()),
                    |state| {
                        state.insert(0, business as i32, |id| {
                            crate::ai_ownership::live_entry_value(
                                id,
                                &businesses,
                                &buildings,
                                w.categories,
                                &image,
                            )
                        })?;
                        state.lists[1].remove(business as i32);
                        Ok(())
                    },
                )
                .unwrap();
        }
        for (i, (a, z)) in player
            .bytes
            .iter()
            .zip(&b[6][h[0] as usize * 0x2000..(h[0] as usize + 1) * 0x2000])
            .enumerate()
        {
            assert_eq!(a, z, "case{case} player byte{i:x}");
        }
        for (address, expected) in [(0x10030000, b[7]), (0x10031000, b[8])] {
            for (i, (a, z)) in storage.lists[&address]
                .iter()
                .flatten()
                .zip(expected)
                .enumerate()
            {
                assert_eq!(a, z, "case{case} list{address:x} byte{i:x}");
            }
        }
    }
    assert!(r.0.is_empty());
}
