//! Original00461180 initial business stock, pricing, rent and schedule stores.
use crate::{
    building_binding::{StockProduct, initial_stock_cost},
    economy::ftol_low_word,
    pe::PeImage,
};

pub struct Product {
    pub value: f32,
    pub markup: f32,
    pub fraction: f32,
}
pub struct Inputs<'a> {
    pub hour: i32,
    /// Original822d40, supplied independently of the phase/hour.
    pub timestamp: u32,
    pub initializer: u8,
    pub info: &'a [u8; 0x74],
    pub products: &'a [Product],
}
fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn float(row: &[u8], at: usize) -> f32 {
    f32::from_bits(word(row, at))
}
fn put_float(row: &mut [u8], at: usize, value: f32) {
    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_word(row: &mut [u8], at: usize, value: u32) {
    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// The charge callback is original462a00(amount,3). It runs after the cost
/// balance store and before retail pricing, and must update the live record.
pub fn initialize(
    business: &mut [u8; 0x220],
    building: &mut [u8; 0x394],
    input: &Inputs<'_>,
    image: &PeImage<'_>,
    charge: &mut impl FnMut(&mut [u8; 0x220], f32) -> Result<(), String>,
) -> Result<(), String> {
    put_word(business, 0x1c, 0);
    let kind = word(business, 0);
    if kind == 1 {
        let price = word(building, 0x28) as i32;
        let multiplier = image.f32_at(if word(business, 4) == 43 {
            0x525084
        } else {
            0x525214
        })?;
        // Native FILD/FTOL roundtrip preserves the signed integer price.
        put_word(building, 0x28, price as u32);
        let rent = (f64::from(price) * f64::from(multiplier)) as f32;
        put_float(business, 0x1dc, rent);
        put_float(business, 0x204, rent);
        let capacity = u16::from_le_bytes(business[0xe..0x10].try_into().unwrap());
        put_float(
            business,
            0x20,
            (f64::from(capacity) * f64::from(rent)) as f32,
        );
    }
    let count = usize::from(input.info[0x4c]);
    if count > 6 {
        return Err("initializer catalog count exceeds six".into());
    }
    if kind == 0 {
        let products = input
            .products
            .get(..count)
            .ok_or("initializer products are truncated")?;
        let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
            .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
            .wrapping_mul(i32::from(building[8]));
        let capacity =
            crate::building_binding::initial_stock_count(volume, float(input.info, 0x58))?;
        let scale = if products.iter().all(|p| p.fraction.is_finite()) {
            let mut sum = crate::x87::Sum::default();
            for product in products {
                sum.add_f32(product.fraction)?;
            }
            if sum.compare(crate::x87::Sum::default()).is_eq() {
                // Unlike4610d0, the initializer divides even for an empty or
                // zero-weight catalog. Its masked infinity/NaN spills lead to
                // FTOL's zero low word in any subsequent target calculation.
                (f64::from(capacity) * (1. / 0f64)) as f32
            } else {
                let mut scale = crate::x87::Sum::from_i32(1);
                scale.div(sum)?;
                scale.mul(crate::x87::Sum::from_i32(capacity));
                scale.to_f32()
            }
        } else {
            let sum = products
                .iter()
                .fold(0f64, |sum, p| sum + f64::from(p.fraction));
            (f64::from(capacity) * (1. / sum)) as f32
        };
        for (slot, product) in products.iter().enumerate() {
            let target = ftol_low_word(f64::from(scale) * f64::from(product.fraction)) as u16;
            for offset in [0x1b8 + slot * 2, 0x1e0 + slot * 2] {
                business[offset..offset + 2].copy_from_slice(&target.to_le_bytes());
            }
            if input.hour >= 6 {
                let offset = 0x80 + slot * 0x34;
                business[offset..offset + 2].copy_from_slice(&target.to_le_bytes());
            }
        }
        let stock_products: Vec<_> = products
            .iter()
            .map(|p| StockProduct {
                value: p.value,
                fraction: p.fraction,
            })
            .collect();
        let cost = initial_stock_cost(volume, float(input.info, 0x58), &stock_products)?;
        if input.hour >= 6 {
            let mut balance = cost.negated();
            balance.add_f32(float(business, 0x30))?;
            put_float(business, 0x30, balance.to_f32());
            if input.initializer == 0 {
                charge(business, cost.negated().to_f32())?;
            }
        }
        put_word(business, 0x20, 0);
        let bias = f64::from_le_bytes(image.at(0x525c38, 8)?.try_into().unwrap());
        let ten = f64::from(image.f32_at(0x5250a0)?);
        let tenth = f64::from(image.f32_at(0x52524c)?);
        for (slot, product) in products.iter().enumerate() {
            let rounded =
                ftol_low_word((f64::from(product.value) * f64::from(product.markup) + bias) * ten);
            let retained_price = f64::from(rounded) * tenth;
            let price = retained_price as f32;
            put_float(business, 0x1c4 + slot * 4, price);
            put_float(business, 0x1ec + slot * 4, retained_price as f32);
            let at = 0x80 + slot * 0x34;
            let stock = u16::from_le_bytes(business[at..at + 2].try_into().unwrap());
            let profit = (f64::from(price) - f64::from(product.value)) * f64::from(stock)
                + f64::from(float(business, 0x20));
            put_float(business, 0x20, profit as f32);
        }
    }
    if kind == 1 {
        put_word(business, 0x6c, 0);
    } else if word(business, 0x6c) != 2 {
        let opening = i32::from(business[0x68] as i8);
        let closing = i32::from(business[0x69] as i8);
        let closed = input.hour < opening || (closing > opening && input.hour >= closing);
        put_word(business, 0x6c, u32::from(closed));
    }
    let elapsed = input.timestamp.wrapping_sub(word(business, 0x214)) as i32;
    if elapsed >= 100 || elapsed == 0 {
        put_word(business, 0x214, input.timestamp);
    }
    let total = (0..count).fold(0_u32, |total, slot| {
        let at = 0x80 + slot * 0x34;
        total.wrapping_add(u32::from(u16::from_le_bytes(
            business[at..at + 2].try_into().unwrap(),
        )))
    });
    put_word(business, 0x218, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a> {
        data: &'a [u8],
        at: usize,
    }
    impl Reader<'_> {
        fn bytes<const N: usize>(&mut self) -> [u8; N] {
            let value = self.data[self.at..self.at + N].try_into().unwrap();
            self.at += N;
            value
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.bytes())
        }
        fn float(&mut self) -> f32 {
            f32::from_bits(self.word())
        }
    }
    #[test]
    #[ignore = "requires original business initialization fixture and executable"]
    fn matches_native_initial_business_state() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_BUSINESS_INITIALIZE").unwrap()).unwrap();
        let mut reader = Reader { data: &data, at: 0 };
        assert_eq!(reader.bytes::<8>(), *b"MTBINIT2");
        assert_eq!(reader.word(), 2);
        let count = reader.word();
        assert_eq!(reader.word(), 0x37f);
        for case in 0..count {
            let controls: [u32; 8] = std::array::from_fn(|_| reader.word());
            let _numeric: [f32; 4] = std::array::from_fn(|_| reader.float());
            let products: [Product; 6] = std::array::from_fn(|_| {
                let _identity = reader.word();
                let value = reader.float();
                let fraction = reader.float();
                let markup = reader.float();
                let _stock = reader.word();
                Product {
                    value,
                    markup,
                    fraction,
                }
            });
            let trace: [u32; 32] = std::array::from_fn(|_| reader.word());
            let before_globals: [u32; 6] = std::array::from_fn(|_| reader.word());
            let after_globals: [u32; 6] = std::array::from_fn(|_| reader.word());
            let mut business = reader.bytes::<0x220>();
            let expected_business = reader.bytes::<0x220>();
            let mut building = reader.bytes::<0x394>();
            let expected_building = reader.bytes::<0x394>();
            let info = reader.bytes::<0x74>();
            assert_eq!(info, reader.bytes::<0x74>());
            let catalog = reader.bytes::<48>();
            assert_eq!(catalog, reader.bytes::<48>());
            let input = Inputs {
                hour: controls[3] as i32,
                timestamp: before_globals[2],
                initializer: controls[4] as u8,
                info: &info,
                products: &products[..controls[6] as usize],
            };
            let mut charges = 0;
            initialize(
                &mut business,
                &mut building,
                &input,
                &image,
                &mut |record, amount| {
                    assert_eq!(record[0xa], 0, "fixture charge requires unowned business");
                    let balance = (f64::from(float(record, 0x1c)) + f64::from(amount)) as f32;
                    put_float(record, 0x1c, balance);
                    charges += 1;
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                charges,
                trace.iter().filter(|&&a| a == 0x462a00).count(),
                "case {case}"
            );
            for (offset, (&actual, &expected)) in
                business.iter().zip(&expected_business).enumerate()
            {
                assert_eq!(actual, expected, "case {case} business+{offset:x}");
            }
            assert_eq!(building, expected_building, "case {case}");
            assert_eq!(word(&business, 0x214), after_globals[4]);
            assert_eq!(word(&business, 0x218), after_globals[5]);
        }
        assert_eq!(reader.at, data.len());
    }
}
