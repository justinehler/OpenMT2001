//! Original470f60 housed-resident demand and476ee0 construction-volume clamp.
use crate::{
    business_pool::BusinessCatalog,
    economy::ResidentAccounts,
    pe::PeImage,
    player_manager::{Player, word},
    x87::Sum,
};
///Reverse traversal of the retained resident prefix; activity bytes do not
///filter this query. Only age!=4 and a nonnegative/sentinel-excluding home do.
pub fn housed_demand(
    commodity: u32,
    residents: &ResidentAccounts,
    commodities: &[[u8; 28]; 32],
) -> Result<i32, String> {
    let price = f32::from_bits(word(
        commodities
            .get(commodity as usize)
            .ok_or("AI demand commodity leaves catalog")?,
        0x14,
    ));
    let mut total = 0i32;
    for resident in residents
        .nodes
        .get(..residents.active_count as usize)
        .ok_or("AI demand prefix leaves resident pool")?
        .iter()
        .rev()
    {
        let row = resident.as_bytes();
        if word(row, 0) == 4 || i16::from_le_bytes(row[14..16].try_into().unwrap()) == -1 {
            continue;
        }
        // Nonpositive prices skip spending reads, even when the account is absent.
        if price.partial_cmp(&0.) != Some(std::cmp::Ordering::Greater) {
            continue;
        }
        let account = residents
            .accounts
            .get(resident.account_index() as usize)
            .ok_or("AI housed resident account unresolved")?;
        let amount = f32::from_bits(word(account.as_bytes(), 4 + commodity as usize * 4));
        let count = if amount.is_finite() && price.is_finite() {
            let mut amount = Sum::from_f32(amount)?;
            amount.div_f32(price)?;
            amount.ftol_low_word()
        } else {
            crate::economy::ftol_low_word(f64::from(amount) / f64::from(price))
        };
        total = total.wrapping_add(count.max(0));
    }
    Ok(total)
}
///476ee0 is read-only except for the two in/out dimensions. The sum keeps
///duplicate commodities and native wrapping integer semantics.
#[allow(clippy::too_many_arguments)]
pub fn clamp(
    player: &Player,
    area: &mut u32,
    floors: &mut u32,
    profiles: &[[u8; 0x650]],
    catalog: &BusinessCatalog,
    products: &[[[u32; 2]; 6]; 45],
    player_count: i32,
    image: &PeImage<'_>,
    mut demand: impl FnMut(u32) -> Result<i32, String>,
) -> Result<(), String> {
    let kind = word(&player.bytes, 0xb90) as usize;
    let info = catalog
        .row(kind)
        .ok_or("AI demand business leaves catalog")?;
    let count = usize::from(info[0x4c]);
    let entries = products
        .get(kind)
        .and_then(|r| r.get(..count))
        .ok_or("AI demand product count exceeds catalog")?;
    let mut total = 0i32;
    for entry in entries {
        total = total.wrapping_add(demand(entry[0])?);
    }
    if total == 0 {
        *area = 0;
        *floors = 0;
        return Ok(());
    }
    if player_count == 0 {
        return Err("AI demand clamp zero player divisor unresolved".into());
    }
    let mut denominator = Sum::from_f32(image.f32_at(0x525cb4)?)?;
    denominator.div(Sum::from_i32(player_count))?;
    let character = word(&player.bytes, 0x150);
    let mut ratio = Sum::from_i32(crate::ai_profiles::adjustment(character).wrapping_add(1000));
    ratio.div(denominator)?;
    let mut volume = Sum::from_i32(total);
    if ratio.compare(Sum::from_i32(0)) == std::cmp::Ordering::Greater {
        volume.div(ratio)?;
    }
    volume
        .div_f32(f32::from_bits(word(info, 0x58)))
        .map_err(|e| format!("476ee0 business{kind} stock-density division: {e}"))?;
    let volume = volume.ftol_low_word();
    if volume <= 0 {
        return Ok(());
    }
    let mut attempts = 0;
    while (floors.wrapping_mul(*area) as i32) > volume {
        attempts += 1;
        super::planning::reduce_from_catalog(player, kind as u32, floors, area, profiles, catalog)?;
        super::planning::normalize_area_from_catalog(player, kind as u32, area, profiles, catalog)?;
        if attempts > 20 {
            break;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        player_manager::put,
        resident::{Resident, SpendingAccount},
    };

    fn resident(age: u32, home: i16, account: Option<usize>, active: u8) -> Resident {
        let mut bytes = [0; 0x14a];
        put(&mut bytes, 0, age);
        bytes[14..16].copy_from_slice(&home.to_le_bytes());
        let mut resident = Resident::from_pool_bytes(bytes, active);
        resident.set_account_index(account);
        resident
    }

    fn missing_account() -> ResidentAccounts {
        ResidentAccounts {
            active_count: 1,
            nodes: vec![resident(0, 0, None, 1)],
            accounts: vec![],
        }
    }

    // Zero demand returns before reading any image constants.
    fn empty_image() -> [u8; 120] {
        let mut bytes = [0; 120];
        bytes[..2].copy_from_slice(b"MZ");
        put(&mut bytes, 0x3c, 64);
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes[84..86].copy_from_slice(&32u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x10bu16.to_le_bytes());
        bytes
    }

    #[test]
    fn nonpositive_price_skips_missing_account() {
        let residents = missing_account();
        for price in [0.0f32, -2.0] {
            let mut commodities = [[0; 28]; 32];
            put(&mut commodities[3], 0x14, price.to_bits());
            assert_eq!(housed_demand(3, &residents, &commodities), Ok(0));
        }
    }

    #[test]
    fn positive_price_still_requires_account() {
        let mut commodities = [[0; 28]; 32];
        put(&mut commodities[3], 0x14, 2.0f32.to_bits());
        assert_eq!(
            housed_demand(3, &missing_account(), &commodities),
            Err("AI housed resident account unresolved".into())
        );
    }

    #[test]
    fn positive_price_preserves_resident_prefix_and_filters() {
        let commodity = 3;
        let mut commodities = [[0; 28]; 32];
        put(&mut commodities[commodity], 0x14, 0.1f32.to_bits());
        let accounts = [3.0f32, -10.0, 1.0]
            .map(|amount| {
                let mut bytes = [0; 144];
                put(&mut bytes, 4 + commodity * 4, amount.to_bits());
                SpendingAccount::from_bytes(bytes)
            })
            .to_vec();
        let residents = ResidentAccounts {
            active_count: 5,
            nodes: vec![
                resident(0, 0, Some(0), 0),
                resident(4, 0, None, 1),
                resident(0, -1, None, 1),
                resident(0, 0, Some(1), 1),
                resident(0, 0, Some(2), 1),
                resident(0, 0, None, 1),
            ],
            accounts,
        };
        // Each quotient truncates separately; negative demand contributes zero.
        assert_eq!(
            housed_demand(commodity as u32, &residents, &commodities),
            Ok(38)
        );
    }

    #[test]
    fn construction_clamp_clears_dimensions_for_nonpositive_prices() {
        let bytes = empty_image();
        let image = PeImage::parse(&bytes).unwrap();
        let player = Player {
            reference: 1,
            bytes: vec![0; 0xb94],
            auxiliaries: vec![],
        };
        let mut rows = [[0; 116]; 45];
        rows[0][0x4c] = 1;
        let catalog = BusinessCatalog::from_rows(rows);
        let mut products = [[[0; 2]; 6]; 45];
        products[0][0][0] = 3;
        let residents = missing_account();
        for price in [0.0f32, -2.0] {
            let mut commodities = [[0; 28]; 32];
            put(&mut commodities[3], 0x14, price.to_bits());
            let (mut area, mut floors) = (8, 5);
            clamp(
                &player,
                &mut area,
                &mut floors,
                &[],
                &catalog,
                &products,
                1,
                &image,
                |id| housed_demand(id, &residents, &commodities),
            )
            .unwrap();
            assert_eq!((area, floors), (0, 0));
        }
    }

    #[test]
    fn unmet_housed_demand_skips_missing_account_for_nonpositive_prices() {
        let bytes = empty_image();
        let image = PeImage::parse(&bytes).unwrap();
        let catalog = BusinessCatalog::from_rows([[0; 116]; 45]);
        let residents = missing_account();
        for price in [0.0f32, -2.0] {
            let mut commodities = [[0; 28]; 32];
            for row in &mut commodities {
                put(row, 0x14, price.to_bits());
            }
            let world = crate::ai_business_ranking::World {
                image: &image,
                catalog: &catalog,
                products: &[[[0; 2]; 6]; 45],
                commodities: &commodities,
                categories: &[[0; 380]; 52],
                buildings: &[],
                businesses: &[],
                counts: &[0; 52 * 45],
                demand: &[[0; 264]; 33],
                residents: &residents,
                time: crate::ai_selection::Time { hour: 0, year: 0 },
            };
            assert_eq!(
                crate::ai_business_ranking::unmet_housed_demand(&world),
                Ok(0)
            );
        }
    }

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
    #[ignore = "requires original housed demand and construction clamp observations"]
    fn matches_native_plan_demand() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PLAN_DEMAND").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIPDM1");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let active_count = r.word();
            let players = r.word() as i32;
            let commodity = r.word();
            let expected = r.word();
            let before: Vec<_> = (0..8).map(|_| r.blob()).collect();
            let after: Vec<_> = (0..8).map(|_| r.blob()).collect();
            let player = Player {
                reference: 0x10010000,
                bytes: before[0].to_vec(),
                auxiliaries: vec![],
            };
            let profiles: Vec<[u8; 0x650]> = before[1]
                .chunks_exact(0x650)
                .map(|x| x.try_into().unwrap())
                .collect();
            let catalog = BusinessCatalog::from_rows(
                before[2]
                    .chunks_exact(116)
                    .map(|x| x.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let products: [[[u32; 2]; 6]; 45] = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [
                        word(before[3], i * 48 + j * 8),
                        word(before[3], i * 48 + j * 8 + 4),
                    ]
                })
            });
            let residents = ResidentAccounts {
                active_count,
                nodes: before[4]
                    .chunks_exact(336)
                    .map(|b| {
                        crate::resident::Resident::from_pool_bytes(
                            b[..0x14a].try_into().unwrap(),
                            b[0x14a],
                        )
                    })
                    .collect(),
                accounts: before[5]
                    .chunks_exact(144)
                    .map(|b| crate::resident::SpendingAccount::from_bytes(b.try_into().unwrap()))
                    .collect(),
            };
            let commodities = before[6]
                .chunks_exact(28)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let mut area = word(before[7], 0);
            let mut floors = word(before[7], 4);
            if entry == 0x470f60 {
                assert_eq!(
                    housed_demand(commodity, &residents, &commodities).unwrap() as u32,
                    expected,
                    "{name} demand"
                );
            } else {
                clamp(
                    &player,
                    &mut area,
                    &mut floors,
                    &profiles,
                    &catalog,
                    &products,
                    players,
                    &image,
                    |id| housed_demand(id, &residents, &commodities),
                )
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            }
            assert_eq!(
                [area, floors],
                [word(after[7], 0), word(after[7], 4)],
                "{name} dimensions"
            );
            for i in 0..7 {
                assert_eq!(before[i], after[i], "{name} unchangedinput{i}");
            }
        }
        assert!(r.0.is_empty());
    }
}
