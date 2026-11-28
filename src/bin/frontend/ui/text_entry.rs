//! Shared single-line input presentation (original 443d80).
use super::{Surface, text_at, text_width};

struct EntryLayout {
    text: [f32; 2],
    caret: [f32; 4],
}

fn layout(
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    centered: bool,
) -> Result<EntryLayout, String> {
    let width = text_width(tab, encoded, 1.)? as f32;
    let metric = |glyph: u8, field: usize| -> Result<i32, String> {
        let at = 4 + usize::from(glyph) * 44 + field;
        Ok(i32::from_le_bytes(
            tab.get(at..at + 4)
                .ok_or("input font metric missing")?
                .try_into()
                .unwrap(),
        ))
    };
    let [l, t, r, b] = bounds;
    if bounds.iter().any(|v| !v.is_finite()) || l >= r || t >= b {
        return Err("invalid input bounds".into());
    }
    // 443f30 seeds layout with "W" before installing the input text. An
    // empty row keeps that seed's origin; a populated row centers its own
    // width. The caret itself never participates in text centering.
    let alignment_width = if encoded.is_empty() {
        text_width(tab, &[b'W' - 32], 1.)? as f32
    } else {
        width
    };
    let x = l.trunc()
        + if centered {
            ((r - l - alignment_width) * 0.5 + 0.5).trunc()
        } else {
            2.
        };
    let y = t.trunc() + ((b - t - metric(b'W' - 32, 36)? as f32) * 0.5).trunc();
    // 443dab measures ASCII "I" at 5365c0; 443ede submits a solid quad.
    // The measured cell is its advance (including the -1 font gap), and
    // its bitmap height. No special block glyph is drawn by the original.
    let caret_width = text_width(tab, &[b'I' - 32], 1.)? as f32;
    let caret_height = metric(b'I' - 32, 36)? as f32;
    Ok(EntryLayout {
        text: [x, y],
        caret: [
            x + width,
            y + 2.,
            x + width + caret_width,
            y + 2. + caret_height,
        ],
    })
}

/// Draw the original style 3 (left) or 4 (centered) input row and block caret.
pub fn text_entry(
    pages: &mut [Surface],
    shapes: &mut Surface,
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    centered: bool,
    active: bool,
) -> Result<(), String> {
    let entry = layout(tab, encoded, bounds, centered)?;
    text_at(
        pages,
        &openmt2001::font::Font::parse(tab)?,
        encoded,
        bounds,
        0xffffffff,
        openmt2001::font::Layout {
            origin: entry.text,
            depth: 0.5,
            scale: 1.,
            gap: -1,
            baseline: 0,
        },
    )?;
    if active {
        // Native 443df7 pulses white alpha with the global UI counter. These
        // cached host panels retain their steady visibility policy. This
        // reference-frame alpha blends to light grey-blue on the input backing.
        let caret = [
            entry.caret[0].max(bounds[0]),
            entry.caret[1].max(bounds[1]),
            entry.caret[2].min(bounds[2]),
            entry.caret[3].min(bounds[3]),
        ];
        if caret[0] < caret[2] && caret[1] < caret[3] {
            shapes.rect(caret, [0xafffffff; 4]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Vec<u8> {
        let mut tab = vec![0; 4 + 95 * 44];
        tab[..4].copy_from_slice(&95u32.to_le_bytes());
        for (glyph, width, height, offset) in
            [('I', 6i32, 11i32, 3i32), ('W', 13, 11, 3), ('A', 9, 11, 3)]
        {
            let at = 4 + (glyph as usize - 32) * 44;
            tab[at + 32..at + 36].copy_from_slice(&width.to_le_bytes());
            tab[at + 36..at + 40].copy_from_slice(&height.to_le_bytes());
            tab[at + 40..at + 44].copy_from_slice(&offset.to_le_bytes());
        }
        tab
    }

    #[test]
    fn caret_follows_text_without_affecting_centering() {
        let entry = layout(
            &font(),
            &[b'A' - 32, b'W' - 32],
            [10., 20., 110., 51.],
            true,
        )
        .unwrap();
        assert_eq!(entry.text, [50., 30.]);
        assert_eq!(entry.caret, [70., 32., 75., 43.]);
    }

    #[test]
    fn empty_input_still_has_a_full_cell_caret() {
        let entry = layout(&font(), &[], [10., 20., 110., 51.], true).unwrap();
        assert_eq!(entry.text, [54., 30.]);
        assert_eq!(entry.caret, [54., 32., 59., 43.]);
        let mut pages = [Surface::solid(), Surface::solid()];
        let mut shapes = Surface::solid();
        text_entry(
            &mut pages,
            &mut shapes,
            &font(),
            &[],
            [10., 20., 110., 51.],
            true,
            true,
        )
        .unwrap();
        assert_eq!(shapes.vertices.len(), 6);
        assert_eq!(&shapes.vertices[0][..2], &[54., 32.]);
        assert_eq!(&shapes.vertices[2][..2], &[59., 43.]);
        assert!(pages.iter().all(|page| page.vertices.is_empty()));
    }

    #[test]
    fn left_aligned_input_only_draws_caret_while_active() {
        let tab = font();
        let entry = layout(&tab, &[b'A' - 32], [10., 20., 110., 51.], false).unwrap();
        assert_eq!(entry.text, [12., 30.]);
        assert_eq!(entry.caret, [20., 32., 25., 43.]);
        let mut pages = [Surface::solid(), Surface::solid()];
        let mut shapes = Surface::solid();
        text_entry(
            &mut pages,
            &mut shapes,
            &tab,
            &[b'A' - 32],
            [10., 20., 110., 51.],
            false,
            false,
        )
        .unwrap();
        assert!(shapes.vertices.is_empty());
        assert_eq!(pages[0].vertices.len(), 6);
    }

    #[test]
    fn long_centered_names_stay_inside_the_field() {
        let mut pages = [Surface::solid(), Surface::solid()];
        let mut shapes = Surface::solid();
        text_entry(
            &mut pages,
            &mut shapes,
            &font(),
            &[b'W' - 32; 10],
            [10., 20., 60., 51.],
            true,
            true,
        )
        .unwrap();
        assert!(
            shapes.vertices.is_empty(),
            "caret beyond the right edge is clipped"
        );
        assert!(!pages[0].vertices.is_empty());
        assert!(
            pages[0]
                .vertices
                .iter()
                .all(|vertex| (9.5..=59.5).contains(&vertex[0]))
        );
    }
}
