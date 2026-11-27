//! Readers for the original `.tt` and `.gtd` texture payloads.
//!
//! A texture payload starts with a little-endian count word followed by one
//! or more 56-byte records.  Each record contains fourteen little-endian
//! words and its raw RGBA texels.  A standalone `.tt` has count `1`; a `.gtd`
//! contains a collection.  The record header's integer dimensions and finite
//! floating-point dimensions are checked against one another before sizing
//! the borrowed texel slice.

use core::fmt;
use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Worker-owned immutable original texture conversion results. The renderer
/// still receives exactly the original loader's bytes; only repeated CPU
/// conversion is avoided. Bound both retained input/output bytes and entries.
#[derive(Default)]
pub struct LoaderCache {
    entries: VecDeque<LoaderEntry>,
    bytes: usize,
}
struct LoaderEntry {
    header: [u32; RECORD_WORDS],
    hash: u64,
    source: Arc<[u8]>,
    pixels: Arc<[u8]>,
    flags: Option<u32>,
}
impl LoaderCache {
    /// Converted pixels and native alpha classification from the same source.
    /// Flags are computed lazily and retained under the existing exact-content
    /// cache key; pixel-only callers do not incur an additional scan.
    pub fn pixels_and_flags(&mut self, image: &TextureRecord<'_>) -> (Arc<[u8]>, u32) {
        let pixels = self.pixels(image);
        let flags = self.flags_for_pixels(image, &pixels);
        (pixels, flags)
    }
    fn flags_for_pixels(&mut self, image: &TextureRecord<'_>, pixels: &Arc<[u8]>) -> u32 {
        // pixels() moves a hit or newly retained conversion to the back.
        // Oversized conversions are not retained and must classify directly.
        if let Some(entry) = self
            .entries
            .back_mut()
            .filter(|entry| Arc::ptr_eq(&entry.pixels, pixels))
        {
            *entry.flags.get_or_insert_with(|| loader_flags(image))
        } else {
            loader_flags(image)
        }
    }
    pub fn pixels(&mut self, image: &TextureRecord<'_>) -> Arc<[u8]> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        image.texels.hash(&mut hash);
        let hash = hash.finish();
        if let Some(index) = self.entries.iter().position(|entry| {
            entry.header == image.header
                && entry.hash == hash
                && entry.source.as_ref() == image.texels
        }) {
            let entry = self.entries.remove(index).unwrap();
            let pixels = entry.pixels.clone();
            self.entries.push_back(entry);
            return pixels;
        }
        let pixels: Arc<[u8]> = loader_pixels(image).into();
        let bytes = pixels.len().saturating_add(image.texels.len());
        if bytes <= 64 * 1024 * 1024 {
            while self.bytes + bytes > 64 * 1024 * 1024 || self.entries.len() >= 1024 {
                let removed = self.entries.pop_front().unwrap();
                self.bytes -= removed.source.len() + removed.pixels.len();
            }
            self.entries.push_back(LoaderEntry {
                header: image.header,
                hash,
                source: Arc::from(image.texels),
                pixels: pixels.clone(),
                flags: None,
            });
            self.bytes += bytes;
        }
        pixels
    }
}

#[test]
fn cached_original_loader_preserves_pixels_and_detects_changed_source() {
    let mut cache = LoaderCache::default();
    let source = [34, 67, 91, 255, 255, 0, 100, 17];
    let record = TextureRecord {
        header: [0; RECORD_WORDS],
        width: 2,
        height: 1,
        texels: &source,
    };
    let first = cache.pixels(&record);
    assert_eq!(first.as_ref(), loader_pixels(&record));
    let repeated = cache.pixels(&record);
    assert!(Arc::ptr_eq(&first, &repeated));
    let mut changed = source;
    changed[0] = 240;
    let record = TextureRecord {
        texels: &changed,
        ..record
    };
    let next = cache.pixels(&record);
    assert_eq!(next.as_ref(), loader_pixels(&record));
    assert!(!Arc::ptr_eq(&first, &next));
    assert_eq!(cache.entries.len(), 2);
}

#[test]
fn cached_loader_flags_follow_source_and_header_changes() {
    let mut cache = LoaderCache::default();
    let source = [34, 67, 91, 255, 255, 0, 100, 255];
    let image = TextureRecord {
        header: [0; RECORD_WORDS],
        width: 2,
        height: 1,
        texels: &source,
    };
    let first = cache.pixels(&image);
    assert_eq!(cache.entries.back().unwrap().flags, None);
    let (pixels, flags) = cache.pixels_and_flags(&image);
    assert!(Arc::ptr_eq(&first, &pixels));
    assert_eq!(flags, loader_flags(&image));
    assert_eq!(cache.entries.back().unwrap().flags, Some(flags));
    let (repeated, repeated_flags) = cache.pixels_and_flags(&image);
    assert!(Arc::ptr_eq(&pixels, &repeated));
    assert_eq!(repeated_flags, flags);

    let mut alpha = source;
    alpha[3] = 127;
    let changed = TextureRecord {
        texels: &alpha,
        ..image.clone()
    };
    let (pixels, changed_flags) = cache.pixels_and_flags(&changed);
    assert_eq!(pixels.as_ref(), loader_pixels(&changed));
    assert_eq!(changed_flags, loader_flags(&changed));
    assert_ne!(changed_flags, flags);
    let mut header = changed.header;
    header[0] = 0x10000002;
    let changed_header = TextureRecord {
        header,
        ..changed.clone()
    };
    let (_, header_flags) = cache.pixels_and_flags(&changed_header);
    assert_eq!(header_flags, loader_flags(&changed_header));
    assert_ne!(header_flags, changed_flags);

    // An output not retained in this cache must not borrow another entry's
    // flags. This also covers the path used by over-budget conversions.
    let unrelated: Arc<[u8]> = loader_pixels(&image).into();
    assert_eq!(
        cache.flags_for_pixels(&image, &unrelated),
        loader_flags(&image)
    );
    assert_eq!(cache.entries.back().unwrap().flags, Some(header_flags));
    assert_eq!(cache.pixels_and_flags(&image).1, flags);
}

const COUNT_SIZE: usize = 4;
const RECORD_WORDS: usize = 14;
const RECORD_HEADER_SIZE: usize = RECORD_WORDS * 4;
const TEXTURE_BYTES_PER_TEXEL: u64 = 4;
const SUPPORTED_SINGLE_COUNT: u32 = 1;

/// One borrowed texture record from a `.tt` or `.gtd` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureRecord<'a> {
    /// All fourteen little-endian record-header words, in original order.
    pub header: [u32; RECORD_WORDS],
    /// Header word one.
    pub width: u32,
    /// Header word two.
    pub height: u32,
    /// Raw four-byte RGBA texels after this record's header.
    pub texels: &'a [u8],
}

/// A borrowed texture collection from an original `.gtd` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureSet<'a> {
    /// Records in their original on-disk order.
    pub textures: Vec<TextureRecord<'a>>,
}

/// Original004ff2f0 alpha classification before registration. Other flag bits
/// survive; one intermediate alpha makes the image blended, even after zeros.
pub fn loader_flags(image: &TextureRecord<'_>) -> u32 {
    let mut flags = image.header[0] & !0x5000 | 0x8000;
    for pixel in image.texels.chunks_exact(4) {
        match pixel[3] {
            255 => {}
            0 => flags = flags & !0x9000 | 0x4000,
            _ => return flags & !0xc000 | 0x1000,
        }
    }
    flags
}

/// Original4f52ac..4f52c2 finalizes combined atlas flags before device upload.
/// A page containing partial alpha cannot retain opaque/binary-alpha flags
/// merely because another image on that page supplied them during packing.
pub fn uploaded_page_flags(flags: u32) -> u32 {
    if flags & 0x1000 != 0 {
        flags & !0xc000
    } else {
        flags
    }
}

#[test]
#[ignore = "requires original texture page upload observations"]
fn matches_native_uploaded_page_flags() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_PAGE_UPLOAD_FLAGS")).unwrap();
    assert_eq!(&data[..8], b"MTUPFL01");
    assert_eq!(data.len(), 8 + 32 * 8);
    for row in data[8..].chunks_exact(8) {
        let before = u32::from_le_bytes(row[..4].try_into().unwrap());
        let after = u32::from_le_bytes(row[4..].try_into().unwrap());
        assert_eq!(uploaded_page_flags(before), after, "flags {before:08x}");
    }
}

/// Original004ff7a0 saturation adjustment,004ff3d0/004ff560 conversions.
/// Explicit f32 stores and f64 intermediates reproduce the x87 FPCW027f path.
/// Alpha is untouched. Keep original source bytes separate from these pixels.
pub fn loader_pixels(image: &TextureRecord<'_>) -> Vec<u8> {
    let mut pixels = image.texels.to_vec();
    for pixel in pixels.chunks_exact_mut(4) {
        let [r, g, b] = [pixel[0], pixel[1], pixel[2]]
            .map(|c| f64::from((f64::from(c) * f64::from(f32::from_bits(0x3b808081))) as f32));
        let value = r.max(g).max(b);
        let low = r.min(g).min(b);
        let saturation = if value == 0. {
            0.
        } else {
            ((value - low) / value) as f32
        };
        let mut hue = if saturation == 0. {
            -1.
        } else {
            let delta = value - low;
            let delta = if delta == 0. { 1_000_000. } else { delta };
            let h = if r == value {
                (g - b) / delta
            } else if g == value {
                (b - r) / delta + 2.
            } else {
                (r - g) / delta + 4.
            } as f32;
            let h = f64::from(h) * 60.;
            if h < 0. { (h + 360.) as f32 } else { h as f32 }
        };
        let saturation =
            (f64::from(saturation) * f64::from(f32::from_bits(0x3fb33333))).min(1.) as f32;
        let rgb = if saturation == 0. {
            [value; 3]
        } else {
            if hue == 360. {
                hue = 0.;
            }
            let h = (f64::from(hue) * f64::from(f32::from_bits(0x3c888889))) as f32;
            let sector = h as i32;
            let fraction = f64::from(h) - f64::from(sector);
            let s = f64::from(saturation);
            let p = ((1. - s) * value) as f32;
            let q = ((1. - fraction * s) * value) as f32;
            let t = ((1. - (1. - fraction) * s) * value) as f32;
            let [p, q, t] = [p, q, t].map(f64::from);
            match sector {
                0 => [value, t, p],
                1 => [q, value, p],
                2 => [p, value, t],
                3 => [p, q, value],
                4 => [t, p, value],
                5 => [value, p, q],
                _ => unreachable!("normalized source hue is in the original six sectors"),
            }
        };
        for (out, channel) in pixel[..3].iter_mut().zip(rgb) {
            *out = (channel * 255.) as u8;
        }
    }
    pixels
}

/// Copy a registered image into a caller-owned square atlas page,004de460.
/// Packing chooses the origin separately. Padding is the registration byte32;
/// a value of1 reserves one texel on each side, unless that axis fills the page.
/// Returns normalized [scale_u, scale_v, offset_u, offset_v] for004f3b50.
pub fn write_atlas_region(
    image: &TextureRecord<'_>,
    page: &mut [u8],
    side: u32,
    origin: [u32; 2],
    padding: u8,
) -> Result<[f32; 4], String> {
    if side == 0
        || !side.is_power_of_two()
        || (u64::from(side) * u64::from(side)).checked_mul(4) != Some(page.len() as u64)
        || (u64::from(image.width) * u64::from(image.height)).checked_mul(4)
            != Some(image.texels.len() as u64)
        || image.width == 0
        || image.height == 0
    {
        return Err("invalid atlas page or source image".into());
    }
    let [x, y] = origin;
    let border = u32::from(padding);
    let horizontal = if u64::from(side) <= u64::from(image.width) + u64::from(border) * 2 {
        0
    } else {
        border
    };
    let vertical = if u64::from(side) <= u64::from(image.height) + u64::from(border) * 2 {
        0
    } else {
        border
    };
    let [horizontal, vertical] = if image.header[0] & 2 == 0 {
        [horizontal, vertical]
    } else {
        [0, 0]
    };
    if x < horizontal
        || y < vertical
        || u64::from(x) + u64::from(image.width) + u64::from(horizontal) > u64::from(side)
        || u64::from(y) + u64::from(image.height) + u64::from(vertical) > u64::from(side)
    {
        return Err("atlas image or border exceeds page".into());
    }
    // Copy complete image rows, repeating the edge row for top/bottom padding.
    // Extend the left/right pixels across those rows to retain the corners.
    let width = image.width as usize * 4;
    let stride = side as usize * 4;
    for row in y - vertical..y + image.height + vertical {
        let source_y = row.saturating_sub(y).min(image.height - 1) as usize;
        let source = &image.texels[source_y * width..(source_y + 1) * width];
        let start = row as usize * stride + x as usize * 4;
        page[start..start + width].copy_from_slice(source);
        let border_bytes = horizontal as usize * 4;
        for pixel in page[start - border_bytes..start].chunks_exact_mut(4) {
            pixel.copy_from_slice(&source[..4]);
        }
        for pixel in page[start + width..start + width + border_bytes].chunks_exact_mut(4) {
            pixel.copy_from_slice(&source[width - 4..]);
        }
    }
    Ok([
        image.width as f32 / side as f32,
        image.height as f32 / side as f32,
        x as f32 / side as f32,
        y as f32 / side as f32,
    ])
}

impl<'a> TextureSet<'a> {
    /// Parse a complete `.gtd` payload and validate every record and texel
    /// range.  A `.tt` payload is also accepted when its count word is `1`.
    pub fn parse(payload: &'a [u8]) -> Result<Self, TextureError> {
        if payload.len() < COUNT_SIZE {
            return Err(TextureError::TruncatedCount {
                actual: payload.len(),
            });
        }

        let count = read_u32(payload, 0);
        let count_usize =
            usize::try_from(count).map_err(|_| TextureError::CountOverflow { count })?;
        let minimum_records = count_usize
            .checked_mul(RECORD_HEADER_SIZE)
            .and_then(|size| COUNT_SIZE.checked_add(size))
            .ok_or(TextureError::CountOverflow { count })?;
        if payload.len() < minimum_records {
            return Err(TextureError::TruncatedRecords {
                count,
                expected_at_least: minimum_records,
                actual: payload.len(),
            });
        }

        let mut textures = Vec::with_capacity(count_usize);
        let mut cursor = COUNT_SIZE;
        for record in 0..count_usize {
            let (texture, next_cursor) = parse_record(payload, record, cursor)?;
            textures.push(texture);
            cursor = next_cursor;
        }
        if cursor != payload.len() {
            return Err(TextureError::TrailingBytes {
                expected: cursor,
                actual: payload.len(),
            });
        }

        Ok(Self { textures })
    }
}

/// A borrowed original standalone `.tt` texture.
///
/// This compatibility view retains the historical 15-word `header` field:
/// its word zero is the standalone count (`1`), followed by the fourteen
/// words exposed by [`TextureRecord::header`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture<'a> {
    /// Count word followed by all fourteen record-header words.
    pub header: [u32; RECORD_WORDS + 1],
    /// Compatibility alias for the former field name; this is the count word.
    pub tag: u32,
    /// Standalone count, always `1` for a successfully parsed `.tt`.
    pub count: u32,
    /// Header word one of the record.
    pub width: u32,
    /// Header word two of the record.
    pub height: u32,
    /// Raw file RGBA texels; parsing performs no premultiplication or color
    /// conversion. [`loader_pixels`] reproduces the native saturation step.
    pub texels: &'a [u8],
}

impl<'a> Texture<'a> {
    /// Parse one standalone `.tt` payload.  The count word must be exactly 1.
    pub fn parse(payload: &'a [u8]) -> Result<Self, TextureError> {
        let count = if payload.len() >= COUNT_SIZE {
            read_u32(payload, 0)
        } else {
            return Err(TextureError::TruncatedCount {
                actual: payload.len(),
            });
        };
        let set = TextureSet::parse(payload)?;
        if count != SUPPORTED_SINGLE_COUNT || set.textures.len() != 1 {
            return Err(TextureError::InvalidTextureCount { count });
        }
        let record = set
            .textures
            .into_iter()
            .next()
            .expect("count one produces one record");
        let mut header = [0; RECORD_WORDS + 1];
        header[0] = count;
        header[1..].copy_from_slice(&record.header);
        Ok(Self {
            header,
            tag: count,
            count,
            width: record.width,
            height: record.height,
            texels: record.texels,
        })
    }
}

fn parse_record<'a>(
    payload: &'a [u8],
    record: usize,
    cursor: usize,
) -> Result<(TextureRecord<'a>, usize), TextureError> {
    let header_end = cursor
        .checked_add(RECORD_HEADER_SIZE)
        .ok_or(TextureError::RecordSizeOverflow { record })?;
    if header_end > payload.len() {
        return Err(TextureError::TruncatedRecordHeader {
            record,
            actual: payload.len().saturating_sub(cursor),
        });
    }

    let header = std::array::from_fn(|word| read_u32(payload, cursor + word * 4));
    let width = header[1];
    let height = header[2];
    let float_width = f32::from_bits(header[3]);
    let float_height = f32::from_bits(header[4]);
    validate_float_dimensions(
        record,
        width,
        height,
        float_width,
        float_height,
        header[3],
        header[4],
    )?;
    let texel_bytes = checked_texel_bytes(record, float_width, float_height)?;
    let texel_end_u64 = u64::try_from(header_end)
        .ok()
        .and_then(|start| start.checked_add(texel_bytes))
        .ok_or(TextureError::PayloadSizeOverflow {
            record,
            width,
            height,
        })?;
    let texel_end =
        usize::try_from(texel_end_u64).map_err(|_| TextureError::PayloadSizeOverflow {
            record,
            width,
            height,
        })?;
    if texel_end > payload.len() {
        return Err(TextureError::TruncatedTexels {
            record,
            expected: texel_end_u64 - header_end as u64,
            actual: payload.len().saturating_sub(header_end),
            width,
            height,
        });
    }

    Ok((
        TextureRecord {
            header,
            width,
            height,
            texels: &payload[header_end..texel_end],
        },
        texel_end,
    ))
}

fn validate_float_dimensions(
    record: usize,
    width: u32,
    height: u32,
    float_width: f32,
    float_height: f32,
    float_width_bits: u32,
    float_height_bits: u32,
) -> Result<(), TextureError> {
    for (floating, bits) in [
        (float_width, float_width_bits),
        (float_height, float_height_bits),
    ] {
        if !floating.is_finite()
            || floating < 0.0
            || f64::from(floating) > f64::from(u32::MAX)
            || floating.fract() != 0.0
        {
            return Err(TextureError::InvalidFloatDimension { record, bits });
        }
    }
    if float_width as u32 != width || float_height as u32 != height {
        return Err(TextureError::DimensionMismatch {
            record,
            width,
            height,
            float_width_bits,
            float_height_bits,
        });
    }
    Ok(())
}

fn checked_texel_bytes(record: usize, width: f32, height: f32) -> Result<u64, TextureError> {
    let width = width as u64;
    let height = height as u64;
    let bytes = width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(TEXTURE_BYTES_PER_TEXEL))
        .ok_or(TextureError::DimensionOverflow {
            record,
            width: width.min(u32::MAX as u64) as u32,
            height: height.min(u32::MAX as u64) as u32,
        })?;
    if bytes > isize::MAX as u64 {
        return Err(TextureError::DimensionOverflow {
            record,
            width: width.min(u32::MAX as u64) as u32,
            height: height.min(u32::MAX as u64) as u32,
        });
    }
    Ok(bytes)
}

/// Errors encountered while decoding an original `.tt` or `.gtd` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureError {
    TruncatedCount {
        actual: usize,
    },
    InvalidTextureCount {
        count: u32,
    },
    CountOverflow {
        count: u32,
    },
    TruncatedRecords {
        count: u32,
        expected_at_least: usize,
        actual: usize,
    },
    RecordSizeOverflow {
        record: usize,
    },
    TruncatedRecordHeader {
        record: usize,
        actual: usize,
    },
    InvalidFloatDimension {
        record: usize,
        bits: u32,
    },
    DimensionMismatch {
        record: usize,
        width: u32,
        height: u32,
        float_width_bits: u32,
        float_height_bits: u32,
    },
    DimensionOverflow {
        record: usize,
        width: u32,
        height: u32,
    },
    PayloadSizeOverflow {
        record: usize,
        width: u32,
        height: u32,
    },
    TruncatedTexels {
        record: usize,
        expected: u64,
        actual: usize,
        width: u32,
        height: u32,
    },
    TrailingBytes {
        expected: usize,
        actual: usize,
    },
}

impl core::error::Error for TextureError {}

impl fmt::Display for TextureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedCount { actual } => {
                write!(formatter, "texture count is truncated ({actual} bytes)")
            }
            Self::InvalidTextureCount { count } => {
                write!(
                    formatter,
                    "standalone texture requires count 1, got {count}"
                )
            }
            Self::CountOverflow { count } => {
                write!(formatter, "texture count {count} overflows table size")
            }
            Self::TruncatedRecords {
                count,
                expected_at_least,
                actual,
            } => write!(
                formatter,
                "texture collection count {count} needs at least {expected_at_least} bytes, got {actual}"
            ),
            Self::RecordSizeOverflow { record } => {
                write!(formatter, "texture record {record} header size overflows")
            }
            Self::TruncatedRecordHeader { record, actual } => write!(
                formatter,
                "texture record {record} header is truncated ({actual} bytes)"
            ),
            Self::InvalidFloatDimension { record, bits } => write!(
                formatter,
                "texture record {record} has invalid float dimension bits 0x{bits:08x}"
            ),
            Self::DimensionMismatch {
                record,
                width,
                height,
                float_width_bits,
                float_height_bits,
            } => write!(
                formatter,
                "texture record {record} integer dimensions {width}x{height} disagree with float bits 0x{float_width_bits:08x}/0x{float_height_bits:08x}"
            ),
            Self::DimensionOverflow {
                record,
                width,
                height,
            } => write!(
                formatter,
                "texture record {record} dimensions {width}x{height} overflow texel size"
            ),
            Self::PayloadSizeOverflow {
                record,
                width,
                height,
            } => write!(
                formatter,
                "texture record {record} payload size overflows for {width}x{height}"
            ),
            Self::TruncatedTexels {
                record,
                expected,
                actual,
                width,
                height,
            } => write!(
                formatter,
                "texture record {record} ({width}x{height}) needs {expected} texel bytes, got {actual}"
            ),
            Self::TrailingBytes { expected, actual } => write!(
                formatter,
                "texture collection ends at {expected} bytes but payload has {actual}"
            ),
        }
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Original 004e4350/004e43c0 mip chain for a power-of-two texture page.
/// Returns reduced levels only; the caller retains the original base level.
/// Page packing and memory-budget reduction are separate upstream operations.
pub fn page_mipmaps(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if !width.is_power_of_two()
        || !height.is_power_of_two()
        || (u64::from(width) * u64::from(height)).checked_mul(4) != Some(rgba.len() as u64)
    {
        return Err("mipmaps require a complete power-of-two RGBA page".into());
    }
    let (mut w, mut h) = (width as usize, height as usize);
    let mut levels: Vec<Vec<u8>> = Vec::new();
    while w > 1 && h > 1 {
        let source = levels.last().map_or(rgba, Vec::as_slice);
        let mut next = vec![0; w * h];
        for y in 0..h / 2 {
            for x in 0..w / 2 {
                let at = (y * 2 * w + x * 2) * 4;
                for c in 0..4 {
                    let sum = u16::from(source[at + c])
                        + u16::from(source[at + 4 + c])
                        + u16::from(source[at + w * 4 + c])
                        + u16::from(source[at + w * 4 + 4 + c]);
                    next[(y * (w / 2) + x) * 4 + c] = ((sum + 2) >> 2) as u8;
                }
            }
        }
        levels.push(next);
        w /= 2;
        h /= 2;
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    #[test]
    fn atlas_row_copy_matches_clamped_pixels() {
        let mut checked = 0;
        for side in [1u32, 2, 4, 8, 16, 32, 64, 256] {
            let mut dimensions = vec![1, 2, 3, side / 2, side];
            dimensions.retain(|&n| n > 0 && n <= side);
            dimensions.sort_unstable();
            dimensions.dedup();
            for &width in &dimensions {
                for &height in &dimensions {
                    let texels: Vec<_> = (0..width as usize * height as usize * 4)
                        .map(|i| (i.wrapping_mul(73).wrapping_add(i / 7)) & 255)
                        .map(|n| n as u8)
                        .collect();
                    for padding in [0u8, 1, 2, 255] {
                        for flags in [0, 2] {
                            let mut header = [0; 14];
                            header[0] = flags;
                            let image = super::TextureRecord {
                                header,
                                width,
                                height,
                                texels: &texels,
                            };
                            for origin in [[0, 0], [1, 1], [u32::from(padding); 2], [side / 2; 2]] {
                                let mut page = vec![0xa5; side as usize * side as usize * 4];
                                let actual = super::write_atlas_region(
                                    &image, &mut page, side, origin, padding,
                                );
                                let Ok(uv) = actual else {
                                    assert!(page.iter().all(|&b| b == 0xa5));
                                    continue;
                                };
                                let border = |size: u32| {
                                    if flags & 2 != 0 || side <= size + u32::from(padding) * 2 {
                                        0
                                    } else {
                                        u32::from(padding)
                                    }
                                };
                                let [x, y] = origin;
                                let mut expected = vec![0xa5; page.len()];
                                for row in y - border(height)..y + height + border(height) {
                                    for col in x - border(width)..x + width + border(width) {
                                        let sy = row.saturating_sub(y).min(height - 1);
                                        let sx = col.saturating_sub(x).min(width - 1);
                                        let source = (sy * width + sx) as usize * 4;
                                        let target = (row * side + col) as usize * 4;
                                        expected[target..target + 4]
                                            .copy_from_slice(&texels[source..source + 4]);
                                    }
                                }
                                assert_eq!(
                                    page, expected,
                                    "side {side}, {width}x{height}, padding {padding}, flags {flags}, origin {origin:?}"
                                );
                                assert_eq!(
                                    uv,
                                    [
                                        width as f32 / side as f32,
                                        height as f32 / side as f32,
                                        x as f32 / side as f32,
                                        y as f32 / side as f32
                                    ]
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 1000, "only {checked} valid atlas writes checked");
    }

    #[test]
    #[ignore = "requires original texture loader observations"]
    fn matches_native_texture_loader() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_TEXTURE_LOADER")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTLOAD01");
        assert_eq!((word(8), word(12)), (3, 0x027f));
        let mut at = 16;
        for case in 0..3 {
            let width = word(at);
            let mut header = [0; 14];
            header[0] = word(at + 4);
            let expected_flags = word(at + 8);
            at += 12;
            let size = width as usize * 4;
            let image = super::TextureRecord {
                header,
                width,
                height: 1,
                texels: &bytes[at..at + size],
            };
            assert_eq!(
                super::loader_flags(&image),
                expected_flags,
                "flags case {case}"
            );
            let actual = super::loader_pixels(&image);
            at += size;
            let expected = &bytes[at..at + size];
            for (index, (a, b)) in actual
                .chunks_exact(4)
                .zip(expected.chunks_exact(4))
                .enumerate()
            {
                assert_eq!(
                    a,
                    b,
                    "case {case} pixel {index}: source {:?}",
                    &image.texels[index * 4..index * 4 + 4]
                );
            }
            at += size;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original atlas-copy observations"]
    fn matches_native_atlas_copy() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_ATLAS_COPY")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTATLAS1");
        assert_eq!((word(8), word(12)), (7, 0x027f));
        let mut at = 16;
        for case in 0..7 {
            let width = word(at);
            let height = word(at + 4);
            let mut header = [0; 14];
            header[0] = word(at + 8);
            let origin = [word(at + 12), word(at + 16)];
            let padding = word(at + 20) as u8;
            at += 24;
            let length = width as usize * height as usize * 4;
            let image = super::TextureRecord {
                header,
                width,
                height,
                texels: &bytes[at..at + length],
            };
            at += length;
            let mut page = bytes[at..at + 256 * 256 * 4].to_vec();
            at += page.len();
            let rectangle =
                super::write_atlas_region(&image, &mut page, 256, origin, padding).unwrap();
            for value in rectangle {
                assert_eq!(value.to_bits(), word(at), "atlas rectangle case {case}");
                at += 4;
            }
            assert_eq!(
                page,
                &bytes[at..at + page.len()],
                "atlas pixels case {case}"
            );
            at += page.len();
        }
        assert_eq!(at, bytes.len());
    }
    use super::*;

    #[test]
    #[ignore = "requires original-instruction mip fixture"]
    fn matches_native_page_mipmaps() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_MIPMAPS")).unwrap();
        assert_eq!(&bytes[..8], b"MTMIP001");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        assert_eq!(count, 520);
        let mut cursor = 12;
        for case in 0..count {
            let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            let (width, height, count) = (word(cursor), word(cursor + 4), word(cursor + 8));
            cursor += 12;
            let size = (width * height * 4) as usize;
            let levels = page_mipmaps(width, height, &bytes[cursor..cursor + size]).unwrap();
            cursor += size;
            assert_eq!(levels.len() as u32 + 1, count);
            for level in levels {
                assert_eq!(
                    level.as_slice(),
                    &bytes[cursor..cursor + level.len()],
                    "page {case}"
                );
                cursor += level.len();
            }
        }
        assert_eq!(cursor, bytes.len());
    }

    fn record(width: u32, height: u32, seed: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(RECORD_HEADER_SIZE + (width * height * 4) as usize);
        let words = [
            0x1000_0000,
            width,
            height,
            (width as f32).to_bits(),
            (height as f32).to_bits(),
            0x1234_5678,
            0x9abc_def0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
        ];
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        for index in 0..(width * height * 4) {
            bytes.push(seed.wrapping_add(index as u8));
        }
        bytes
    }

    fn collection(records: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for record in records {
            bytes.extend_from_slice(record);
        }
        bytes
    }

    #[test]
    fn parses_collection_and_single_texture_compatibility_view() {
        let first = record(2, 1, 0x10);
        let second = record(1, 2, 0x20);
        let bytes = collection(&[first, second]);
        let set = TextureSet::parse(&bytes).expect("valid collection");
        assert_eq!(set.textures.len(), 2);
        assert_eq!(set.textures[0].header[1], 2);
        assert_eq!(
            set.textures[0].texels,
            &[0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17]
        );
        assert_eq!(set.textures[1].header[2], 2);

        let single_bytes = collection(&[record(2, 1, 0x30)]);
        let texture = Texture::parse(&single_bytes).expect("valid single texture");
        assert_eq!(texture.count, 1);
        assert_eq!(texture.tag, 1);
        assert_eq!(texture.header[1], 0x1000_0000);
        assert_eq!(texture.texels.len(), 8);
    }

    #[test]
    fn rejects_truncated_count_and_record_table() {
        assert_eq!(
            TextureSet::parse(&[1, 0, 0]).unwrap_err(),
            TextureError::TruncatedCount { actual: 3 }
        );
        assert_eq!(
            TextureSet::parse(&1u32.to_le_bytes()).unwrap_err(),
            TextureError::TruncatedRecords {
                count: 1,
                expected_at_least: 4 + RECORD_HEADER_SIZE,
                actual: 4,
            }
        );
    }

    #[test]
    fn rejects_truncated_record_header_and_texels() {
        let mut header_only = 1u32.to_le_bytes().to_vec();
        header_only.extend_from_slice(&[0; RECORD_HEADER_SIZE - 1]);
        assert_eq!(
            TextureSet::parse(&header_only).unwrap_err(),
            TextureError::TruncatedRecords {
                count: 1,
                expected_at_least: COUNT_SIZE + RECORD_HEADER_SIZE,
                actual: COUNT_SIZE + RECORD_HEADER_SIZE - 1,
            }
        );

        let mut truncated = collection(&[record(2, 2, 0)]);
        truncated.pop();
        assert_eq!(
            TextureSet::parse(&truncated).unwrap_err(),
            TextureError::TruncatedTexels {
                record: 0,
                expected: 16,
                actual: 15,
                width: 2,
                height: 2,
            }
        );
    }

    #[test]
    fn rejects_dimension_mismatch_nonfinite_and_trailing_bytes() {
        let mut mismatch = collection(&[record(2, 2, 0)]);
        mismatch[4 + 12..4 + 16].copy_from_slice(&(3f32.to_bits()).to_le_bytes());
        assert!(matches!(
            TextureSet::parse(&mismatch),
            Err(TextureError::DimensionMismatch { record: 0, .. })
        ));

        let mut nan = collection(&[record(1, 1, 0)]);
        nan[4 + 12..4 + 16].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(matches!(
            TextureSet::parse(&nan),
            Err(TextureError::InvalidFloatDimension { record: 0, .. })
        ));

        // u32::MAX rounds up to 2^32 in f32; a saturating float cast must
        // not make that out-of-range dimension appear to agree with MAX.
        let mut rounded_max = collection(&[record(1, 1, 0)]);
        rounded_max[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        rounded_max[16..20].copy_from_slice(&(4294967296f32).to_bits().to_le_bytes());
        assert!(matches!(
            TextureSet::parse(&rounded_max),
            Err(TextureError::InvalidFloatDimension { record: 0, .. })
        ));

        let mut trailing = collection(&[record(1, 1, 0)]);
        trailing.push(0xff);
        assert_eq!(
            TextureSet::parse(&trailing).unwrap_err(),
            TextureError::TrailingBytes {
                expected: 64,
                actual: 65,
            }
        );
    }

    #[test]
    fn rejects_single_texture_count_mismatch_and_dimension_overflow() {
        let mut two = collection(&[record(1, 1, 0), record(1, 1, 0)]);
        assert_eq!(
            Texture::parse(&two).unwrap_err(),
            TextureError::InvalidTextureCount { count: 2 }
        );
        two[0..4].copy_from_slice(&2u32.to_le_bytes());
        assert!(matches!(TextureSet::parse(&two), Ok(TextureSet { .. })));

        assert_eq!(
            checked_texel_bytes(3, f32::MAX, f32::MAX).unwrap_err(),
            TextureError::DimensionOverflow {
                record: 3,
                width: u32::MAX,
                height: u32::MAX,
            }
        );
    }

    #[test]
    #[ignore = "requires OPENMT2001_ARCHIVE_DIR and OPENMT2001_ARCHIVE_BIN"]
    fn parses_all_original_gtd_and_tt_entries_when_paths_are_supplied() {
        let dir_path = crate::fixtures::path("ARCHIVE_DIR");
        let bin_path = crate::fixtures::path("ARCHIVE_BIN");
        let dir = std::fs::read(dir_path).expect("read original archive.DIR");
        let payload = std::fs::read(bin_path).expect("read original archive.bin");
        let index = crate::archive::ArchiveIndex::parse(&dir, payload.len() as u64)
            .expect("parse original archive.DIR");
        let mut gtd_count = 0;
        let mut gtd_textures = 0;
        let mut tt_count = 0;
        for entry in &index.entries {
            let start = usize::try_from(entry.offset).expect("32-bit offset fits usize");
            let end = start + usize::try_from(entry.length).expect("32-bit length fits usize");
            let bytes = &payload[start..end];
            if entry.path.to_ascii_lowercase().ends_with(".gtd") {
                let set = TextureSet::parse(bytes).expect("parse original .gtd payload");
                gtd_count += 1;
                gtd_textures += set.textures.len();
            } else if entry.path.to_ascii_lowercase().ends_with(".tt") {
                Texture::parse(bytes).expect("parse original .tt payload");
                tt_count += 1;
            }
        }
        assert_eq!(gtd_count, 17);
        assert_eq!(gtd_textures, 1_566);
        assert_eq!(tt_count, 1_021);
    }
}
