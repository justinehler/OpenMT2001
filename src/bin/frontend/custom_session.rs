//! Supplied custom content stays in the ordinary authored single-player VM.
//! Files are buffered before execution; Lua receives no filesystem callback.
use openmt2001::{level_catalog::Level, lua4::Runtime};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub fn valid_level(level: &Level) -> bool {
    level.bytes[2] == 1
        && level.single_player()
        && !level.tutorial()
        && level.id() / 100000 == 5
        && std::str::from_utf8(level.directory()).is_ok_and(|name| {
            let name = name.replace('\\', "/");
            let parts = Path::new(&name).components().collect::<Vec<_>>();
            parts.len() >= 3
                && parts.iter().all(|p| matches!(p, Component::Normal(_)))
                && parts
                    .first()
                    .is_some_and(|p| p.as_os_str().eq_ignore_ascii_case("custom"))
        })
}

pub fn same_directory(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(&a, &b)| {
            let normalize = |c: u8| {
                if c == b'\\' {
                    b'/'
                } else {
                    c.to_ascii_lowercase()
                }
            };
            normalize(a) == normalize(b)
        })
}

fn contained(root: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let Some(path) = crate::campaign_store::original_path(root, name)? else {
        return Ok(None);
    };
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("custom script leaves the supplied scripts directory".into());
    }
    Ok(Some(path))
}

pub fn read_optional(root: &Path, relative: &str) -> Result<Option<Vec<u8>>, String> {
    contained(root, relative)?
        .map(|p| fs::read(&p).map_err(|e| format!("{}: {e}", p.display())))
        .transpose()
}

pub fn read(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    read_optional(root, relative)?
        .ok_or_else(|| format!("missing supplied custom script {relative}"))
}

/// Stage the selected custom folder and the supplied map library. C_RunScript
/// still resolves only buffers, including calls made by later hourly events.
/// Stable filename order is a modern portability policy, not Windows order.
pub fn register_sources(runtime: &mut Runtime, root: &Path, directory: &str) -> Result<(), String> {
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let root = canonical_root.as_path();
    let selected = contained(root, directory)?.ok_or("custom scenario directory missing")?;
    let maps = contained(root, "MAPS")?.ok_or("supplied map library missing")?;
    let mut total = 0usize;
    for (folder, maximum) in [(selected, 64usize), (maps, 256usize)] {
        let mut entries = fs::read_dir(&folder)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        entries.retain(|p| p.extension().is_some_and(|s| s.eq_ignore_ascii_case("lua")));
        if entries.len() > maximum {
            return Err("custom script bundle exceeds file bound".into());
        }
        entries.sort_by_key(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase()
        });
        for pair in entries.windows(2) {
            if pair[0]
                .file_name()
                .unwrap()
                .eq_ignore_ascii_case(pair[1].file_name().unwrap())
            {
                return Err("ambiguous custom script filename case".into());
            }
        }
        for path in entries {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "custom bundle path outside scripts")?
                .to_str()
                .ok_or("custom script path is not UTF-8")?
                .replace('\\', "/");
            let checked = contained(root, &relative)?.ok_or("custom script disappeared")?;
            if !checked.is_file() {
                return Err("custom script entry is not a file".into());
            }
            let size = fs::metadata(&checked).map_err(|e| e.to_string())?.len();
            if size > 2 * 1024 * 1024 {
                return Err("custom script exceeds source bound".into());
            }
            let bytes = fs::read(&checked).map_err(|e| e.to_string())?;
            total = total
                .checked_add(bytes.len())
                .ok_or("custom bundle size overflow")?;
            if total > 8 * 1024 * 1024 {
                return Err("custom script bundle exceeds byte bound".into());
            }
            runtime.script_buffer(&format!("scripts/{relative}"), &bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "custom_session_tests.rs"]
mod tests;
