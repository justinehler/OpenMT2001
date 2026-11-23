//! Offline original Multiplayer catalog and portable tag18 metadata only.
//! No interpreter, UI, Session or save-container policy. Additive checksums are
//! noncryptographic provenance; external receipts, not this record, pin SHA256.
use std::{
    fs::{self, File, Metadata},
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_MAPS: usize = 256;
pub const MAX_BASENAME: usize = 255;
/// Modern bounded input policy, not a recovered original byte limit.
pub const MAX_SOURCE_LEN: u32 = 1024 * 1024;
const V6_HEADER: usize = 28;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapIdentity {
    LegacyEmpty,
    Named {
        basename: String,
        checksum: u32,
        source_len: u32,
    },
    MaterializedUnknown,
}
#[derive(Debug)]
pub struct ResolvedMap {
    pub identity: MapIdentity,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub level_id: u32,
    pub target: f32,
    pub map: MapIdentity,
}
#[derive(Debug)]
pub struct Catalog {
    names: Vec<String>,
}

fn basename(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > MAX_BASENAME
        || !name.is_ascii()
        || !name.to_ascii_lowercase().ends_with(".lua")
        || name.starts_with('.')
        || name.ends_with([' ', '.'])
        || name.bytes().any(|b| {
            !b.is_ascii_graphic() && b != b' '
                || matches!(
                    b,
                    b'%' | b'/' | b'\\' | b':' | b'<' | b'>' | b'"' | b'|' | b'?' | b'*'
                )
        })
        || name == "intromap.lua"
    {
        return Err("unsafe or excluded Skirmish map basename".into());
    }
    // Avoid Windows device paths even when researching on Unix.
    let stem = name.split('.').next().unwrap().to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
        })
    {
        return Err("reserved map basename".into());
    }
    Ok(())
}
fn metadata(path: &Path, directory: bool) -> Result<Metadata, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.file_type().is_symlink()
        || if directory {
            !meta.is_dir()
        } else {
            !meta.is_file()
        }
    {
        return Err(format!(
            "map input is a symlink or wrong file type: {}",
            path.display()
        ));
    }
    if !directory && meta.len() > u64::from(MAX_SOURCE_LEN) {
        return Err(format!(
            "map source exceeds bounded input limit: {}",
            path.display()
        ));
    }
    Ok(meta)
}
/// Windows logical component lookup, refusing case ambiguity and symlinks.
fn child(parent: &Path, logical: &str, directory: bool) -> Result<PathBuf, String> {
    metadata(parent, true)?;
    let mut found = None;
    for entry in fs::read_dir(parent).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|s| s.eq_ignore_ascii_case(logical))
        {
            if found.is_some() {
                return Err(format!("ambiguous map path component {logical}"));
            }
            found = Some(entry.path());
        }
    }
    let path = found.ok_or_else(|| format!("missing map path component {logical}"))?;
    metadata(&path, directory)?;
    Ok(path)
}
fn maps_root(scripts: &Path) -> Result<PathBuf, String> {
    metadata(scripts, true)?;
    let root = scripts.canonicalize().map_err(|e| e.to_string())?;
    let maps = child(&root, "MAPS", true)?;
    if !maps
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&root)
    {
        return Err("map directory escapes supplied scripts root".into());
    }
    Ok(maps)
}
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len() && a.modified().ok() == b.modified().ok()
    }
}
fn read_launch(maps: &Path, name: &str) -> Result<Vec<u8>, String> {
    let path = child(maps, name, false)?;
    let before = metadata(&path, false)?;
    let maps_canonical = maps.canonicalize().map_err(|e| e.to_string())?;
    if path.canonicalize().map_err(|e| e.to_string())?.parent() != Some(maps_canonical.as_path()) {
        return Err("map launch source escapes MAPS".into());
    }
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    if !opened.is_file() || !same_file(&before, &opened) {
        return Err("map source changed during open".into());
    }
    let mut bytes = Vec::new();
    file.take(u64::from(MAX_SOURCE_LEN) + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let after = metadata(&path, false)?;
    if bytes.len() > MAX_SOURCE_LEN as usize
        || !same_file(&before, &after)
        || before.len() != bytes.len() as u64
        || after.len() != bytes.len() as u64
        || before.modified().ok() != after.modified().ok()
    {
        return Err("map source exceeds limit or changed during read".into());
    }
    Ok(bytes)
}
impl Catalog {
    /// Dedicated flat class only. Portable ASCII case-folded sort + raw tie-break
    /// is a modern deterministic choice, not original FindFirstFile order.
    pub fn discover(scripts: &Path) -> Result<Self, String> {
        let maps = maps_root(scripts)?;
        let picker = child(&maps, "Multiplayer", true)?;
        let mut names = Vec::<String>::new();
        for entry in fs::read_dir(&picker).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "non-UTF8 map directory entry")?;
            // Match the safe portable *.lua class; never recurse into directories.
            if !name.to_ascii_lowercase().ends_with(".lua") {
                continue;
            }
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err("symlink in map catalog".into());
            }
            if meta.is_dir() {
                continue;
            }
            if name == "intromap.lua" {
                continue;
            }
            basename(&name)?;
            metadata(&entry.path(), false)?;
            if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                return Err("case-ambiguous map catalog".into());
            }
            if names.len() == MAX_MAPS {
                return Err("map catalog exceeds256 entries".into());
            }
            // Enumeration identity and launch input are deliberately separate.
            child(&maps, &name, false)?;
            names.push(name);
        }
        names.sort_by(|a, b| {
            a.to_ascii_lowercase()
                .cmp(&b.to_ascii_lowercase())
                .then(a.cmp(b))
        });
        Ok(Self { names })
    }
    pub fn names(&self) -> &[String] {
        &self.names
    }
    pub fn default_index(&self) -> Option<usize> {
        self.names
            .iter()
            .position(|n| n == "empty.lua")
            .or_else(|| (!self.names.is_empty()).then_some(0))
    }
    /// Recheck exact catalog membership, confinement and separate launch input.
    /// Read once; caller executes THESE owned bytes, never reopens a UI promise.
    pub fn resolve(&self, scripts: &Path, name: &str) -> Result<ResolvedMap, String> {
        basename(name)?;
        if !self.names.iter().any(|n| n == name) {
            return Err("map is not in selected catalog".into());
        }
        let maps = maps_root(scripts)?;
        let picker = child(&maps, "Multiplayer", true)?;
        child(&picker, name, false)?;
        let bytes = read_launch(&maps, name)?;
        let identity = if name == "empty.lua" && bytes.is_empty() {
            MapIdentity::LegacyEmpty
        } else {
            MapIdentity::Named {
                basename: name.into(),
                checksum: bytes
                    .iter()
                    .fold(0u32, |sum, b| sum.wrapping_add(u32::from(*b))),
                source_len: bytes.len() as u32,
            }
        };
        Ok(ResolvedMap { identity, bytes })
    }
}
impl Selection {
    fn validate(&self) -> Result<(), String> {
        if !(300000..=300006).contains(&self.level_id) || !self.target.is_finite() {
            return Err("invalid offline NET selection".into());
        }
        if let MapIdentity::Named {
            basename: name,
            source_len,
            ..
        } = &self.map
        {
            basename(name)?;
            if *source_len > MAX_SOURCE_LEN {
                return Err("map provenance source length exceeds limit".into());
            }
        }
        Ok(())
    }
    /// v5: exactly[level,targetbits,0,1]. v6:28-byte LE header
    /// [level,targetbits,kind,1,source_len,checksum,basename_len] + raw basename.
    /// kind1 Named; kind2 MaterializedUnknown (all tail words zero).
    pub fn encode(&self, version: u32) -> Result<Vec<u8>, String> {
        self.validate()?;
        let (kind, len, sum, name) = match (&self.map, version) {
            (MapIdentity::LegacyEmpty, 5) => (0, 0, 0, ""),
            (
                MapIdentity::Named {
                    basename,
                    checksum,
                    source_len,
                },
                6,
            ) => (1, *source_len, *checksum, basename.as_str()),
            (MapIdentity::MaterializedUnknown, 6) => (2, 0, 0, ""),
            _ => {
                return Err(
                    "map identity cannot be represented in requested metadata version".into(),
                );
            }
        };
        let mut bytes = Vec::new();
        for word in [self.level_id, self.target.to_bits(), kind, 1] {
            bytes.extend(word.to_le_bytes());
        }
        if version == 6 {
            for word in [len, sum, name.len() as u32] {
                bytes.extend(word.to_le_bytes());
            }
            bytes.extend(name.as_bytes());
        }
        Ok(bytes)
    }
    pub fn decode(version: u32, bytes: &[u8]) -> Result<Self, String> {
        if !matches!(version, 5 | 6)
            || (version == 5 && bytes.len() != 16)
            || (version == 6 && !(V6_HEADER..=V6_HEADER + MAX_BASENAME).contains(&bytes.len()))
        {
            return Err("invalid map selection record length/version".into());
        }
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        if word(12) != 1 {
            return Err("map selection is not offline1".into());
        }
        let map = if version == 5 {
            if word(8) != 0 {
                return Err("unsupported legacy map capability".into());
            }
            MapIdentity::LegacyEmpty
        } else {
            let source_len = word(16);
            let checksum = word(20);
            let n = usize::try_from(word(24)).map_err(|_| "map name length outside host range")?;
            if n > MAX_BASENAME || V6_HEADER.checked_add(n) != Some(bytes.len()) {
                return Err("map selection has invalid name length/trailing data".into());
            }
            match word(8) {
                1 if n > 0 => MapIdentity::Named {
                    basename: std::str::from_utf8(&bytes[V6_HEADER..])
                        .map_err(|_| "map basename is not UTF8")?
                        .into(),
                    checksum,
                    source_len,
                },
                2 if n == 0 && source_len == 0 && checksum == 0 => MapIdentity::MaterializedUnknown,
                _ => return Err("unsupported named/materialized map capability".into()),
            }
        };
        let selection = Self {
            level_id: word(0),
            target: f32::from_bits(word(4)),
            map,
        };
        selection.validate()?;
        Ok(selection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omt-skirmish-maps-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir(path.join("Maps")).unwrap();
            fs::create_dir(path.join("Maps/Multiplayer")).unwrap();
            Self(path)
        }
        fn pair(&self, name: &str, bytes: &[u8]) {
            fs::write(self.0.join("Maps/Multiplayer").join(name), bytes).unwrap();
            fs::write(self.0.join("Maps").join(name), bytes).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn dedicated_catalog_default_order_and_actual_fresh_bytes() {
        let f = Fixture::new();
        f.pair("z.lua", b"picker");
        f.pair("A.lua", b"a");
        fs::write(f.0.join("Maps/root-only.lua"), b"not selectable").unwrap();
        fs::create_dir(f.0.join("Maps/Multiplayer/directory.lua")).unwrap();
        fs::write(f.0.join("Maps/Multiplayer/intromap.lua"), b"excluded").unwrap();
        let c = Catalog::discover(&f.0).unwrap();
        assert_eq!(c.names(), ["A.lua", "z.lua"]);
        assert_eq!(c.default_index(), Some(0));
        // Picker payload is not the executed bytes. A launch edit before resolve
        // is read and returned honestly, not claimed to match an old UI checksum.
        fs::write(f.0.join("Maps/z.lua"), [255, 2, 7]).unwrap();
        let resolved = c.resolve(&f.0, "z.lua").unwrap();
        assert_eq!(resolved.bytes, [255, 2, 7]);
        assert_eq!(
            resolved.identity,
            MapIdentity::Named {
                basename: "z.lua".into(),
                checksum: 264,
                source_len: 3
            }
        );
        assert!(c.resolve(&f.0, "root-only.lua").is_err());
        assert!(c.resolve(&f.0, "../z.lua").is_err());
        f.pair("empty.lua", b"");
        let c = Catalog::discover(&f.0).unwrap();
        assert_eq!(c.names()[c.default_index().unwrap()], "empty.lua");
        assert_eq!(
            c.resolve(&f.0, "empty.lua").unwrap().identity,
            MapIdentity::LegacyEmpty
        );
        fs::write(f.0.join("Maps/empty.lua"), b"x").unwrap();
        assert!(matches!(
            c.resolve(&f.0, "empty.lua").unwrap().identity,
            MapIdentity::Named { .. }
        ));
        fs::remove_file(f.0.join("Maps/z.lua")).unwrap();
        assert!(c.resolve(&f.0, "z.lua").is_err());
        assert!(Catalog::discover(&f.0).is_err());
        let empty = Fixture::new();
        let c = Catalog::discover(&empty.0).unwrap();
        assert!(c.names().is_empty());
        assert_eq!(c.default_index(), None);
    }
    #[test]
    fn unsafe_ambiguous_oversized_and_over_cap_refuse() {
        for name in [
            "x%.lua", "../x.lua", "x\\y.lua", "x:y.lua", "x\0.lua", "é.lua", ".lua", "CON.lua",
        ] {
            assert!(basename(name).is_err(), "{name:?}");
        }
        assert!(basename(&format!("{}.lua", "a".repeat(252))).is_err());
        let f = Fixture::new();
        f.pair("a.lua", b"");
        f.pair("A.lua", b"");
        // Case-sensitive filesystems exercise Windows logical ambiguity.
        if fs::read_dir(f.0.join("Maps/Multiplayer")).unwrap().count() == 2 {
            assert!(Catalog::discover(&f.0).is_err());
        }
        let f = Fixture::new();
        f.pair("large.lua", b"");
        File::options()
            .write(true)
            .open(f.0.join("Maps/large.lua"))
            .unwrap()
            .set_len(u64::from(MAX_SOURCE_LEN) + 1)
            .unwrap();
        assert!(Catalog::discover(&f.0).is_err());
        let f = Fixture::new();
        for i in 0..MAX_MAPS {
            f.pair(&format!("m{i:03}.lua"), b"");
        }
        assert_eq!(Catalog::discover(&f.0).unwrap().names().len(), 256);
        f.pair("overflow.lua", b"");
        assert!(Catalog::discover(&f.0).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_picker_launch_and_directory_are_not_inputs() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        f.pair("ok.lua", b"x");
        let c = Catalog::discover(&f.0).unwrap();
        fs::remove_file(f.0.join("Maps/ok.lua")).unwrap();
        symlink(f.0.join("Maps/Multiplayer/ok.lua"), f.0.join("Maps/ok.lua")).unwrap();
        assert!(c.resolve(&f.0, "ok.lua").is_err());
        let f = Fixture::new();
        f.pair("ok.lua", b"x");
        symlink(
            f.0.join("Maps/ok.lua"),
            f.0.join("Maps/Multiplayer/link.lua"),
        )
        .unwrap();
        assert!(Catalog::discover(&f.0).is_err());
        let f = Fixture::new();
        fs::rename(f.0.join("Maps/Multiplayer"), f.0.join("actual")).unwrap();
        symlink(f.0.join("actual"), f.0.join("Maps/Multiplayer")).unwrap();
        assert!(Catalog::discover(&f.0).is_err());
    }
    #[test]
    fn metadata_legacy_golden_named_unknown_and_cold_without_source() {
        // Literal golden v5 NET1,2500.0,map0,offline1; no encoder-built oracle.
        let golden = [
            0xe1, 0x93, 0x04, 0x00, 0x00, 0x40, 0x1c, 0x45, 0, 0, 0, 0, 1, 0, 0, 0,
        ];
        let legacy = Selection::decode(5, &golden).unwrap();
        assert_eq!(legacy.level_id, 300001);
        assert_eq!(legacy.target, 2500.);
        assert_eq!(legacy.map, MapIdentity::LegacyEmpty);
        assert_eq!(legacy.encode(5).unwrap(), golden);
        assert!(legacy.encode(6).is_err());
        let named = Selection {
            level_id: 300001,
            target: 2500.,
            map: MapIdentity::Named {
                basename: "monopolymap.lua".into(),
                checksum: 12345,
                source_len: 222,
            },
        };
        let bytes = named.encode(6).unwrap();
        assert_eq!(bytes.len(), 28 + 15);
        assert_eq!(&bytes[8..16], &[1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&bytes[16..28], &[222, 0, 0, 0, 57, 48, 0, 0, 15, 0, 0, 0]);
        assert_eq!(Selection::decode(6, &bytes).unwrap(), named);
        assert!(named.encode(5).is_err()); // no unknown/nonempty -> legacy map0 alias
        let unknown = Selection {
            map: MapIdentity::MaterializedUnknown,
            ..legacy
        };
        let bytes = unknown.encode(6).unwrap();
        assert_eq!(bytes.len(), 28);
        assert_eq!(&bytes[16..], &[0; 12]);
        assert_eq!(Selection::decode(6, &bytes).unwrap(), unknown);
        assert!(unknown.encode(5).is_err());
        // Codec intentionally does no source resolution; cold restore can retain
        // named provenance without installed files or replaying a development map.
        let directory = Fixture::new();
        drop(directory);
        assert_eq!(
            Selection::decode(6, &named.encode(6).unwrap()).unwrap(),
            named
        );
    }
    #[test]
    fn metadata_rejects_malformed_capability_lengths_targets_and_paths() {
        let good = Selection {
            level_id: 300006,
            target: 10.,
            map: MapIdentity::Named {
                basename: "a.lua".into(),
                checksum: 0,
                source_len: 0,
            },
        }
        .encode(6)
        .unwrap();
        for (at, value) in [
            (0, 299999),
            (4, f32::NAN.to_bits()),
            (8, 0),
            (12, 0),
            (16, MAX_SOURCE_LEN + 1),
            (24, u32::MAX),
        ] {
            let mut b = good.clone();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(Selection::decode(6, &b).is_err());
        }
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(Selection::decode(6, &trailing).is_err());
        for end in 0..good.len() {
            assert!(Selection::decode(6, &good[..end]).is_err());
        }
        let mut path = good.clone();
        path[28] = b'%';
        assert!(Selection::decode(6, &path).is_err());
        let mut unknown = good.clone();
        unknown[8..12].copy_from_slice(&2u32.to_le_bytes());
        assert!(Selection::decode(6, &unknown).is_err());
        assert!(Selection::decode(7, &good).is_err());
        assert!(Selection::decode(5, &good).is_err());
    }
    #[test]
    #[ignore = "requires supplied patched original OPENMT2001_SCRIPTS; no Lua execution"]
    fn supplied_25_catalog_and_monopolymap_owned_launch_bytes() {
        let root = PathBuf::from(std::env::var("OPENMT2001_SCRIPTS").unwrap());
        let catalog = Catalog::discover(&root).unwrap();
        assert_eq!(catalog.names().len(), 25);
        assert_eq!(
            catalog.names()[catalog.default_index().unwrap()],
            "empty.lua"
        );
        assert!(catalog.names().iter().any(|s| s == "jonmap011.lua"));
        assert!(
            !catalog
                .names()
                .iter()
                .any(|s| s == "intromap.lua" || s == "tempmap.lua")
        );
        // Read player-supplied bytes; original Lua payload is never bundled.
        let maps = maps_root(&root).unwrap();
        let expected = fs::read(child(&maps, "monopolymap.lua", false).unwrap()).unwrap();
        assert_eq!(expected.len(), 222);
        let resolved = catalog.resolve(&root, "monopolymap.lua").unwrap();
        assert_eq!(resolved.bytes, expected);
        assert_eq!(
            resolved.identity,
            MapIdentity::Named {
                basename: "monopolymap.lua".into(),
                checksum: expected.iter().map(|b| u32::from(*b)).sum(),
                source_len: 222
            }
        );
        let picker = child(&maps, "Multiplayer", true).unwrap();
        for name in catalog.names() {
            let resolved = catalog.resolve(&root, name).unwrap();
            assert_eq!(
                resolved.bytes,
                fs::read(child(&picker, name, false).unwrap()).unwrap(),
                "{name}"
            );
        }
    }
}
