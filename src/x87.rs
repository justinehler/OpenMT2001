//! Finite arithmetic at original x87 PC64 precision. This is not a complete
//! FPU emulator: it models finite nearest-even arithmetic, f32 stores and FTOL.

pub mod portable_trig;

#[derive(Clone, Copy, Debug, Default)]
pub struct Sum {
    negative: bool,
    significand: u64,
    exponent: i32,
}

fn round_shift(value: u128, shift: u32) -> u128 {
    if shift == 0 {
        return value;
    }
    if shift >= 128 {
        return 0;
    }
    let whole = value >> shift;
    let tail = value & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    whole + u128::from(tail > half || (tail == half && whole & 1 != 0))
}

impl Sum {
    /// Compare finite extended values without first rounding to a host float.
    /// Both signs of zero compare equal, as with original FCOM.
    pub fn compare(self, other: Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let left_negative = self.negative && self.significand != 0;
        let right_negative = other.negative && other.significand != 0;
        if left_negative != right_negative {
            return if left_negative {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let magnitude = match (self.significand == 0, other.significand == 0) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => {
                (self.exponent, self.significand).cmp(&(other.exponent, other.significand))
            }
        };
        if left_negative {
            magnitude.reverse()
        } else {
            magnitude
        }
    }

    pub fn from_f32(value: f32) -> Result<Self, &'static str> {
        let bits = value.to_bits();
        let exponent = (bits >> 23) & 255;
        if exponent == 255 {
            return Err("nonfinite operand in finite x87 sum");
        }
        let mantissa = (bits & 0x7fffff) | if exponent != 0 { 1 << 23 } else { 0 };
        let negative = bits >> 31 != 0;
        if mantissa == 0 {
            return Ok(Self {
                negative,
                ..Self::default()
            });
        }
        let shift = 63 - (31 - mantissa.leading_zeros());
        Ok(Self {
            negative,
            significand: u64::from(mantissa) << shift,
            exponent: if exponent == 0 {
                -149 - shift as i32
            } else {
                exponent as i32 - 150 - shift as i32
            },
        })
    }

    /// Exact FLD of a finite binary64 operand into the wider x87 significand.
    pub fn from_f64(value: f64) -> Result<Self, &'static str> {
        let bits = value.to_bits();
        let exponent = (bits >> 52) & 0x7ff;
        if exponent == 0x7ff {
            return Err("nonfinite operand in finite x87 sum");
        }
        let mantissa = (bits & 0x000f_ffff_ffff_ffff) | if exponent != 0 { 1 << 52 } else { 0 };
        let negative = bits >> 63 != 0;
        if mantissa == 0 {
            return Ok(Self {
                negative,
                ..Self::default()
            });
        }
        let shift = mantissa.leading_zeros();
        Ok(Self {
            negative,
            significand: mantissa << shift,
            exponent: if exponent == 0 {
                -1074 - shift as i32
            } else {
                exponent as i32 - 1075 - shift as i32
            },
        })
    }

    /// Exact FILD of the signed low word returned by the original FTOL helper.
    pub fn from_i32(value: i32) -> Self {
        let magnitude = value.unsigned_abs();
        if magnitude == 0 {
            return Self::default();
        }
        let shift = 63 - (31 - magnitude.leading_zeros());
        Self {
            negative: value < 0,
            significand: u64::from(magnitude) << shift,
            exponent: -(shift as i32),
        }
    }

    /// Multiply by one finite f32, retaining the original64-bit significand.
    /// Power-of-two factors (including the original f64 literal0.5) are exact.
    pub fn mul_f32(&mut self, value: f32) -> Result<(), &'static str> {
        self.mul(Self::from_f32(value)?);
        Ok(())
    }

    /// FMUL with another finite extended operand, rounded once to PC64.
    pub fn mul(&mut self, other: Self) {
        let negative = self.negative ^ other.negative;
        let product = u128::from(self.significand) * u128::from(other.significand);
        if product == 0 {
            *self = Self {
                negative,
                ..Self::default()
            };
            return;
        }
        let length = 128 - product.leading_zeros();
        let shift = length - 64;
        let mut significand = round_shift(product, shift);
        let mut exponent = self.exponent + other.exponent + shift as i32;
        if significand >> 64 != 0 {
            significand >>= 1;
            exponent += 1;
        }
        *self = Self {
            negative,
            significand: significand as u64,
            exponent,
        };
    }

    /// FSQRT at PC64 nearest-even precision. Integer remainder comparison
    /// avoids a binary64 approximation and its possible double rounding.
    pub fn sqrt(self) -> Result<Self, &'static str> {
        if self.significand == 0 {
            return Ok(self);
        }
        if self.negative {
            return Err("negative operand in finite x87 square root");
        }
        let shift = if self.exponent & 1 == 0 { 64 } else { 63 };
        let radicand = u128::from(self.significand) << shift;
        let root = radicand.isqrt();
        // (root + 1/2)^2 has a fractional quarter, so an integer radicand
        // cannot tie. The remainder exceeds root exactly when rounding up.
        let significand = root + u128::from(radicand - root * root > root);
        Ok(Self {
            negative: false,
            significand: significand as u64,
            exponent: (self.exponent - shift) / 2,
        })
    }

    /// FCHS preserves the significand, including signed zero.
    pub fn negated(mut self) -> Self {
        self.negative = !self.negative;
        self
    }

    /// Divide by a finite nonzero f32 at PC64 nearest-even precision.
    /// Exceptional operands and extended exponent overflow are outside this
    /// finite layer; callers must handle native zero-denominator paths explicitly.
    pub fn div_f32(&mut self, value: f32) -> Result<(), &'static str> {
        self.div(Self::from_f32(value)?)
    }

    /// FDIV by a finite nonzero extended operand at PC64 precision.
    pub fn div(&mut self, other: Self) -> Result<(), &'static str> {
        if other.significand == 0 {
            return Err("zero divisor in finite x87 division");
        }
        let negative = self.negative ^ other.negative;
        if self.significand == 0 {
            self.negative = negative;
            return Ok(());
        }
        let shift = if self.significand >= other.significand {
            63
        } else {
            64
        };
        let numerator = u128::from(self.significand) << shift;
        let denominator = u128::from(other.significand);
        let mut significand = numerator / denominator;
        let remainder = numerator % denominator;
        significand += u128::from(
            remainder * 2 > denominator || (remainder * 2 == denominator && significand & 1 != 0),
        );
        let mut exponent = self.exponent - other.exponent - shift;
        if significand >> 64 != 0 {
            significand >>= 1;
            exponent += 1;
        }
        *self = Self {
            negative,
            significand: significand as u64,
            exponent,
        };
        Ok(())
    }

    /// Original50e928 truncates to signed64-bit and returns its low32 bits.
    /// Out-of-range FISTP gives integer-indefinite, whose low word is zero.
    pub fn ftol_low_word(self) -> i32 {
        if self.significand == 0 || self.exponent >= 0 {
            // With normalized64-bit significands, nonnegative exponents are
            // outside signed64 range, except -2^63 whose low word is also0.
            return 0;
        }
        let shift = self.exponent.unsigned_abs();
        if shift >= 64 {
            return 0;
        }
        let magnitude = self.significand >> shift;
        let low = magnitude as u32;
        if self.negative {
            low.wrapping_neg() as i32
        } else {
            low as i32
        }
    }

    pub fn add_f32(&mut self, value: f32) -> Result<(), &'static str> {
        self.add(Self::from_f32(value)?);
        Ok(())
    }

    pub fn add(&mut self, other: Self) {
        if self.significand == 0 {
            let negative = self.negative && other.negative;
            *self = other;
            if self.significand == 0 {
                self.negative = negative;
            }
            return;
        }
        if other.significand == 0 {
            return;
        }
        let (large, small) = if self.exponent >= other.exponent {
            (*self, other)
        } else {
            (other, *self)
        };
        let distance = (large.exponent - small.exponent) as u32;
        if distance > 64 {
            *self = large;
            // The spacing just below a power of two is half the spacing above.
            // At distance65, an opposite-sign operand can cross that midpoint.
            if distance == 65
                && large.significand == 1u64 << 63
                && large.negative != small.negative
                && small.significand > 1u64 << 63
            {
                self.significand = u64::MAX;
                self.exponent -= 1;
            }
            return;
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
        if value == 0 {
            *self = Self::default();
            return;
        }
        let length = 128 - value.leading_zeros();
        let mut exponent = small.exponent;
        let mut significand = if length <= 64 {
            exponent -= (64 - length) as i32;
            value << (64 - length)
        } else {
            exponent += (length - 64) as i32;
            round_shift(value, length - 64)
        };
        if significand >> 64 != 0 {
            significand >>= 1;
            exponent += 1;
        }
        *self = Self {
            negative,
            significand: significand as u64,
            exponent,
        };
    }

    /// Deterministic finite atan2 for projection. Range reduction and the
    /// convergent odd series use PC64 operations; this is not a general x87
    /// transcendental emulator. Native comparisons validate final f32 stores.
    pub fn atan2(self, x: Self) -> Result<Self, &'static str> {
        let pi = Self {
            negative: false,
            significand: 0xc90f_daa2_2168_c235,
            exponent: -62,
        };
        let mut half_pi = pi;
        half_pi.exponent -= 1;
        if self.significand == 0 {
            return Ok(if x.negative {
                if self.negative { pi.negated() } else { pi }
            } else {
                self
            });
        }
        if x.significand == 0 {
            return Ok(if self.negative {
                half_pi.negated()
            } else {
                half_pi
            });
        }
        let mut y = self;
        y.negative = false;
        let mut abs_x = x;
        abs_x.negative = false;
        let inverted = y.compare(abs_x).is_gt();
        let (mut ratio, denominator) = if inverted { (abs_x, y) } else { (y, abs_x) };
        ratio.div(denominator)?;
        for _ in 0..2 {
            let mut denominator = ratio;
            denominator.mul(ratio);
            denominator.add(Self::from_i32(1));
            denominator = denominator.sqrt()?;
            denominator.add(Self::from_i32(1));
            ratio.div(denominator)?;
        }
        let mut square = ratio;
        square.mul(ratio);
        square = square.negated();
        let mut power = ratio;
        let mut angle = ratio;
        for i in 1..25 {
            power.mul(square);
            let mut term = power;
            term.div(Self::from_i32(2 * i + 1))?;
            angle.add(term);
        }
        angle.exponent += 2;
        if inverted {
            let mut complement = half_pi;
            complement.add(angle.negated());
            angle = complement;
        }
        if x.negative {
            let mut complement = pi;
            complement.add(angle.negated());
            angle = complement;
        }
        Ok(if self.negative {
            angle.negated()
        } else {
            angle
        })
    }

    /// FSTP64 nearest-even store, including subnormal and overflow boundaries.
    pub fn to_f64(self) -> f64 {
        let sign = u64::from(self.negative) << 63;
        if self.significand == 0 {
            return f64::from_bits(sign);
        }
        let mut exponent = self.exponent + 63;
        if exponent < -1022 {
            let shift = -1074 - self.exponent;
            let fraction = if shift >= 0 {
                round_shift(u128::from(self.significand), shift as u32)
            } else {
                u128::from(self.significand) << (-shift as u32)
            };
            return f64::from_bits(sign | fraction as u64);
        }
        let mut mantissa = round_shift(u128::from(self.significand), 11) as u64;
        if mantissa == 1 << 53 {
            mantissa >>= 1;
            exponent += 1;
        }
        if exponent > 1023 {
            return f64::from_bits(sign | 0x7ff0000000000000);
        }
        f64::from_bits(sign | (((exponent + 1023) as u64) << 52) | (mantissa & 0xfffffffffffff))
    }

    pub fn to_f32(self) -> f32 {
        let sign = u32::from(self.negative) << 31;
        if self.significand == 0 {
            return f32::from_bits(sign);
        }
        let mut exponent = self.exponent + 63;
        if exponent < -126 {
            let shift = -149 - self.exponent;
            let fraction = if shift >= 0 {
                round_shift(u128::from(self.significand), shift as u32)
            } else {
                u128::from(self.significand) << (-shift as u32)
            };
            return f32::from_bits(sign | fraction as u32);
        }
        let mut mantissa = round_shift(u128::from(self.significand), 40) as u32;
        if mantissa == 1 << 24 {
            mantissa >>= 1;
            exponent += 1;
        }
        if exponent > 127 {
            return f32::from_bits(sign | 0x7f800000);
        }
        f32::from_bits(sign | (((exponent + 127) as u32) << 23) | (mantissa & 0x7fffff))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn double_store_rounding_boundaries() {
        for bits in [
            0,
            1,
            0x000fffffffffffff,
            0x0010000000000000,
            0x3ff0000000000000,
            0x7fefffffffffffff,
            0x8000000000000001,
        ] {
            assert_eq!(
                Sum::from_f64(f64::from_bits(bits))
                    .unwrap()
                    .to_f64()
                    .to_bits(),
                bits
            );
        }
        let mut tie = Sum::from_f64(1.).unwrap();
        tie.add(Sum::from_f64(2f64.powi(-53)).unwrap());
        assert_eq!(tie.to_f64(), 1.);
        tie.add(Sum::from_f64(2f64.powi(-63)).unwrap());
        assert_eq!(tie.to_f64().to_bits(), 1f64.to_bits() + 1);
        let mut subnormal = Sum::from_f64(f64::from_bits(1)).unwrap();
        subnormal.mul_f32(0.5).unwrap();
        assert_eq!(subnormal.to_f64().to_bits(), 0);
        subnormal.mul_f32(3.).unwrap();
        assert_eq!(subnormal.to_f64().to_bits(), 2);
        let mut overflow = Sum::from_f64(f64::MAX).unwrap();
        overflow.mul_f32(2.).unwrap();
        assert_eq!(overflow.to_f64(), f64::INFINITY);
    }

    #[test]
    #[ignore = "requires original PC64 vector length and normalization observations"]
    fn matches_native_vector_norm() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_X87_NORM")).unwrap();
        assert_eq!(&data[..8], b"MTX87NRM");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let vector: [f32; 3] = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            at += 12;
            let squares = vector.map(|value| {
                let mut value = Sum::from_f32(value).unwrap();
                value.mul(value);
                value
            });
            let mut total = squares[0];
            total.add(squares[1]);
            total.add(squares[2]);
            let length = total.sqrt().unwrap();
            assert_eq!(
                length.to_f32().to_bits(),
                word(at),
                "case {case} length store"
            );
            at += 4;
            let significand = u64::from_le_bytes(data[at..at + 8].try_into().unwrap());
            let exponent = u16::from_le_bytes(data[at + 8..at + 10].try_into().unwrap());
            assert_eq!(
                length.significand, significand,
                "case {case} full PC64 significand"
            );
            if significand != 0 {
                assert_eq!(
                    length.exponent,
                    i32::from(exponent) - 16383 - 63,
                    "case {case} exponent"
                );
            }
            at += 10;
            let normalized = if length.significand == 0 {
                [0xffc00000; 3]
            } else {
                let mut inverse = Sum::from_f32(1.).unwrap();
                inverse.div(length).unwrap();
                vector.map(|value| {
                    let mut result = inverse;
                    result.mul_f32(value).unwrap();
                    result.to_f32().to_bits()
                })
            };
            for (axis, bits) in normalized.into_iter().enumerate() {
                assert_eq!(bits, word(at), "case {case} axis {axis}");
                at += 4;
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires PC64 binary64 load/add and original FTOL oracle"]
    fn matches_pc64_binary64_operands() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_X87_F64")).unwrap();
        assert_eq!(&data[..8], b"MTX87F64");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let bits = u64::from_le_bytes(data[at..at + 8].try_into().unwrap());
            let mut value = Sum::from_f64(f64::from_bits(bits)).unwrap();
            value.add_f32(f32::from_bits(word(at + 8))).unwrap();
            assert_eq!(value.to_f32().to_bits(), word(at + 12), "case {case} store");
            assert_eq!(
                value.ftol_low_word() as u32,
                word(at + 16),
                "case {case} FTOL"
            );
            at += 20;
        }
        assert_eq!(at, data.len());
        assert!(Sum::from_f64(f64::INFINITY).is_err());
        assert!(Sum::from_f64(f64::NAN).is_err());
    }

    #[test]
    #[ignore = "requires PC64 arithmetic and original FTOL oracle"]
    fn matches_pc64_arithmetic() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_X87_ARITHMETIC")).unwrap();
        assert_eq!(&data[..8], b"MTX87MUL");
        let mut words = data[8..]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        assert_eq!(words.next(), Some(0x037f));
        let cases = words.next().unwrap();
        assert!(cases > 2000);
        for case in 0..cases {
            let count = words.next().unwrap();
            let mut value = Sum::from_f32(f32::from_bits(words.next().unwrap())).unwrap();
            for _ in 0..count {
                let opcode = words.next().unwrap();
                let operand = f32::from_bits(words.next().unwrap());
                match opcode {
                    0 => value.add_f32(operand).unwrap(),
                    1 => value.mul_f32(operand).unwrap(),
                    2 => value.div_f32(operand).unwrap(),
                    3 => value = value.negated(),
                    _ => panic!("unknown opcode {opcode}"),
                }
            }
            assert_eq!(
                value.to_f32().to_bits(),
                words.next().unwrap(),
                "f32 case {case}"
            );
            assert_eq!(
                value.ftol_low_word() as u32,
                words.next().unwrap(),
                "FTOL case {case}"
            );
        }
        assert_eq!(words.next(), None);
    }

    #[test]
    #[ignore = "requires original PC64 property/business value oracle"]
    fn matches_native_value_products() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_X87_PRODUCTS")).unwrap();
        assert_eq!(&data[..8], b"MTX87PRD");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(16)), (1, 0x037f));
        let mut at = 20;
        for case in 0..word(12) {
            let kind = word(at);
            let length = word(at + 4) as usize;
            let ftol_valid = word(at + 8) != 0;
            let expected = word(at + 12);
            let expected_ftol = word(at + 16) as i32;
            at += 24;
            let input = |i| f32::from_bits(word(at + i * 4));
            let result = if kind == 1 {
                assert_eq!(length, 1);
                let mut value = Sum::from_f32(input(0)).unwrap();
                value.mul_f32(0.5).unwrap();
                value.add_f32(1.).unwrap();
                value.mul_f32(375.).unwrap();
                value.mul_f32(0.02).unwrap();
                value.add_f32(0.5).unwrap();
                assert!(ftol_valid);
                let integer = value.ftol_low_word();
                assert_eq!(integer, expected_ftol, "FTOL case{case}");
                let mut value = Sum::from_i32(integer);
                value.mul_f32(50.).unwrap();
                value
            } else {
                assert_eq!((kind, length), (2, 8));
                let index = ((word(at) + 3) % 4) as usize;
                let mut value = Sum::from_f32(input(2 + index)).unwrap();
                value.add_f32(input(6)).unwrap();
                let mut half = Sum::from_f32(input(7)).unwrap();
                half.mul_f32(0.5).unwrap();
                value.add(half);
                value
            };
            assert_eq!(
                result.to_f32().to_bits(),
                expected,
                "stored value case{case}"
            );
            at += length * 4;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires PC64 CPU arithmetic oracle"]
    fn matches_pc64_additions() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_X87_ADD")).unwrap();
        assert_eq!(&data[..8], b"MTX87ADD");
        let mut words = data[8..]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        assert_eq!(words.next(), Some(0x37f));
        let cases = words.next().unwrap();
        assert!(cases > 2000);
        for case in 0..cases {
            let length = words.next().unwrap();
            let mut sum = Sum::from_f32(f32::from_bits(words.next().unwrap())).unwrap();
            for _ in 1..length {
                sum.add_f32(f32::from_bits(words.next().unwrap())).unwrap();
            }
            assert_eq!(sum.to_f32().to_bits(), words.next().unwrap(), "case {case}");
        }
        assert_eq!(words.next(), None);
    }
}
