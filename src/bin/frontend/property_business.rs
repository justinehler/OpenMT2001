//! Original403660/402f50/4033c0 business rows over the retained live world.
use openmt2001::{level_startup::World, pe::PeImage, x87::Sum};
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Integer(i32),
    Money(f32),
    Ratio(u32, u32),
    Label(u32),
}
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Heading(u8),
    Business {
        slot: usize,
        label: u32,
        value: Value,
        accent: bool,
        negative: bool,
    },
    Total {
        owner: u8,
        amount: f32,
    },
    Blank,
    Empty,
}
#[derive(Clone, Debug)]
pub struct Report {
    pub profit: [Vec<Vec<Row>>; 2],
    pub efficiency: [Vec<Vec<Row>>; 2],
    pub value: Vec<Vec<Row>>,
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn short(b: &[u8], at: usize) -> u32 {
    u32::from(u16::from_le_bytes(b[at..at + 2].try_into().unwrap()))
}
fn float(b: &[u8], at: usize) -> f32 {
    f32::from_bits(word(b, at))
}
fn accent(
    b: &[u8; 544],
    info: &[u8; 116],
    products: &[[u32; 2]; 6],
    hour: u32,
    today: bool,
) -> bool {
    let count = usize::from(info[0x4c]);
    match word(b, 0) {
        0 => {
            let offset = if today || hour < 6 {
                0x80
            } else {
                0x8a + usize::from(b[0x209]) * 2
            };
            if today && hour < 6 {
                false
            } else {
                (0..count).map(|i| short(b, offset + i * 0x34)).sum::<u32>() == 0
            }
        }
        1 if today && hour >= 6 => {
            let occupied = if word(b, 4) == 42 {
                short(b, 0x10)
            } else {
                (0..count)
                    .filter(|&i| products[i][0] != u32::MAX)
                    .map(|i| short(b, 0x82 + i * 0x34 + usize::from(b[0x209]) * 2))
                    .sum()
            };
            occupied == short(b, 0xe)
        }
        1 if !today => short(b, 0x12) == short(b, 0xe),
        _ => false,
    }
}
pub struct SavedInputs<'a> {
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub info: &'a [[u8; 116]],
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub hour: u32,
    pub tick: u32,
}
impl Report {
    pub fn from_runtime(w: &World, players: usize, image: &PeImage<'_>) -> Result<Self, String> {
        let memberships =
            crate::block_information::land_rent::runtime_memberships(&w.categories, &w.buildings)?;
        Self::from_memberships(
            &SavedInputs {
                categories: &w.categories,
                buildings: &w.buildings,
                businesses: &w.ledgers.businesses,
                info: w.catalog.info.rows(),
                products: &w.catalog.products,
                hour: w.clock.calendar[0],
                tick: w.clock.timing[0],
            },
            players,
            image,
            &memberships,
        )
    }
    /// Saved membership-index representation; runtime uses separate native links.
    pub fn from_saved_indices(
        w: &SavedInputs<'_>,
        players: usize,
        image: &PeImage<'_>,
    ) -> Result<Self, String> {
        let categories: Vec<_> = w.categories.iter().map(|c| c.as_slice()).collect();
        let memberships = crate::block_information::land_rent::saved_memberships(&categories)?;
        Self::from_memberships(w, players, image, &memberships)
    }
    pub(super) fn from_memberships(
        w: &SavedInputs<'_>,
        players: usize,
        image: &PeImage<'_>,
        memberships: &[Vec<usize>],
    ) -> Result<Self, String> {
        if memberships.len() != w.categories.len() {
            return Err("property report membership categories".into());
        }
        let refs: Vec<u32> = (1..=players as u32).collect();
        let price_world = openmt2001::business_takeover::PriceWorld {
            categories: w.categories,
            buildings: w.buildings,
            businesses: w.businesses,
            player_references: &refs,
        };
        let mut report = Self {
            profit: std::array::from_fn(|_| Vec::new()),
            efficiency: std::array::from_fn(|_| Vec::new()),
            value: Vec::new(),
        };
        for (category_id, membership) in memberships.iter().enumerate() {
            let mut entries = Vec::new();
            for &slot in membership {
                let building = w
                    .buildings
                    .get(slot)
                    .ok_or("property report building index")?;
                let id = word(building, 0x1c);
                if id == u32::MAX {
                    continue;
                }
                let b = w
                    .businesses
                    .get(id as usize)
                    .ok_or("property report business index")?;
                let kind = word(b, 4) as usize;
                if kind >= 45 || usize::from(b[0xa]) > players || b[0x209] >= 4 {
                    return Err("property report business identity".into());
                }
                entries.push((slot, building, b, kind));
            }
            for period in 0..2 {
                let today = period == 1;
                let mut profit = Vec::new();
                let mut efficiency = Vec::new();
                let mut values = Vec::new();
                for owner in (1..=players as u8).chain(std::iter::once(0)) {
                    let matching: Vec<_> = entries
                        .iter()
                        .filter(|(_, _, b, _)| b[0xa] == owner)
                        .collect();
                    if matching.is_empty() {
                        continue;
                    }
                    profit.push(Row::Heading(owner));
                    efficiency.push(Row::Heading(owner));
                    values.push(Row::Heading(owner));
                    let mut total = 0f32;
                    for &&(slot, building, b, kind) in &matching {
                        let info = &w.info[kind];
                        if info[0x4c] > 6 {
                            return Err("property report product count".into());
                        }
                        let label = word(info, 8);
                        let active_accent = accent(b, info, &w.products[kind], w.hour, today);
                        let amount = if today {
                            let mut amount = Sum::from_f32(image.f32_at(0x525040)?)?;
                            for field in 0..13 {
                                amount.add_f32(float(b, 0x24 + field * 4))
                                    .map_err(|e| format!("property profit category{category_id} building{slot} field{:x}: {e}", 0x24 + field * 4))?;
                            }
                            amount
                        } else {
                            Sum::from_f32(
                                openmt2001::ai_actions::liquidation_score::recent_income(b),
                            )?
                        };
                        let amount = amount.ftol_low_word();
                        let mut group = Sum::from_f32(total)?;
                        group.add(Sum::from_i32(amount));
                        total = group.to_f32();
                        profit.push(Row::Business {
                            slot,
                            label,
                            value: Value::Integer(amount),
                            accent: active_accent,
                            negative: amount < 0,
                        });
                        let value = match word(b, 0) {
                            2 => Value::Label(1392),
                            class => {
                                let capacity = if class == 1 {
                                    short(b, 0xe)
                                } else {
                                    let volume = (i32::from(building[7]) - i32::from(building[5])
                                        + 1)
                                    .wrapping_mul(
                                        i32::from(building[6]) - i32::from(building[4]) + 1,
                                    )
                                    .wrapping_mul(i32::from(building[8]));
                                    let mut capacity = Sum::from_i32(volume);
                                    capacity.mul_f32(float(info, 0x58))
                                        .map_err(|e| format!("property capacity category{category_id} building{slot} kind{kind}: {e}"))?;
                                    capacity.ftol_low_word() as u32
                                };
                                let occupied = if kind == 42 {
                                    short(b, if today { 0x10 } else { 0x12 })
                                } else {
                                    let history = if today {
                                        usize::from(b[0x209])
                                    } else {
                                        usize::from((b[0x209] + 3) & 3)
                                    };
                                    (0..usize::from(info[0x4c]))
                                        .filter(|&i| w.products[kind][i][0] != u32::MAX)
                                        .map(|i| short(b, 0x82 + i * 0x34 + history * 2))
                                        .sum()
                                };
                                Value::Ratio(occupied, capacity)
                            }
                        };
                        efficiency.push(Row::Business {
                            slot,
                            label,
                            value,
                            accent: active_accent,
                            negative: false,
                        });
                        let value = if owner == 0 {
                            openmt2001::business_takeover::price(slot, &price_world, w.tick, image)
                                .map_err(|e| {
                                    format!(
                                        "property value category{category_id} building{slot}: {e}"
                                    )
                                })?
                        } else {
                            openmt2001::ai_actions::liquidation_score::business_value(
                                b,
                                w.buildings,
                                image,
                            )?
                        };
                        values.push(Row::Business {
                            slot,
                            label,
                            value: Value::Money(value.to_f32()),
                            accent: false,
                            negative: false,
                        });
                    }
                    profit.push(Row::Total {
                        owner,
                        amount: total,
                    });
                    if owner != 0 {
                        profit.push(Row::Blank);
                    }
                }
                if entries.is_empty() {
                    profit.push(Row::Empty);
                    efficiency.push(Row::Empty);
                    values.push(Row::Empty);
                }
                report.profit[period].push(profit);
                report.efficiency[period].push(efficiency);
                if period == 0 {
                    report.value.push(values);
                }
            }
        }
        Ok(report)
    }
    pub fn rows(&self, category: usize, tab: usize, previous: bool) -> &[Row] {
        match tab {
            2 => &self.profit[usize::from(!previous)][category],
            3 => &self.efficiency[usize::from(!previous)][category],
            4 => &self.value[category],
            _ => &[],
        }
    }
}

#[cfg(test)]
#[path = "property_business_test.rs"]
mod tests;
