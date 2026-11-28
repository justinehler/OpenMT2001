//! Shared Lua Random.ZeroTo binding and its caller-owned static stream.
use crate::lua4::Value;

/// Original4db450: numeric upper bound is truncated by50e928, the separate
/// static RNG at93aee8 advances once, then signed division returns 0..upper.
/// Larger uses are bounded here because native `inc ecx` overflows at i32::MAX.
pub fn zero_to(state: &mut u32, args: &[Value]) -> Result<f64, String> {
    let [Value::Number(upper)] = args else {
        return Err("Random.ZeroTo requires one numeric upper bound".into());
    };
    if !upper.is_finite() || *upper < 0. || *upper >= i32::MAX as f64 {
        return Err("Random.ZeroTo upper bound leaves supported native range".into());
    }
    let divisor = (*upper as i32 + 1) as u32;
    *state = state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
    let value = ((*state >> 16) & 0x7fff) % divisor;
    Ok(f64::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_native_static_stream() {
        // 4db450 uses node93aee8, not the process RNG. Check two consecutive
        // calls and the full saved LCG word so a reseed cannot masquerade as
        // an independent draw.
        let mut state = 0x1234_5678;
        assert_eq!(zero_to(&mut state, &[Value::Number(2.)]).unwrap(), 2.);
        assert_eq!(state, 0xb3e9_7b5b);
        assert_eq!(zero_to(&mut state, &[Value::Number(2.)]).unwrap(), 1.);
        assert_eq!(state, 0x5b3f_58b2);
    }
}
