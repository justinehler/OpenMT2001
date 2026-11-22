//! Original46e6a0 retail choice and its retained-world queries.
use crate::{
    ai_selection::Time,
    business_pool::BusinessCatalog,
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};
pub struct World<'a> {
    pub image: &'a PeImage<'a>,
    pub catalog: &'a BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub counts: &'a [u32; 52 * 45],
    pub demand: &'a [[u8; 264]; 33],
    pub residents: &'a crate::economy::ResidentAccounts,
    pub time: Time,
}
fn constant(w: &World<'_>, a: u32) -> Result<f32, String> {
    Ok(f32::from_le_bytes(w.image.at(a, 4)?.try_into().unwrap()))
}
fn compare(a: i32, b: i32) -> i32 {
    match a.cmp(&b) {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => -1,
    }
}
fn next(player: &mut Player) -> i32 {
    let v = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, v);
    ((v >> 16) & 0x7fff) as i32
}
fn category(player: &Player, w: &World<'_>) -> Result<usize, String> {
    let id = word(&player.bytes, 0xb68) as usize;
    if id >= w.categories.len() {
        return Err("AI business ranking category missing".into());
    }
    Ok(id)
}
fn day(kind: i32, w: &World<'_>) -> Result<bool, String> {
    Ok(w.catalog
        .row(kind as usize)
        .ok_or("AI ranking business catalog missing")?[0x5c]
        < 18)
}

///4715e0 selects the earliest eligible kind with the greatest product count;
///471580 fallback permits count0 but still compares its first product slot.
pub fn commodity_kind(commodity: u32, multiple: bool, w: &World<'_>) -> Result<i32, String> {
    let mut best = -1;
    let mut largest = 0;
    for (kind, row) in w.catalog.rows().iter().enumerate() {
        if kind == 44 || row[0x30] == 0 || word(row, 0x3c) == 0 || word(row, 0xc) != 0 {
            continue;
        }
        let count = usize::from(row[0x4c]);
        if multiple {
            let products = w.products[kind]
                .get(..count)
                .ok_or("AI ranking product count exceeds table")?;
            if count > largest
                && products
                    .iter()
                    .any(|p| p[0] != u32::MAX && p[0] == commodity)
            {
                largest = count;
                best = kind as i32;
            }
        } else if count <= 1 && w.products[kind][0][0] == commodity {
            return Ok(kind as i32);
        }
    }
    Ok(best)
}
fn kind_for(commodity: u32, w: &World<'_>) -> Result<i32, String> {
    let kind = commodity_kind(commodity, true, w)?;
    if kind >= 0 {
        Ok(kind)
    } else {
        commodity_kind(commodity, false, w)
    }
}
///477140 returns true even when no corresponding business exists.
pub fn absent_citywide(commodity: u32, w: &World<'_>) -> Result<bool, String> {
    let kind = kind_for(commodity, w)?;
    if kind <= -1 {
        return Ok(true);
    }
    if w.catalog.row(kind as usize).unwrap()[0x30] == 0 {
        return Ok(true);
    }
    Ok((0..52).all(|c| w.counts[c * 45 + kind as usize] as i32 <= 0))
}
///4773f0 first-day checks current stock; later days compare previous sales
///with capacity. Neither branch filters ownership or active flags.
pub fn expansion(category: usize, kind: i32, w: &World<'_>) -> Result<bool, String> {
    let mut stock = 0i32;
    let mut sold = 0i32;
    let mut capacity = 0i32;
    crate::ai_strategy_queries::visit_chain(&w.categories[category], w.buildings, |building| {
        let id = word(building, 0x1c) as i32;
        if id < 0 {
            return Ok(());
        }
        let row = w
            .businesses
            .get(id as usize)
            .ok_or("AI expansion business missing")?;
        if word(row, 4) as i32 != kind {
            return Ok(());
        }
        let info = w
            .catalog
            .row(kind as usize)
            .ok_or("AI expansion kind missing")?;
        if w.time.first_day() {
            for i in 0..usize::from(info[0x4c]) {
                stock = stock.wrapping_add(i32::from(u16::from_le_bytes(
                    row.get(0x80 + i * 52..0x82 + i * 52)
                        .ok_or("AI stock slot exceeds business")?
                        .try_into()
                        .unwrap(),
                )));
            }
        } else {
            if kind == 42 {
                sold = sold.wrapping_add(i32::from(u16::from_le_bytes(
                    row[0x12..0x14].try_into().unwrap(),
                )));
            } else {
                let slot = (usize::from(row[0x209]) + 3) % 4;
                for i in 0..6 {
                    if i < usize::from(info[0x4c]) && w.products[kind as usize][i][0] != u32::MAX {
                        sold = sold.wrapping_add(i32::from(u16::from_le_bytes(
                            row[0x82 + i * 52 + slot * 2..0x84 + i * 52 + slot * 2]
                                .try_into()
                                .unwrap(),
                        )));
                    }
                }
            }
            // The original adds two signed integers in x87 before FTOL.
            let mut sum = Sum::from_i32(crate::ai_demand::capacity(row, w.buildings, info)?);
            sum.add(Sum::from_i32(capacity));
            capacity = sum.ftol_low_word();
        }
        Ok(())
    })?;
    Ok(if w.time.first_day() {
        stock <= 0
    } else {
        sold >= capacity
    })
}
///470d90 subtracts each active business's capacity from housed demand. It
///rounds the accumulated commodity count to f32 before every subtraction.
pub fn unmet_housed_demand(w: &World<'_>) -> Result<i32, String> {
    let mut counts = [0i32; 32];
    for (i, v) in counts.iter_mut().enumerate() {
        *v = crate::ai_actions::demand::housed_demand(i as u32, w.residents, w.commodities)?;
    }
    for row in w.businesses {
        let kind = word(row, 4) as usize;
        if row[0x21c] == 0 || kind == 42 {
            continue;
        }
        let info = w
            .catalog
            .row(kind)
            .ok_or("AI housing demand business kind missing")?;
        let products = w.products[kind]
            .get(..usize::from(info[0x4c]))
            .ok_or("AI housing demand products exceed table")?;
        if products.is_empty() {
            continue;
        }
        let capacity = crate::ai_demand::capacity(row, w.buildings, info)?;
        for product in products {
            let target = counts
                .get_mut(product[0] as usize)
                .ok_or("AI housing demand commodity missing")?;
            let mut value = Sum::from_f32(Sum::from_i32(*target).to_f32())?;
            value.add(Sum::from_i32(capacity).negated());
            *target = value.ftol_low_word().max(0);
        }
    }
    Ok(counts
        .into_iter()
        .filter(|&n| n > 0)
        .fold(0i32, i32::wrapping_add))
}
///4771f0 housing gate, including both native RNG draws and first-day fallback.
pub fn housing_eligible(player: &mut Player, w: &World<'_>) -> Result<bool, String> {
    let homeless = w
        .residents
        .nodes
        .get(..w.residents.active_count as usize)
        .ok_or("AI housing resident prefix missing")?
        .iter()
        .filter(|r| {
            word(r.as_bytes(), 0) != 4
                && i16::from_le_bytes(r.as_bytes()[14..16].try_into().unwrap()) == -1
        })
        .count() as i32;
    let mut percentage = Sum::from_i32(homeless);
    percentage.mul_f32(constant(w, 0x525058)?)?;
    let percentage = if w.residents.active_count == 0 {
        0
    } else {
        percentage.div(Sum::from_i32(w.residents.active_count as i32))?;
        percentage.ftol_low_word()
    };
    let adjustment = crate::ai_profiles::adjustment(word(&player.bytes, 0x150))
        .wrapping_add(1000)
        .wrapping_mul(100);
    let mut chance = Sum::from_i32(adjustment);
    chance.mul_f32(constant(w, 0x525c94)?)?;
    let chance = chance.to_f32();
    if homeless <= 10 {
        return Ok(false);
    }
    if w.time.first_day()
        && next(player) % 101 < percentage
        && ((next(player) % 101) as f32) < chance
    {
        let mut count = 0;
        crate::ai_strategy_queries::visit_chain(
            &w.categories[category(player, w)?],
            w.buildings,
            |b| {
                if word(b, 0x24) == 1 {
                    count += 1;
                }
                Ok(())
            },
        )?;
        if count == 0 {
            return Ok(true);
        }
    }
    if unmet_housed_demand(w)? >= 250 || percentage <= 10 {
        return Ok(false);
    }
    let mut threshold = Sum::from_f32(chance)?;
    threshold.mul_f32(constant(w, 0x52506c)?)?;
    threshold.mul_f32(constant(w, 0x5250e0)?)?;
    threshold.add_f32(constant(w, 0x525064)?)?;
    let area_threshold = threshold.ftol_low_word().wrapping_mul(49) / 100;
    if f32::from_bits(word(&player.bytes, 4)).partial_cmp(&constant(w, 0x525ca0)?)
        != Some(std::cmp::Ordering::Greater)
        && f32::from_bits(word(&player.bytes, 4)) != constant(w, 0x525ca0)?
    {
        return Ok(false);
    }
    Ok(
        crate::ai_strategy_queries::occupied_area(
            &w.categories[category(player, w)?],
            w.buildings,
        )? >= area_threshold,
    )
}

///46e6a0. Only the independently reconstructed47c630 planner is a callback.
///Its status/cost outputs persist across successive probes, as in the stack.
pub fn rank(
    player: &mut Player,
    profiles: &[[u8; 0x650]],
    mode: i32,
    w: &World<'_>,
    mut plan: impl FnMut(&mut Player, &mut [u32; 2]) -> Result<bool, String>,
) -> Result<i32, String> {
    let mut primary = [[-1i32; 2]; 32];
    let mut rare = [[-1i32; 2]; 32];
    let mut secondary = [[-1i32; 2]; 45];
    let mut margins = [[0u32; 3]; 45];
    let (mut n, mut nr, mut ns) = (0usize, 0usize, 0usize);
    let mut outputs = [0u32; 2];
    let (mut blocked, mut funds) = (false, false);
    let mut cost = 9_999_999i32;
    let failed = |outputs: &[u32; 2], cost: &mut i32, blocked: &mut bool, funds: &mut bool| {
        *cost = (*cost).min(outputs[1] as i32);
        if outputs[0] == 0 {
            *blocked = true;
        } else if outputs[0] == 1 {
            *funds = true;
        }
    };
    for demand in w.demand {
        let commodity = word(demand, 0);
        let score = word(demand, 4) as i32;
        if score <= 0 {
            continue;
        }
        let kind = kind_for(commodity, w)?;
        if kind < 0 || kind == 43 {
            continue;
        }
        if (mode == 0 && !day(kind, w)?) || (mode == 1 && day(kind, w)?) {
            continue;
        }
        put(&mut player.bytes, 0xb90, kind as u32);
        if !plan(player, &mut outputs)? {
            failed(&outputs, &mut cost, &mut blocked, &mut funds);
            put(&mut player.bytes, 0xb90, u32::MAX);
            continue;
        }
        if (player.bytes[0xb58] != 0) == day(kind, w)? {
            let count = w.counts[category(player, w)? * 45 + kind as usize];
            if count == 0 {
                *primary
                    .get_mut(n)
                    .ok_or("AI ranking primary list overflow")? = [score, kind];
                n += 1;
                if !w.time.first_day() && absent_citywide(commodity, w)? {
                    *rare.get_mut(nr).ok_or("AI ranking rare list overflow")? = [score, kind];
                    nr += 1;
                }
            }
            if count == 1 && expansion(category(player, w)?, kind, w)? {
                secondary[ns] = [score, kind];
                ns += 1;
            }
        }
        let c = w
            .commodities
            .get(commodity as usize)
            .ok_or("AI ranking commodity missing")?;
        let price = f32::from_bits(word(c, 0x14));
        let multiplier = f32::from_bits(word(c, 0x18));
        if price.is_finite() && multiplier.is_finite() {
            let mut margin = Sum::from_f32(multiplier)?;
            margin.mul_f32(price)?;
            margin.add(Sum::from_f32(price)?.negated());
            if margin.compare(Sum::default()).is_gt() {
                margins[kind as usize][0] = kind as u32;
                margins[kind as usize][1] = margin.ftol_low_word() as u32;
            }
        }
    }
    crate::native_sort::sort(&mut margins, &|a, b| compare(a[1] as i32, b[1] as i32));
    let positive = margins.iter().filter(|r| r[1] as i32 > 0).count();
    if positive > 1 {
        let difference = (margins[0][1] as i32).wrapping_sub(margins[positive - 1][1] as i32);
        let mut divisor = Sum::from_i32(difference);
        divisor.add(Sum::from_f32(constant(w, 0x525034)?)?.negated());
        let mut multiplier = Sum::from_f32(constant(w, 0x52516c)?)?;
        let factor = if divisor.compare(Sum::default()).is_eq() {
            if difference == 1 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        } else {
            multiplier.div(divisor)?;
            multiplier.to_f32()
        };
        for i in 0..positive {
            let mut value = Sum::from_i32(i as i32);
            value.add_f32(constant(w, 0x525034)?)?;
            margins[positive - 1 - i][2] = if factor.is_finite() {
                value.mul_f32(factor)?;
                value.to_f32()
            } else {
                factor
            }
            .to_bits();
        }
        for row in &mut primary[..n] {
            for margin in &margins[..positive] {
                if row[1] as u32 == margin[0] {
                    let factor = f32::from_bits(margin[2]);
                    row[0] = if factor.is_finite() {
                        let mut value = Sum::from_i32(row[0]);
                        value.mul_f32(factor)?;
                        value.ftol_low_word()
                    } else {
                        crate::economy::ftol_low_word(f64::from(row[0]) * f64::from(factor))
                    };
                }
            }
        }
    }
    crate::native_sort::sort(&mut primary, &|a, b| compare(a[0], b[0]));
    crate::native_sort::sort(&mut secondary[..32], &|a, b| compare(a[0], b[0]));
    let factor = crate::ai_profiles::factor(w.image,0x47d360,word(&player.bytes,0x150))?;
    if n > 0 {
        let threshold =
            primary[0][0].wrapping_mul(crate::economy::ftol_low_word(f64::from(factor))) / 100;
        let old = n;
        for row in &mut primary[1..old] {
            if row[0] < threshold {
                *row = [0, -1];
                n -= 1;
            }
        }
    }
    for row in &rare[..nr] {
        if row[0] > 10
            && !primary[..n].iter().any(|p| p[1] == row[1])
            && w.counts[category(player, w)? * 45 + row[1] as usize] as i32 <= 0
        {
            *primary.get_mut(n).ok_or("AI ranking reinsert overflow")? = *row;
            n += 1;
        }
    }
    crate::native_sort::sort(&mut primary, &|a, b| compare(a[0], b[0]));
    if n > 0 {
        let profile = profiles
            .get(word(&player.bytes, 0x150) as usize)
            .ok_or("AI ranking profile missing")?;
        let preference = *profile
            .get(0x5ba + (word(&player.bytes, 0x13f4) as usize) * 36)
            .ok_or("AI ranking difficulty missing")?;
        let mut limit = Sum::from_i32(100 - i32::from(preference));
        limit.mul_f32(constant(w, 0x5250e0)?)?;
        limit.mul(Sum::from_i32(n as i32));
        let limit = limit.ftol_low_word().min(n as i32);
        let divisor = limit.wrapping_add(1);
        if divisor == 0 {
            return Err("AI ranking native division by zero".into());
        }
        let index = next(player) % divisor;
        return Ok(primary
            .get(index as usize)
            .ok_or("AI ranking selection leaves stack list")?[1]);
    }
    for row in &secondary[..ns] {
        put(&mut player.bytes, 0xb90, row[1] as u32);
        if plan(player, &mut outputs)? {
            let result = word(&player.bytes, 0xb90) as i32;
            put(&mut player.bytes, 0xb90, u32::MAX);
            return Ok(result);
        }
        failed(&outputs, &mut cost, &mut blocked, &mut funds);
        put(&mut player.bytes, 0xb90, u32::MAX);
    }
    if blocked {
        player.bytes[0xb78] = 1;
    }
    if funds {
        player.bytes[0xb77] = 1;
        put(&mut player.bytes, 0xb60, cost as u32);
    }
    if !blocked && !funds {
        player.bytes[0xb79] = 1;
    }
    Ok(-1)
}

#[test]
#[ignore = "requires original business-ranking observations"]
fn matches_native_business_ranking() {
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
    fn rows<const N: usize>(b: &[u8]) -> Vec<[u8; N]> {
        b.chunks_exact(N).map(|v| v.try_into().unwrap()).collect()
    }
    let data =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_BUSINESS_RANKING").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut r = Reader(&data);
    assert_eq!(r.take(8), b"MTBRNK01");
    assert_eq!(r.word(), 0x37f);
    let cases = r.word();
    let mut saved_population_cases = 0;
    let mut supplied_resident_cases = 0;
    for case in 0..cases {
        let h: [u32; 8] = std::array::from_fn(|_| r.word());
        if h[5] == 500 {
            assert!(matches!(h[1], 0x4771f0 | 0x470d90));
            saved_population_cases += 1;
        }
        if matches!(h[0], 28 | 29) {
            assert_eq!(h[5], 500);
            supplied_resident_cases += 1;
        }
        let blobs: Vec<_> = (0..12).map(|_| r.blob()).collect();
        let after = r.blob();
        let mut trace = vec![];
        for _ in 0..h[7] {
            let header: [u32; 5] = std::array::from_fn(|_| r.word());
            trace.push((header, r.blob(), r.blob()));
        }
        let mut player = Player {
            reference: 0x10010000,
            bytes: blobs[0].to_vec(),
            auxiliaries: vec![],
        };
        let profiles = rows::<0x650>(blobs[1]);
        let categories = rows::<380>(blobs[2]);
        let buildings = rows::<916>(blobs[3]);
        let businesses = rows::<544>(blobs[4]);
        let catalog = BusinessCatalog::from_rows(rows::<116>(blobs[5]).try_into().unwrap());
        let products = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                [
                    word(blobs[6], i * 48 + j * 8),
                    word(blobs[6], i * 48 + j * 8 + 4),
                ]
            })
        });
        let commodities = rows::<28>(blobs[7]);
        let demand = rows::<264>(blobs[8]);
        let counts = std::array::from_fn(|i| word(blobs[9], i * 4));
        let residents = crate::economy::ResidentAccounts {
            active_count: h[5],
            nodes: blobs[10]
                .chunks_exact(336)
                .map(|b| {
                    crate::resident::Resident::from_pool_bytes(
                        b[..0x14a].try_into().unwrap(),
                        b[0x14a],
                    )
                })
                .collect(),
            accounts: blobs[11]
                .chunks_exact(144)
                .map(|b| crate::resident::SpendingAccount::from_bytes(b.try_into().unwrap()))
                .collect(),
        };
        let w = World {
            image: &image,
            catalog: &catalog,
            products: &products,
            commodities: commodities.as_slice().try_into().unwrap(),
            categories: categories.as_slice().try_into().unwrap(),
            buildings: &buildings,
            businesses: &businesses,
            counts: &counts,
            demand: demand.as_slice().try_into().unwrap(),
            residents: &residents,
            time: Time {
                hour: h[3] as i32,
                year: h[4] as i32,
            },
        };
        let mut cursor = 0;
        let result = match h[1] {
            0x46e6a0 => rank(&mut player, &profiles, h[2] as i32, &w, |p, outputs| {
                let (header, before, after) = &trace[cursor];
                cursor += 1;
                assert_eq!(p.bytes, *before, "case{case} planplayer");
                assert_eq!(*outputs, [header[1], header[2]], "case{case} outputs");
                *outputs = [header[3], header[4]];
                p.bytes.copy_from_slice(after);
                Ok(header[0] != 0)
            })
            .unwrap() as u32,
            0x4771f0 => u32::from(housing_eligible(&mut player, &w).unwrap()),
            0x470d90 => unmet_housed_demand(&w).unwrap() as u32,
            0x471580 => commodity_kind(h[0] % 8, false, &w).unwrap() as u32,
            0x4715e0 => commodity_kind(h[0] % 8, true, &w).unwrap() as u32,
            0x477140 => u32::from(absent_citywide(h[0] % 8, &w).unwrap()),
            0x4773f0 => u32::from(expansion(0, (h[0] % 8) as i32, &w).unwrap()),
            _ => panic!("unknown ranking entry"),
        };
        assert_eq!(
            result, h[6],
            "case{case} seed{} entry{:x} result",
            h[0], h[1]
        );
        assert_eq!(player.bytes, after, "case{case} fullplayer");
        assert_eq!(cursor, trace.len(), "case{case} complete trace");
    }
    assert_eq!(saved_population_cases, 12, "500-resident native decisions missing");
    assert_eq!(supplied_resident_cases, 4, "supplied-resident native decisions missing");
    assert!(r.0.is_empty());
}
