use super::{Sum, round_shift};

/// Explicit caller context: CRT startup selects PC53; 4b9c3c and each
/// 4816e9 select PC64. Loads/stores and FTOL retain their own widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Pc53,
    Pc64,
}
impl Precision {
    fn bits(self) -> u32 {
        match self {
            Self::Pc53 => 53,
            Self::Pc64 => 64,
        }
    }

    // Round the exact integer result once, keeping Sum's normalized layout
    // and extended exponent range even at PC53 (unlike a binary64 spill).
    fn rounded(self, negative: bool, value: u128, mut exponent: i32) -> Sum {
        if value == 0 {
            return Sum {
                negative,
                ..Sum::default()
            };
        }
        let length = 128 - value.leading_zeros();
        let bits = self.bits();
        let mut value = if length > bits {
            exponent += (length - bits) as i32;
            round_shift(value, length - bits)
        } else {
            value
        };
        if 128 - value.leading_zeros() > bits {
            value >>= 1;
            exponent += 1;
        }
        let shift = value.leading_zeros() - 64;
        Sum {
            negative,
            significand: (value << shift) as u64,
            exponent: exponent - shift as i32,
        }
    }

    pub fn add(self, left: Sum, right: Sum) -> Sum {
        if self == Self::Pc64 {
            let mut value = left;
            value.add(right);
            return value;
        }
        if left.significand == 0 || right.significand == 0 {
            let value = if left.significand == 0 { right } else { left };
            return self.rounded(
                if value.significand == 0 {
                    left.negative && right.negative
                } else {
                    value.negative
                },
                u128::from(value.significand),
                value.exponent,
            );
        }
        let (large, small) = if left.exponent >= right.exponent {
            (left, right)
        } else {
            (right, left)
        };
        let distance = (large.exponent - small.exponent) as u32;
        // Every PC53 operand here is an exact load or a preceding PC53 result.
        // A term more than 64 places below it cannot reach a PC53 midpoint.
        if distance > 64 {
            return self.rounded(
                large.negative,
                u128::from(large.significand),
                large.exponent,
            );
        }
        let a = u128::from(large.significand) << distance;
        let b = u128::from(small.significand);
        let (value, negative) = if large.negative == small.negative {
            (a + b, large.negative)
        } else if a >= b {
            (a - b, large.negative)
        } else {
            (b - a, small.negative)
        };
        self.rounded(negative && value != 0, value, small.exponent)
    }

    pub fn mul(self, left: Sum, right: Sum) -> Sum {
        self.rounded(
            left.negative ^ right.negative,
            u128::from(left.significand) * u128::from(right.significand),
            left.exponent + right.exponent,
        )
    }

    pub fn div(self, left: Sum, right: Sum) -> Result<Sum, &'static str> {
        if right.significand == 0 {
            return Err("zero divisor in finite x87 division");
        }
        if left.significand == 0 {
            return Ok(Sum {
                negative: left.negative ^ right.negative,
                ..left
            });
        }
        let shift = self.bits() - u32::from(left.significand >= right.significand);
        let numerator = u128::from(left.significand) << shift;
        let denominator = u128::from(right.significand);
        let whole = numerator / denominator;
        let remainder = numerator % denominator;
        let value = whole
            + u128::from(
                remainder * 2 > denominator || (remainder * 2 == denominator && whole & 1 != 0),
            );
        Ok(self.rounded(
            left.negative ^ right.negative,
            value,
            left.exponent - right.exponent - shift as i32,
        ))
    }

    pub fn sqrt(self, value: Sum) -> Result<Sum, &'static str> {
        if value.significand == 0 {
            return Ok(value);
        }
        if value.negative {
            return Err("negative operand in finite x87 square root");
        }
        let shift = 2 * self.bits() - 64 - u32::from(value.exponent & 1 != 0);
        let radicand = u128::from(value.significand) << shift;
        let root = radicand.isqrt();
        let rounded = root + u128::from(radicand - root * root > root);
        Ok(self.rounded(false, rounded, (value.exponent - shift as i32) / 2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pc53_rounds_products_once_without_a_pc64_midpoint() {
        let a = Sum::from_f64(1. + 2f64.powi(-26)).unwrap();
        let b = Sum::from_f64(1. + 2f64.powi(-27) - 2f64.powi(-52)).unwrap();
        // Exact product is just below the midpoint below 1 + 3*2^-27.
        // Rounding to PC64 first loses its negative 2^-78 residual.
        let expected = (1. + 3. * 2f64.powi(-27) - 2f64.powi(-52)).to_bits();
        assert_eq!(Precision::Pc53.mul(a, b).to_f64().to_bits(), expected);
        assert_ne!(Precision::Pc64.mul(a, b).to_f64().to_bits(), expected);
    }

    #[test]
    fn pc53_retains_the_extended_exponent_range() {
        let large = Sum::from_f64(2f64.powi(1023)).unwrap();
        let two = Sum::from_i32(2);
        let product = Precision::Pc53.mul(large, two);
        assert!(product.to_f64().is_infinite());
        assert_eq!(
            Precision::Pc53.div(product, two).unwrap().to_f64(),
            2f64.powi(1023)
        );
        let small = Sum::from_f64(f64::from_bits(1)).unwrap();
        let quotient = Precision::Pc53.div(small, two).unwrap();
        assert_eq!(quotient.to_f64(), 0.);
        assert_eq!(Precision::Pc53.mul(quotient, two).to_f64().to_bits(), 1);
    }

    #[test]
    fn pc53_sqrt_and_division_round_at_the_operation() {
        let one = Sum::from_i32(1);
        let two = Sum::from_i32(2);
        assert_eq!(
            Precision::Pc53.sqrt(two).unwrap().to_f64().to_bits(),
            0x3ff6a09e667f3bcd
        );
        assert_eq!(
            Precision::Pc53
                .div(one, Sum::from_i32(3))
                .unwrap()
                .to_f64()
                .to_bits(),
            0x3fd5555555555555
        );
    }
}
