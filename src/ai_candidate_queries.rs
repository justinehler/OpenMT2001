//! Original property eligibility queries used by candidate production.
use crate::{ai_strategy_queries as queries, pe::PeImage, player_manager::word, x87::Sum};

///474be0 performs read-only edge probes but unconditionally sets AL=1 at
///474de5 after the five iterations, even when every placement probe fails.
///Only zero width/depth take the false return. This is not placement admission;
///the later construction helper must still find a genuinely valid footprint.
pub fn footprint_probe(width: u32, depth: u32) -> bool {
    width != 0 && depth != 0
}

///473230/473420: ordered business types, original time windows and footprint
///enumeration. Existing mode skips the candidate for an inactive kind -1 row.
pub fn feasible(
    existing: bool,
    windows: (bool, bool),
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    catalog: &crate::business_pool::BusinessCatalog,
) -> Result<bool, String> {
    let (build, buy) = windows;
    if existing {
        let mut invalid = false;
        queries::visit_chain(category, buildings, |row| {
            let id = word(row, 0x1c) as i32;
            if id >= 0 {
                let business = businesses
                    .get(id as usize)
                    .ok_or("candidate feasibility business leaves pool")?;
                invalid |= word(business, 0) == 0 && word(business, 4) == u32::MAX;
            }
            Ok(())
        })?;
        if invalid {
            return Ok(false);
        }
    }
    for kind in 0..45u32 {
        let retail = catalog
            .row(kind as usize)
            .ok_or("candidate catalog row missing")?[0x5c]
            < 18;
        if if build { !retail } else { !buy || retail } {
            continue;
        }
        let mut rows = [0u32; 44];
        let mut count = 0;
        crate::ai_actions::planning::footprints_from_catalog(kind, &mut rows, &mut count, catalog)?;
        if count > 22 {
            return Err("candidate feasibility footprint count leaves buffer".into());
        }
        for row in rows[..count as usize * 2].chunks_exact(2) {
            if footprint_probe(row[0], row[1]) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 46c900/46c9f0: composition of non-kind1 linked businesses and largest
/// footprint of the opposite business class. Empty categories are eligible.
pub fn window_gate(
    build: bool,
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    catalog: &crate::business_pool::BusinessCatalog,
    image: &PeImage<'_>,
) -> Result<bool, String> {
    let (mut total, mut matching, mut largest) = (0i32, 0i32, 0i32);
    queries::visit_chain(category, buildings, |row| {
        let id = word(row, 0x1c) as i32;
        if word(row, 0x24) == 1 || id < 0 {
            return Ok(());
        }
        let business = businesses
            .get(id as usize)
            .ok_or("AI window business outside pool")?;
        let kind = word(business, 4);
        if kind > 45 {
            return Err("AI window business class outside original table".into());
        }
        let retail = catalog
            .row(kind as usize)
            .ok_or("AI window catalog row missing")?[0x5c]
            < 18;
        if retail == build {
            matching += 1;
        } else {
            let area = (i32::from(row[6]) - i32::from(row[4]) + 1)
                * (i32::from(row[7]) - i32::from(row[5]) + 1);
            largest = largest.max(area);
        }
        total += 1;
        Ok(())
    })?;
    if total <= 0 {
        return Ok(true);
    }
    let mut ratio = Sum::from_i32(matching);
    ratio.mul_f32(image.f32_at(0x525058)?)?;
    ratio.div(Sum::from_i32(total))?;
    let threshold = Sum::from_f32(image.f32_at(if build { 0x52506c } else { 0x525248 })?)?;
    Ok(ratio.compare(threshold) != std::cmp::Ordering::Less && largest < 9)
}

/// 473ec0 counts all four seven-cell edges, including duplicate corners.
pub fn vacant_perimeter(category: &[u8; 380]) -> i32 {
    use crate::ai_messages::offers::execution::placement;
    let mut total = 0;
    for edge in 0..4 {
        for i in 0..7 {
            let (x, y) = match edge {
                0 => (i, 0),
                1 => (i, 6),
                2 => (0, i),
                _ => (6, i),
            };
            total += i32::from(placement(category, [x, y, 1, 1]));
        }
    }
    total
}

/// 46e540 excludes only the input category; negative group slots still read
/// the original pre-category byte (retained at building599+24d for -1).
pub fn owned_siblings(
    category: u32,
    owner: u32,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    image: &PeImage<'_>,
) -> Result<i32, String> {
    let group = image.at(0x5385a4 + category * 88, 1)?[0];
    let mut count = 0;
    for id in queries::group(image, u32::from(group))? {
        if id != category as i32
            && i32::from(queries::category_owner(id, categories, buildings)?)
                == owner.wrapping_add(1) as i32
        {
            count += 1;
        }
    }
    Ok(count)
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
    #[ignore = "requires original candidate world-query observations"]
    fn matches_native_candidate_queries() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_CANDIDATE_QUERIES")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTCQRY02");
        assert_eq!(r.word(), 0x037f);
        for index in 0..r.word() {
            let h: [u32; 4] = std::array::from_fn(|_| r.word());
            let categories: Box<[[u8; 380]; 52]> = r
                .blob()
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .into_boxed_slice()
                .try_into()
                .unwrap();
            let buildings: Vec<[u8; 916]> = r
                .blob()
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            let businesses: Vec<[u8; 544]> = r
                .blob()
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(116)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let result = match h[0] {
                0x46c900 | 0x46c9f0 => u32::from(
                    window_gate(
                        h[0] == 0x46c900,
                        &categories[h[1] as usize],
                        &buildings,
                        &businesses,
                        &catalog,
                        &image,
                    )
                    .unwrap(),
                ),
                0x473ec0 => vacant_perimeter(&categories[h[1] as usize]) as u32,
                0x46e540 => {
                    owned_siblings(h[1], h[2], &categories, &buildings, &image).unwrap() as u32
                }
                _ => panic!(),
            };
            assert_eq!(
                result,
                if matches!(h[0], 0x46c900 | 0x46c9f0) {
                    h[3] & 255
                } else {
                    h[3]
                },
                "case{index}: {:x}",
                h[0]
            );
        }
        assert!(r.0.is_empty());
    }

    #[test]
    #[ignore = "requires complete original candidate feasibility observations"]
    fn matches_native_candidate_feasibility() {
        use crate::player_manager::Player;
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_CANDIDATE_FEASIBILITY")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTCFEAS2");
        assert_eq!(r.word(), 0x037f);
        for index in 0..r.word() {
            let h: [u32; 5] = std::array::from_fn(|_| r.word());
            let player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let profiles = vec![r.blob().try_into().unwrap()];
            let category = r.blob().try_into().unwrap();
            let buildings = vec![r.blob().try_into().unwrap()];
            let businesses = vec![r.blob().try_into().unwrap()];
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(116)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let result = if h[0] == 0x474be0 {
                footprint_probe(h[2], h[3])
            } else {
                feasible(
                    h[0] == 0x473420,
                    crate::ai_selection::windows(&player, &profiles, h[1] as i32).unwrap(),
                    &category,
                    &buildings,
                    &businesses,
                    &catalog,
                )
                .unwrap()
            };
            assert_eq!(u32::from(result), h[4], "case{index}");
        }
        assert!(r.0.is_empty());
    }
}
