//! Deterministic mathematical trig on the complete retained Sum operand.
//! Does not emulate an x87 transcendental's internal period or microcode.
//! The conditional mathematical absolute error bound is 2^-56 for |x|<2^192.
//! Native cache/pose equality is a separate, bounded fixture property.
use super::Sum;

type Result<T> = std::result::Result<T, &'static str>;

// floor((2/pi)*2^512), little-endian limbs. Root must independently verify
// these constants; the campaign receipt binds the rational certificate to source.
const TWO_OVER_PI: [u64; 8] = [
    0xe882_35f5_2ebb_4484,
    0xfe1d_eb1c_b129_a73e,
    0x0649_2eea_09d1_921c,
    0xb724_6e3a_424d_d2e0,
    0xfe51_63ab_debb_c561,
    0xdb62_9599_3c43_9041,
    0xfc27_57d1_f534_ddc0,
    0xa2f9_836e_4e44_1529,
];
const HALF_PI: Sum = Sum {
    negative: false,
    significand: 0xc90f_daa2_2168_c235,
    exponent: -63,
};
const TINY: Sum = Sum {
    negative: false,
    significand: 1 << 63,
    exponent: -95, // 2^-32
};

// The extra limb accommodates the largest fractional denominator (607 bits).
#[derive(Clone, Copy)]
struct Bits([u64; 10]);

impl Bits {
    fn product(mantissa: u64) -> Self {
        let mut words = [0; 10];
        let mut carry = 0u128;
        for (i, limb) in TWO_OVER_PI.into_iter().enumerate() {
            // (2^64-1)^2 + (2^64-2) < 2^128: no u128 overflow.
            let next = u128::from(limb) * u128::from(mantissa) + carry;
            words[i] = next as u64;
            carry = next >> 64;
        }
        words[8] = carry as u64;
        Self(words)
    }

    fn bit(&self, at: usize) -> bool {
        at < 640 && self.0[at / 64] & (1u64 << (at % 64)) != 0
    }

    fn keep_below(&mut self, bits: usize) {
        for (i, word) in self.0.iter_mut().enumerate() {
            if i * 64 >= bits {
                *word = 0;
            } else if (i + 1) * 64 > bits {
                *word &= (1u64 << (bits % 64)) - 1;
            }
        }
    }

    // Replace r by 2^bits-r modulo 2^bits, with exact limb arithmetic.
    fn complement_below(&mut self, bits: usize) {
        let mut carry = true;
        for word in &mut self.0 {
            let (next, overflow) = (!*word).overflowing_add(u64::from(carry));
            *word = next;
            carry = overflow;
        }
        self.keep_below(bits);
    }

    // One nearest-even rounding of the exact dyadic integer / 2^denominator.
    // No f32/f64 conversion occurs before this retained PC64 remainder.
    fn fraction(self, denominator: usize, negative: bool) -> Sum {
        let Some(top_word) = self.0.iter().rposition(|&word| word != 0) else {
            return Sum { negative, ..Sum::default() };
        };
        let top = top_word * 64 + 63 - self.0[top_word].leading_zeros() as usize;
        let mut mantissa = 0u64;
        for i in 0..64 {
            if i <= top && self.bit(top - i) {
                mantissa |= 1u64 << (63 - i);
            }
        }
        let mut exponent = top as i32 - 63 - denominator as i32;
        if top >= 64 {
            let guard = top - 64;
            let sticky = (0..guard).any(|i| self.bit(i));
            if self.bit(guard) && (sticky || mantissa & 1 != 0) {
                let (next, carry) = mantissa.overflowing_add(1);
                mantissa = if carry { 1 << 63 } else { next };
                exponent += i32::from(carry);
            }
        }
        Sum { negative, significand: mantissa, exponent }
    }
}

fn reduced(abs: Sum) -> Result<(u8, Sum)> {
    // e is the exponent of the integer significand, not the float exponent.
    // abs < 2^192 covers every finite f32 load and the proven common bound
    // abs < 2^130. This is not an arbitrary small normal-play admission limit.
    if abs.exponent > 128 {
        return Err("portable trig operand outside proved camera exponent envelope");
    }
    // Caller handles abs <= 2^-32, so e >= -95 and 384 <= shift <= 607.
    let shift = (512 - abs.exponent) as usize;
    let mut product = Bits::product(abs.significand);
    let mut quadrant = u8::from(product.bit(shift))
        | (u8::from(product.bit(shift + 1)) << 1);
    // Round the approximate multiple of pi/2 to nearest, ties upward. Either
    // choice at an exact tie is valid if fraction and quadrant stay paired.
    let upper = product.bit(shift - 1);
    product.keep_below(shift);
    if upper {
        quadrant = quadrant.wrapping_add(1) & 3;
        product.complement_below(shift);
    }
    let mut r = product.fraction(shift, upper);
    r.mul(HALF_PI);
    Ok((quadrant, r))
}

fn small_pair(r: Sum) -> Result<(Sum, Sum)> {
    let mut negative_square = r;
    negative_square.mul(r);
    negative_square = negative_square.negated();
    let mut sine_term = r;
    let mut sine = r;
    let mut cosine_term = Sum::from_i32(1);
    let mut cosine = cosine_term;
    // r is at most pi/4 plus PC64 rounding. Separate convergent recurrences
    // preserve PC64 throughout; no platform libm, FP mode or FMA dependency.
    for n in 1..=24 {
        sine_term.mul(negative_square);
        sine_term.div(Sum::from_i32((2 * n) * (2 * n + 1)))?;
        sine.add(sine_term);
        cosine_term.mul(negative_square);
        cosine_term.div(Sum::from_i32((2 * n - 1) * (2 * n)))?;
        cosine.add(cosine_term);
    }
    Ok((sine, cosine))
}

/// Deterministic mathematical approximation, not a correctly-rounded or native
/// FSIN/FCOS claim. Exact raw input state is never modified or normalized.
#[derive(Default)]
pub struct PortableTrig;

impl PortableTrig {
    pub fn pair(&mut self, value: Sum) -> Result<(Sum, Sum)> {
        let mut abs = value;
        abs.negative = false;
        if abs.compare(TINY).is_le() {
            // At |x| <= 2^-32 these are the correctly rounded mathematical
            // PC64 values (including signed zero), without reduction loss.
            return Ok((value, Sum::from_i32(1)));
        }
        let (quadrant, r) = reduced(abs)?;
        let (s, c) = small_pair(r)?;
        let (mut sine, cosine) = match quadrant {
            0 => (s, c),
            1 => (c, s.negated()),
            2 => (s.negated(), c.negated()),
            3 => (c.negated(), s),
            _ => unreachable!(),
        };
        if value.negative { sine = sine.negated(); }
        Ok((sine, cosine))
    }
}

impl crate::camera::primary::AngularTrig for PortableTrig {
    fn sine(&mut self, retained_yaw: Sum) -> Result<Sum> {
        Ok(self.pair(retained_yaw)?.0)
    }
    fn cosine(&mut self, rounded_yaw: Sum) -> Result<Sum> {
        Ok(self.pair(rounded_yaw)?.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_zero_tiny_cutoff_and_camera_envelope() {
        let mut trig=PortableTrig;
        for negative in [false,true] {
            let zero=Sum{negative,..Sum::default()};
            let (s,c)=trig.pair(zero).unwrap();assert_eq!(s.to_f32().to_bits(),u32::from(negative)<<31);assert_eq!(c.to_f32(),1.);
            let tiny=Sum{negative,..TINY};let (s,c)=trig.pair(tiny).unwrap();assert_eq!(s.compare(tiny),std::cmp::Ordering::Equal);assert_eq!(c.compare(Sum::from_i32(1)),std::cmp::Ordering::Equal);
            for significand in [(1<<63)+1,u64::MAX] {assert!(trig.pair(Sum{negative,significand,exponent:-95}).is_ok());}
            assert!(trig.pair(Sum{negative,significand:u64::MAX,exponent:128}).is_ok());
            assert!(trig.pair(Sum{negative,significand:1<<63,exponent:129}).is_err());
        }
    }
}
