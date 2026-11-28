//! Selected-block lists: font3, styles0/2, and palette0/9 (405744..4057b3,
//! 430480, 431a10). Coordinates are in the original 640x480 UI space.
use crate::ui::{self, Surface};

pub const FONT_TABLE: &str = "Standard/fonts/tahoma10bldnoantip.tab";
pub const FONT_PAGES: std::ops::Range<usize> = 6..8;
// 4406a0 measures the font's nine-pixel W and adds the two-pixel gap
// installed at 405789/405793: 11 UI units, or 17.6 pixels at 1024x768.
pub const ROW_PITCH: f32 = 11.;
pub const VALUE_PALETTE: usize = 9;

pub fn load_font(
    textures: &[openmt2001::texture::TextureRecord<'_>],
    read_table: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> Result<(Vec<Surface>, Vec<u8>), String> {
    let pages = textures
        .get(FONT_PAGES)
        .ok_or("missing block report font3 pages")?;
    let tab = read_table(FONT_TABLE)?;
    openmt2001::font::Font::parse(&tab)?;
    Ok((pages.iter().map(Surface::new).collect(), tab))
}

pub struct Layout {
    pub bounds: [f32; 4],
}
impl Layout {
    pub fn line(&self, row: usize, right: bool) -> [f32; 4] {
        let [l, t, r, _] = self.bounds;
        let y = t + 3. + row as f32 * ROW_PITCH;
        [l + if right { 3. } else { 2. }, y, r - 3., y + ROW_PITCH]
    }
    pub fn heading(&self, row: usize) -> [f32; 4] {
        let [l, t, r, _] = self.bounds;
        let y = t + 3. + row as f32 * ROW_PITCH;
        [l + 1., y, r - 1., y + 10.]
    }
    pub fn visible_rows(&self) -> usize {
        ((self.bounds[3] - self.bounds[1] - 5.) / ROW_PITCH).max(0.) as usize
    }
}

/// Both columns retain full-size glyphs. The original moves a value to the
/// following line if the two columns would overlap (430480/431a10).
pub fn paint_information_pair(
    pages: &mut [Surface],
    tab: &[u8],
    layout: &Layout,
    row: usize,
    text: [&[u8]; 2],
    palette: &[u32; 18],
) -> Result<usize, String> {
    let colors = [palette[0], palette[VALUE_PALETTE]];
    let [label, value] = text;
    let left = layout.line(row, false);
    let mut right = layout.line(row, true);
    let label_width = ui::text_width(tab, label, 1.)? as f32;
    let value_width = ui::text_width(tab, value, 1.)? as f32;
    let wrapped =
        !value.is_empty() && label_width + value_width > layout.bounds[2] - layout.bounds[0] - 2.;
    if wrapped {
        right = layout.line(row + 1, true);
    }
    let font = openmt2001::font::Font::parse(tab)?;
    let paint = |pages: &mut [Surface], text: &[u8], origin, color| -> Result<(), String> {
        for glyph in font.layout(
            text,
            openmt2001::font::Layout {
                origin,
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[glyph.page].quad(glyph.positions.map(|p| [p[0], p[1]]), glyph.uv, [color; 4]);
        }
        Ok(())
    };
    paint(pages, label, [left[0], left[1]], colors[0])?;
    if !value.is_empty() {
        // 440756 rounds the relative right-aligned origin before adding the
        // truncated widget left; glyph submission then applies the half-pixel.
        right[0] = layout.bounds[0].trunc()
            + (layout.bounds[2] - layout.bounds[0] - value_width - 3. + 0.5).trunc();
        paint(pages, value, [right[0], right[1]], colors[1])?;
    }
    Ok(if wrapped { 2 } else { 1 })
}

/// Linked report rows keep the existing label fitting so their source-row
/// indices continue to identify the business under the pointer. Extend the
/// clipping rectangle below the line advance: the glyph's baseline offset is
/// independent of the distance between rows.
pub fn paint_linked_pair(
    pages: &mut [Surface],
    tab: &[u8],
    layout: &Layout,
    row: usize,
    text: [&[u8]; 2],
    colors: [u32; 2],
) -> Result<(), String> {
    let [label, value] = text;
    let mut left = layout.line(row, false);
    let mut right = layout.line(row, true);
    left[3] += ROW_PITCH;
    right[3] += ROW_PITCH;
    if !value.is_empty() {
        let width = ui::text_width(tab, value, 1.)? as f32;
        left[2] = (right[2] - width - 3.).max(left[0]);
    }
    ui::text_fit(pages, tab, label, left, false, colors[0])?;
    if !value.is_empty() {
        let width = ui::text_width(tab, value, 1.)? as f32;
        right[0] = layout.bounds[0].trunc()
            + (layout.bounds[2] - layout.bounds[0] - width - 3. + 0.5).trunc();
        right[2] = layout.bounds[2];
        ui::text(pages, tab, value, right, false, colors[1])?;
    }
    Ok(())
}

/// 440650 alignment styles used by the block-view pages' titles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// Style 0: two units in from the left, top-aligned.
    Left = 0,
    /// Style 1: centred across the width, top-aligned.
    Centre = 1,
    /// Style 2: flush right, top-aligned.
    Right = 2,
    /// Style 4: centred across the width and down the height.
    Middle = 4,
}

/// 4055f0's lease page over the block panel `[left, top, right, bottom]`.
/// 40560c truncates the panel origin plus 40 for the first row; this host
/// submits the panel frame at origin 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeasePage {
    /// 405aef..405c53: Block owned by, Guide Price, then Offer/Selling Price.
    /// Each label and its right-aligned value share one control's bounds.
    pub labels: [([f32; 4], Align); 3],
    /// 405c64..405d5b: the amount's centred title over the spinner.
    pub amount: [f32; 4],
    /// 401300's navy backing under the amount.
    pub backing: [f32; 4],
}
impl LeasePage {
    pub const TOP: f32 = 40.;
    pub fn new([l, _, r, _]: [f32; 4]) -> Self {
        // The controls keep the original's literal bottom of 20, so each
        // shows a single row; the third label is centred and moved 20 down.
        let label = |row: f32, align| ([l + 4., Self::TOP + row * 12., r - 4., 20.], align);
        let mut heading = label(2., Align::Centre);
        heading.0[1] += 20.;
        heading.0[3] += 20.;
        let amount = [l + 20., Self::TOP + 60., r - 20., Self::TOP + 80.];
        Self {
            labels: [label(0., Align::Left), label(1., Align::Left), heading],
            amount,
            backing: [
                amount[0] + 2.,
                amount[1] + 1.,
                amount[2] - 2.,
                amount[3] - 7.,
            ],
        }
    }
}

/// 405eff..405fb4: the closed-rights message title on the Build page, which
/// 404eb0 fills with 1201..1203.
pub fn rights_message_bounds([l, t, r, _]: [f32; 4]) -> [f32; 4] {
    [l + 5., t + 80., r - 5., t + 180.]
}

/// A 43e9a0 text control with 440540's defaults (no insets, no line gap,
/// palette row colour), wrapped to its width by 4e7a90 and placed by 4406a0.
/// The line advance is the font's W height (nine units for font3). Returns
/// the number of rows drawn.
pub fn paint_title(
    pages: &mut [Surface],
    tab: &[u8],
    bounds: [f32; 4],
    text: &[u8],
    align: Align,
    color: u32,
) -> Result<usize, String> {
    let font = openmt2001::font::Font::parse(tab)?;
    let text = text.strip_suffix(&[0]).unwrap_or(text);
    if text.is_empty() {
        return Ok(0);
    }
    let [l, t, r, b] = bounds;
    let width = r - l;
    let (offsets, wrapped) =
        openmt2001::prompt_queue::wrap_with_gap(tab, text, width.trunc() as i32, -1)?;
    // Each terminal offset includes one separator that the row omits.
    let rows = offsets
        .windows(2)
        .map(|range| &wrapped[range[0]..(range[1] - 1).min(wrapped.len())])
        .collect::<Vec<_>>();
    let at = 4 + usize::from(b'W' - 32) * 44 + 36;
    let pitch = i32::from_le_bytes(
        tab.get(at..at + 4)
            .ok_or("title font W height metric")?
            .try_into()
            .unwrap(),
    ) as f32;
    // 440540 leaves at least one visible row, however short the control.
    let visible = ((b - t) / pitch).max(1.) as usize;
    let count = rows.len().min(visible);
    let top = t.trunc()
        + match align {
            Align::Middle => ((b - t - rows.len() as f32 * pitch) * 0.5).trunc(),
            _ => 0.,
        };
    for (index, row) in rows.into_iter().take(count).enumerate() {
        let row_width = ui::text_width(tab, row, 1.)? as f32;
        let x = match align {
            Align::Left => 2.,
            Align::Centre | Align::Middle => ((width - row_width) * 0.5 + 0.5).trunc(),
            Align::Right => (width - row_width + 0.5).trunc(),
        };
        for glyph in font.layout(
            row,
            openmt2001::font::Layout {
                origin: [l.trunc() + x, top + index as f32 * pitch],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[glyph.page].quad(glyph.positions.map(|p| [p[0], p[1]]), glyph.uv, [color; 4]);
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_font() -> Vec<u8> {
        let mut bytes = vec![0; 4 + 128 * 44];
        bytes[..4].copy_from_slice(&128u32.to_le_bytes());
        for glyph in 0..128 {
            let at = 4 + glyph * 44;
            for (offset, value) in [(32, 8i32), (36, 9), (40, 3)] {
                bytes[at + offset..at + offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            bytes[at..at + 4].copy_from_slice(&0.25f32.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn block_report_text_output_uses_bold_font_pitch_palette_and_right_alignment() {
        // Give each texture pair a different size, so using font0's pages
        // with font3's metrics also fails this production loader/draw test.
        let pixels = [255; 48];
        let textures = (0..12)
            .map(|i| openmt2001::texture::TextureRecord {
                header: [0; 14],
                width: i + 1,
                height: 1,
                texels: &pixels[..(i as usize + 1) * 4],
            })
            .collect::<Vec<_>>();
        let (font_pages, tab) = load_font(&textures, |path| {
            assert_eq!(path, "Standard/fonts/tahoma10bldnoantip.tab");
            Ok(synthetic_font())
        })
        .unwrap();
        assert_eq!(
            font_pages.iter().map(|p| p.width).collect::<Vec<_>>(),
            [7, 8]
        );
        let layout = Layout {
            bounds: [30., 100., 187., 320.],
        };
        let mut palette = [0; 18];
        palette[0] = 0xffffffff;
        palette[3] = 0xff288be1;
        palette[9] = 0xff6ed1e1;
        for linked in [false, true] {
            let mut pages = font_pages.clone();
            for row in 0..3 {
                let text: [&[u8]; 2] = [&[1, 2], &[3, 4, 5]];
                let colors = [palette[0], palette[VALUE_PALETTE]];
                if linked {
                    paint_linked_pair(&mut pages, &tab, &layout, row, text, colors).unwrap();
                } else {
                    assert_eq!(
                        paint_information_pair(&mut pages, &tab, &layout, row, text, &palette)
                            .unwrap(),
                        1
                    );
                }
            }
            let vertices = pages[0].draw().vertices;
            assert_eq!(vertices.len(), 3 * 5 * 6);
            for row in 0..3 {
                let label = &vertices[row * 30..row * 30 + 12];
                let value = &vertices[row * 30 + 12..row * 30 + 30];
                assert_eq!(label[0][0], 31.5);
                assert_eq!(label[0][1], 114.5 + row as f32 * 11.);
                assert_eq!(&label[0][3..6], &[1.; 3]);
                assert_eq!(value[0][0], 162.5);
                assert_eq!(value[13][0], 184.5);
                assert_eq!(&value[0][3..6], &[110. / 255., 209. / 255., 225. / 255.]);
                assert_eq!(label[0][6], 0.25);
                // Nine-pixel glyph height remains intact: no fitting to font0's
                // old ten-pixel boxes and no scaling of long labels.
                assert_eq!(label[0][1] - label[2][1], 9.);
            }
            assert!(pages[1].vertices.is_empty());
        }
        assert_eq!(layout.visible_rows(), 19);
    }

    #[test]
    fn overlapping_value_moves_to_next_full_size_row() {
        let mut pages = [Surface::solid(), Surface::solid()];
        let layout = Layout {
            bounds: [30., 100., 60., 320.],
        };
        assert_eq!(
            paint_information_pair(
                &mut pages,
                &synthetic_font(),
                &layout,
                0,
                [&[1, 2], &[3, 4, 5]],
                &[0xffffffff; 18]
            )
            .unwrap(),
            2
        );
        assert_eq!(
            pages[0].vertices[12][1] - pages[0].vertices[0][1],
            ROW_PITCH
        );
    }

    /// Glyph origin: Font::layout puts the quad's top at origin + rise (3)
    /// and both edges half a unit out.
    fn origin(page: &Surface, glyph: usize) -> [f32; 2] {
        let v = &page.vertices[glyph * 6..];
        [v[0][0] + 0.5, v[2][1] + 0.5 - 3.]
    }

    #[test]
    fn titles_follow_original_alignment_and_w_height_rows() {
        let tab = synthetic_font();
        let title = |bounds, text: &[u8], align| {
            let mut pages = [Surface::solid(), Surface::solid()];
            let rows = paint_title(&mut pages, &tab, bounds, text, align, 0xffffffff).unwrap();
            assert!(pages[1].vertices.is_empty());
            (rows, pages[0].clone())
        };
        // Two glyphs advance 8-1 each: 14 units wide in a 156.6-unit control.
        let bounds = [30.4, 40., 187., 20.];
        for (align, x) in [
            (Align::Left, 32.),
            (Align::Centre, 101.),
            (Align::Right, 173.),
        ] {
            let (rows, page) = title(bounds, &[1, 2], align);
            assert_eq!(rows, 1);
            assert_eq!(origin(&page, 0), [x, 40.], "{align:?}");
            assert_eq!(&page.vertices[0][3..6], &[1.; 3]);
        }
        // Style4 also centres the W height in an 88x25 button.
        let (_, page) = title([10., 20., 98., 45.], &[1, 2], Align::Middle);
        assert_eq!(origin(&page, 0), [47., 28.]);
        // Rows wrap at spaces (glyph 0) and advance by the nine-unit W height,
        // with no line gap.
        let text = [1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1];
        let (rows, page) = title([0., 40., 30., 80.], &text, Align::Left);
        assert_eq!(rows, 3);
        assert_eq!(
            [0, 3, 6].map(|glyph| origin(&page, glyph)),
            [[2., 40.], [2., 49.], [2., 58.]]
        );
        // A control shows the rows that fit, and always at least one.
        assert_eq!(title([0., 40., 30., 60.], &text, Align::Left).0, 2);
        assert_eq!(title([0., 40., 30., 20.], &text, Align::Left).0, 1);
    }

    #[test]
    fn landlord_pages_follow_the_original_builder() {
        // The block panel's bounds in the original 640x480 interface.
        let panel = [30.4, 29.6, 188.8, 298.4];
        let page = LeasePage::new(panel);
        assert_eq!(
            page.labels,
            [
                ([34.4, 40., 184.8, 20.], Align::Left),
                ([34.4, 52., 184.8, 20.], Align::Left),
                ([34.4, 84., 184.8, 40.], Align::Centre),
            ]
        );
        assert_eq!(page.amount, [50.4, 100., 168.8, 120.]);
        assert_eq!(page.backing, [52.4, 101., 166.8, 113.]);
        // At 1024x768 the navy bar covers x 84..267, y 162..181, and the
        // heading's control ends above it.
        assert_eq!(
            page.backing.map(|v| (v * 1.6).round()),
            [84., 162., 267., 181.]
        );
        assert!(page.labels[2].0[1] + 9. + 3. < page.backing[1]);
        assert_eq!(rights_message_bounds(panel), [35.4, 109.6, 183.8, 209.6]);
    }

    #[test]
    #[ignore = "requires original font3 metrics and executable palette"]
    fn block_report_style_matches_original_assets() {
        let tab = std::fs::read(openmt2001::fixtures::path("INSTALLER").join(FONT_TABLE)).unwrap();
        let sample = 4 + usize::from(b'W' - 32) * 44;
        let height = i32::from_le_bytes(tab[sample + 36..sample + 40].try_into().unwrap());
        assert_eq!(height as f32 + 2., ROW_PITCH);
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        for (index, color) in [(0, 0xffffffffu32), (VALUE_PALETTE, 0xff6ed1e1)] {
            assert_eq!(
                image.at(0x536288 + index as u32 * 4, 4).unwrap(),
                color.to_le_bytes()
            );
        }
    }
}
