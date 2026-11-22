//! Original business planning profile, RNG and footprint helpers.
use crate::{
    business_pool::BusinessCatalog,
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};
fn group(catalog: &BusinessCatalog, kind: u32) -> Result<usize, String> {
    Ok(catalog
        .row(kind as usize)
        .ok_or("AI planning catalog type missing")?[0x5e] as usize)
}
fn profile<'a>(player: &Player, profiles: &'a [[u8; 0x650]]) -> Result<&'a [u8; 0x650], String> {
    profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or_else(|| "AI planning profile missing".into())
}
fn random(player: &mut Player, divisor: u32) -> Result<u32, String> {
    let seed = word(&player.bytes, 0x224)
        .wrapping_mul(word(&player.bytes, 0x228))
        .wrapping_add(word(&player.bytes, 0x22c));
    put(&mut player.bytes, 0x224, seed);
    if divisor == 0 {
        return Err("AI planning native RNG division by zero".into());
    }
    Ok(((seed >> 16) & 0x7fff) % divisor)
}
///47dd40 reads the group's maximum footprint area.
pub fn preferred_area_from_catalog(
    character: u32,
    kind: u32,
    profiles: &[[u8; 0x650]],
    catalog: &BusinessCatalog,
) -> Result<u32, String> {
    Ok(*profiles
        .get(character as usize)
        .ok_or("AI planning profile missing")?
        .get(0x42 + group(catalog, kind)? * 35)
        .ok_or("AI planning group leaves profile")? as u32)
}
///47db30 preserves each random draw, unsigned final floor clamps and profile weights.
pub fn floor_count_from_catalog(
    player: &mut Player,
    character: u32,
    profiles: &[[u8; 0x650]],
    catalog: &BusinessCatalog,
) -> Result<u32, String> {
    let weights = profiles
        .get(character as usize)
        .ok_or("AI floor profile missing")?
        .get(0x36..0x40)
        .ok_or("AI floor weights missing")?;
    let n = random(player, 10)? as usize + 1;
    let mut total = weights[..n].iter().map(|&v| u32::from(v)).sum::<u32>();
    let selected = random(player, total + 1)? + 1;
    let mut floor = 0;
    for (i, &weight) in weights[..n].iter().enumerate() {
        total = total.wrapping_sub(u32::from(weight));
        if selected > total {
            floor = i as u32 + 1;
            break;
        }
    }
    let adjustment = crate::ai_profiles::adjustment(character);
    let threshold = adjustment
        .wrapping_mul(5)
        .wrapping_add(5000)
        .wrapping_mul(20)
        / 2000;
    let kind = word(&player.bytes, 0xb90);
    let record = catalog
        .row(kind as usize)
        .ok_or("AI planning catalog type missing")?;
    let maximum = word(record, 0x44);
    let minimum = word(record, 0x48);
    let available = (maximum.wrapping_sub(floor) as i32).max(0) as u32;
    if random(player, 101)? as i32 >= threshold {
        floor = floor.wrapping_add(random(player, available + 1)?);
    }
    Ok(floor.min(maximum).max(minimum))
}
///474080/474100 housing footprint/floor limits, retaining chained PC64 products.
pub fn housing_limit(character: u32, floors: bool, image: &PeImage<'_>) -> Result<u32, String> {
    let mut value = Sum::from_i32(crate::ai_profiles::adjustment(character).wrapping_add(1000));
    for a in [
        0x525058,
        0x525c94,
        if floors { 0x5250a0 } else { 0x525030 },
        0x5250e0,
    ] {
        value.mul_f32(image.f32_at(a)?)?;
    }
    let value = value.ftol_low_word();
    if floors {
        return Ok(value as u32);
    }
    Ok(match 7i32.wrapping_sub(value) {
        i32::MIN..=3 => 6,
        4 => 8,
        5 => 9,
        6 => 10,
        7 => 12,
        _ => 0,
    })
}
///47dd90 plus475480. Caller handles original out-of-range diagnostic.
pub fn style(player: &mut Player, category: &[u8; 0x17c], mode: i32) -> Result<u32, String> {
    let value = crate::economy::ftol_low_word(f64::from(f32::from_bits(word(category, 0x84))));
    let tier = if value <= 2 {
        0
    } else if value <= 4 {
        1
    } else {
        2
    };
    let roll = random(player, 101)?;
    Ok(if matches!(mode, 1 | 2) {
        match tier {
            2 => 1 + u32::from(roll > 10),
            1 => 1 + u32::from(roll <= 10),
            _ => {
                if roll <= 20 {
                    0
                } else {
                    1 + u32::from(roll > 50)
                }
            }
        }
    } else {
        match tier {
            2 => {
                if roll <= 5 {
                    0
                } else {
                    1 + u32::from(roll > 20)
                }
            }
            1 => {
                if roll <= 10 {
                    0
                } else {
                    1 + u32::from(roll >= 90)
                }
            }
            _ => {
                if roll <= 5 {
                    2
                } else {
                    u32::from(roll <= 20)
                }
            }
        }
    })
}
///475f70 shrinks area first, then floors; native comparisons are signed.
pub fn reduce_from_catalog(
    player: &Player,
    kind: u32,
    floors: &mut u32,
    area: &mut u32,
    profiles: &[[u8; 0x650]],
    catalog: &BusinessCatalog,
) -> Result<(), String> {
    let min_floor = word(
        catalog
            .row(kind as usize)
            .ok_or("AI planning catalog type missing")?,
        0x48,
    );
    let min_area =
        profile(player, profiles)?[0x40 + group(catalog, word(&player.bytes, 0xb90))? * 35] as u32;
    if (*area as i32) > min_area as i32 {
        *area = area.wrapping_sub(1);
    } else if (*floors as i32) > min_floor as i32 {
        *floors = floors.wrapping_sub(1);
    }
    Ok(())
}
///46ee90 floors the requested area to a listed profile footprint, or minimum.
pub fn normalize_area_from_catalog(
    player: &Player,
    kind: u32,
    area: &mut u32,
    profiles: &[[u8; 0x650]],
    catalog: &BusinessCatalog,
) -> Result<(), String> {
    if kind == 44 {
        return Ok(());
    }
    let p = profile(player, profiles)?;
    let start = 0x40 + group(catalog, kind)? * 35;
    let minimum = u32::from(p[start]);
    loop {
        if (0..10).any(|i| u32::from(p[start + 6 + i * 3]) == *area) {
            return Ok(());
        }
        *area = area.wrapping_sub(1);
        if (*area as i32) < minimum as i32 {
            *area = minimum;
            return Ok(());
        }
    }
}
///476b30's early business-count cash reserve gate.
pub fn reserve_blocked(player: &Player, cost: i32, image: &PeImage<'_>) -> Result<bool, String> {
    if u16::from_le_bytes(player.bytes[0xb44..0xb46].try_into().unwrap()) >= 6 {
        return Ok(false);
    }
    let cash = f32::from_bits(word(&player.bytes, 4));
    if cash.is_nan() {
        return Ok(true);
    }
    let mut available = Sum::from_f32(cash)?;
    available.add_f32(-image.f32_at(0x525ca0)?)?;
    Ok(available.compare(Sum::from_i32(cost)) == std::cmp::Ordering::Less)
}
///46ba80 writes only the selected pairs and count, retaining all tail bytes.
pub fn footprints_from_catalog(
    kind: u32,
    output: &mut [u32; 44],
    count: &mut u32,
    catalog: &BusinessCatalog,
) -> Result<(), String> {
    let rows: Vec<[u32; 2]> = if kind == 44 {
        (1..=6)
            .rev()
            .flat_map(|w| (1..=w).rev().map(move |h| [w, h]))
            .collect()
    } else if kind == 42 {
        vec![[3, 5], [3, 4], [3, 3], [2, 5], [2, 4], [2, 3]]
    } else {
        match group(catalog, kind)? {
            0 => vec![[2, 2], [1, 4], [1, 3], [1, 2], [1, 1]],
            1 => vec![[2, 4], [2, 3], [2, 2], [1, 5], [1, 4], [1, 3], [1, 2]],
            2 => vec![[3, 3], [2, 3], [2, 2]],
            3 => vec![[4, 4], [3, 5], [3, 4], [3, 3], [2, 5], [2, 4], [2, 3]],
            4 => vec![
                [4, 6],
                [4, 5],
                [4, 4],
                [3, 6],
                [3, 5],
                [3, 4],
                [3, 3],
                [2, 6],
                [2, 5],
                [2, 4],
            ],
            _ => return Ok(()),
        }
    };
    for (pair, values) in output.chunks_exact_mut(2).zip(&rows) {
        pair.copy_from_slice(values);
    }
    *count = rows.len() as u32;
    Ok(())
}
///46bd50 retains pairs starting at the first exact matching area. Its native
/// fill clears22 words, despite the44-word surrounding allocation.
pub fn trim_footprints(area: u32, output: &mut [u32; 44], count: &mut u32) -> Result<(), String> {
    let length = (*count as i32).max(0) as usize;
    if length > 22 {
        return Err("AI footprint trim count exceeds original buffer".into());
    }
    let first = (0..length)
        .find(|&i| output[i * 2].wrapping_mul(output[i * 2 + 1]) == area)
        .unwrap_or(length);
    *count = count.wrapping_sub(first as u32);
    let copy = output[first * 2..length * 2].to_vec();
    output[..22].fill(u32::MAX);
    output[..copy.len()].copy_from_slice(&copy);
    Ok(())
}

/// Convenience PE-baseline wrappers; live sessions use the catalog variants.
pub fn preferred_area(
    character: u32,
    kind: u32,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
) -> Result<u32, String> {
    preferred_area_from_catalog(character, kind, profiles, &BusinessCatalog::from_pe(image)?)
}
pub fn floor_count(
    player: &mut Player,
    character: u32,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
) -> Result<u32, String> {
    floor_count_from_catalog(
        player,
        character,
        profiles,
        &BusinessCatalog::from_pe(image)?,
    )
}
pub fn reduce(
    player: &Player,
    kind: u32,
    floors: &mut u32,
    area: &mut u32,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
) -> Result<(), String> {
    reduce_from_catalog(
        player,
        kind,
        floors,
        area,
        profiles,
        &BusinessCatalog::from_pe(image)?,
    )
}
pub fn normalize_area(
    player: &Player,
    kind: u32,
    area: &mut u32,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
) -> Result<(), String> {
    normalize_area_from_catalog(
        player,
        kind,
        area,
        profiles,
        &BusinessCatalog::from_pe(image)?,
    )
}
pub fn footprints(
    kind: u32,
    output: &mut [u32; 44],
    count: &mut u32,
    image: &PeImage<'_>,
) -> Result<(), String> {
    footprints_from_catalog(kind, output, count, &BusinessCatalog::from_pe(image)?)
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
    #[ignore = "requires original AI planning helper observations"]
    fn matches_native_helpers() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PLANNING").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIPLN2");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let [entry, character, kind, mode, cost, requested, expected] =
                std::array::from_fn(|_| r.word());
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let profiles: Vec<[u8; 0x650]> = r
                .blob()
                .chunks_exact(0x650)
                .map(|x| x.try_into().unwrap())
                .collect();
            let category = r.blob();
            let mut outputs = r.blob().to_vec();
            let catalog = BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(116)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let pa = r.blob();
            let proa = r.blob();
            let ca = r.blob();
            let oa = r.blob();
            let catalog_after = r.blob();
            let mut count = word(&outputs, 0xb0);
            let mut area = word(&outputs, 0xb4);
            let mut pairs = std::array::from_fn(|i| word(&outputs, i * 4));
            let value = match entry {
                0x47db30 => Some(
                    floor_count_from_catalog(&mut player, character, &profiles, &catalog).unwrap(),
                ),
                0x47dd40 => {
                    Some(preferred_area_from_catalog(character, kind, &profiles, &catalog).unwrap())
                }
                0x47dd90 => {
                    Some(style(&mut player, category.try_into().unwrap(), mode as i32).unwrap())
                }
                0x474080 | 0x474100 => {
                    Some(housing_limit(character, entry == 0x474100, &image).unwrap())
                }
                0x475f70 => {
                    reduce_from_catalog(&player, kind, &mut count, &mut area, &profiles, &catalog)
                        .unwrap();
                    None
                }
                0x46ee90 => {
                    normalize_area_from_catalog(&player, kind, &mut area, &profiles, &catalog)
                        .unwrap();
                    None
                }
                0x476b30 => Some(u32::from(
                    reserve_blocked(&player, cost as i32, &image).unwrap(),
                )),
                0x46ba80 => {
                    footprints_from_catalog(kind, &mut pairs, &mut count, &catalog).unwrap();
                    None
                }
                0x46bd50 => {
                    trim_footprints(requested, &mut pairs, &mut count).unwrap();
                    None
                }
                _ => panic!("unknown entry"),
            };
            if let Some(v) = value {
                assert_eq!(
                    v,
                    if entry == 0x476b30 {
                        expected & 255
                    } else {
                        expected
                    },
                    "{name}"
                );
            }
            for (i, v) in pairs.iter().enumerate() {
                put(&mut outputs, i * 4, *v);
            }
            put(&mut outputs, 0xb0, count);
            put(&mut outputs, 0xb4, area);
            assert_eq!(player.bytes, pa, "{name} player");
            assert_eq!(outputs, oa, "{name} outputs");
            assert_eq!(profiles.as_flattened(), proa, "{name} profiles");
            assert_eq!(category, ca, "{name} category");
            assert_eq!(
                catalog.rows().as_flattened(),
                catalog_after,
                "{name} catalog"
            );
        }
        assert!(r.0.is_empty());
    }
}
