//! Process-retained native4c1380 text resources, not a panel/World owner.
//! No native addresses are manufactured from modern allocations. Page ordinals
//! identify records in the selected archive collection, not native handles.
use super::case_zero_text::{
    ErrorWord, FontData, Fonts, Group, Literals, ScratchByte, SelectorState,
};
use openmt2001::{
    archive::ArchiveIndex,
    font::Font,
    pe::PeImage,
    texture::{TextureRecord, TextureSet},
};
use std::{
    collections::BTreeMap,
    fs,
    ops::Range,
    path::{Path, PathBuf},
};

// The packed language table parser is library code (#99).
pub use openmt2001::packed_language::PackedLanguage;

#[derive(Debug)]
pub struct TableSource {
    pub slot: u32,
    pub pe_stem: u32,
    pub native_stem: Vec<u8>,
    pub path: PathBuf,
    pub count: u32,
    pub owned: bool,
}
#[derive(Debug)]
pub struct PageSource {
    pub slot: u32,
    pub page: usize,
    pub ordinal: usize,
    pub record: Range<usize>,
}
#[derive(Debug)]
pub struct FontPages {
    pub name: String,
    pub archive_range: Range<usize>,
    bytes: Vec<u8>,
    pub records: Vec<PageSource>,
}
impl FontPages {
    fn parse(name: String, archive_range: Range<usize>, bytes: Vec<u8>) -> Result<Self, String> {
        let parsed = TextureSet::parse(&bytes).map_err(|e| format!("{name}: {e}"))?;
        if parsed.textures.len() < 12 {
            return Err(format!("{name}: twelve selected font pages required"));
        }
        let mut records = Vec::new();
        let mut at = 4usize;
        for (ordinal, record) in parsed.textures.iter().enumerate() {
            let end = at
                .checked_add(56)
                .and_then(|n| n.checked_add(record.texels.len()))
                .ok_or("font page range overflow")?;
            if ordinal < 12 {
                records.push(PageSource {
                    slot: (ordinal / 2) as u32,
                    page: ordinal % 2,
                    ordinal,
                    record: at..end,
                });
            }
            at = end;
        }
        Ok(Self {
            name,
            archive_range,
            bytes,
            records,
        })
    }
    pub fn page(&self, slot: u32, page: usize) -> Result<TextureRecord<'_>, String> {
        let selected = self
            .records
            .iter()
            .find(|r| r.slot == slot && r.page == page)
            .ok_or("selected font page absent")?;
        TextureSet::parse(&self.bytes)
            .map_err(|e| e.to_string())?
            .textures
            .get(selected.ordinal)
            .cloned()
            .ok_or_else(|| "selected font record absent".into())
    }
}
/// Move-only process owner. Internal snapshots are transaction-local only.
/// No Clone implementation and no move-out manager API: panels borrow it.
#[derive(Debug)]
pub struct UiTextResources {
    pub(super) construction_identity: super::main_construction::ResourceIdentity,
    manager: Box<Fonts>,
    pub literals: Literals,
    pub tables: Vec<TableSource>,
    pub pages: FontPages,
    pub language_stem: PathBuf,
    pub native_language_stem: Vec<u8>,
    pub board_stem: Option<PathBuf>,
}
impl UiTextResources {
    pub fn fonts(&self) -> &Fonts {
        &self.manager
    }
    pub fn table(&self, slot: u32) -> Result<&[u8], String> {
        self.manager
            .fonts
            .get(&slot)
            .map(|f| f.table.as_slice())
            .ok_or_else(|| format!("shared font{slot} table absent"))
    }
    pub fn surfaces(&self, slot: u32) -> Result<Vec<crate::ui::Surface>, String> {
        (0..2)
            .map(|page| {
                self.pages
                    .page(slot, page)
                    .map(|record| crate::ui::Surface::new(&record))
            })
            .collect()
    }
    pub fn fonts_mut(&mut self) -> &mut Fonts {
        &mut self.manager
    }
    pub fn load(
        executable: &[u8],
        installer: &Path,
        language: &Path,
        index: &ArchiveIndex,
        archive: &[u8],
    ) -> Result<Self, String> {
        let pe = PeImage::parse(executable)?;
        //640 is the admitted frontend coordinate domain; other resolution
        //selection requires the actual display-width producer, not a fallback.
        let name = ntbs(&pe, 0x547430, 64)?;
        let name = String::from_utf8(name).map_err(|_| "font collection path is not ASCII")?;
        let entry = index
            .find(&name)
            .ok_or_else(|| format!("missing shared font collection {name}"))?;
        let first = usize::try_from(entry.offset).map_err(|_| "font archive offset")?;
        let last = usize::try_from(
            entry
                .offset
                .checked_add(entry.length)
                .ok_or("font archive range overflow")?,
        )
        .map_err(|_| "font archive end")?;
        let bytes = archive
            .get(first..last)
            .ok_or("shared font collection archive range")?
            .to_vec();
        let pages = FontPages::parse(name, first..last, bytes)?;
        let selected = language
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("selected language directory absent/non-UTF8")?;
        let native_language_stem = expand_one(&ntbs(&pe, 0x54cb20, 64)?, selected.as_bytes())?;
        let language_stem = language.join("tycoon");
        let packed = PackedLanguage::load(&language_stem)?;
        let mut scratch = vec![ScratchByte::Unwritten; 1024];
        write_filename(&mut scratch, &native_language_stem, b".off")?;
        write_filename(&mut scratch, &native_language_stem, b".bin")?;
        let mut fonts = BTreeMap::new();
        let mut tables = Vec::new();
        for (slot, pe_stem) in [0x54cb08, 0x54cae4, 0x54cac4, 0x54caa0, 0x54ca84, 0x54ca60]
            .into_iter()
            .enumerate()
        {
            let native_stem = ntbs(&pe, pe_stem, 128)?;
            let filename = native_stem
                .rsplit(|&b| b == b'\\')
                .next()
                .ok_or("font stem absent")?;
            let filename = format!(
                "{}.tab",
                std::str::from_utf8(filename).map_err(|_| "font filename not ASCII")?
            );
            let path = case_file(&installer.join("Standard/fonts"), &filename)?;
            let table =
                fs::read(&path).map_err(|e| format!("font{slot} {}: {e}", path.display()))?;
            Font::parse(&table).map_err(|e| format!("font{slot} {}: {e}", path.display()))?;
            let count = u32::from_le_bytes(table[..4].try_into().unwrap());
            tables.push(TableSource {
                slot: slot as u32,
                pe_stem,
                native_stem,
                path,
                count,
                owned: true,
            });
            fonts.insert(
                slot as u32,
                FontData {
                    table,
                    baseline: 0,
                    scale: 1.,
                },
            );
        }
        //Static selector cache/counters are actual PE process-image products.
        //They are not fields of the newly allocated unknown manager heap.
        let image = super::process_image::Cells::process_allocation(executable)?;
        let selector = SelectorState {
            cached: image.word(0x6aad08)?,
            pending: [image.word(0x6aad00)?, image.word(0x6aad0c)?],
            notify: 0,
        };
        let manager = Fonts {
            fonts,
            language: packed,
            current: 0,
            gap: -1,
            line_mode: 1,
            scratch,
            formatted: empty_group(),
            temporary: empty_group(),
            error: ErrorWord::Unknown,
            selector,
            context: u32::MAX,
            glyph_callback: 0x4554e0,
            ready_callback: 0,
            four_callback: 0,
            pair: [2243, 2244],
            tail: Some(2245),
        };
        Ok(Self {
            construction_identity: super::main_construction::ResourceIdentity::new(),
            manager: Box::new(manager),
            literals: Literals::from_pe(&pe)?,
            tables,
            pages,
            language_stem,
            native_language_stem,
            board_stem: None,
        })
    }
    ///4922e0 replaces the selected board suffix atomically in the process owner.
    pub fn install_usa_board(&mut self, executable: &[u8], installer: &Path) -> Result<(), String> {
        self.install_board(
            executable,
            installer,
            crate::board_selection::Name::default(),
        )
    }
    pub fn install_board(
        &mut self,
        executable: &[u8],
        installer: &Path,
        name: crate::board_selection::Name,
    ) -> Result<(), String> {
        let path = name.directory(installer).join("tycoon");
        if self.board_stem.as_ref() == Some(&path) {
            return Ok(());
        }
        let board = PackedLanguage::load(&path)?;
        let native = expand_one(
            &ntbs(&PeImage::parse(executable)?, 0x542324, 128)?,
            name.as_str().as_bytes(),
        )?;
        let mut language = self.manager.language.clone();
        language.overlay(2242, &board)?;
        let mut scratch = self.manager.scratch.clone();
        write_filename(&mut scratch, &native, b".bin")?;
        write_filename(&mut scratch, &native, b".off")?;
        self.manager.language = language;
        self.manager.scratch = scratch;
        self.board_stem = Some(path);
        Ok(())
    }
}
pub(super) fn empty_group() -> Group {
    Group {
        bytes: Vec::new(),
        offsets: Vec::new(),
        count: 0,
    }
}
pub(super) fn write_filename(
    scratch: &mut [ScratchByte],
    stem: &[u8],
    extension: &[u8],
) -> Result<(), String> {
    let n = stem
        .len()
        .checked_add(extension.len())
        .and_then(|n| n.checked_add(1))
        .ok_or("manager filename overflow")?;
    if n > scratch.len() {
        return Err("manager filename exceeds native1024 scratch".into());
    }
    for (i, byte) in stem
        .iter()
        .chain(extension)
        .copied()
        .chain(std::iter::once(0))
        .enumerate()
    {
        scratch[i] = ScratchByte::Byte(byte);
    }
    Ok(())
}
fn ntbs(pe: &PeImage<'_>, address: u32, max: usize) -> Result<Vec<u8>, String> {
    let b = pe.at(address, max)?;
    let n = b
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("PE text resource{address:x} unterminated"))?;
    Ok(b[..n].to_vec())
}
fn expand_one(format: &[u8], value: &[u8]) -> Result<Vec<u8>, String> {
    let at = format
        .windows(2)
        .position(|b| b == b"%s")
        .ok_or("native language path lacks selected-language placeholder")?;
    if format[..at].contains(&b'%')
        || format[at + 2..].contains(&b'%')
        || value.iter().any(|&b| matches!(b, b'/' | b'\\' | 0 | b'%'))
    {
        return Err("unsupported native language path format/selection".into());
    }
    Ok([&format[..at], value, &format[at + 2..]].concat())
}
fn case_file(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let mut matches = fs::read_dir(directory)
        .map_err(|e| format!("{}: {e}", directory.display()))?
        .map(|entry| entry.map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
    let path = matches
        .next()
        .ok_or_else(|| {
            format!(
                "required font table {name} absent in {}",
                directory.display()
            )
        })?
        .path();
    if matches.next().is_some() {
        return Err(format!(
            "ambiguous case-insensitive original font filename {name}"
        ));
    }
    Ok(path)
}
#[cfg(test)]
#[path = "ui_text_resource_tests.rs"]
mod tests;
