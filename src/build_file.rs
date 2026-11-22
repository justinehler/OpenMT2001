//! Original version-9 BuildFile.dat tables and deterministic model selection.
//! Layout: native 004d36c0; search: native 004d4e50 (random-pick flag disabled).

#[derive(Debug)]
pub struct BuildFile<'a> {
    pub header: [u32; 25],
    pub points: &'a [u8],
    pub point_attributes: &'a [u8],
    pub polygons: &'a [u8],
    pub attribute_ranges: &'a [u8],
    pub layers: &'a [u8],
    pub models: &'a [u8],
    pub details: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct ModelQuery {
    pub width: u8,
    pub depth: u8,
    pub height: u8,
    /// 0 accepts any, 1..3 selects a quality class.
    pub quality: u8,
    /// 0 accepts any, 1 corner, 2 edge.
    pub placement: u8,
    /// 0 accepts any; native search accepts categories 1..12.
    pub category: u8,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelMatch {
    pub index: usize,
    pub transposed: bool,
}

#[derive(Debug, Clone)]
pub struct BuildingLayer {
    pub base: f32,
    pub height: f32,
    pub repetitions: u8,
    pub kind: u8,
    pub surface: u8,
    pub repeat_weight: u8,
    pub flags: u8,
    pub parameters: [u32; 2],
    pub outline: Vec<OutlinePoint>,
}

#[derive(Debug, Clone, Copy)]
pub struct OutlinePoint {
    pub position: [f32; 2],
    pub attributes: [u8; 2],
}

impl<'a> BuildFile<'a> {
    /// Layer expansion from 004d46c0 and 004d4a20. This is the building profile
    /// before native facade/roof tessellation, decorations and world transforms.
    pub fn profile(&self, index: usize, height: u8) -> Result<Vec<BuildingLayer>, String> {
        let model = self
            .models
            .chunks_exact(28)
            .nth(index)
            .ok_or("missing building model")?;
        if height >= 250 {
            return Err("native building profile height limit is 249".into());
        }
        let first = word(model, 0) as usize;
        let count = word(model, 4) as usize;
        if count > 25 {
            return Err("too many layers for native building profile".into());
        }
        let records: Vec<_> = self
            .layers
            .chunks_exact(32)
            .skip(first)
            .take(count)
            .collect();
        let weights: Vec<_> = records.iter().map(|r| r[18]).collect();
        let mut repetitions = vec![1u8; count];
        let mut seed = 0u32; // Original resets its private generator for each model.
        let mut second_phase = false;
        for _ in 4..height {
            let mut capacities: Vec<i32> = weights
                .iter()
                .zip(&repetitions)
                .map(|(&weight, &used)| i32::from(weight) - i32::from(used))
                .collect();
            if capacities.iter().sum::<i32>() == 0 {
                second_phase = true;
            }
            if second_phase {
                capacities = weights
                    .iter()
                    .map(|&w| if w > 1 { i32::from(w) } else { 0 })
                    .collect();
            }
            let total: i32 = capacities.iter().sum();
            if total == 0 {
                break;
            }
            seed = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            //4d478e uses signed IDIV, including negative capacity totals.
            let pick = ((seed >> 16) & 0x7fff) as i32 % total;
            let mut accumulated = 0;
            let chosen = capacities.iter().position(|&capacity| {
                accumulated += capacity;
                pick < accumulated
            });
            //4d48d4 returns false on a failed pick. The caller4d4a20 ignores
            //that result and builds every layer using these partial counts.
            let Some(chosen) = chosen else {
                break;
            };
            repetitions[chosen] = repetitions[chosen].wrapping_add(1);
        }
        let mut base = 0f64;
        let mut result = Vec::with_capacity(count);
        for (record, repetitions) in records.into_iter().zip(repetitions) {
            let packed = word(self.polygons, word(record, 0) as usize * 4);
            let attributes = word(self.attribute_ranges, word(record, 4) as usize * 4) & 0xffffff;
            let mut outline = Vec::with_capacity((packed >> 24) as usize);
            for i in 0..(packed >> 24) as usize {
                let at = ((packed & 0xffffff) as usize + i) * 8;
                let a = self.point_attributes[attributes as usize + i];
                outline.push(OutlinePoint {
                    position: [
                        f32::from_bits(word(self.points, at)),
                        f32::from_bits(word(self.points, at + 4)),
                    ],
                    attributes: [a & 31, a >> 5],
                });
            }
            // Native holds the accumulating height in x87 across layers.
            // f64 exactly represents these f32 * u8 products and their sums
            // for the supplied corpus; native differential tests check output.
            let thickness = f64::from(f32::from_bits(word(record, 12))) * f64::from(repetitions);
            result.push(BuildingLayer {
                base: base as f32,
                height: thickness as f32,
                repetitions,
                kind: record[16],
                surface: record[17],
                repeat_weight: record[18],
                flags: record[28],
                parameters: [word(record, 20), word(record, 24)],
                outline,
            });
            if record[28] & 1 == 0 {
                base += thickness;
            }
        }
        Ok(result)
    }
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 100 {
            return Err("truncated BuildFile header".into());
        }
        let header = std::array::from_fn(|i| word(bytes, i * 4));
        if header[0] != 0x4c6d4266 || header[1] != 9 {
            return Err("expected fBmL version 9".into());
        }
        if header[2] as u64 != bytes.len() as u64 {
            return Err("BuildFile declared size mismatch".into());
        }
        let table = |count_word: usize, stride: usize| -> Result<&'a [u8], String> {
            let size = (header[count_word] as usize)
                .checked_mul(stride)
                .ok_or("BuildFile table overflow")?;
            let offset = header[count_word + 1] as usize;
            let end = offset.checked_add(size).ok_or("BuildFile table overflow")?;
            if size != 0 && offset < 100 {
                return Err("BuildFile table overlaps header".into());
            }
            bytes
                .get(offset..end)
                .ok_or_else(|| "BuildFile table outside file".into())
        };
        let result = Self {
            header,
            points: table(11, 8)?,
            point_attributes: table(13, 1)?,
            polygons: table(15, 4)?,
            attribute_ranges: table(17, 4)?,
            layers: table(19, 32)?,
            models: table(21, 28)?,
            details: table(23, 32)?,
        };
        for model in result.models.chunks_exact(28) {
            check_range(word(model, 0), word(model, 4), result.layers.len() / 32)?;
            check_range(word(model, 8), word(model, 12), result.details.len() / 32)?;
        }
        for layer in result.layers.chunks_exact(32) {
            let polygon = word(layer, 0) as usize;
            let attributes = word(layer, 4) as usize;
            if polygon >= result.polygons.len() / 4
                || attributes >= result.attribute_ranges.len() / 4
            {
                return Err("BuildFile layer references missing polygon/attributes".into());
            }
            let packed = word(result.polygons, polygon * 4);
            let attribute_packed = word(result.attribute_ranges, attributes * 4);
            check_range(packed & 0xffffff, packed >> 24, result.points.len() / 8)?;
            check_range(
                attribute_packed & 0xffffff,
                packed >> 24,
                result.point_attributes.len(),
            )?;
        }
        Ok(result)
    }

    /// Preserves original enumeration order and corner transpose bit, including
    /// the native choice to mark a square corner template as transposed.
    pub fn search(&self, query: ModelQuery) -> Result<Vec<ModelMatch>, String> {
        if !(1..=7).contains(&query.width)
            || !(1..=7).contains(&query.depth)
            || query.quality > 3
            || query.placement > 2
            || query.category > 12
            || query.limit == 0
        {
            return Err("invalid model search parameters".into());
        }
        let mut matches = Vec::new();
        if query.height == 0 {
            return Ok(matches);
        }
        for (index, record) in self.models.chunks_exact(28).enumerate() {
            let height = record[26] as i8;
            if (query.height < 4 && i16::from(height) != i16::from(query.height))
                || (query.height >= 4 && height < 4)
            {
                continue;
            }
            if query.quality != 0 && query.quality != record[25] {
                continue;
            }
            if query.category != 0 && query.category != record[24] {
                continue;
            }
            if query.placement != 0 && query.placement != record[27] {
                continue;
            }
            let w = record[21];
            let d = record[22];
            let transposed = record[27] == 1 && w == query.depth && d == query.width;
            if transposed || (w == query.width && d == query.depth) {
                matches.push(ModelMatch { index, transposed });
                if matches.len() == query.limit {
                    break;
                }
            }
        }
        Ok(matches)
    }
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn check_range(start: u32, count: u32, available: usize) -> Result<(), String> {
    if u64::from(start) + u64::from(count) > available as u64 {
        return Err("BuildFile referenced range outside table".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires private BuildFile and original-instruction profile observations"]
    fn matches_native_profiles() {
        check_native_profiles("OPENMT2001_NATIVE_PROFILES");
    }
    #[test]
    #[ignore = "requires original profiles including zero layer weights"]
    fn matches_native_zero_profiles() {
        check_native_profiles("OPENMT2001_NATIVE_ZERO_PROFILES");
    }
    fn check_native_profiles(key: &str) {
        let data =
            std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").expect("OPENMT2001_BUILD_FILE"))
                .unwrap();
        let native = std::fs::read(std::env::var(key).expect(key)).unwrap();
        let file = BuildFile::parse(&data).unwrap();
        let (mut at, count) = if &native[..8] == b"MTPROF02" {
            assert_eq!(word(&native, 8), 0x37f);
            (16, word(&native, 12))
        } else {
            assert_eq!(&native[..8], b"MTPROFIL");
            (12, word(&native, 8))
        };
        for case in 0..count {
            let index = word(&native, at) as usize;
            let height = word(&native, at + 4) as u8;
            at += 8;
            let observed = &native[at..at + 0x1ca80];
            let layers = file.profile(index, height).unwrap();
            assert_eq!(layers.len(), word(observed, 24) as usize, "case {case}");
            for (i, layer) in layers.iter().enumerate() {
                let r = &observed[28 + i * 0x122c..];
                assert_eq!(
                    u32::from(layer.flags & 1),
                    word(r, 0),
                    "case {case} layer {i} flags"
                );
                assert_eq!(u32::from(layer.kind), word(r, 4));
                assert_eq!(
                    layer.base.to_bits(),
                    word(r, 12),
                    "case {case} layer {i} base"
                );
                assert_eq!(
                    layer.height.to_bits(),
                    word(r, 16),
                    "case {case} layer {i} height"
                );
                assert_eq!(u32::from(layer.surface), word(r, 20));
                assert_eq!(
                    u32::from(layer.repetitions),
                    word(r, 24),
                    "case {case} layer {i} repeat"
                );
                assert_eq!(u32::from(layer.repeat_weight), word(r, 28));
                assert_eq!(layer.parameters, [word(r, 32), word(r, 36)]);
                assert_eq!(layer.outline.len(), word(r, 40) as usize);
                for (p, point) in layer.outline.iter().enumerate() {
                    assert_eq!(
                        point.position[0].to_bits(),
                        word(r, 44 + p * 36),
                        "case {case} layer {i} point {p} x"
                    );
                    assert_eq!(
                        point.position[1].to_bits(),
                        word(r, 48 + p * 36),
                        "case {case} layer {i} point {p} y"
                    );
                    assert_eq!(u32::from(point.attributes[0]), word(r, 52 + p * 36));
                    assert_eq!(u32::from(point.attributes[1]), word(r, 56 + p * 36));
                }
            }
            at += 0x1ca80;
        }
        assert_eq!(at, native.len());
    }
    #[test]
    #[ignore = "requires private BuildFile and original-instruction search observations"]
    fn matches_native_search() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").expect("OPENMT2001_BUILD_FILE"))
                .unwrap();
        let native = std::fs::read(
            std::env::var("OPENMT2001_NATIVE_SEARCH").expect("OPENMT2001_NATIVE_SEARCH"),
        )
        .unwrap();
        let file = BuildFile::parse(&data).unwrap();
        assert_eq!(&native[..8], b"MTSEARCH");
        let mut at = 12;
        for case in 0..word(&native, 8) {
            let fields: [u32; 8] = std::array::from_fn(|i| word(&native, at + i * 4));
            at += 32;
            let [w, d, h, q, p, c, limit, count] = fields;
            let found = file
                .search(ModelQuery {
                    width: w as u8,
                    depth: d as u8,
                    height: h as u8,
                    quality: q as u8,
                    placement: p as u8,
                    category: c as u8,
                    limit: limit as usize,
                })
                .unwrap();
            let packed: Vec<_> = found
                .iter()
                .map(|m| m.index as u32 | if m.transposed { 0x80000000 } else { 0 })
                .collect();
            let expected: Vec<_> = (0..count as usize)
                .map(|i| word(&native, at + i * 4))
                .collect();
            assert_eq!(packed, expected, "native search {case}: {fields:?}");
            at += count as usize * 4;
        }
        assert_eq!(at, native.len());
    }
    #[test]
    fn rejects_truncated_or_inconsistent_tables() {
        assert!(BuildFile::parse(&[0; 99]).is_err());
        let mut data = vec![0; 100];
        data[..4].copy_from_slice(&0x4c6d4266u32.to_le_bytes());
        data[4..8].copy_from_slice(&9u32.to_le_bytes());
        data[8..12].copy_from_slice(&100u32.to_le_bytes());
        data[44..48].copy_from_slice(&1u32.to_le_bytes());
        data[48..52].copy_from_slice(&100u32.to_le_bytes());
        assert!(BuildFile::parse(&data).is_err());
    }
}
