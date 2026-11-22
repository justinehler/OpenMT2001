//! Original4687e0 clears an eligible large building when a group lacks room.
use crate::{
    ai_maintenance, ai_strategy_queries,
    player_manager::{word, Player},
};
///475be0: every nonnegative category in the group has the same nonzero owner.
pub fn complete_group(
    category: u32,
    categories: &[[u8; 380]; 52],
    image: &crate::pe::PeImage<'_>,
) -> Result<bool, String> {
    let owner = categories
        .get(category as usize)
        .ok_or("expansion category missing")?[5] as i8;
    if owner == 0 {
        return Ok(false);
    }
    let group = image.at(0x5385a4 + category * 88, 1)?[0];
    for id in ai_strategy_queries::group(image, u32::from(group))? {
        if id > -1
            && categories
                .get(id as usize)
                .ok_or("expansion group member missing")?[5] as i8
                != owner
        {
            return Ok(false);
        }
    }
    Ok(true)
}
///477820 counts kind43 businesses, including inactive and other-owned entries.
pub fn special_count(
    category: &[u8; 380],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
) -> Result<i32, String> {
    let mut count = 0i32;
    ai_strategy_queries::visit_chain(category, buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id > -1
            && word(
                businesses
                    .get(id as usize)
                    .ok_or("expansion business missing")?,
                4,
            ) == 43
        {
            count = count.wrapping_add(1);
        }
        Ok(())
    })?;
    Ok(count)
}
pub fn clear_space(
    player: &mut Player,
    footprints: &[[i32; 2]; 52],
    w: &mut ai_maintenance::World<'_>,
) -> Result<(), String> {
    for (id, footprint) in footprints.iter().enumerate() {
        if player.bytes[0x178 + id] == 0 || !complete_group(id as u32, w.categories, w.image)? {
            continue;
        }
        let category = &w.categories[id];
        if special_count(category, w.buildings, w.businesses)? > 0 {
            continue;
        }
        if footprint[0] >= 6 {
            let (mut width, mut depth) = (3, 2);
            let mut position = [0; 2];
            if super::placement::place(player, &mut position, &mut width, &mut depth, |r| {
                Ok(crate::ai_messages::offers::execution::placement(
                    category,
                    r.map(|v| v as u32),
                ))
            })? {
                return Ok(());
            }
            continue;
        }
        let mut next = word(category, 0x15c);
        let mut visited = 0;
        while next != 0 {
            if visited >= w.buildings.len() {
                return Err("expansion building chain cycles".into());
            }
            visited += 1;
            let offset = next
                .checked_sub(0x591318)
                .ok_or("expansion building precedes pool")?;
            if offset % 916 != 0 {
                return Err("expansion building unaligned".into());
            }
            let b = w
                .buildings
                .get(offset as usize / 916)
                .ok_or("expansion building missing")?;
            let dx = i32::from(b[6]) - i32::from(b[4]);
            let dy = i32::from(b[7]) - i32::from(b[5]);
            if word(b, 0x1c) as i32 > -1
                && word(b, 0x24) == 2
                && (dx + 1).wrapping_mul(dy + 1) >= 6
                && dx > 1
                && dy > 1
            {
                let building = word(b, 0);
                let quote = ai_maintenance::demolition_quote(offset / 916, w)?;
                let bits = quote.to_f32().to_bits();
                let mut threshold = quote;
                threshold.add_f32(w.image.f32_at(0x525058)?)?;
                let cash = f32::from_bits(word(&player.bytes, 4));
                if cash.is_nan() || threshold.compare(crate::x87::Sum::from_f32(cash)?).is_lt() {
                    ai_maintenance::proposal(false, player, [id as u32, 0, building, bits], w)?;
                    return Ok(());
                }
            }
            next = word(b, 0x44);
        }
    }
    Ok(())
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
    #[ignore = "requires whole original expansion observations"]
    fn matches_native_expansion() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_EXPANSION").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIEXP1");
        assert_eq!(r.word(), 0x37f);
        let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        for i in 0..r.word() {
            let expected_identity = r.word();
            let prior = r.take(104).try_into().unwrap();
            let b: Vec<_> = (0..7).map(|_| r.blob()).collect();
            let after = r.blob();
            let mut p = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![],
            };
            let categories = b[1]
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let buildings: Vec<[u8; 916]> = b[2]
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            let businesses: Vec<[u8; 544]> = b[3]
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                b[4].chunks_exact(116)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let footprints = b[5]
                .chunks_exact(8)
                .map(|b| [word(b, 0) as i32, word(b, 4) as i32])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let mut counts = b[6]
                .chunks_exact(4)
                .map(|b| word(b, 0))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let mut identity = 99;
            let mut w = ai_maintenance::World {
                image: &image,
                profiles: &[],
                categories: &categories,
                buildings: &buildings,
                businesses: &businesses,
                catalog: &catalog,
                entries: &[],
                counts: &mut counts,
                identity: &mut identity,
                prior,
            };
            clear_space(&mut p, &footprints, &mut w).unwrap();
            assert_eq!(p.bytes, after, "case{i} player");
            assert_eq!(identity, expected_identity, "case{i} identity");
            assert_eq!(
                counts
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
                b[6]
            );
        }
        assert!(r.0.is_empty());
    }
}
///470d40 counts non-tourists whose native housing mode is1 (no home).
pub fn homeless(residents: &crate::economy::ResidentAccounts) -> Result<i32, String> {
    let mut count = 0i32;
    for r in residents
        .nodes
        .get(..residents.active_count as usize)
        .ok_or("expansion resident prefix missing")?
        .iter()
        .rev()
    {
        let b = r.as_bytes();
        if word(b, 0) != 4 && b[0xe..0x10] == [255, 255] {
            count = count.wrapping_add(1);
        }
    }
    Ok(count)
}
///468920 builds kind43 from its ranked native32-byte candidate records.
#[allow(clippy::too_many_arguments)]
pub fn build(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    footprints: &[[i32; 2]; 52],
    statuses: &[u8; 6],
    image: &crate::pe::PeImage<'_>,
    world: &mut super::retained::World<'_>,
    mut groups: impl FnMut(&Player) -> Result<i32, String>,
    mut diagnostic: impl FnMut(super::Call, &mut Player, &mut [[u8; 0x650]]) -> Result<(), String>,
) -> Result<(), String> {
    use crate::{player_manager::put, x87::Sum};
    let homeless = homeless(world.residents)?;
    let cash = f32::from_bits(word(&player.bytes, 4));
    if *statuses
        .get(player.bytes[0x1bc] as usize)
        .ok_or("expansion player status missing")?
        != 0
        || homeless < 30
        || cash.is_nan()
        || cash < image.f32_at(0x525040)?
    {
        return Ok(());
    }
    if groups(player)? == 0 {
        return Ok(());
    }
    let mut budget = Sum::from_f32(cash)?;
    budget.mul_f32(image.f32_at(0x52508c)?)?;
    budget.add_f32(cash)?;
    let mut budget = budget.ftol_low_word();
    let mut floor = Sum::from_f32(cash)?;
    floor.add_f32(image.f32_at(0x525ca0)?)?;
    let floor = Sum::from_f32(floor.to_f32())?;
    if !Sum::from_i32(budget).compare(floor).is_gt() {
        budget = floor.ftol_low_word();
    }
    let catalog = world
        .catalog
        .row(43)
        .ok_or("expansion kind43 catalog missing")?;
    let maximum = word(catalog, 0x44) as i32;
    let minimum = word(catalog, 0x48) as i32;
    let sizes = [[4u32, 4], [3, 5], [3, 4], [3, 3], [2, 5], [2, 4], [2, 3]];
    let mut rows = [[u32::MAX; 8]; 52];
    let mut size_index = 0usize;
    let mut accepted = false;
    let mut dimensions = [0u32; 2];
    let mut position = [0u32; 2];
    let mut floors = 0i32;
    let mut capacity = 0i32;
    let mut cost = 0i32;
    let mut count = 0usize;
    for (id, footprint) in footprints.iter().enumerate() {
        if player.bytes[0x178 + id] == 0
            || !complete_group(id as u32, world.categories, image)?
            || footprint[0] < 6
        {
            continue;
        }
        if !accepted {
            loop {
                //After seven failed sizes the original pointer advances into its adjacent
                //candidate array, initially filled with -1. Keep that observable alias.
                dimensions = if size_index < 7 {
                    sizes[size_index]
                } else {
                    let index = (size_index - 7) * 2;
                    let flat = rows.as_flattened();
                    [
                        *flat
                            .get(index)
                            .ok_or("native expansion size pointer leaves frame")?,
                        *flat
                            .get(index + 1)
                            .ok_or("native expansion size pointer leaves frame")?,
                    ]
                };
                let area = (dimensions[0] as i32).wrapping_mul(dimensions[1] as i32);
                floors = homeless
                    .checked_div(area)
                    .ok_or("native expansion area division fault")?;
                capacity = floors.wrapping_mul(area);
                let [mut width, mut depth] = dimensions;
                accepted = super::placement::place_in_category(
                    player,
                    &world.categories[id],
                    &mut position,
                    &mut width,
                    &mut depth,
                )?;
                dimensions = [width, depth];
                if accepted {
                    floors = floors.min(maximum);
                    let mut volume = floors.wrapping_mul(area);
                    let quote = super::retained::quote(
                        &[id as u32, volume as u32, 1, 43, 0, 0, 0, 0, 0, 0],
                        player,
                        world,
                        image,
                    )?;
                    *world.construction_components = quote.components;
                    cost = quote.total;
                    while cost > budget {
                        volume = volume.wrapping_sub(area);
                        floors = floors.wrapping_sub(1);
                        let quote = super::retained::quote(
                            &[id as u32, volume as u32, 1, 43, 0, 0, 0, 0, 0, 0],
                            player,
                            world,
                            image,
                        )?;
                        *world.construction_components = quote.components;
                        cost = quote.total;
                        if floors < minimum {
                            accepted = false;
                            break;
                        }
                    }
                    if accepted {
                        break;
                    }
                }
                size_index += 1;
                if size_index > 6 {
                    accepted = false;
                    break;
                }
            }
        }
        if accepted && capacity > 0 {
            rows[count] = [
                position[0],
                position[1],
                dimensions[0],
                dimensions[1],
                cost as u32,
                capacity.wrapping_sub(cost).wrapping_add(20000) as u32,
                floors as u32,
                id as u32,
            ];
            count += 1;
        }
    }
    crate::native_sort::sort(&mut rows, &|a, b| match (a[5] as i32).cmp(&(b[5] as i32)) {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => -1,
    });
    let old_category = word(&player.bytes, 0xb68);
    put(&mut player.bytes, 0xb68, u32::MAX);
    let old_kind = word(&player.bytes, 0xb90);
    put(&mut player.bytes, 0xb90, 43);
    let mut outputs = [u32::MAX; 2];
    let mut success = false;
    for row in rows {
        let id = row[7];
        if id == u32::MAX {
            continue;
        }
        let mut style =
            super::planning::style(player, &world.categories[id as usize], world.mode as i32)?
                as i32;
        put(&mut player.bytes, 0xb68, id);
        let mut state = super::Planning {
            outputs,
            ..Default::default()
        };
        let first = super::retained::choose(
            player,
            profiles,
            image,
            super::Input {
                execute: 0,
                style_override: 0,
                style: 0,
            },
            &mut state,
            world,
            &mut diagnostic,
        )?;
        outputs = state.outputs;
        if !first {
            while style > 0 {
                style -= 1;
                let mut state = super::Planning {
                    outputs,
                    ..Default::default()
                };
                if super::retained::choose(
                    player,
                    profiles,
                    image,
                    super::Input {
                        execute: 0,
                        style_override: 1,
                        style: style as u32,
                    },
                    &mut state,
                    world,
                    &mut diagnostic,
                )? {
                    success = true;
                }
                outputs = state.outputs;
            }
        }
        if first || success {
            super::proposals::construction(
                player,
                [
                    id,
                    1,
                    43,
                    style as u32,
                    Sum::from_i32(row[4] as i32).to_f32().to_bits(),
                    row[0],
                    row[1],
                    row[2],
                    row[3],
                    row[6],
                ],
                world.proposal_prior,
                world.proposal_identity,
                |call, p, _| {
                    if let super::proposals::Call::Diagnostic(address) = call {
                        diagnostic(super::Call::new(0x4e6960, &[1, 0, address]), p, profiles)?;
                        Ok(Sum::default())
                    } else {
                        Err("expansion proposal unexpectedly queried gameplay".into())
                    }
                },
            )?;
            break;
        }
    }
    put(&mut player.bytes, 0xb68, old_category);
    put(&mut player.bytes, 0xb90, old_kind);
    Ok(())
}
#[cfg(test)]
mod build_tests {
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
    fn rows<const N: usize>(b: &[u8]) -> Vec<[u8; N]> {
        b.chunks_exact(N).map(|b| b.try_into().unwrap()).collect()
    }
    #[test]
    #[ignore = "requires original complete expansion/planning/proposal observations"]
    fn matches_native_expansion_build() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_EXPANSION_BUILD").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIEXB1");
        assert_eq!(r.word(), 0x37f);
        let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        for case in 0..r.word() {
            let mode = r.word();
            let status = r.word() as u8;
            let groups = r.word() as i32;
            let n = r.word();
            let expected_identity = r.word();
            let prior = r.take(104).try_into().unwrap();
            let b: Vec<_> = (0..12).map(|_| r.blob()).collect();
            let a: Vec<_> = (0..12).map(|_| r.blob()).collect();
            let mut p = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![],
            };
            let mut profiles = rows::<0x650>(b[1]);
            let categories = rows::<380>(b[2]).try_into().unwrap();
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                rows::<116>(b[3]).try_into().unwrap(),
            );
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [word(b[4], i * 48 + j * 8), word(b[4], i * 48 + j * 8 + 4)]
                })
            });
            let residents = crate::economy::ResidentAccounts {
                active_count: n,
                nodes: b[5]
                    .chunks_exact(336)
                    .map(|b| {
                        crate::resident::Resident::from_pool_bytes(
                            b[..330].try_into().unwrap(),
                            b[330],
                        )
                    })
                    .collect(),
                accounts: rows::<144>(b[6])
                    .into_iter()
                    .map(crate::resident::SpendingAccount::from_bytes)
                    .collect(),
            };
            let commodities = rows::<28>(b[7]).try_into().unwrap();
            let buildings = rows::<916>(b[8]);
            let mut counts = std::array::from_fn(|i| word(b[9], i * 4));
            let mut components = std::array::from_fn(|i| f32::from_bits(word(b[10], i * 4)));
            let footprints =
                std::array::from_fn(|i| [word(b[11], i * 8) as i32, word(b[11], i * 8 + 4) as i32]);
            let mut identity = 9;
            let mut world = super::super::retained::World {
                categories: &categories,
                buildings: &buildings,
                catalog: &catalog,
                products: &products,
                commodities: &commodities,
                residents: &residents,
                player_references: &[0x10010000; 6],
                player_count: 4,
                category_counts: &mut counts,
                proposal_identity: &mut identity,
                construction_components: &mut components,
                hour: 10,
                mode,
                allowance: 10000,
                affordability_override: false,
                discount: 0.1,
                proposal_prior: prior,
            };
            build(
                &mut p,
                &mut profiles,
                &footprints,
                &[status; 6],
                &image,
                &mut world,
                |_| Ok(groups),
                |_, _, _| Ok(()),
            )
            .unwrap_or_else(|e| panic!("case{case}: {e}"));
            assert_eq!(identity, expected_identity, "case{case} identity");
            for (i, (actual, expected)) in p.bytes.iter().zip(a[0]).enumerate() {
                assert_eq!(actual, expected, "case{case} player+{i:x}");
            }
            assert_eq!(profiles.concat(), a[1], "case{case} profiles");
            assert_eq!(
                counts
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
                a[9],
                "case{case} counts"
            );
            assert_eq!(
                components.map(f32::to_bits),
                std::array::from_fn(|i| word(a[10], i * 4)),
                "case{case} components"
            );
            for i in [2, 3, 4, 5, 6, 7, 8, 11] {
                assert_eq!(b[i], a[i], "case{case} input{i}");
            }
        }
        assert!(r.0.is_empty());
    }
}
