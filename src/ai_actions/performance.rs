//! Original46eda0 ownership performance grades and472420 business score.
use crate::{
    business_pool::BusinessCatalog,
    economy::{BusinessDayInputs, Utilities},
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};
pub struct World<'a, 'image> {
    pub catalog: &'a BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    ///Telecom, gas, water, electrical, matching authoritative Utilities.
    pub enabled: [bool; 4],
    pub image: &'a PeImage<'image>,
}
pub fn score(
    record: &[u8; 544],
    inputs: &BusinessDayInputs<'_>,
    utilities: &Utilities,
    catalog: &BusinessCatalog,
    image: &PeImage<'_>,
    mut diagnostic: impl FnMut(u32) -> Result<(), String>,
) -> Result<i32, String> {
    let kind = word(record, 4) as usize;
    let row = catalog
        .row(kind)
        .ok_or("AI performance kind leaves catalog")?;
    let (gross, cost) = match word(row, 0xc) {
        0 => {
            let mut total = Sum::from_i32(0);
            for (slot, id) in inputs.catalogs.commodities[kind].iter().take(6).enumerate() {
                if *id == u32::MAX {
                    break;
                }
                total.add_f32(f32::from_bits(word(record, 0xa8 + slot * 52)))?;
            }
            (
                total.ftol_low_word(),
                crate::ai_business::estimate_cost_extended(record, inputs, utilities)?
                    .ftol_low_word(),
            )
        }
        1 => {
            let units = u16::from_le_bytes(record[0x10..0x12].try_into().unwrap());
            let rent =
                crate::economy::ftol_low_word(f64::from(f32::from_bits(word(record, 0x1dc))));
            (
                i32::from(units).wrapping_mul(rent),
                crate::ai_business::estimate_cost_extended(record, inputs, utilities)?
                    .ftol_low_word(),
            )
        }
        _ => {
            diagnostic(0x53d5d8)?;
            (0, 0)
        }
    };
    let mut margin = Sum::from_i32(cost);
    margin.mul_f32(image.f32_at(0x52502c)?)?;
    let margin = margin.ftol_low_word();
    if gross >= cost.wrapping_add(margin) {
        return Ok(100);
    }
    if gross < margin {
        return Ok(0);
    }
    gross
        .wrapping_mul(100)
        .checked_div(cost)
        .ok_or_else(|| "original AI performance integer division fault".into())
}
pub fn refresh(
    player: &Player,
    ownership: &mut crate::ai_ownership::Storage,
    w: &World<'_, '_>,
    mut diagnostic: impl FnMut(u32) -> Result<(), String>,
) -> Result<(), String> {
    let allocated = u16::from_le_bytes(player.bytes[0xb46..0xb48].try_into().unwrap()) as usize;
    if allocated == 0 {
        return Ok(());
    }
    let rows = ownership
        .lists
        .get_mut(&word(&player.bytes, 0xb3c))
        .ok_or("AI performance ownership allocation missing")?
        .get_mut(..allocated)
        .ok_or("AI performance ownership allocation truncated")?;
    let buildings: Vec<[u8; 64]> = w
        .buildings
        .iter()
        .map(|r| r[..64].try_into().unwrap())
        .collect();
    let mut catalogs = crate::shopping::Catalogs {
        commodities: std::array::from_fn(|_| vec![]),
        fractions: std::array::from_fn(|_| vec![]),
    };
    for kind in 0..45 {
        let count = usize::from(w.catalog.row(kind).unwrap()[0x4c]);
        for item in w.products[kind]
            .get(..count)
            .ok_or("AI performance products exceed six")?
        {
            catalogs.commodities[kind].push(item[0]);
            catalogs.fractions[kind].push(f32::from_bits(item[1]));
        }
    }
    let prices = w.commodities.map(|r| f32::from_bits(word(&r, 0x14)));
    let settings = crate::economy::BusinessDaySettings::from_catalog(w.catalog);
    let owners = w.categories.map(|r| r[5]);
    let inputs = BusinessDayInputs {
        buildings: &buildings,
        building_categories: &[],
        category_owners: &owners,
        catalogs: &catalogs,
        unit_prices: &prices,
        settings: &settings,
    };
    let utilities = Utilities {
        owners: [2, 35, 39, 42].map(|i| w.categories[i][5]),
        enabled: w.enabled,
        totals: [0.; 4],
    };
    for row in rows {
        if row[0] == 0 {
            continue;
        }
        let record = w
            .businesses
            .get(word(row, 4) as usize)
            .ok_or("AI performance business missing")?;
        if word(record, 4) == 44 {
            continue;
        }
        let value = score(
            record,
            &inputs,
            &utilities,
            w.catalog,
            w.image,
            &mut diagnostic,
        )?;
        let grade = if value >= 100 {
            1
        } else if value >= 75 {
            2
        } else if value >= 50 {
            3
        } else if value >= 0 {
            4
        } else {
            5
        };
        if word(row, 0xc) == grade {
            let age = word(row, 0x10).wrapping_add(1);
            put(row, 0x10, age);
        } else {
            put(row, 0xc, grade);
            put(row, 0x10, 0);
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
    #[ignore = "requires whole original ownership performance observations"]
    fn matches_native_performance() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PERFORMANCE").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIPRF1");
        assert_eq!(r.word(), 0x37f);
        for case in 0..r.word() {
            let enabled = std::array::from_fn(|_| r.word() != 0);
            let scores: [u32; 12] = std::array::from_fn(|_| r.word());
            let b: Vec<_> = (0..8).map(|_| r.blob()).collect();
            let a: Vec<_> = (0..8).map(|_| r.blob()).collect();
            let player = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![],
            };
            let rows: Vec<[u8; 28]> = b[1]
                .chunks_exact(28)
                .map(|x| x.try_into().unwrap())
                .collect();
            let mut ownership = crate::ai_ownership::Storage {
                lists: std::collections::BTreeMap::from([(0x10030000, rows)]),
            };
            let businesses: Vec<[u8; 544]> = b[2]
                .chunks_exact(544)
                .map(|x| x.try_into().unwrap())
                .collect();
            let buildings: Vec<[u8; 916]> = b[3]
                .chunks_exact(916)
                .map(|x| x.try_into().unwrap())
                .collect();
            let categories = b[4]
                .chunks_exact(380)
                .map(|x| x.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let catalog = BusinessCatalog::from_rows(
                b[5].chunks_exact(116)
                    .map(|x| x.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [word(b[6], i * 48 + j * 8), word(b[6], i * 48 + j * 8 + 4)]
                })
            });
            let commodities = b[7]
                .chunks_exact(28)
                .map(|x| x.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let world = World {
                catalog: &catalog,
                products: &products,
                commodities: &commodities,
                categories: &categories,
                buildings: &buildings,
                businesses: &businesses,
                enabled,
                image: &image,
            };
            refresh(&player, &mut ownership, &world, |_| Ok(())).unwrap();
            assert_eq!(
                ownership.lists[&0x10030000].concat(),
                a[1],
                "case{case} ownership"
            );
            for j in [0, 2, 3, 4, 5, 6, 7] {
                assert_eq!(b[j], a[j], "case{case} unchanged{j}");
            }
            let small_buildings: Vec<[u8; 64]> = buildings
                .iter()
                .map(|b| b[..64].try_into().unwrap())
                .collect();
            let settings = crate::economy::BusinessDaySettings::from_catalog(&catalog);
            let owners = categories.map(|r| r[5]);
            let prices = commodities.map(|r| f32::from_bits(word(&r, 0x14)));
            let mut catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| vec![]),
                fractions: std::array::from_fn(|_| vec![]),
            };
            for kind in 0..45 {
                for item in &products[kind][..catalog.row(kind).unwrap()[0x4c] as usize] {
                    catalogs.commodities[kind].push(item[0]);
                    catalogs.fractions[kind].push(f32::from_bits(item[1]));
                }
            }
            let inputs = BusinessDayInputs {
                buildings: &small_buildings,
                building_categories: &[],
                category_owners: &owners,
                catalogs: &catalogs,
                unit_prices: &prices,
                settings: &settings,
            };
            let utilities = Utilities {
                owners: [2, 35, 39, 42].map(|i| owners[i]),
                enabled,
                totals: [0.; 4],
            };
            for (i, b) in businesses.iter().enumerate() {
                assert_eq!(
                    score(b, &inputs, &utilities, &catalog, &image, |_| Ok(())).unwrap() as u32,
                    scores[i],
                    "case{case} business{i}score"
                );
            }
        }
        assert!(r.0.is_empty());
    }
}
