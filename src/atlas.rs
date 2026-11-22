//! Original256-page atlas placement,004de930/004de2a0/004de370.
//! Pixel copying and mip generation remain in texture; registration supplies
//! group and padding. Dedicated pages are a separate original registration path.

const SIDE: usize = 256;

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub width: u32,
    pub height: u32,
    pub group: i32,
    pub padding: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub page: usize,
    pub origin: [u32; 2],
}

struct Page {
    group: i32,
    closed: bool,
    occupied: Vec<u8>,
    rows: [u16; SIDE],
    columns: [u16; SIDE],
}

#[derive(Default)]
pub struct Layout {
    pages: Vec<Page>,
}

impl Layout {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Original004df1f0 uses two pairwise-swap passes, not a stable library
    /// sort. Equal-area order can change as larger later entries move forward.
    pub fn packing_order(inputs: &[Input]) -> Vec<usize> {
        let mut order: Vec<_> = (0..inputs.len()).collect();
        for i in 0..order.len() {
            for j in i + 1..order.len() {
                if inputs[order[j]].group < inputs[order[i]].group {
                    order.swap(i, j);
                }
            }
        }
        for i in 0..order.len() {
            for j in i + 1..order.len() {
                let a = inputs[order[i]];
                let b = inputs[order[j]];
                if a.group == b.group
                    && (a.width as f32 * a.height as f32) < (b.width as f32 * b.height as f32)
                {
                    order.swap(i, j);
                }
            }
        }
        order
    }

    pub fn pack(&mut self, inputs: &[Input]) -> Result<Vec<Region>, String> {
        let mut regions = vec![
            Region {
                page: 0,
                origin: [0; 2]
            };
            inputs.len()
        ];
        for index in Self::packing_order(inputs) {
            regions[index] = self.place(inputs[index])?;
        }
        Ok(regions)
    }

    /// Place in registration order, for callers that already applied sorting.
    pub fn place(&mut self, input: Input) -> Result<Region, String> {
        if input.width == 0
            || input.height == 0
            || input.width > SIDE as u32
            || input.height > SIDE as u32
        {
            return Err("image does not fit original256 atlas path".into());
        }
        let padding = usize::from(input.padding) * 2;
        let horizontal = if input.width as usize + padding >= SIDE {
            0
        } else {
            padding
        };
        let vertical = if input.height as usize + padding >= SIDE {
            0
        } else {
            padding
        };
        let width = input.width as usize + horizontal;
        let height = input.height as usize + vertical;
        for page_index in 0..1224 {
            if page_index == self.pages.len() {
                self.pages.push(Page {
                    group: input.group,
                    closed: false,
                    occupied: vec![0; SIDE * SIDE],
                    rows: [0; SIDE],
                    columns: [0; SIDE],
                });
            }
            let page = &mut self.pages[page_index];
            if page.closed || page.group != input.group {
                continue;
            }
            let mut found = None;
            if input.width <= input.height {
                'search_rows: for y in (0..=SIDE - height).step_by(4) {
                    for x in (usize::from(page.rows[y])..=SIDE - width).step_by(4) {
                        if page.fits(x, y, width, height) {
                            found = Some((x, y));
                            break 'search_rows;
                        }
                    }
                }
            } else {
                'search_columns: for x in (0..=SIDE - width).step_by(4) {
                    for y in (usize::from(page.columns[x])..=SIDE - height).step_by(4) {
                        if page.fits(x, y, width, height) {
                            found = Some((x, y));
                            break 'search_columns;
                        }
                    }
                }
            }
            if let Some((x, y)) = found {
                for row in y..y + height {
                    page.occupied[row * SIDE + x..row * SIDE + x + width].fill(1);
                    if x <= usize::from(page.rows[row]) {
                        page.rows[row] = page.rows[row].wrapping_add(width as u16);
                    }
                }
                for col in x..x + width {
                    if y <= usize::from(page.columns[col]) {
                        page.columns[col] = page.columns[col].wrapping_add(height as u16);
                    }
                }
                page.closed = input.width as usize + padding >= SIDE
                    && input.height as usize + padding >= SIDE;
                return Ok(Region {
                    page: page_index,
                    origin: [(x + horizontal / 2) as u32, (y + vertical / 2) as u32],
                });
            }
        }
        Err("original atlas page limit exceeded".into())
    }
}

impl Page {
    fn fits(&self, x: usize, y: usize, width: usize, height: usize) -> bool {
        // Original checks the perimeter at four-pixel intervals. Do not replace
        // this with a full rectangle occupancy test: it changes native placement.
        (y..y + height).step_by(4).all(|row| {
            self.occupied[row * SIDE + x] == 0 && self.occupied[row * SIDE + x + width - 1] == 0
        }) && (x..x + width).step_by(4).all(|col| {
            self.occupied[y * SIDE + col] == 0 && self.occupied[(y + height - 1) * SIDE + col] == 0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original atlas layout observations"]
    fn matches_native_atlas_layout() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_ATLAS_LAYOUT").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTALAY01");
        assert_eq!((word(8), word(12)), (6, 0x027f));
        let mut at = 16;
        for case in 0..6 {
            let sorted = word(at) != 0;
            let count = word(at + 4) as usize;
            let page_count = word(at + 8) as usize;
            at += 12;
            let mut inputs = Vec::new();
            let mut expected = Vec::new();
            for _ in 0..count {
                inputs.push(Input {
                    width: word(at),
                    height: word(at + 4),
                    group: word(at + 8) as i32,
                    padding: word(at + 12) as u8,
                });
                expected.push(Region {
                    page: word(at + 16) as usize,
                    origin: [word(at + 20), word(at + 24)],
                });
                at += 28;
            }
            let mut layout = Layout::default();
            let actual = if sorted {
                layout.pack(&inputs).unwrap()
            } else {
                inputs
                    .into_iter()
                    .map(|input| layout.place(input).unwrap())
                    .collect()
            };
            assert_eq!(actual, expected, "atlas regions case {case}");
            assert_eq!(layout.page_count(), page_count);
            for (index, page) in layout.pages.iter().enumerate() {
                assert_eq!(page.group, word(at) as i32);
                assert_eq!(page.closed, word(at + 4) != 0);
                at += 8;
                assert_eq!(
                    page.occupied
                        .iter()
                        .zip(&bytes[at..at + SIDE * SIDE])
                        .position(|(a, b)| a != b),
                    None,
                    "atlas occupancy case {case} page {index}"
                );
                at += SIDE * SIDE;
                for hint in page.rows.into_iter().chain(page.columns) {
                    assert_eq!(
                        hint,
                        u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()),
                        "atlas hint case {case} page {index}"
                    );
                    at += 2;
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
}
