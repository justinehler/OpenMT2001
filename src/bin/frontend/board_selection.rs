//! Original board catalog (4926b0), language overlay (4922e0) and profile choice.
use openmt2001::packed_language::PackedLanguage;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name([u8; 128]);
impl Default for Name {
    fn default() -> Self {
        let mut name = [0; 128];
        name[..3].copy_from_slice(b"usa");
        Self(name)
    }
}
impl Name {
    ///4c2880 uses the installed DefaultBoard, or USA when absent/empty.
    pub fn initial() -> Self {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Registry::{
                HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6432KEY, RRF_ZEROONFAILURE,
                RegGetValueA,
            };
            let mut value = [0u8; 128];
            let mut length = value.len() as u32;
            // Read-only, bounded REG_SZ query; all pointers live through the call.
            let result = unsafe {
                RegGetValueA(
                    HKEY_LOCAL_MACHINE,
                    c"Software\\Infogrames Interactive\\Monopoly Tycoon"
                        .as_ptr()
                        .cast(),
                    c"DefaultBoard".as_ptr().cast(),
                    RRF_RT_REG_SZ | RRF_ZEROONFAILURE | RRF_SUBKEY_WOW6432KEY,
                    std::ptr::null_mut(),
                    value.as_mut_ptr().cast(),
                    &mut length,
                )
            };
            if result == 0 {
                let end = value.iter().position(|b| *b == 0).unwrap_or(value.len());
                if let Ok(name) = Self::parse(&value[..end]) {
                    return name;
                }
            }
        }
        Self::default()
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty()
            || bytes.len() >= 128
            || !bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            return Err("invalid board folder name".into());
        }
        let mut name = [0; 128];
        name[..bytes.len()].copy_from_slice(bytes);
        Ok(Self(name))
    }
    pub fn as_str(&self) -> &str {
        // Only ASCII folder identifiers enter this value.
        std::str::from_utf8(self.bytes()).unwrap_or("")
    }
    fn bytes(&self) -> &[u8] {
        &self.0[..self.0.iter().position(|b| *b == 0).unwrap_or(128)]
    }
    pub fn directory(&self, installer: &Path) -> PathBuf {
        installer.join("Standard/fonts/board").join(self.as_str())
    }
    pub fn language(&self, installer: &Path) -> Result<PackedLanguage, String> {
        PackedLanguage::load(&self.directory(installer).join("tycoon"))
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path.with_extension("board")) {
            Ok(bytes) => Self::parse(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::initial()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, options: &Path) -> Result<(), String> {
        let path = options.with_extension("board");
        fs::create_dir_all(path.parent().ok_or("board profile directory missing")?)
            .map_err(|e| e.to_string())?;
        let temporary = path.with_extension("board.tmp");
        fs::write(&temporary, self.bytes()).map_err(|e| e.to_string())?;
        fs::rename(temporary, path).map_err(|e| e.to_string())
    }
}

pub struct Entry {
    pub name: Name,
    pub label: Vec<u8>,
}

///4926b0 scans board subdirectories, ignoring dot/underscore directories, and
/// obtains each label from its tycoon row1 (installed at2242). Windows enumerates
/// these folders by name; retain that order on filesystems with unsorted readdir.
pub fn catalog(installer: &Path) -> Result<Vec<Entry>, String> {
    let root = installer.join("Standard/fonts/board");
    let mut directories = fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    directories.sort_by_key(|entry| entry.file_name().to_ascii_lowercase());
    let mut entries = Vec::new();
    for directory in directories {
        if !directory.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let folder = directory.file_name();
        let Some(folder) = folder.to_str() else {
            continue;
        };
        if folder.starts_with(['.', '_']) || !directory.path().join("tycoon.bin").is_file() {
            continue;
        }
        let name = Name::parse(folder.as_bytes())?;
        let packed = name.language(installer)?;
        entries.push(Entry {
            name,
            label: packed.row(1)?.to_vec(),
        });
        if entries.len() == 100 {
            break;
        }
    }
    Ok(entries)
}

pub fn overlay(strings: &mut [Vec<u8>], board: &PackedLanguage) -> Result<(), String> {
    let rows = board.rows();
    let destination = strings
        .get_mut(2242..)
        .ok_or("board language destination missing")?;
    if destination.len() != rows.len().saturating_sub(1) {
        return Err("board language suffix length differs".into());
    }
    for (slot, row) in destination.iter_mut().zip(rows.into_iter().skip(1)) {
        *slot = row.to_vec();
    }
    Ok(())
}
