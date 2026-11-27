//! Original44-byte bitmap glyph tables and4e7730/4554e0 screen layout.
//! Text consists of encoded glyph indices from the original language table.

pub struct Font<'a> {
    records: &'a [u8],
}
#[derive(Clone, Copy)]
pub struct Layout {
    pub origin: [f32; 2],
    pub depth: f32,
    pub scale: f32,
    pub gap: i32,
    pub baseline: i32,
}
#[derive(Debug)]
pub struct GlyphQuad {
    /// Font textures are paired: index95 starts the second texture.
    pub page: usize,
    pub positions: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
}
impl<'a> Font<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let header = bytes.get(..4).ok_or("truncated font table")?;
        let count = u32::from_le_bytes(header.try_into().unwrap()) as usize;
        if count == 0 || count > 256 || bytes.len() != 4 + count * 44 {
            return Err("invalid glyph table length".into());
        }
        let records = &bytes[4..];
        for record in records.chunks_exact(44) {
            if record[..32]
                .chunks_exact(4)
                .any(|v| !f32::from_le_bytes(v.try_into().unwrap()).is_finite())
            {
                return Err("non-finite glyph UV".into());
            }
            if [32, 36]
                .into_iter()
                .any(|at| i32::from_le_bytes(record[at..at + 4].try_into().unwrap()) < 0)
            {
                return Err("negative glyph dimensions".into());
            }
        }
        Ok(Self { records })
    }

    pub fn layout(&self, encoded: &[u8], layout: Layout) -> Result<Vec<GlyphQuad>, String> {
        if !layout
            .origin
            .into_iter()
            .chain([layout.depth, layout.scale])
            .all(f32::is_finite)
            || layout.scale <= 0.
        {
            return Err("invalid glyph layout scale or origin".into());
        }
        let mut x = layout.origin[0];
        let mut quads = Vec::with_capacity(encoded.len());
        for &glyph in encoded {
            let record = self
                .records
                .get(usize::from(glyph) * 44..usize::from(glyph) * 44 + 44)
                .ok_or("glyph index exceeds font table")?;
            let metric = |at| i32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            let width = truncate(f64::from(metric(32)) * f64::from(layout.scale))?;
            let height = truncate(f64::from(metric(36)) * f64::from(layout.scale))?;
            let y = (f64::from(layout.baseline)
                + f64::from(metric(40))
                + f64::from(layout.origin[1])) as f32;
            // Original callback truncates X/Y then applies the D3D8 half-pixel.
            let left = f64::from(truncate(f64::from(x))?) - 0.5;
            let top = f64::from(truncate(f64::from(y))?) - 0.5;
            let right = (left + f64::from(width)) as f32;
            let bottom = (top + f64::from(height)) as f32;
            let (left, top) = (left as f32, top as f32);
            let z = layout.depth;
            quads.push(GlyphQuad {
                page: usize::from(glyph >= 95),
                positions: [
                    [left, bottom, z],
                    [right, bottom, z],
                    [right, top, z],
                    [left, top, z],
                ],
                uv: std::array::from_fn(|i| {
                    std::array::from_fn(|axis| {
                        let at = (i * 2 + axis) * 4;
                        f32::from_le_bytes(record[at..at + 4].try_into().unwrap())
                    })
                }),
            });
            let advance =
                truncate(f64::from(metric(32).wrapping_add(layout.gap)) * f64::from(layout.scale))?;
            x = (f64::from(x) + f64::from(advance)) as f32;
        }
        Ok(quads)
    }
}

fn truncate(value: f64) -> Result<i32, String> {
    let value = value.trunc();
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err("glyph layout exceeds supported native integer conversion range".into());
    }
    Ok(value as i32)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_out_of_range_layout() {
        let mut bytes = vec![0; 48];
        bytes[..4].copy_from_slice(&1u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&1i32.to_le_bytes());
        bytes[40..44].copy_from_slice(&1i32.to_le_bytes());
        let font = super::Font::parse(&bytes).unwrap();
        let layout = super::Layout {
            origin: [0., 0.],
            depth: 0.5,
            scale: 1.,
            gap: 0,
            baseline: 0,
        };
        assert!(
            font.layout(
                &[0],
                super::Layout {
                    scale: f32::MAX,
                    ..layout
                }
            )
            .is_err()
        );
        assert!(
            font.layout(
                &[0],
                super::Layout {
                    origin: [f32::MAX, 0.],
                    ..layout
                }
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires original font layout fixture"]
    fn matches_native_glyph_layout() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_FONT")).unwrap();
        assert_eq!(&bytes[..8], b"MTFNT001");
        let mut at = 8;
        fn word(bytes: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        let count = word(&bytes, &mut at);
        for _ in 0..count {
            let length = word(&bytes, &mut at) as usize;
            let cases = word(&bytes, &mut at);
            let font = super::Font::parse(&bytes[at..at + length]).unwrap();
            at += length;
            for _ in 0..cases {
                let length = word(&bytes, &mut at) as usize;
                let encoded = &bytes[at..at + length];
                at += length;
                let scale = f32::from_bits(word(&bytes, &mut at));
                let gap = word(&bytes, &mut at) as i32;
                let baseline = word(&bytes, &mut at) as i32;
                let origin = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
                let counts = [word(&bytes, &mut at), word(&bytes, &mut at)];
                let quads = font
                    .layout(
                        encoded,
                        super::Layout {
                            origin,
                            depth: 0.5,
                            scale,
                            gap,
                            baseline,
                        },
                    )
                    .unwrap();
                for (page, count) in counts.into_iter().enumerate() {
                    let actual: Vec<_> = quads.iter().filter(|q| q.page == page).collect();
                    assert_eq!(actual.len(), count as usize);
                    for quad in &actual {
                        for point in quad.positions {
                            for value in point {
                                assert_eq!(value.to_bits(), word(&bytes, &mut at));
                            }
                        }
                    }
                    for quad in &actual {
                        for point in quad.uv {
                            for value in point {
                                assert_eq!(value.to_bits(), word(&bytes, &mut at));
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
}
