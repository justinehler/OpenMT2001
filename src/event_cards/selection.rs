//! Original46f500..4701d3 card-target selection, including50fab1 candidate sort.
use super::ai::Choice;
use crate::{
    business_pool::BusinessCatalog,
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};

pub struct World<'a> {
    pub buildings: &'a [[u8; 0x394]],
    pub businesses: &'a [[u8; 544]],
    pub categories: &'a [[u8; 0x17c]],
    pub catalog: &'a BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 0x1c]; 32],
    /// Live telecom, gas, water, electricity enable bytes.
    pub utilities: [u8; 4],
    pub hour: i32,
    pub tick: u32,
    /// Original signed word at7a3f90, read when no candidate was accepted.
    pub unselected_building: i16,
}
fn short(bytes: &[u8], at: usize) -> i32 {
    i32::from(i16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}
fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}
fn ext(v: f32) -> Result<Sum, String> {
    Ok(Sum::from_f32(v)?)
}
fn greater(v: Sum, b: f32) -> Result<bool, String> {
    Ok(!b.is_nan() && v.compare(ext(b)?).is_gt())
}
fn prior(b: &[u8]) -> f32 {
    float(b, 0x58 + 4 * ((usize::from(b[0x209]) + 3) % 4))
}
fn blocked(category: usize, image: &PeImage<'_>) -> Result<bool, String> {
    Ok(matches!(
        image.at(0x5385a4 + category as u32 * 88, 1)?[0],
        0 | 10 | 11
    ))
}
impl World<'_> {
    fn chain(&self, category: usize) -> Result<Vec<(usize, usize)>, String> {
        let c = self
            .categories
            .get(category)
            .ok_or("card selection category outside pool")?;
        let mut reference = word(c, 0x15c);
        let mut result = vec![];
        let mut visited = 0;
        while reference != 0 {
            let offset = reference.wrapping_sub(0x591318);
            if !offset.is_multiple_of(0x394) {
                return Err("card selection building unaligned".into());
            }
            let at = (offset / 0x394) as usize;
            let b = self
                .buildings
                .get(at)
                .ok_or("card selection building outside pool")?;
            let business = word(b, 0x1c) as i32;
            if business > -1 {
                if business as usize >= self.businesses.len() {
                    return Err("card selection business outside pool".into());
                }
                result.push((at, business as usize));
            }
            reference = word(b, 0x44);
            visited += 1;
            if visited > self.buildings.len() {
                return Err("card selection building cycle".into());
            }
        }
        Ok(result)
    }
    fn volume(&self, b: &[u8]) -> Result<i32, String> {
        let b = self
            .buildings
            .get(short(b, 8) as usize)
            .ok_or("card stock building outside pool")?;
        Ok((i32::from(b[7]) - i32::from(b[5]) + 1)
            .wrapping_mul(i32::from(b[6]) - i32::from(b[4]) + 1)
            .wrapping_mul(i32::from(b[8])))
    }
    fn stock(&self, b: &[u8]) -> Result<Sum, String> {
        let kind = word(b, 4) as usize;
        let info = self
            .catalog
            .row(kind)
            .ok_or("card stock catalog unresolved")?;
        let rows = self
            .products
            .get(kind)
            .ok_or("card stock products unresolved")?;
        let count = usize::from(info[0x4c]);
        if count > 6 {
            return Err("card stock count exceeds catalog".into());
        }
        let mut units = Sum::from_i32(self.volume(b)?);
        units.mul_f32(float(info, 0x58))?;
        let units = units.ftol_low_word();
        let mut sum = Sum::default();
        for row in &rows[..count] {
            sum.add(ext(f32::from_bits(row[1]))?);
        }
        if sum.compare(Sum::default()).is_eq() {
            return Ok(Sum::default());
        }
        let mut scale = Sum::from_i32(1);
        scale.div(sum)?;
        scale.mul(Sum::from_i32(units));
        let mut total = Sum::default();
        for row in &rows[..count] {
            let c = self
                .commodities
                .get(row[0] as usize)
                .ok_or("card stock commodity outside catalog")?;
            let mut value = ext(float(c, 0x14))?;
            value.mul_f32(f32::from_bits(row[1]))?;
            value.mul(scale);
            total.add(value);
        }
        Ok(total)
    }
    fn utility_cost(&self, p: &Player, b: &[u8]) -> Result<f32, String> {
        let info = self
            .catalog
            .row(word(b, 4) as usize)
            .ok_or("card utility catalog unresolved")?;
        let volume = Sum::from_i32(self.volume(b)?).to_f32();
        let mut parts = [Sum::default(); 4];
        for (i, category) in [2, 35, 39, 42].into_iter().enumerate() {
            if p.bytes[0x178 + category] == 0 && self.utilities[i] != 0 {
                parts[i] = ext(volume)?;
                parts[i].mul_f32(float(info, 0x60 + 4 * i))?;
                if i < 3 {
                    parts[i] = ext(parts[i].to_f32())?;
                }
            }
        }
        let mut sum = parts[3];
        for i in [2, 1, 0] {
            sum.add(parts[i]);
        }
        Ok(sum.to_f32())
    }
    fn group(&self, category: usize, image: &PeImage<'_>) -> Result<Vec<usize>, String> {
        let color = image.at(0x5385a4 + category as u32 * 88, 1)?[0];
        let mut group = vec![];
        for i in 0..4 {
            let id = word(image.at(0x5397ac + u32::from(color) * 60 + i * 4, 4)?, 0) as i32;
            if id != -1 {
                if id < 0 || id as usize >= self.categories.len() {
                    return Err("card monopoly category outside pool".into());
                }
                group.push(id as usize);
            }
        }
        Ok(group)
    }
    fn monopoly(&self, category: usize, image: &PeImage<'_>) -> Result<bool, String> {
        let owner = self.categories[category][5];
        if owner == 0 {
            return Ok(false);
        }
        Ok(self
            .group(category, image)?
            .into_iter()
            .all(|i| self.categories[i][5] == owner))
    }
    fn rent_age(&self, building: usize, image: &PeImage<'_>) -> Result<i32, String> {
        crate::business_report::rent_age(
            building,
            self.buildings,
            self.businesses,
            self.categories,
            self.tick,
            image,
        )
    }
}
/// The comparator intentionally treats any NaN as equal. All50/100 slots,
/// including untouchedFFFFFFFF padding, participate in the original quicksort.
fn compare(a: [u32; 2], b: [u32; 2]) -> i32 {
    let (a, b) = (f32::from_bits(a[1]), f32::from_bits(b[1]));
    if b > a {
        1
    } else if b < a {
        -1
    } else {
        0
    }
}
fn sort(rows: &mut [[u32; 2]]) {
    crate::native_sort::sort(rows, &compare);
}

fn finish(
    rows: &[[u32; 2]],
    cursor: usize,
    p: &mut Player,
    world: &World<'_>,
    out: &mut Choice,
) -> Result<(), String> {
    let selected = if cursor == 0 {
        u32::MAX
    } else {
        let random = if cursor >= 2 {
            let state = word(&p.bytes, 0x224)
                .wrapping_mul(word(&p.bytes, 0x228))
                .wrapping_add(word(&p.bytes, 0x22c));
            put(&mut p.bytes, 0x224, state);
            (((state >> 16) & 0x7fff) % cursor.min(3) as u32) as usize
        } else {
            0
        };
        // FLD/FSTP quiets the signaling all-ones padding if it reaches the top.
        out.score = f32::from_bits(rows[0][1]);
        rows[random][0]
    };
    out.target = if selected == u32::MAX {
        i32::from(world.unselected_building)
    } else {
        short(
            world
                .businesses
                .get(selected as usize)
                .ok_or("card chosen business outside pool")?,
            8,
        )
    };
    Ok(())
}

pub fn choose(
    kind: u32,
    p: &mut Player,
    world: &World<'_>,
    image: &PeImage<'_>,
) -> Result<Choice, String> {
    let mut out = Choice::default();
    let owner = i32::from(p.bytes[0x1bc]) + 1;
    let mut best = if matches!(kind, 5 | 6) {
        f32::from_bits(0xc7c34f80)
    } else {
        0.
    };
    let mut best_rent = 0i32;
    let mut own_seen = false;
    let mut cursor = 0;
    let mut rows = vec![[u32::MAX; 2]; if kind == 8 { 50 } else { 100 }];
    for category in 0..52 {
        if if kind == 8 {
            world.monopoly(category, image)? && blocked(category, image)?
        } else {
            blocked(category, image)?
        } {
            continue;
        }
        if kind == 7 && p.bytes[0x178 + category] == 0 {
            continue;
        }
        let mut category_score = 0.;
        let mut rent = 0i32;
        if kind == 7 {
            out.count = out.count.wrapping_add(1);
        }
        for (building, business) in world.chain(category)? {
            let b = &world.businesses[business];
            let same = i32::from(b[0xa] as i8) == owner;
            if matches!(kind, 5 | 6) {
                if same {
                    own_seen = true;
                } else {
                    let mut score = ext(prior(b))?;
                    score.add(ext(category_score)?);
                    category_score = score.to_f32();
                }
                continue;
            }
            if kind == 1 {
                out.count = out.count.wrapping_add(1);
                let mut score = ext(float(b, 0x20))?;
                score.add(ext(prior(b))?.negated());
                score.add(ext(category_score)?);
                category_score = score.to_f32();
                continue;
            }
            if kind == 7 {
                if !same {
                    rent = rent.wrapping_add(world.rent_age(building, image)?);
                }
                continue;
            }
            let info = world
                .catalog
                .row(word(b, 4) as usize)
                .ok_or("card selection catalog unresolved")?;
            if matches!(kind, 0 | 4) {
                if (info[0x5c] < 18) != (world.hour <= 9) || same || b[0xa] == 0 {
                    continue;
                }
            } else if matches!(kind, 2 | 3) {
                if !same || (kind == 3 && (info[0x5c] < 18) != (world.hour <= 14)) {
                    continue;
                }
            } else if kind == 8 {
                if same {
                    continue;
                }
            } else {
                return Err("unknown event selector".into());
            }
            out.count = out.count.wrapping_add(1);
            let mut score = if matches!(kind, 0 | 4) {
                if word(b, 0) == 0 {
                    let mut v = world.stock(b)?;
                    v.add(ext(prior(b))?);
                    ext(v.to_f32())?
                } else {
                    ext(prior(b))?
                }
            } else if matches!(kind, 2 | 3) {
                let mut v = ext(float(b, 0x20))?;
                v.add(ext(prior(b))?.negated());
                v
            } else {
                let stock = if word(b, 0) == 0 {
                    world.stock(b)?.to_f32()
                } else {
                    0.
                };
                let mut expense = ext(stock)?;
                expense.add(ext(world.utility_cost(p, b)?)?);
                expense.add(Sum::from_i32(if p.bytes[0x178 + category] != 0 {
                    0
                } else {
                    i32::from(u16::from_le_bytes(b[0x18..0x1a].try_into().unwrap()))
                }));
                let mut revenue = Sum::default();
                for i in 0..usize::from(info[0x4c]).min(6) {
                    if world.products[word(b, 4) as usize][i][0] == u32::MAX {
                        break;
                    }
                    revenue.add(ext(float(b, 0xa8 + i * 0x34))?);
                }
                revenue.add(expense.negated());
                revenue
            };
            if kind == 8 && score.compare(ext(image.f32_at(0x525ca8)?)?).is_lt() {
                score = Sum::default();
            }
            let threshold = if matches!(kind, 0 | 4) {
                0x525ba4
            } else {
                0x525178
            };
            if !greater(score, best)?
                || (kind != 8 && p.bytes[0x13dd] == 0 && !greater(score, image.f32_at(threshold)?)?)
            {
                continue;
            }
            best = score.to_f32();
            rows[cursor] = [business as u32, best.to_bits()];
            cursor += 1;
            if cursor == rows.len() {
                cursor = 0;
            }
        }
        if matches!(kind, 5 | 6) {
            if p.bytes[0x178 + category] != 0 && category_score > 0. {
                let mut reduction = ext(category_score)?;
                reduction.mul_f32(image.f32_at(0x52502c)?)?;
                let mut score = ext(category_score)?;
                score.add(reduction.negated());
                category_score = score.to_f32();
            }
            if !own_seen && category_score > best {
                best = category_score;
                out.target = category as i32;
            }
        }
        if kind == 1 && category_score > best {
            best = category_score;
            out.target = category as i32;
        }
        if kind == 7 && rent > best_rent {
            best_rent = rent;
            out.target = category as i32;
            out.score = Sum::from_i32(rent).to_f32();
        }
    }
    if matches!(kind, 5 | 6) {
        out.score = best;
    } else if !matches!(kind, 1 | 7) {
        sort(&mut rows);
        if kind == 8 {
            for i in 0..rows.len() {
                let first = f32::from_bits(rows[0][1]);
                let limit = if first.is_nan() {
                    0
                } else {
                    let mut v = ext(first)?;
                    v.mul_f32(image.f32_at(0x525b64)?)?;
                    v.ftol_low_word()
                };
                if (limit as f64) > f64::from(f32::from_bits(rows[i][1])) {
                    rows[i][1] = 0;
                }
            }
            sort(&mut rows);
        }
        finish(&rows, cursor, p, world, &mut out)?;
    }
    Ok(out)
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
    #[ignore = "requires complete original event-selector observations"]
    fn matches_native_selection() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_EVENT_SELECTION").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTECSE01");
        assert_eq!(r.word(), 2);
        assert_eq!(r.word(), 0x037f);
        for case in 0..r.word() {
            let [kind, hour, tick, unselected, enabled, target, score, count] =
                std::array::from_fn::<_, 8, _>(|_| r.word());
            let mut p = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let buildings: Vec<[u8; 0x394]> = r
                .blob()
                .chunks_exact(0x394)
                .map(|b| b.try_into().unwrap())
                .collect();
            let businesses: Vec<[u8; 544]> = r
                .blob()
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            let categories: Vec<[u8; 0x17c]> = r
                .blob()
                .chunks_exact(0x17c)
                .map(|b| b.try_into().unwrap())
                .collect();
            let catalog = BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(116)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let raw = r.blob();
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| [word(raw, i * 48 + j * 8), word(raw, i * 48 + j * 8 + 4)])
            });
            let commodities: [[u8; 28]; 32] = r
                .blob()
                .chunks_exact(28)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let expected = r.blob();
            let world = World {
                buildings: &buildings,
                businesses: &businesses,
                categories: &categories,
                catalog: &catalog,
                products: &products,
                commodities: &commodities,
                utilities: [enabled as u8; 4],
                hour: hour as i32,
                tick,
                unselected_building: unselected as i16,
            };
            let result = choose(kind, &mut p, &world, &image)
                .unwrap_or_else(|e| panic!("case{case} kind{kind}: {e}"));
            assert_eq!(
                [result.target as u32, result.score.to_bits(), result.count],
                [target, score, count],
                "case{case} kind{kind}"
            );
            assert_eq!(p.bytes, expected, "player case{case}");
        }
        assert!(r.0.is_empty());
    }
}
