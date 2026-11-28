//! Original building/business binding calculations, independent of host I/O.

pub struct StockProduct {
    pub value: f32,
    pub fraction: f32,
}

/// Live inputs consumed by445720 and its cost helpers. Owner references are
/// opaque identities used only for equality, never host addresses.
pub struct ConstructionInputs<'a> {
    pub precision: crate::x87::Precision,
    pub volume: i32,
    pub kind: u32,
    pub quality: i8,
    pub category_value: f32,
    pub hour: i32,
    pub discount: f32,
    pub owner: u32,
    pub utility_owners: [u32; 4],
    pub business: &'a [u8; 0x74],
    /// Original ordered7f3ed0 catalog, with live commodity values resolved.
    pub products: &'a [StockProduct],
}

pub struct ConstructionCost {
    pub total: i32,
    /// Original6175fc/617600/617604/617608, in address order.
    pub components: [f32; 4],
}

impl ConstructionCost {
    /// Original AddBuilding4da946..4da962: FTOL low DWORD, signed FILD,
    /// then FSTP f32. The constructor copies these bits without conversion.
    pub fn building_argument_bits(&self) -> u32 {
        (self.total as f32).to_bits()
    }
}

/// Original4610d0/461180 capacity FILD, FMUL and FTOL low-word sequence.
pub(crate) fn initial_stock_count(volume: i32, rate: f32) -> Result<i32, String> {
    if !rate.is_finite() {
        // Original FTOL's masked invalid conversion returns integer-indefinite;
        // these callers reload only its zero low DWORD with signed FILD.
        return Ok(0);
    }
    let mut count = crate::x87::Sum::from_i32(volume);
    count.mul_f32(rate)?;
    Ok(count.ftol_low_word())
}

/// Original4610d0 retains its accumulator in x87. Its caller stores a rounded
/// component but adds the retained value to the running construction total.
pub fn initial_stock_cost(
    volume: i32,
    rate: f32,
    products: &[StockProduct],
) -> Result<crate::x87::Sum, String> {
    initial_stock_cost_with_precision(volume, rate, products, crate::x87::Precision::Pc64)
}

fn initial_stock_cost_with_precision(
    volume: i32,
    rate: f32,
    products: &[StockProduct],
    precision: crate::x87::Precision,
) -> Result<crate::x87::Sum, String> {
    use crate::x87::Sum;
    let count = if rate.is_finite() {
        precision
            .mul(Sum::from_i32(volume), Sum::from_f32(rate)?)
            .ftol_low_word()
    } else {
        0
    };
    if products.iter().any(|p| !p.fraction.is_finite()) {
        let sum = products.iter().fold(0f64, |a, p| a + f64::from(p.fraction));
        if sum.is_nan() {
            // FCOM's unordered result sets the same tested bit as equality.
            return Ok(Sum::default());
        }
        return Err("nonfinite initial-stock fraction sum".into());
    }
    let mut sum = Sum::default();
    for product in products {
        sum = precision.add(sum, Sum::from_f32(product.fraction)?);
    }
    if sum.compare(Sum::default()).is_eq() {
        return Ok(Sum::default());
    }
    let mut scale = Sum::from_i32(1);
    scale = precision.div(scale, sum)?;
    scale = precision.mul(scale, Sum::from_i32(count));
    let mut total = Sum::default();
    for product in products {
        let mut term = Sum::from_f32(product.value)?;
        term = precision.mul(term, Sum::from_f32(product.fraction)?);
        term = precision.mul(term, scale);
        total = precision.add(total, term);
    }
    Ok(total)
}

/// Original445720, including the pre-six-hour branch and the later stock,
/// rent and utility charges. Inputs must reflect the current live settings;
/// immutable image defaults alone do not represent a configured game.
pub fn construction_cost(
    input: &ConstructionInputs<'_>,
    image: &crate::pe::PeImage<'_>,
) -> Result<ConstructionCost, String> {
    let type_address = input
        .kind
        .checked_mul(0x2e)
        .and_then(|n| n.checked_add(0x5365f0))
        .ok_or("building type address overflow")?;
    let type_row = image.at(type_address, 4)?;
    let unit_price = u16::from_le_bytes(type_row[..2].try_into().unwrap());
    let base_price = u16::from_le_bytes(type_row[2..].try_into().unwrap());
    construction_cost_words(input, image, [unit_price, base_price])
}
/// Quote using the configured Session resource; literal image helper remains
/// available for original observations and geometry previews.
pub fn construction_cost_with_parameters(
    input: &ConstructionInputs<'_>,
    parameters: &crate::building_parameters::BuildingKinds,
    image: &crate::pe::PeImage<'_>,
) -> Result<ConstructionCost, String> {
    let [rent, cost, _] = parameters.row(input.kind)?;
    construction_cost_words(input, image, [rent, cost])
}
fn construction_cost_words(
    input: &ConstructionInputs<'_>,
    image: &crate::pe::PeImage<'_>,
    [unit_price, base_price]: [u16; 2],
) -> Result<ConstructionCost, String> {
    let word = |offset| u32::from_le_bytes(input.business[offset..offset + 4].try_into().unwrap());
    let float = |offset| f32::from_bits(word(offset));
    let double = |address| -> Result<f64, String> {
        Ok(f64::from_le_bytes(
            image.at(address, 8)?.try_into().unwrap(),
        ))
    };
    let quality = match input.quality {
        0 => 1_f32,
        1 => 1.2_f32,
        _ => 1.5_f32,
    };
    let installation_quality = image.f32_at(match input.quality {
        0 => 0x525034,
        1 => 0x525b10,
        _ => 0x5251e8,
    })?;
    use crate::x87::Sum;
    let precision = input.precision;
    let f = |value| Sum::from_f32(value);
    let scale = precision
        .add(
            precision.mul(
                Sum::from_i32(input.volume),
                Sum::from_f64(double(0x525b08)?)?,
            ),
            Sum::from_f64(double(0x525b00)?)?,
        )
        .to_f32();
    let category = precision
        .add(
            precision.mul(f(input.category_value)?, f(image.f32_at(0x52502c)?)?),
            f(image.f32_at(0x525034)?)?,
        )
        .to_f32();
    let mut base = precision.mul(
        precision.mul(Sum::from_i32(i32::from(base_price)), f(scale)?),
        precision.mul(f(quality)?, f(category)?),
    );
    if input.kind != 2 {
        // The installation DWORD is zero-extended before FILD qword.
        let mut installation = Sum::from_f64(f64::from(word(0x54)))?;
        for value in [installation_quality, category, scale] {
            installation = precision.mul(installation, f(value)?);
        }
        base = precision.add(base, installation);
    }
    let mut components = [base.to_f32(), 0., 0., 0.];
    let mut total = components[0];
    if input.hour >= 6 {
        if input.kind == 0 {
            let count = usize::from(input.business[0x4c]);
            let products = input
                .products
                .get(..count)
                .ok_or("construction stock catalog is truncated")?;
            let stock =
                initial_stock_cost_with_precision(input.volume, float(0x58), products, precision)?;
            components[1] = stock.to_f32();
            total = precision.add(stock, f(total)?).to_f32();
        }
        let volume = input.volume as f32;
        // 461550 returns ST0 retained; its component FST does not pop it.
        // Zero/negative volume takes the native masked-invalid FTOL path.
        if volume <= 0. {
            let volume = f64::from(volume);
            let rent_base = f64::from(unit_price) * f64::from(category) * f64::from(quality);
            let one = double(0x525c40)?;
            let rent = (rent_base
                - (one - one / volume.sqrt()) * (f64::from(input.discount) * rent_base))
                * volume;
            components[2] = rent as f32;
            total = (rent + f64::from(total)) as f32;
        } else {
            let rent_base = precision.mul(
                precision.mul(Sum::from_i32(i32::from(unit_price)), f(category)?),
                f(quality)?,
            );
            let one = Sum::from_f64(double(0x525c40)?)?;
            let reciprocal = precision.div(one, precision.sqrt(f(volume)?)?)?;
            let reduction = precision.add(one, reciprocal.negated());
            let discount = precision.mul(f(input.discount)?, rent_base);
            let rent = precision.mul(
                precision.add(rent_base, precision.mul(reduction, discount).negated()),
                f(volume)?,
            );
            components[2] = rent.to_f32();
            total = precision.add(rent, f(total)?).to_f32();
        }
        for (index, owner) in input.utility_owners.iter().enumerate() {
            if *owner != input.owner {
                let charge = precision.mul(f(volume)?, f(float(0x60 + index * 4))?);
                components[3] = if index == 0 {
                    charge
                } else {
                    precision.add(charge, f(components[3])?)
                }
                .to_f32();
            }
        }
        let final_total = if total.is_finite() && components[3].is_finite() {
            precision.add(f(components[3])?, f(total)?).ftol_low_word()
        } else {
            0
        };
        return Ok(ConstructionCost {
            total: final_total,
            components,
        });
    }
    Ok(ConstructionCost {
        total: crate::economy::ftol_low_word(f64::from(total)),
        components,
    })
}

/// Rent calculation at4635d4..46369c, before the hour-dependent accounting
/// branch. `category_value` is live category+84; `discount` is live53be78.
/// Shared with rent refresh at461590. Preserve the volume/quality f32 spills
/// and PC64 x87 intermediates used by normal simulation (control037f).
pub fn initial_rent(
    building: &[u8; 0x394],
    category_value: f32,
    discount: f32,
    image: &crate::pe::PeImage<'_>,
) -> Result<u16, String> {
    use crate::x87::Sum;

    let area = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1);
    let volume = area.wrapping_mul(i32::from(building[8])) as f32;
    let quality = image.f32_at(match building[0x2c] as i8 {
        0 => 0x525034,
        1 => 0x525b10,
        _ => 0x5251e8,
    })?;
    let half = image.f32_at(0x52502c)?;
    let category_one = image.f32_at(0x525034)?;
    let unit_price = i32::from_le_bytes(building[0x28..0x2c].try_into().unwrap());
    let one = f64::from_le_bytes(image.at(0x525c40, 8)?.try_into().unwrap());
    if volume <= 0.
        || [category_value, half, category_one, quality, discount]
            .iter()
            .any(|value| !value.is_finite())
        || !one.is_finite()
    {
        // Masked invalid sqrt/division or a nonfinite final value reaches
        // FTOL's integer-indefinite result, whose consumed low word is zero.
        return Ok(0);
    }
    let mut base = Sum::from_f32(category_value)?;
    base.mul_f32(half)?;
    base.add_f32(category_one)?;
    base.mul(Sum::from_i32(unit_price));
    base.mul_f32(quality)?;

    let one = Sum::from_f64(one)?;
    let mut reciprocal = one;
    reciprocal.div(Sum::from_f32(volume)?.sqrt()?)?;
    let mut reduction = one;
    reduction.add(reciprocal.negated());
    let mut discounted_base = Sum::from_f32(discount)?;
    discounted_base.mul(base);
    reduction.mul(discounted_base);
    base.add(reduction.negated());
    base.mul_f32(volume)?;
    // FTOL truncates toward zero to signed64; the store consumes only AX.
    Ok(base.ftol_low_word() as u16)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/building_image.rs"
    ));

    #[test]
    fn initial_rent_retains_extended_precision_until_truncation() {
        let bytes = fixture();
        let image = crate::pe::PeImage::parse(&bytes).unwrap();
        let mut building = [0; 0x394];
        building[6] = 2;
        building[7] = 2;
        building[8] = 1;
        for (rate, expected) in [(10i32, 60), (5, 30), (-10, (-60i32) as u16), (20000, 54464)] {
            building[0x28..0x2c].copy_from_slice(&rate.to_le_bytes());
            assert_eq!(
                super::initial_rent(&building, 0., 0.5, &image).unwrap(),
                expected
            );
        }
        building[6] = 0;
        building[7] = 0;
        building[0x28..0x2c].copy_from_slice(&10i32.to_le_bytes());
        // The district factor is just below one in PC64, but rounds to one
        // in binary64. Neither a binary64 spill nor an epsilon may precede FTOL.
        assert_eq!(
            super::initial_rent(&building, -2f32.powi(-60), 0.5, &image).unwrap(),
            9
        );
        for discount in [0.5, f32::INFINITY, f32::NAN] {
            building[8] = 0;
            assert_eq!(
                super::initial_rent(&building, 0., discount, &image).unwrap(),
                0
            );
        }
        building[8] = 1;
        assert_eq!(
            super::initial_rent(&building, f32::INFINITY, 0.5, &image).unwrap(),
            0
        );
        building[4] = 2;
        assert_eq!(super::initial_rent(&building, 0., 0.5, &image).unwrap(), 0);
    }

    #[test]
    #[ignore = "requires original4610d0 PC64 observations"]
    fn matches_native_initial_stock() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_INITIAL_STOCK")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTISTK01");
        assert_eq!(word(8), 0x37f);
        let mut at = 16;
        for case in 0..word(12) {
            let volume = word(at) as i32;
            let rate = f32::from_bits(word(at + 4));
            let count = word(at + 8) as usize;
            let old = f32::from_bits(word(at + 12));
            at += 16;
            let products: Vec<_> = (0..count)
                .map(|i| super::StockProduct {
                    value: f32::from_bits(word(at + i * 8)),
                    fraction: f32::from_bits(word(at + i * 8 + 4)),
                })
                .collect();
            at += count * 8;
            let expected = f32::from_bits(word(at));
            let expected_wide = f64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap());
            let expected_added = word(at + 12);
            at += 16;
            let result = super::initial_stock_cost(volume, rate, &products);
            if !expected_wide.is_finite() {
                assert!(
                    result.is_err(),
                    "case {case} nonfinite cost remains explicit"
                );
                continue;
            }
            let mut result = result.unwrap();
            assert_eq!(
                result.to_f32().to_bits(),
                expected.to_bits(),
                "case {case} f32"
            );
            assert_eq!(
                result.to_f64().to_bits(),
                expected_wide.to_bits(),
                "case {case} f64"
            );
            result.add_f32(old).unwrap();
            assert_eq!(
                result.to_f32().to_bits(),
                expected_added,
                "case {case} caller addition"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original445720 construction-cost observations"]
    fn matches_native_construction_cost() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILDING_COST")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTBCOST2");
        assert_eq!(word(8), 2);
        assert_eq!(word(16), 0x37f);
        let mut at = 20;
        for case in 0..word(12) {
            let mut business = [0; 0x74];
            business[0x54..0x58].copy_from_slice(&bytes[at + 28..at + 32]);
            business[0x58..0x5c].copy_from_slice(&bytes[at + 36..at + 40]);
            business[0x60..0x70].copy_from_slice(&bytes[at + 40..at + 56]);
            let count = word(at + 77) as usize;
            business[0x4c] = count as u8;
            let products: Vec<_> = (0..count)
                .map(|i| {
                    let item = at + 81 + i * 12;
                    super::StockProduct {
                        value: f32::from_bits(word(item + 4)),
                        fraction: f32::from_bits(word(item + 8)),
                    }
                })
                .collect();
            let input = super::ConstructionInputs {
                precision: crate::x87::Precision::Pc64,
                volume: word(at + 4) as i32,
                kind: word(at + 8),
                quality: word(at + 16) as i8,
                category_value: f32::from_bits(word(at + 32)),
                hour: word(at + 24) as i32,
                discount: f32::from_bits(word(at + 56)),
                owner: word(at + 20),
                utility_owners: std::array::from_fn(|i| word(at + 61 + i * 4)),
                business: &business,
                products: &products,
            };
            let actual = super::construction_cost(&input, &image).unwrap();
            at += 81 + count * 12;
            let expected_total = i64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
            assert_eq!(i64::from(actual.total), expected_total, "total case {case}");
            for (i, component) in actual.components.into_iter().enumerate() {
                assert_eq!(
                    component.to_bits(),
                    word(at + 8 + i * 4),
                    "component {i}, case {case}"
                );
            }
            assert_eq!(word(at + 24), 0x100f0110);
            at += 28;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original463560 rent instructions and supplied image"]
    fn matches_native_initial_rent() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_INITIAL_BUSINESS_RENT")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTBRENT1");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let mut building = [0; 0x394];
            building[4..9].copy_from_slice(&bytes[at..at + 5]);
            building[0x2c] = bytes[at + 5];
            building[0x28..0x2c].copy_from_slice(&bytes[at + 8..at + 12]);
            let rent = super::initial_rent(
                &building,
                f32::from_bits(word(at + 12)),
                f32::from_bits(word(at + 16)),
                &image,
            )
            .unwrap();
            at += 20;
            let mut actual = bytes[at..at + 0x220].to_vec();
            actual[0x18..0x1a].copy_from_slice(&rent.to_le_bytes());
            at += 0x220;
            assert_eq!(actual, bytes[at..at + 0x220], "case {case}");
            at += 0x220;
        }
        assert_eq!(at, bytes.len());
    }
}
