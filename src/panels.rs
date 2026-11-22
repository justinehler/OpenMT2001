//! Original Font Group And Panels registration,004bd550.
//! Paths come from the supplied v1.4 image; textures remain borrowed archive
//! data. This reconstructs registration without executing the original loader.

use crate::texture::TextureRecord;
use crate::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage, texture::TextureSet};
use core::ops::Range;

pub struct FontPanels<'a> {
    /// Registration order before004df1f0 sorts by group and image area.
    pub textures: Vec<TextureRecord<'a>>,
    pub billboards: Range<usize>,
    /// Prefix-cache keys retained for subsequent original startup registration.
    pub cache_keys: Vec<String>,
}

impl<'a> FontPanels<'a> {
    pub fn from_v14(
        image: &PeImage<'_>,
        index: &ArchiveIndex,
        archive: &'a [u8],
        display_width: u32,
    ) -> Result<Self, String> {
        let payload = |path: &str| -> Result<&'a [u8], String> {
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing panel resource {path}"))?;
            let end = entry
                .offset
                .checked_add(entry.length)
                .ok_or("panel range overflow")?;
            if end > archive.len() as u64 {
                return Err("panel resource outside archive".into());
            }
            Ok(&archive[entry.offset as usize..end as usize])
        };
        let path_at = |address| -> Result<&str, String> {
            let bytes = image.at(address, 260)?;
            let end = bytes
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated panel path")?;
            core::str::from_utf8(&bytes[..end]).map_err(|_| "invalid panel path".into())
        };
        let font = match display_width {
            640 | 1280 => 0x547430,
            800 | 1600 => 0x547450,
            _ => 0x547470,
        };
        // Mode-1 font preloads do not register textures. The chosen font,
        // cursors and billboards register every record with dedicated flag0.
        let mut textures = TextureSet::parse(payload(path_at(font)?)?)
            .map_err(|e| e.to_string())?
            .textures;
        textures.extend(
            TextureSet::parse(payload(path_at(0x547410)?)?)
                .map_err(|e| e.to_string())?
                .textures,
        );
        let start = textures.len();
        textures.extend(
            TextureSet::parse(payload(path_at(0x5473e4)?)?)
                .map_err(|e| e.to_string())?
                .textures,
        );
        let billboards = start..textures.len();

        // Ordered path operands of the46 original004fff00 calls. Repeated
        // meshes are intentional. Parse their actual name tables so cache
        // registration follows original inputs rather than a flattened list.
        let paths = [
            0x5473b8, 0x547398, 0x54736c, 0x547340, 0x547314, 0x5472e0, 0x5472b0, 0x547280,
            0x547244, 0x547210, 0x5471e8, 0x5471b8, 0x547314, 0x54718c, 0x54715c, 0x547130,
            0x547108, 0x5470d0, 0x54709c, 0x54706c, 0x547108, 0x547040, 0x547014, 0x546fe4,
            0x546fb4, 0x546f84, 0x546f58, 0x546f2c, 0x546f00, 0x546ed4, 0x546ea8, 0x546ed4,
            0x546e7c, 0x546e50, 0x546e20, 0x546df4, 0x546ed4, 0x547108, 0x546f84, 0x546dc4,
            0x546d94, 0x546d60, 0x546d30, 0x546cfc, 0x546ccc, 0x546e50,
        ];
        let mut keys: Vec<&str> = Vec::new();
        for address in paths {
            let mesh = CachedMesh::parse(payload(path_at(address)?)?)?;
            for name in mesh.texture_names {
                //004ffad0 strips through the final backslash for non-GTD
                //textures;004fe510 compares only the query length, ASCII-folded.
                //Every original panel reference here takes the TT branch.
                if name.to_ascii_lowercase().contains(".gtd") {
                    return Err("panel GTD cache references are not implemented".into());
                }
                let key = name.rsplit('\\').next().unwrap();
                if keys.iter().any(|seen| {
                    seen.as_bytes()
                        .get(..key.len())
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key.as_bytes()))
                }) {
                    continue;
                }
                let mut set = TextureSet::parse(payload(key)?).map_err(|e| e.to_string())?;
                if set.textures.len() != 1 || set.textures[0].header[0] & 2 != 0 {
                    return Err("panel texture needs unsupported dedicated registration".into());
                }
                textures.push(set.textures.remove(0));
                keys.push(key);
            }
        }
        Ok(Self {
            textures,
            billboards,
            cache_keys: keys.into_iter().map(str::to_owned).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original panel atlas observations, archive and v1.4 executable"]
    fn matches_native_panel_atlas() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let panels = FontPanels::from_v14(
            &PeImage::parse(&executable).unwrap(),
            &index,
            &archive,
            1280,
        )
        .unwrap();
        let bytes = read("OPENMT2001_NATIVE_PANEL_ATLAS");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTPANEL2");
        assert_eq!((word(8), word(16)), (45, 0x027f));
        assert_eq!(panels.textures.len(), 45);
        assert_eq!(panels.billboards, 20..29);
        let inputs: Vec<_> = panels
            .textures
            .iter()
            .map(|t| crate::atlas::Input {
                width: t.width,
                height: t.height,
                group: 0,
                padding: 1,
            })
            .collect();
        let mut layout = crate::atlas::Layout::default();
        let regions = layout.pack(&inputs).unwrap();
        assert_eq!(layout.page_count(), word(12) as usize);
        let mut pages = vec![vec![0; 256 * 256 * 4]; layout.page_count()];
        let mut page_flags = vec![0; layout.page_count()];
        let mut at = 20;
        let mut normalized = Vec::new();
        let mut expected_uv = Vec::new();
        for (i, raw) in panels.textures.iter().enumerate() {
            let flags = crate::texture::loader_flags(raw);
            assert_eq!(
                (flags, raw.width, raw.height),
                (word(at), word(at + 4), word(at + 8)),
                "source metadata {i}"
            );
            assert_eq!(
                regions[i],
                crate::atlas::Region {
                    page: word(at + 12) as usize,
                    origin: [word(at + 16), word(at + 20)]
                },
                "region {i}"
            );
            expected_uv.push(std::array::from_fn::<_, 4, _>(|j| word(at + 24 + j * 4)));
            at += 40;
            let pixels = crate::texture::loader_pixels(raw);
            assert_eq!(pixels, &bytes[at..at + pixels.len()], "loaded pixels {i}");
            at += pixels.len();
            normalized.push(pixels);
        }
        for i in crate::atlas::Layout::packing_order(&inputs) {
            let mut record = panels.textures[i].clone();
            record.header[0] = crate::texture::loader_flags(&record);
            record.texels = &normalized[i];
            let region = regions[i];
            page_flags[region.page] |= record.header[0] & 0xd000;
            let uv = crate::texture::write_atlas_region(
                &record,
                &mut pages[region.page],
                256,
                region.origin,
                1,
            )
            .unwrap();
            assert_eq!(uv.map(f32::to_bits), expected_uv[i], "normalized UV {i}");
        }
        for (i, page) in pages.iter().enumerate() {
            assert_eq!(
                (page_flags[i], 0),
                (word(at), word(at + 4)),
                "page flags/group {i}"
            );
            let levels = crate::texture::page_mipmaps(256, 256, page).unwrap();
            assert_eq!(levels.len() + 1, word(at + 8) as usize);
            at += 12;
            for (level, pixels) in std::iter::once(page).chain(&levels).enumerate() {
                assert_eq!(
                    pixels
                        .iter()
                        .zip(&bytes[at..at + pixels.len()])
                        .position(|(a, b)| a != b),
                    None,
                    "page {i} mip {level}"
                );
                at += pixels.len();
            }
        }
        assert_eq!(at, bytes.len());
    }
}
