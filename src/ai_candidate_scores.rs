//! Original existing-property candidate contributions, including native
//! global business-slot traversal inside the commodity-coverage calculation.
use crate::{
    ai_strategy_queries::visit_chain,
    business_pool::BusinessCatalog,
    pe::PeImage,
    player_manager::{Player, word},
    x87::Sum,
};

pub struct World<'a> {
    pub catalog: &'a BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub category: &'a [u8; 380],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub image: &'a PeImage<'a>,
}

///472ed0 counts owned and473030 counts other nonzero owners. Only business
///DWORD0==0 participates; opening-hour classification follows window priority.
pub fn ownership(
    own: bool,
    player: &Player,
    windows: (bool, bool),
    world: &World<'_>,
) -> Result<i32, String> {
    let mut count = 0;
    visit_chain(world.category, world.buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id < 0 {
            return Ok(());
        }
        let business = world
            .businesses
            .get(id as usize)
            .ok_or("candidate score business leaves pool")?;
        if word(business, 0) != 0 {
            return Ok(());
        }
        let owner = i32::from(business[10] as i8);
        let matches = owner == i32::from(player.bytes[0x1bc]) + 1;
        if if own { !matches } else { owner == 0 || matches } {
            return Ok(());
        }
        let kind = word(business, 4);
        let day = world
            .catalog
            .row(kind as usize)
            .ok_or("candidate score kind leaves catalog")?[0x5c]
            < 18;
        if if windows.0 { day } else { windows.1 && !day } {
            count += 1;
        }
        Ok(())
    })?;
    Ok(count)
}

///Original473740: the inner products loop reads successive global business slots,
///rather than all products of the current category's business. Keep that
///original indexing, and count each category business type only once.
pub fn coverage(windows: (bool, bool), world: &World<'_>) -> Result<i32, String> {
    let (mut day, mut night) = (0, 0);
    for (id, row) in world.commodities.iter().enumerate() {
        if word(row, 0) == 31 {
            continue;
        }
        if id <= 30 {
            if world.image.at(0x4739a4 + id as u32, 1)?[0] == 0 && windows.0 && row[16] != 0 {
                day += 1;
            }
            if world.image.at(0x4739cc + id as u32, 1)?[0] == 0 {
                continue;
            }
        }
        if windows.1 && row[16] != 0 {
            night += 1;
        }
    }
    let choose_day = day > night;
    let denominator = if choose_day { day } else { night };
    let mut seen = [false; 45];
    let mut numerator = 0;
    visit_chain(world.category, world.buildings, |b| {
        let id = word(b, 0x1c) as i32;
        if id < 0 {
            return Ok(());
        }
        let business = world
            .businesses
            .get(id as usize)
            .ok_or("candidate coverage business leaves pool")?;
        let kind = word(business, 4) as usize;
        let catalog = world
            .catalog
            .row(kind)
            .ok_or("candidate coverage kind leaves catalog")?;
        if (catalog[0x5c] < 18) != choose_day || seen[kind] {
            return Ok(());
        }
        for i in 0..usize::from(catalog[0x4c]) {
            let global = world
                .businesses
                .get(i)
                .ok_or("candidate coverage global slot outside pool")?;
            let global_kind = word(global, 4) as usize;
            let row = world
                .catalog
                .row(global_kind)
                .ok_or("candidate coverage global kind outside catalog")?;
            let commodity = if i < usize::from(row[0x4c]) {
                world
                    .products
                    .get(global_kind)
                    .and_then(|p| p.get(i))
                    .ok_or("candidate product crosses six-slot table")?[0] as i32
            } else {
                -1
            };
            let active = if commodity == -1 {
                world.image.at(0x53be8c, 1)?[0] != 0
            } else {
                world
                    .commodities
                    .get(commodity as usize)
                    .ok_or("candidate commodity outside table")?[16]
                    != 0
            };
            numerator += i32::from(active);
        }
        seen[kind] = true;
        Ok(())
    })?;
    if denominator == 0 {
        return Ok(0);
    }
    let mut ratio = Sum::from_i32(numerator);
    ratio.mul_f32(world.image.f32_at(0x525058)?)?;
    ratio.div(Sum::from_i32(denominator))?;
    Ok(ratio.ftol_low_word())
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
    #[ignore = "requires complete original existing-candidate score observations"]
    fn matches_native_candidate_scores() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_CANDIDATE_SCORES")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTCPSC01");
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
            let catalog = BusinessCatalog::from_rows(
                r.blob()
                    .chunks_exact(116)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let commodities: [[u8; 28]; 32] = r
                .blob()
                .chunks_exact(28)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let product_bytes = r.blob();
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [
                        word(product_bytes, i * 48 + j * 8),
                        word(product_bytes, i * 48 + j * 8 + 4),
                    ]
                })
            });
            let world = World {
                catalog: &catalog,
                products: &products,
                commodities: &commodities,
                category: &category,
                buildings: &buildings,
                businesses: &businesses,
                image: &image,
            };
            let windows = crate::ai_selection::windows(&player, &profiles, h[1] as i32).unwrap();
            let actual = [
                coverage(windows, &world).unwrap() as u32,
                ownership(true, &player, windows, &world).unwrap() as u32,
                ownership(false, &player, windows, &world).unwrap() as u32,
            ];
            assert_eq!(actual, &h[2..], "case{index}, seed{} hour{}", h[0], h[1]);
        }
        assert!(r.0.is_empty());
    }
}
