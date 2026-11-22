//! Original407010 building report and406d00/406e50 detail pages.
//! Read-only snapshots preserve the original arithmetic and language identifiers.
use crate::{player_manager::word, x87::Sum};
mod axes;
pub mod text;
pub use axes::AxisItem;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Language(u32),
    /// Original one-based business owner; zero selects city ownership text1116.
    Owner(u8),
    Money(f32),
    /// Original 43a8c0 formats the land-rent summary as whole currency.
    IntegerMoney(i32),
    Number(f64),
    Integer(u32),
    Empty,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: u32,
    pub value: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub summary: Vec<Row>,
    pub income: Vec<Row>,
    pub expenses: Vec<Row>,
    /// Original430450 style, conditional accent, and accent-enabled arguments.
    pub highlight: [u32; 3],
}
pub struct Inputs<'a> {
    pub business: &'a [u8; 544],
    pub building: &'a [u8; 916],
    pub category: &'a [u8; 380],
    pub info: &'a [u8; 116],
    pub products: &'a [[u32; 2]; 6],
    pub commodities: &'a [[u8; 28]; 32],
    pub owns_business_and_land: bool,
    pub hour: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub points: [[f32; 2]; 4],
    /// Original rectangles are x,y,width,height, in640-pixel UI coordinates.
    pub rect: [f32; 4],
    pub plot: [f32; 4],
    /// x/y minima followed by x/y spans, after original42ccc0 padding.
    pub domain: [f32; 4],
}
pub type Stroke = [[[f32; 2]; 4]; 3];
impl Graph {
    /// Original42d970 mitered four-pixel stroke, in native submission order.
    /// Finite geometry uses PC64 arithmetic and original f32 spills. The CRT
    /// acos/FSIN pair uses host f64 transcendental functions; native vertex
    /// captures bound the resulting screen-coordinate error in the tests.
    pub fn stroke(&self) -> Result<Stroke, String> {
        let project = |point: [f32; 2]| -> Result<[Sum; 2], String> {
            let mut x = Sum::from_f32(point[0])?;
            x.add_f32(-self.domain[0])?;
            x.mul_f32(self.plot[2])?;
            x.div_f32(self.domain[2])?;
            x.add_f32(self.plot[0])?;
            let mut y = Sum::from_f32(point[1])?;
            y.add_f32(-self.domain[1])?;
            y.mul_f32(self.plot[3])?;
            y.div_f32(self.domain[3])?;
            let mut bottom = Sum::from_f32(self.plot[3])?;
            bottom.add_f32(self.plot[1])?;
            bottom.add(y.negated());
            Ok([x, bottom])
        };
        let normalize = |mut v: [Sum; 2]| -> Result<[Sum; 2], String> {
            let mut length = v[0];
            length.mul(v[0]);
            let mut y = v[1];
            y.mul(v[1]);
            length.add(y);
            let length = length.sqrt()?;
            if !length.compare(Sum::default()).is_eq() {
                v[0].div(length)?;
                v[1].div(length)?;
            }
            Ok(v)
        };
        let mut plus = [[0.; 2]; 4];
        let mut minus = [[0.; 2]; 4];
        for i in 0..4 {
            let center = project(self.points[i])?.map(Sum::to_f32);
            let previous = if i == 0 {
                [
                    2. * self.points[0][0] - self.points[1][0],
                    self.points[1][1],
                ]
            } else {
                self.points[i - 1]
            };
            let next = if i == 3 {
                [
                    2. * self.points[3][0] - self.points[2][0],
                    self.points[2][1],
                ]
            } else {
                self.points[i + 1]
            };
            let direction = |p| -> Result<[Sum; 2], String> {
                let mut v = project(p)?;
                for j in 0..2 {
                    v[j].add_f32(-center[j])?;
                }
                normalize(v)
            };
            let previous = direction(previous)?;
            let next = direction(next)?;
            let mut normal_y = next[0];
            normal_y.add(previous[0].negated());
            let mut normal_x = previous[1];
            normal_x.add(next[1].negated());
            let (nx, ny) = (normal_x.to_f32(), normal_y.to_f32());
            let mut length = normal_y;
            length.mul_f32(ny)?;
            let mut nx2 = Sum::from_f32(nx)?;
            nx2.mul_f32(nx)?;
            length.add(nx2);
            let length = length.sqrt()?;
            let normal = if length.compare(Sum::default()).is_eq() {
                [nx, ny]
            } else {
                let mut x = Sum::from_f32(nx)?;
                x.div(length)?;
                let mut y = Sum::from_f32(ny)?;
                y.div(length)?;
                [x.to_f32(), y.to_f32()]
            };
            let mut dot = next[0];
            dot.mul(previous[0]);
            let mut dy = next[1];
            dy.mul(previous[1]);
            dot.add(dy);
            let dot = dot.to_f32().clamp(-1., 1.);
            let sine = (f64::from(dot).acos() * 0.5).sin();
            let mut scale = Sum::from_f64(if sine < f64::from(0.1f32) {
                10.
            } else {
                1. / sine
            })?;
            scale.mul_f32(4.)?;
            scale.mul_f32(0.5)?;
            let mut dx = scale;
            dx.mul_f32(normal[0])?;
            let dx = dx.to_f32();
            let mut dy = scale;
            dy.mul_f32(normal[1])?;
            for (side, sign) in [(&mut plus, 1.), (&mut minus, -1.)] {
                let mut x = Sum::from_f32(center[0])?;
                x.add_f32(sign * dx)?;
                let mut y = Sum::from_f32(center[1])?;
                y.add(if sign > 0. { dy } else { dy.negated() });
                side[i] = [x.to_f32(), y.to_f32()];
            }
        }
        Ok(std::array::from_fn(|at| {
            let i = 2 - at;
            [minus[i], plus[i], plus[i + 1], minus[i + 1]]
        }))
    }
}
///4066e0 data and rectangle construction, followed by42ccc0 domain fitting.
/// Axis/text rasterization and42d970 thick-line joins belong to the renderer.
pub fn graph(
    business: &[u8; 544],
    calendar: [u32; 2],
    bounds: [f32; 4],
    image: &crate::pe::PeImage<'_>,
) -> Result<Graph, String> {
    let year = calendar[0]
        .wrapping_add(calendar[1].wrapping_mul(10))
        .wrapping_add(1930);
    let mut total = Sum::from_f32(0.)?;
    for at in (0x24..0x58).step_by(4) {
        total.add_f32(float(business, at))?;
    }
    let points = std::array::from_fn(|i| {
        [
            year.wrapping_sub((3 - i) as u32 * 5) as i32 as f32,
            if i == 3 {
                total.to_f32()
            } else {
                float(
                    business,
                    0x58 + usize::from(business[0x209].wrapping_add(i as u8 + 1) & 3) * 4,
                )
            },
        ]
    });
    let mut left = Sum::from_f32(bounds[0])?;
    left.add_f32(image.f32_at(0x525020)?)?;
    let mut top = Sum::from_f32(bounds[1])?;
    top.add_f32(image.f32_at(0x525088)?)?;
    let mut width = Sum::from_f32(bounds[2])?;
    width.add_f32(-bounds[0])?;
    width.add_f32(-image.f32_at(0x525084)?)?;
    let rect = [
        left.ftol_low_word() as f32,
        top.ftol_low_word() as f32,
        width.ftol_low_word() as f32,
        102.,
    ];
    let mut plot_left = Sum::from_f32(rect[0])?;
    plot_left.add_f32(image.f32_at(0x525080)?)?;
    let mut plot_width = Sum::from_f32(rect[2])?;
    plot_width.add_f32(-image.f32_at(0x52507c)?)?;
    let plot = [plot_left.to_f32(), rect[1], plot_width.to_f32(), 92.];
    let mut min = [f32::MAX, 0f32];
    let mut max = [-f32::MAX, 0f32];
    for p in points {
        for i in 0..2 {
            if p[i] < min[i] {
                min[i] = p[i];
            }
            if p[i] > max[i] {
                max[i] = p[i];
            }
        }
    }
    let mut delta = Sum::from_f32(max[1])?;
    delta.add_f32(-min[1])?;
    let (lower_padding, upper_padding) = if delta.compare(Sum::default()).is_eq() {
        (Sum::from_f32(1.)?, 10.)
    } else {
        let mut pad = Sum::from_f32(delta.to_f32())?;
        pad.mul_f32(image.f32_at(0x525048)?)?;
        (pad, pad.to_f32())
    };
    let mut upper = Sum::from_f32(upper_padding)?;
    upper.add_f32(max[1])?;
    let mut lower = Sum::from_f32(min[1])?;
    lower.add(lower_padding.negated());
    let (upper, lower) = (upper.to_f32(), lower.to_f32());
    let domain = if max[0] > min[0] && upper > lower {
        let mut x = Sum::from_f32(max[0])?;
        x.add_f32(-min[0])?;
        let mut y = Sum::from_f32(upper)?;
        y.add_f32(-lower)?;
        [min[0], lower, x.to_f32(), y.to_f32()]
    } else {
        [0.; 4]
    };
    Ok(Graph {
        points,
        rect,
        plot,
        domain,
    })
}
fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}
fn short(bytes: &[u8], at: usize) -> u32 {
    u32::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

///4455c0: remaining rent-age percentage, waived when the linked category owner
/// holds the selected building's complete color group. The linked building and
/// selected building are intentionally resolved separately, as in native code.
pub fn rent_age(
    building: usize,
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    categories: &[[u8; 380]],
    tick: u32,
    image: &crate::pe::PeImage<'_>,
) -> Result<i32, String> {
    let b = buildings
        .get(building)
        .ok_or("rent-age selected building outside pool")?;
    let business = businesses
        .get(word(b, 0x1c) as usize)
        .ok_or("rent-age business outside pool")?;
    let linked = i16::from_le_bytes(business[8..10].try_into().unwrap());
    let linked = buildings
        .get(linked as usize)
        .ok_or("rent-age linked building outside pool")?;
    let category = |building: &[u8; 916]| -> Result<&[u8; 380], String> {
        let offset = word(building, 0x40)
            .checked_sub(0x617628)
            .ok_or("rent-age category before pool")?;
        if !offset.is_multiple_of(380) {
            return Err("rent-age category reference unaligned".into());
        }
        categories
            .get((offset / 380) as usize)
            .ok_or("rent-age category outside pool".into())
    };
    //462f30 returns a signed16-bit category id, followed by a separate lookup.
    let id = i16::from_le_bytes(category(linked)?[..2].try_into().unwrap());
    let owner = categories
        .get(id as usize)
        .ok_or("rent-age linked category id outside pool")?[5];
    if (owner as i8) < 0 {
        return Err("rent-age owner outside original roster".into());
    }
    if owner != 0 {
        let selected = word(category(b)?, 0);
        let color = u32::from(image.at(0x5385a4 + selected * 88, 1)?[0]);
        let mut count = 0;
        let mut owned = 0;
        for i in 0..4 {
            let id = word(image.at(0x5397ac + color * 60 + i * 4, 4)?, 0) as i32;
            if id == -1 {
                continue;
            }
            let row = categories
                .get(id as usize)
                .ok_or("rent-age group outside category pool")?;
            count += 1;
            owned += usize::from(row[5] == owner);
        }
        if count != 0 && count == owned {
            return Ok(0);
        }
    }
    let age = ((u64::from(tick.wrapping_sub(word(b, 0x30))) * 0xd1b71759) >> 41) as i32;
    Ok(100i32.wrapping_sub(age).max(0))
}
fn row(label: u32, value: Value) -> Row {
    Row { label, value }
}

///462740(true) computes the report accent. The original writes business+21d;
/// this snapshot computes the same byte without modifying simulation state.
fn accent(w: &Inputs<'_>, count: usize) -> Result<bool, String> {
    if w.hour < 6 {
        return Ok(false);
    }
    let b = w.business;
    match word(b, 0) {
        0 => Ok((0..count).map(|i| short(b, 0x80 + i * 0x34)).sum::<u32>() == 0),
        1 => {
            let residents = if word(b, 4) == 42 {
                short(b, 0x10)
            } else {
                let period = usize::from(b[0x209]);
                if period >= 4 {
                    return Err("business report history period outside four".into());
                }
                (0..count)
                    .filter(|&i| w.products[i][0] != u32::MAX)
                    .map(|i| short(b, 0x82 + i * 0x34 + period * 2))
                    .sum()
            };
            Ok(residents == short(b, 0xe))
        }
        _ => Ok(false),
    }
}

pub fn inspect(w: &Inputs<'_>, image: &crate::pe::PeImage<'_>) -> Result<Report, String> {
    let b = w.business;
    let class = word(b, 0);
    let count = usize::from(w.info[0x4c]);
    if count > 6 {
        return Err("business report product count exceeds six".into());
    }
    let mut income = Sum::from_f32(0.)?;
    let mut income_rows = Vec::new();
    if class == 1 {
        income = Sum::from_f32(float(b, 0x50))?;
        income_rows.push(row(69, Value::Integer(short(b, 0x10))));
    } else {
        for i in 0..count {
            let id = w.products[i][0];
            if id == u32::MAX {
                break;
            }
            let commodity = w
                .commodities
                .get(id as usize)
                .ok_or("business report commodity outside table")?;
            let value = float(b, 0xa4 + i * 0x34);
            income.add_f32(value)?;
            income_rows.push(row(word(commodity, 4), Value::Money(value)));
        }
    }
    let income = income.to_f32();
    //406c90 spills rent+stock, then retains the remaining four additions in ST0.
    let mut expense = Sum::from_f32(float(b, 0x28))?;
    if class != 1 {
        expense.add_f32(float(b, 0x30))?;
    }
    let mut expense = Sum::from_f32(expense.to_f32())?;
    expense.add_f32(float(b, 0x40))?;
    for at in [0x38, 0x44, 0x3c] {
        expense.add_f32(float(b, at))?;
    }
    let expense = expense.to_f32();
    let mut profit = Sum::from_f32(expense)?;
    profit.add_f32(income)?;
    let profit = profit.to_f32();
    let accented = accent(w, count)?;
    let highlight = [
        if profit < 0. { 10 } else { 9 },
        if accented {
            if profit < 0. { 2 } else { 3 }
        } else {
            0
        },
        u32::from(accented),
    ];
    let mut summary = vec![row(1408, Value::Owner(b[10]))];
    if class == 2 {
        let volume = (i32::from(w.building[7]) - i32::from(w.building[5]) + 1)
            .wrapping_mul(i32::from(w.building[6]) - i32::from(w.building[4]) + 1)
            .wrapping_mul(i32::from(w.building[8]));
        let mut size = Sum::from_f64(f64::from(volume))?;
        size.mul_f32(image.f32_at(0x52508c)?)?;
        summary.push(row(1409, Value::Number(size.to_f64())));
        summary.push(row(1410, Value::Number(f64::from(float(w.category, 0x84)))));
        return Ok(Report {
            summary,
            income: vec![],
            expenses: vec![],
            highlight: [0; 3],
        });
    }
    summary.push(row(
        1158,
        if w.owns_business_and_land {
            Value::Language(44)
        } else {
            Value::IntegerMoney(short(b, 0x18) as i32)
        },
    ));
    summary.push(row(
        1616,
        Value::Language(if word(b, 0x6c) == 0 { 1378 } else { 1379 }),
    ));
    summary.push(row(
        1411,
        match w.building[0x2c] as i8 {
            0 => Value::Language(1656),
            1 => Value::Language(1655),
            2 => Value::Language(1657),
            _ => Value::Empty,
        },
    ));
    summary.extend([
        row(63, Value::Money(income)),
        row(64, Value::Money(-expense)),
        row(1412, Value::Money(profit)),
    ]);
    income_rows.push(row(66, Value::Money(income)));
    let mut expenses = vec![row(
        73,
        if w.owns_business_and_land {
            Value::Language(44)
        } else {
            Value::Money(-float(b, 0x28))
        },
    )];
    if class != 1 {
        expenses.push(row(74, Value::Money(-float(b, 0x30))));
    }
    for (label, at) in [(75, 0x44), (76, 0x38), (77, 0x40), (78, 0x3c)] {
        expenses.push(row(label, Value::Money(-float(b, at))));
    }
    expenses.push(row(66, Value::Money(-expense)));
    Ok(Report {
        summary,
        income: income_rows,
        expenses,
        highlight,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires private native building graph fixture"]
    fn matches_native_graph() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_BUSINESS_GRAPH").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        assert_eq!(&bytes[..8], b"MTBGRP02");
        assert_eq!(word(&bytes, 8), 0x37f);
        assert_eq!(bytes.len(), 16 + word(&bytes, 12) as usize * 840);
        let mut largest_error = 0f32;
        for (case, b) in bytes[16..].chunks_exact(840).enumerate() {
            let business = b[..544].try_into().unwrap();
            let calendar = [word(b, 544), word(b, 548)];
            let bounds = std::array::from_fn(|i| float(b, 552 + i * 4));
            let actual = graph(business, calendar, bounds, &image).unwrap();
            let stroke = actual.stroke().unwrap();
            let actual = actual
                .points
                .into_iter()
                .flatten()
                .chain(actual.rect)
                .chain(actual.plot)
                .chain(actual.domain)
                .map(f32::to_bits)
                .collect::<Vec<_>>();
            let expected = b[568..648]
                .chunks_exact(4)
                .map(|v| word(v, 0))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "case{case}");
            for (segment, quad) in stroke.iter().enumerate() {
                let capture = &b[648 + segment * 64..648 + (segment + 1) * 64];
                for (v, xy) in quad.iter().enumerate() {
                    for (axis, &coordinate) in xy.iter().enumerate() {
                        let native = float(capture, v * 12 + axis * 4);
                        let error = (coordinate - native).abs();
                        largest_error = largest_error.max(error);
                        assert!(
                            error <= 0.0002,
                            "case{case} segment{segment} vertex{v} axis{axis}: host{coordinate} native{native} error{error}"
                        );
                    }
                    assert_eq!(float(capture, v * 12 + 8), 0.5);
                    assert_eq!(word(capture, 48 + v * 4), 0x9fbf0000);
                }
            }
        }
        println!("Largest native graph stroke coordinate difference: {largest_error} pixels");
    }
    #[test]
    #[ignore = "requires private native building rent-age fixture"]
    fn matches_native_rent_age() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILDING_RENT_AGE").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        assert_eq!(&bytes[..8], b"MTBRAG01");
        assert_eq!(bytes.len(), 12 + word(&bytes, 8) as usize * 72);
        for (case, b) in bytes[12..].chunks_exact(72).enumerate() {
            let mut buildings = [[0u8; 916]; 2];
            let mut businesses = [[0u8; 544]; 1];
            let mut categories = [[0u8; 380]; 52];
            for (i, c) in categories.iter_mut().enumerate() {
                c[..4].copy_from_slice(&(i as u32).to_le_bytes());
                c[5] = b[16 + i];
            }
            for (i, building) in buildings.iter_mut().enumerate() {
                building[0x40..0x44]
                    .copy_from_slice(&(0x617628 + word(b, i * 4) * 380).to_le_bytes());
            }
            buildings[0][0x30..0x34].copy_from_slice(&b[12..16]);
            businesses[0][8..10].copy_from_slice(&1u16.to_le_bytes());
            assert_eq!(
                rent_age(0, &buildings, &businesses, &categories, word(b, 8), &image).unwrap()
                    as u32,
                word(b, 68),
                "case{case}"
            );
        }
    }
    #[test]
    #[ignore = "requires private whole native business report fixture"]
    fn matches_native_business_reports() {
        let path = std::env::var("OPENMT2001_NATIVE_BUSINESS_REPORT").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut at = 0;
        fn take<const N: usize>(b: &[u8], at: &mut usize) -> [u8; N] {
            let out = b[*at..*at + N].try_into().unwrap();
            *at += N;
            out
        }
        fn u32le(b: &[u8], at: &mut usize) -> u32 {
            u32::from_le_bytes(take(b, at))
        }
        fn rows(b: &[u8], at: &mut usize) -> Vec<Row> {
            (0..u32le(b, at))
                .map(|_| {
                    let label = u32le(b, at);
                    let kind = u32le(b, at);
                    let value = u64::from_le_bytes(take(b, at));
                    row(
                        label,
                        match kind {
                            0 => Value::Language(value as u32),
                            1 => Value::Owner(value as u8),
                            2 => Value::Money(f32::from_bits(value as u32)),
                            6 => Value::IntegerMoney(value as u32 as i32),
                            3 => Value::Number(f64::from_bits(value)),
                            4 => Value::Integer(value as u32),
                            5 => Value::Empty,
                            _ => panic!("report kind{kind}"),
                        },
                    )
                })
                .collect()
        }
        assert_eq!(&take::<8>(&bytes, &mut at), b"MTBRPT01");
        assert_eq!(u32le(&bytes, &mut at), 0x37f);
        for case in 0..u32le(&bytes, &mut at) {
            let business = take(&bytes, &mut at);
            let building = take(&bytes, &mut at);
            let category = take(&bytes, &mut at);
            let info = take(&bytes, &mut at);
            let products = std::array::from_fn(|_| std::array::from_fn(|_| u32le(&bytes, &mut at)));
            let commodities = std::array::from_fn(|_| take(&bytes, &mut at));
            let owns_business_and_land = u32le(&bytes, &mut at) != 0;
            let hour = u32le(&bytes, &mut at) as i32;
            let expected = Report {
                summary: rows(&bytes, &mut at),
                income: rows(&bytes, &mut at),
                expenses: rows(&bytes, &mut at),
                highlight: std::array::from_fn(|_| u32le(&bytes, &mut at)),
            };
            let actual = inspect(
                &Inputs {
                    business: &business,
                    building: &building,
                    category: &category,
                    info: &info,
                    products: &products,
                    commodities: &commodities,
                    owns_business_and_land,
                    hour,
                },
                &image,
            )
            .unwrap();
            assert_eq!(actual, expected, "case{case}");
        }
        assert_eq!(at, bytes.len());
    }
}
