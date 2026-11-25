//! Nominal v1.4 music volume transfer for the validated UI percentage domain.
//!
//! Member 4acce0 signed-loads the percentage, multiplies by binary32 0.01,
//! and stores binary32 before mapper 4a94b0. The mapper returns hundredths dB
//! via truncation toward zero; IBasicAudio consumes that integer command.
//! Portable f64 ln approximates the x87 transfer; all-level integer equality
//! across original precision/control modes has not been established.

/// Keep the existing modern u8/clamp policy. This is not the original raw
/// signed32/exceptional-input API, whose out-of-domain behavior differs.
pub fn command(level: u8) -> i32 {
    let level = level.min(100);
    if level == 0 {
        return -10_000;
    }
    let percent = f32::from_bits(0x3c23_d70a);
    // This integer * binary32 product is exact in f64 for 1..=100. Only the
    // final cast rounds to binary32, matching the member's normalization store
    // under ordinary nearest/even rounding. Do not use level / 100 here.
    let normalized = (f64::from(level) * f64::from(percent)) as f32;
    // Keep the affine/log stage above binary32 precision. Native x87 uses
    // FYL2X/FLDLN2 and its current control word, not the host's f64 ln routine.
    let affine = f64::from(normalized) * 0.5 + 0.5;
    (affine.ln() / 5_f64.ln() * 10_000.).trunc() as i32
}

pub fn amplitude(level: u8) -> f32 {
    if level == 0 {
        // DirectShow designates -10000 as silence. Exact-zero PCM is the
        // modern mute choice, not a claim about original driver quantization.
        return 0.;
    }
    // Hundredths dB -> amplitude, not power: 20 * log10(amplitude).
    10_f64.powf(f64::from(command(level)) / 2_000.) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_derived_commands_and_nominal_amplitudes() {
        // Independent literal expectations from the pinned original formula:
        // g25=1/4 -> untruncated command in (-2920.30,-2920.29),
        // g50=1/2 -> untruncated command in (-1787.48,-1787.46).
        // The expected amplitudes below are fixed decimal approximations to
        // 10^(-2920/2000) and 10^(-1787/2000), not helper-generated values.
        for (level, word, gain, tolerance) in [
            (0, -10_000, 0., 0.),
            (25, -2_920, 0.034673685, 0.00000002),
            (50, -1_787, 0.127791, 0.000001),
            (100, 0, 1., 0.),
            (255, 0, 1., 0.),
        ] {
            assert_eq!(command(level), word, "music percentage {level}");
            assert!((amplitude(level) - gain).abs() <= tolerance,
                "music percentage {level}");
        }
        // The positive branch does not converge smoothly to the zero mute.
        assert!((0.007..0.008).contains(&amplitude(1)));
    }
}
