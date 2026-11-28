//! Original430480 two-column admission and4419f0 per-line palette selection.
use crate::{pe::PeImage, x87::Sum};

/// Widths come from the last left line and first incoming right line in the
/// original font. Overflow inserts a blank right line, then pads the left list.
pub fn value_wrap(field: [f32; 2], margin: i32, widths: [i32; 2]) -> Result<bool, String> {
    let mut available = Sum::from_f32(field[1])?;
    available.add_f32(-field[0])?;
    available.add(Sum::from_i32(margin.wrapping_mul(2)).negated());
    Ok(widths[0].wrapping_add(widths[1]) > available.ftol_low_word())
}

///440b10 stores bytes, and4419f0 alternates palettes only when flag==1.
pub fn color(image: &PeImage<'_>, style: [u32; 3], ui_tick: u32) -> Result<u32, String> {
    let index = palette_index(style, ui_tick);
    Ok(u32::from_le_bytes(
        image
            .at(0x536288 + u32::from(index) * 4, 4)?
            .try_into()
            .unwrap(),
    ))
}

pub fn palette_index(style: [u32; 3], ui_tick: u32) -> u8 {
    if style[2] as u8 == 1 && ui_tick & 31 > 16 {
        style[1] as u8
    } else {
        style[0] as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original430480/4419f0 executions"]
    fn matches_original_text_layout_and_palette() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_REPORT_TEXT")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        assert_eq!(&bytes[..8], b"MTBTXT01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x37f);
        let wraps = word(12) as usize;
        let colors = word(16) as usize;
        assert_eq!(bytes.len(), 20 + wraps * 24 + colors * 20);
        for i in 0..wraps {
            let at = 20 + i * 24;
            assert_eq!(
                value_wrap(
                    [f32::from_bits(word(at)), f32::from_bits(word(at + 4))],
                    word(at + 8) as i32,
                    [word(at + 12) as i32, word(at + 16) as i32]
                )
                .unwrap(),
                word(at + 20) != 0,
                "wrap{i}"
            );
        }
        for i in 0..colors {
            let at = 20 + wraps * 24 + i * 20;
            assert_eq!(
                color(
                    &image,
                    [word(at), word(at + 4), word(at + 8)],
                    word(at + 12)
                )
                .unwrap(),
                word(at + 16),
                "color{i}"
            );
        }
    }
}
