//! Original building-panel actions4073a0/407560/407720.
//! These are producer admission rules; command consumers remain authoritative.
use crate::{business_pool::BusinessCatalog, pe::PeImage, player_manager::word, x87::Sum};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Trade,
    Demolish,
    Convert { selected_label: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    Ui {
        address: u32,
        arguments: [u32; 3],
        count: usize,
    },
    Disable(Action),
    Sale {
        business: u32,
    },
    Takeover {
        building: u32,
    },
    Demolish {
        category: i32,
        building: u32,
        amount: f32,
    },
    Convert {
        kind: u32,
        building: u32,
    },
    ClearConversionSelection,
}
pub struct World<'a, 'data> {
    pub building_parameters: crate::building_parameters::BuildingKinds,
    pub image: &'a PeImage<'data>,
    pub selected: Option<usize>,
    pub selected_category: i32,
    pub current_player: u32,
    pub player_references: &'a [u32],
    pub cash: f32,
    pub allowance: i32,
    pub affordability_override: bool,
    pub hour: i32,
    pub ownership_tick: u32,
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a mut [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub catalog: &'a BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
}
///4610d0: exact PC64 product accumulator, including signed FTOL low word.
pub fn initial_stock(
    kind: usize,
    volume: i32,
    catalog: &BusinessCatalog,
    products: &[[[u32; 2]; 6]; 45],
    commodities: &[[u8; 28]; 32],
) -> Result<Sum, String> {
    let row = catalog
        .row(kind)
        .ok_or("initial stock kind outside catalog")?;
    let rows = products
        .get(kind)
        .ok_or("initial stock products outside catalog")?
        .get(..usize::from(row[0x4c]))
        .ok_or("initial stock product count outside six")?;
    let mut resolved = Vec::with_capacity(rows.len());
    for row in rows {
        let commodity = commodities
            .get(row[0] as usize)
            .ok_or("initial stock commodity outside table")?;
        resolved.push(crate::building_binding::StockProduct {
            value: f32::from_bits(word(commodity, 0x14)),
            fraction: f32::from_bits(row[1]),
        });
    }
    crate::building_binding::initial_stock_cost(volume, f32::from_bits(word(row, 0x58)), &resolved)
}
fn afford(
    value: f32,
    w: &mut World<'_, '_>,
    call: &mut impl FnMut(Call, &mut World<'_, '_>) -> Result<(), String>,
) -> Result<bool, String> {
    let result =
        crate::construction::affordable(w.cash, w.allowance, value, w.affordability_override);
    if !result {
        for &(address, args) in &crate::construction::INSUFFICIENT_CASH_CALLS {
            let mut arguments = [0; 3];
            arguments[..args.len()].copy_from_slice(args);
            call(
                Call::Ui {
                    address,
                    arguments,
                    count: args.len(),
                },
                w,
            )?;
        }
    }
    Ok(result)
}
fn selected(w: &World<'_, '_>) -> Result<usize, String> {
    w.selected
        .filter(|i| *i < w.buildings.len())
        .ok_or("building action selected pointer outside pool".into())
}
pub fn activate(
    action: Action,
    w: &mut World<'_, '_>,
    mut call: impl FnMut(Call, &mut World<'_, '_>) -> Result<(), String>,
) -> Result<(), String> {
    if w.selected.is_none() {
        return Ok(());
    }
    match action {
        Action::Trade => {
            let id = selected(w)?;
            let index = word(&w.buildings[id], 0x1c);
            if index == u32::MAX {
                return Ok(());
            }
            let business = w
                .businesses
                .get(index as usize)
                .ok_or("trade business outside pool")?;
            let owner = business[0xa] as i8;
            let owner = if owner == 0 {
                0
            } else {
                *w.player_references
                    .get((i32::from(owner) - 1) as usize)
                    .ok_or("trade owner outside roster")?
            };
            if owner == w.current_player {
                let identity = word(business, 0x210);
                call(Call::Disable(action), w)?;
                call(Call::Sale { business: identity }, w)?;
            } else {
                let value = crate::business_takeover::price(
                    id,
                    &crate::business_takeover::PriceWorld {
                        categories: w.categories,
                        buildings: w.buildings,
                        businesses: w.businesses,
                        player_references: w.player_references,
                    },
                    w.ownership_tick,
                    w.image,
                )?
                .to_f32();
                if !afford(value, w, &mut call)? {
                    return Ok(());
                }
                call(Call::Disable(action), w)?;
                let building = word(&w.buildings[selected(w)?], 0);
                call(Call::Takeover { building }, w)?;
            }
            let id = selected(w)?;
            w.buildings[id][0x3e] = 1;
        }
        Action::Demolish => {
            let value = crate::demolition::quote_with_parameters(
                &w.buildings[selected(w)?],
                w.categories,
                &w.building_parameters,
                w.image,
            )?
            .to_f32();
            if !afford(value, w, &mut call)? {
                return Ok(());
            }
            call(Call::Disable(action), w)?;
            let id = selected(w)?;
            let amount = crate::demolition::quote_with_parameters(
                &w.buildings[id],
                w.categories,
                &w.building_parameters,
                w.image,
            )?
            .to_f32();
            call(
                Call::Demolish {
                    category: w.selected_category,
                    building: word(&w.buildings[id], 0),
                    amount,
                },
                w,
            )?;
            let id = selected(w)?;
            w.buildings[id][0x3c] = 1;
        }
        Action::Convert { selected_label } => {
            let id = selected(w)?;
            let building = &w.buildings[id];
            let (first, last) = match word(building, 0x24) {
                0 => (0, 41),
                1 => (42, 43),
                _ => (0, 0),
            };
            let current = word(
                w.businesses
                    .get(word(building, 0x1c) as usize)
                    .ok_or("convert requires current business")?,
                4,
            );
            for kind in first..=last {
                let id = selected(w)?;
                let building = &w.buildings[id];
                let info = w.catalog.row(kind).unwrap();
                if !crate::business_conversion::fits(building, info)?
                    || kind as u32 == current
                    || selected_label != word(info, 8)
                {
                    continue;
                }
                let mut price = crate::business_conversion::quote(
                    id as u32,
                    kind as u32,
                    w.categories,
                    w.buildings,
                    w.catalog,
                    w.image,
                )?
                .to_f32();
                if w.hour >= 6 {
                    let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
                        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
                        .wrapping_mul(i32::from(building[8]));
                    let mut stock =
                        initial_stock(kind, volume, w.catalog, w.products, w.commodities)?;
                    stock.add_f32(price)?;
                    price = stock.to_f32();
                }
                if !afford(price, w, &mut call)? {
                    continue;
                }
                let id = selected(w)?;
                call(
                    Call::Convert {
                        kind: kind as u32,
                        building: word(&w.buildings[id], 0),
                    },
                    w,
                )?;
                let id = selected(w)?;
                w.buildings[id][0x3d] = 1;
                call(Call::Disable(action), w)?;
                call(Call::ClearConversionSelection, w)?;
                break;
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires whole original building-panel action observations"]
fn matches_native_controls() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_CONTROLS")).unwrap();
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    assert_eq!(&data[..8], b"MTBCTRL1");
    assert_eq!(word(&data, 8), 0x37f);
    let mut at = 16;
    for case in 0..word(&data, 12) {
        let h: [u32; 9] = std::array::from_fn(|_| {
            let v = word(&data, at);
            at += 4;
            v
        });
        let mut b = vec![];
        for _ in 0..12 {
            let n = word(&data, at) as usize;
            at += 4;
            b.push(&data[at..at + n]);
            at += n;
        }
        let count = word(&data, at);
        at += 4;
        let expected: Vec<Vec<u32>> = (0..count)
            .map(|_| {
                let n = word(&data, at);
                at += 4;
                (0..n)
                    .map(|_| {
                        let v = word(&data, at);
                        at += 4;
                        v
                    })
                    .collect()
            })
            .collect();
        let categories: Vec<[u8; 380]> = b[0]
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect();
        let categories = categories.try_into().unwrap();
        let mut buildings: Vec<[u8; 916]> = b[1]
            .chunks_exact(916)
            .map(|r| r.try_into().unwrap())
            .collect();
        let businesses: Vec<[u8; 544]> = b[2]
            .chunks_exact(544)
            .map(|r| r.try_into().unwrap())
            .collect();
        let catalog: Vec<[u8; 116]> = b[3]
            .chunks_exact(116)
            .map(|r| r.try_into().unwrap())
            .collect();
        let catalog = BusinessCatalog::from_rows(catalog.try_into().unwrap());
        let products = std::array::from_fn(|i| {
            std::array::from_fn(|j| [word(b[4], i * 48 + j * 8), word(b[4], i * 48 + j * 8 + 4)])
        });
        let commodities: Vec<[u8; 28]> = b[5]
            .chunks_exact(28)
            .map(|r| r.try_into().unwrap())
            .collect();
        let commodities = commodities.try_into().unwrap();
        let mut panel = b[7].to_vec();
        let mut controls = b[8].to_vec();
        let mut world = World {
            building_parameters: crate::building_parameters::BuildingKinds::from_image(&image)
                .unwrap(),
            image: &image,
            selected: (h[3] != 0).then_some(0),
            selected_category: h[4] as i32,
            current_player: 0x10030000,
            player_references: &[0x10030000, 0x10030400, 0x10030800],
            cash: f32::from_bits(word(b[6], 4)),
            allowance: h[7] as i32,
            affordability_override: h[8] != 0,
            hour: h[5] as i32,
            ownership_tick: h[6],
            categories: &categories,
            buildings: &mut buildings,
            businesses: &businesses,
            catalog: &catalog,
            products: &products,
            commodities: &commodities,
        };
        let action = match h[1] {
            0 => Action::Trade,
            1 => Action::Demolish,
            _ => Action::Convert {
                selected_label: h[2],
            },
        };
        let mut trace = vec![];
        activate(action, &mut world, |call, _| {
            match call {
                Call::Ui {
                    address,
                    arguments,
                    count,
                } => {
                    let mut row = vec![address];
                    row.extend_from_slice(&arguments[..count]);
                    trace.push(row)
                }
                Call::Disable(action) => {
                    let index = match action {
                        Action::Trade => 0,
                        Action::Demolish => 1,
                        _ => 2,
                    };
                    controls[index * 0x400 + 0x333] = 1;
                    controls[index * 0x400 + 0x335] = 0;
                }
                Call::Sale { business } => trace.push(vec![0x48ae60, business]),
                Call::Takeover { building } => trace.push(vec![0x445300, building]),
                Call::Demolish {
                    category,
                    building,
                    amount,
                } => trace.push(vec![0x48ad20, category as u32, building, amount.to_bits()]),
                Call::Convert { kind, building } => trace.push(vec![0x48adf0, kind, building]),
                Call::ClearConversionSelection => {
                    panel[0xb0] = 0;
                    trace.push(vec![0x4309e0]);
                    panel[0xb4..0xb8].copy_from_slice(&u32::MAX.to_le_bytes());
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(trace, expected, "case{case} calls");
        assert_eq!(
            buildings.iter().flatten().copied().collect::<Vec<_>>(),
            b[9],
            "case{case} buildings"
        );
        assert_eq!(panel, b[10], "case{case} panel");
        assert_eq!(controls, b[11], "case{case} controls");
    }
    assert_eq!(at, data.len());
}
