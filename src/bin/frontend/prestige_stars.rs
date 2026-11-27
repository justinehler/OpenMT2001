//! Original42bdd0 prestige-star count and sprite placement after projection.
use openmt2001::x87::Sum;
pub fn positions(
    prestige: f32,
    point: [f32; 2],
    size: [u32; 2],
    bias: f64,
) -> Result<Vec<[f32; 2]>, String> {
    // Masked x87 nonfinite conversion returns indefinite with low word zero.
    let count = if prestige.is_finite() {
        let mut value = Sum::from_f32(prestige)?;
        value.add(Sum::from_f64(bias)?);
        value.ftol_low_word().clamp(0, 5) as usize
    } else {
        0
    };
    // Original42a4b7 initializes these per-count offsets, in submission order.
    let offsets: &[[f32; 2]] = match count {
        0 => &[],
        1 => &[[0., 0.]],
        2 => &[[-8., 0.], [8., 0.]],
        3 => &[[-13., 0.], [13., 0.], [0., 0.]],
        4 => &[[-8., -8.], [8., -8.], [8., 8.], [-8., 8.]],
        _ => &[[-8., -8.], [8., -8.], [-13., 8.], [13., 8.], [0., 8.]],
    };
    Ok(offsets
        .iter()
        .map(|offset| {
            std::array::from_fn(|i| {
                (f64::from(point[i]) - f64::from(size[i] / 2) + f64::from(offset[i])) as f32
            })
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original prestige sprite fixture"]
    fn matches_native_prestige_stars() {
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_PRESTIGE_STARS")).unwrap();
        assert_eq!(&b[..8], b"MTPSTAR1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let count = word(&mut at);
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let bias = f64::from_le_bytes(image.at(0x525050, 8).unwrap().try_into().unwrap());
        for case in 0..count {
            let prestige = f32::from_bits(word(&mut at));
            let point = [
                f32::from_bits(word(&mut at)) * 0.5,
                f32::from_bits(word(&mut at)) * 0.5,
            ];
            let size = [word(&mut at), word(&mut at)];
            let n = word(&mut at) as usize;
            let actual = positions(prestige, point, size, bias).unwrap();
            assert_eq!(actual.len(), n, "case{case}");
            for p in actual {
                assert_eq!(
                    p.map(f32::to_bits),
                    [word(&mut at), word(&mut at)],
                    "case{case}"
                );
                assert_eq!(word(&mut at), 109);
                assert_eq!(word(&mut at), 1.5f32.to_bits());
                assert_eq!(word(&mut at), u32::MAX);
            }
        }
        assert_eq!(at, b.len());
    }
}
