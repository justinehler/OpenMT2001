//! Era road markings (original 004b8d20).
//!
//! `gamedata2\roads\roads.tm` carries two built-in 1930s pages. While drawing
//! the city, the original replaces them with the current era's
//! `gamedata\roadtextures\road_<decade>_A.tga` and `_B.tga` whenever the era
//! index (00822958, clamped to 7) differs from its cached era (00545658, -1 at
//! startup). The two files follow the mesh's part order. Each file is an
//! 18-byte TGA header followed by 32-bit BGRA rows, which are written to the
//! page in stored order: halved with the 004e43c0 box filter until they reach
//! the page width, then with red and blue exchanged.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The last era with its own pair of road textures (2000s).
pub const LAST_ERA: u32 = 7;
const HEADER_BYTES: usize = 18;
/// Pixel bytes the original reads after the header.
const PIXEL_BYTES: usize = 0x40000;

/// The era index whose textures are drawn: 00822958 clamped to 7.
pub fn selected_era(era: u32) -> u32 {
    era.min(LAST_ERA)
}

/// First year of the era's textures, 1930..2000.
pub fn decade(era: u32) -> u32 {
    1930 + 10 * selected_era(era)
}

/// The era's two file names in mesh part order (table at 0054565c).
pub fn file_names(era: u32) -> [String; 2] {
    let decade = decade(era);
    ["A", "B"].map(|variant| format!("road_{decade}_{variant}.tga"))
}

/// Converts one road TGA into page pixels of `page_width` squared, in the
/// RGBA byte order of the engine's texture pages.
pub fn page_pixels(tga: &[u8], page_width: u32) -> Result<Vec<u8>, String> {
    let header = tga
        .get(..HEADER_BYTES)
        .ok_or("road texture is shorter than its TGA header")?;
    let width = u32::from(u16::from_le_bytes([header[12], header[13]]));
    let height = u32::from(u16::from_le_bytes([header[14], header[15]]));
    if header[2] != 2 || header[16] != 32 {
        return Err("road texture must be an uncompressed 32-bit TGA".into());
    }
    // The original assumes square power-of-two pages no larger than its
    // 256x256 buffer and halves until the widths agree.
    if width != height
        || !width.is_power_of_two()
        || !page_width.is_power_of_two()
        || page_width > width
        || width as usize * width as usize * 4 > PIXEL_BYTES
    {
        return Err(format!(
            "road texture {width}x{height} does not reduce to a {page_width} page"
        ));
    }
    let length = width as usize * width as usize * 4;
    let stored = tga
        .get(HEADER_BYTES..HEADER_BYTES + length)
        .ok_or("road texture pixels are truncated")?;
    let reductions = (width / page_width).trailing_zeros() as usize;
    let mut pixels = if reductions == 0 {
        stored.to_vec()
    } else {
        crate::texture::page_mipmaps(width, width, stored)?
            .into_iter()
            .nth(reductions - 1)
            .ok_or("road texture reduction is out of range")?
    };
    // 004b8d20 writes ((p >> 16) & 255) + ((p & 255) << 16) + (p & 0xff00ff00).
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(pixels)
}

/// Reads the era's road textures from the installed `roadtextures` folder and
/// keeps the last converted pair, so rebuilding a scene in the same era does
/// not read the files again.
#[derive(Default)]
pub struct Loader {
    converted: Option<Converted>,
}

struct Converted {
    era: u32,
    widths: [u32; 2],
    pages: [Arc<[u8]>; 2],
}

impl Loader {
    /// Pages for `era` at the two page widths, in mesh part order.
    pub fn pages(
        &mut self,
        directory: &Path,
        era: u32,
        widths: [u32; 2],
    ) -> Result<[Arc<[u8]>; 2], String> {
        let era = selected_era(era);
        if let Some(converted) = &self.converted
            && (converted.era, converted.widths) == (era, widths)
        {
            return Ok(converted.pages.clone());
        }
        let [a, b] = file_names(era);
        let load = |name: &str, width: u32| -> Result<Arc<[u8]>, String> {
            let path = find_file(directory, name)?;
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(page_pixels(&bytes, width)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .into())
        };
        let pages = [load(&a, widths[0])?, load(&b, widths[1])?];
        self.converted = Some(Converted {
            era,
            widths,
            pages: pages.clone(),
        });
        Ok(pages)
    }
}

/// Finds `name` in `directory` without regard to case: the installer ships
/// lower-case names while the original asks for `_A`/`_B`.
fn find_file(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let exact = directory.join(name);
    if exact.is_file() {
        return Ok(exact);
    }
    let entries =
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
        {
            return Ok(entry.path());
        }
    }
    Err(format!("missing road texture {}", exact.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tga(width: u16, pixel: impl Fn(usize) -> [u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0; HEADER_BYTES];
        bytes[2] = 2;
        bytes[12..14].copy_from_slice(&width.to_le_bytes());
        bytes[14..16].copy_from_slice(&width.to_le_bytes());
        bytes[16] = 32;
        bytes[17] = 8;
        for index in 0..usize::from(width) * usize::from(width) {
            bytes.extend(pixel(index));
        }
        bytes
    }

    #[test]
    fn maps_each_era_to_its_decade_and_clamps_after_2000() {
        assert_eq!(file_names(0), ["road_1930_A.tga", "road_1930_B.tga"]);
        assert_eq!(file_names(2), ["road_1950_A.tga", "road_1950_B.tga"]);
        for era in 0..=7 {
            assert_eq!(decade(era), 1930 + 10 * era);
        }
        assert_eq!(file_names(8), ["road_2000_A.tga", "road_2000_B.tga"]);
        assert_eq!(selected_era(u32::MAX), 7);
    }

    #[test]
    fn exchanges_red_and_blue_and_keeps_stored_row_order() {
        let source = tga(4, |i| [i as u8, 0x40, 0x80, 0xff]);
        let pixels = page_pixels(&source, 4).unwrap();
        assert_eq!(&pixels[..4], &[0x80, 0x40, 0, 0xff]);
        assert_eq!(&pixels[15 * 4..], &[0x80, 0x40, 15, 0xff]);
    }

    #[test]
    fn halves_with_the_rounded_box_filter_until_the_page_width() {
        // A 4x4 image whose top-left 2x2 block is B = 0, 1, 2, 4.
        let source = tga(4, |i| {
            let b = match (i % 4, i / 4) {
                (0, 0) => 0,
                (1, 0) => 1,
                (0, 1) => 2,
                (1, 1) => 4,
                _ => 0,
            };
            [b, 0, 0, 0xff]
        });
        let half = page_pixels(&source, 2).unwrap();
        assert_eq!(half.len(), 2 * 2 * 4);
        // (0 + 1 + 2 + 4 + 2) >> 2 = 2, now in the red byte.
        assert_eq!(&half[..4], &[0, 0, 2, 0xff]);
        let quarter = page_pixels(&source, 1).unwrap();
        assert_eq!(quarter.len(), 4);
        assert!(page_pixels(&source, 8).is_err());
    }

    #[test]
    fn rejects_truncated_or_unsupported_files() {
        let mut source = tga(4, |_| [0; 4]);
        source.pop();
        assert!(page_pixels(&source, 4).is_err());
        let mut paletted = tga(4, |_| [0; 4]);
        paletted[16] = 24;
        assert!(page_pixels(&paletted, 4).is_err());
        assert!(page_pixels(&[0; 4], 4).is_err());
    }

    #[test]
    fn loader_reads_each_era_once_and_matches_names_without_case() {
        let directory = crate::fixtures::scratch("road-texture-loader");
        let directory = directory.join("roadtextures");
        std::fs::create_dir_all(&directory).unwrap();
        for (name, blue) in [("road_1950_a.tga", 1u8), ("ROAD_1950_B.TGA", 2)] {
            std::fs::write(directory.join(name), tga(2, |_| [blue, 0, 0, 0xff])).unwrap();
        }
        let mut loader = Loader::default();
        let pages = loader.pages(&directory, 2, [2, 2]).unwrap();
        assert_eq!(&pages[0][..4], &[0, 0, 1, 0xff]);
        assert_eq!(&pages[1][..4], &[0, 0, 2, 0xff]);
        // The same era is served from the converted pair, even without files.
        for name in ["road_1950_a.tga", "ROAD_1950_B.TGA"] {
            std::fs::remove_file(directory.join(name)).unwrap();
        }
        let again = loader.pages(&directory, 2, [2, 2]).unwrap();
        assert!(Arc::ptr_eq(&pages[0], &again[0]));
        assert!(loader.pages(&directory, 3, [2, 2]).is_err());
    }

    #[test]
    #[ignore = "requires the original installer's road textures"]
    fn converts_original_1950_road_textures() {
        let directory = crate::fixtures::path("INSTALLER").join("Standard/roadtextures");
        let mut loader = Loader::default();
        let pages = loader.pages(&directory, 2, [256, 256]).unwrap();
        for (page, name) in pages.iter().zip(file_names(2)) {
            assert_eq!(page.len(), 256 * 256 * 4, "{name}");
            let bytes = std::fs::read(find_file(&directory, &name).unwrap()).unwrap();
            assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), 256);
            assert_eq!(u16::from_le_bytes([bytes[14], bytes[15]]), 256);
            for index in [0, 1000, 256 * 128 + 128, 256 * 256 - 1] {
                let stored = &bytes[HEADER_BYTES + index * 4..][..4];
                let drawn = &page[index * 4..][..4];
                assert_eq!(
                    drawn,
                    &[stored[2], stored[1], stored[0], stored[3]],
                    "{name} pixel {index}"
                );
            }
        }
        assert_ne!(pages[0], pages[1]);
        // A smaller page averages each 2x2 block of stored pixels.
        let half = loader.pages(&directory, 2, [128, 128]).unwrap();
        assert_eq!(half[0].len(), 128 * 128 * 4);
        let bytes = std::fs::read(find_file(&directory, "road_1950_A.tga").unwrap()).unwrap();
        let stored =
            |x: usize, y: usize, c: usize| u16::from(bytes[HEADER_BYTES + (y * 256 + x) * 4 + c]);
        for (drawn, c) in [(0, 2), (1, 1), (2, 0), (3, 3)] {
            let sum = stored(0, 0, c) + stored(1, 0, c) + stored(0, 1, c) + stored(1, 1, c);
            assert_eq!(u16::from(half[0][drawn]), (sum + 2) >> 2);
        }
    }
}
