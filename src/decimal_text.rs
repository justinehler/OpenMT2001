//! Two-decimal text as the original's linked CRT prints a float with `%.2f`.

/// Original linked CRT %.2f rounds an exact halfway magnitude away from zero.
/// A float's24-bit significand times100 fits exactly in f64; large float values
/// already have no fractional part. The CRT retains17 significant decimal digits.
pub fn decimal2(value: f32) -> Result<String, String> {
    if !value.is_finite() {
        return Err("non-finite UI monetary value".into());
    }
    if value == 0. {
        return Ok("0.00".into());
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value.abs() >= 1e17 {
        let mut digits = format!("{:.0}", value.abs()).into_bytes();
        if digits.len() > 17 {
            let round = digits[17] >= b'5';
            digits[17..].fill(b'0');
            if round {
                for i in (0..17).rev() {
                    if digits[i] != b'9' {
                        digits[i] += 1;
                        break;
                    }
                    digits[i] = b'0';
                    if i == 0 {
                        digits.insert(0, b'1');
                    }
                }
            }
        }
        return Ok(format!("{sign}{}.00", String::from_utf8(digits).unwrap()));
    }
    let scaled = f64::from(value.abs()) * 100.;
    if scaled.fract() == 0.5 {
        let cents = scaled.ceil() as u64;
        Ok(format!("{sign}{}.{:02}", cents / 100, cents % 100))
    } else {
        Ok(format!("{value:.2}"))
    }
}
