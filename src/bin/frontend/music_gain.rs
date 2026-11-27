//! Nominal v1.4 shared volume transfer for validated music and fixed ledger/UI-click inputs.
//!
//! Member 4acce0 signed-loads the percentage, multiplies by binary32 0.01,
//! and stores binary32 before mapper 4a94b0. The mapper returns hundredths dB
//! via truncation toward zero; IBasicAudio consumes that integer command.
//! Portable f64 ln approximates the x87 transfer; all-level integer equality
//! across original precision/control modes has not been established.

/// Keep the existing modern u8/clamp policy. This is not the original raw
/// signed32/exceptional-input API, whose out-of-domain behavior differs.
pub(crate) fn normalized_percentage(level: u8) -> f32 {
    let percent = f32::from_bits(0x3c23_d70a);
    // One binary32 preference/member store, under ordinary nearest/even.
    (f64::from(level.min(100)) * f64::from(percent)) as f32
}

pub fn command(level: u8) -> i32 {
    normalized_command(normalized_percentage(level))
}

/// Shared 4a94b0 nominal mapper. Callers supply the original final binary32
/// argument. Audio callers use the bounded finite music/WAV domain (-.02..1).
/// Raw signed32/exceptional input and x87 transcendental equality remain outside it.
pub(crate) fn normalized_command(normalized: f32) -> i32 {
    if normalized == 0. {
        return -10_000;
    }
    let affine = f64::from(normalized) * 0.5 + 0.5;
    (affine.ln() / 5_f64.ln() * 10_000.).trunc() as i32
}

pub fn amplitude(level: u8) -> f32 {
    normalized_amplitude(normalized_percentage(level))
}

pub(crate) fn normalized_amplitude(normalized: f32) -> f32 {
    if normalized == 0. {
        // Modern exact-zero mute, not an original driver quantization claim.
        return 0.;
    }
    10_f64.powf(f64::from(normalized_command(normalized)) / 2_000.) as f32
}

/// 4ab420/4abad0: source*binary32(.01)*stored factor, then ONE f32 store.
/// Existing audio callers supply small signed source words (-2..100), so both
/// products are exact in f64 before that store. This is not a raw32/x87 API.
pub(crate) fn source_normalized(factor: f32, source_level: i32) -> f32 {
    (f64::from(factor) * (f64::from(source_level) * f64::from(f32::from_bits(0x3c23_d70a)))) as f32
}
pub(crate) fn source_amplitude(factor: f32, source_level: i32) -> f32 {
    normalized_amplitude(source_normalized(factor, source_level))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_source_words_match_independent_nominal_literals() {
        // Fraction RN32 + Decimal100 expectations, not production-generated.
        for (factor_bits, source, bits, command, gain) in [
            (0x3f800000, -2, 0xbca3d70a, -4432, 0.006081350),
            (0x3f800000, 0, 0, -10000, 0.),
            (0x3f800000, 80, 0x3f4ccccc, -654, 0.47097734),
            (0x3e800000, 80, 0x3e4ccccc, -3173, 0.025911944),
            (0x3f199999, 60, 0x3eb851eb, -2396, 0.06338697),
        ] {
            let factor = f32::from_bits(factor_bits);
            let normalized = source_normalized(factor, source);
            assert_eq!(normalized.to_bits(), bits);
            assert_eq!(normalized_command(normalized), command);
            assert!((source_amplitude(factor, source) - gain).abs() < 1e-6);
        }
    }

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
            assert!(
                (amplitude(level) - gain).abs() <= tolerance,
                "music percentage {level}"
            );
        }
        // The positive branch does not converge smoothly to the zero mute.
        assert!((0.007..0.008).contains(&amplitude(1)));
    }
}
