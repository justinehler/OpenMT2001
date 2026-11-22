//! Original42ad50 civic/utility icons and strategic short-label placement.
use openmt2001::pe::PeImage;
#[derive(Clone, Copy, Debug)]
pub struct Icon {
    pub texture: usize,
    pub color: u32,
}
pub fn icon(image: &PeImage<'_>, text_id: u32) -> Result<Option<Icon>, String> {
    for row in image.at(0x535640, 17 * 12)?.chunks_exact(12) {
        let word = |i| u32::from_le_bytes(row[i..i + 4].try_into().unwrap());
        if word(4) == text_id {
            return Ok(Some(Icon {
                texture: word(0) as usize,
                color: word(8),
            }));
        }
    }
    Ok(None)
}
pub fn label_point(point: [f32; 2], width: f32) -> [f32; 2] {
    [
        (f64::from(point[0]) - f64::from((width * 0.5).trunc())) as f32,
        (f64::from(point[1]) + 19.) as f32,
    ]
}
pub fn background(point: [f32; 2]) -> [f32; 4] {
    [
        point[0] - 15.,
        point[1] - 15.,
        point[0] + 16.,
        point[1] + 16.,
    ]
}
pub fn icon_point(point: [f32; 2], size: [u32; 2]) -> [f32; 2] {
    std::array::from_fn(|i| point[i] - (size[i] / 2) as f32)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original strategic label fixture"]
    fn matches_native_strategic_labels() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_STRATEGIC_LABELS").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTSTLBL1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let count = word(&mut at);
        for _ in 0..count {
            let id = word(&mut at);
            let point = std::array::from_fn(|_| f32::from_bits(word(&mut at)) * 0.5);
            let width = f32::from_bits(word(&mut at));
            let size = std::array::from_fn(|_| word(&mut at));
            for v in label_point(point, width) {
                assert_eq!(v.to_bits(), word(&mut at));
            }
            let present = word(&mut at) != 0;
            let actual = icon(&image, id).unwrap();
            assert_eq!(actual.is_some(), present);
            if let Some(actual) = actual {
                for v in background(point) {
                    assert_eq!(v.to_bits(), word(&mut at));
                }
                assert_eq!(actual.color, word(&mut at));
                for v in icon_point(point, size) {
                    assert_eq!(v.to_bits(), word(&mut at));
                }
                assert_eq!(actual.texture, word(&mut at) as usize);
            }
        }
        assert_eq!(at, b.len());
    }
}
