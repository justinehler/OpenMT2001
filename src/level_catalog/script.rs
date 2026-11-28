//! Buffer-only original GAMESTART registration callbacks. Filesystem discovery
//! and checksums are supplied by the host; scripts receive no host I/O callback.
use super::{Catalog, Labels, Registration};
use crate::{
    economy::ftol_low_word,
    lua4::{Runtime, Value},
    pe::PeImage,
};

pub fn register_language(runtime: &mut Runtime, image: &PeImage<'_>) -> Result<(), String> {
    // These97 constant registrations are contiguous original4dc6f5..4dce89.
    for i in 0..97 {
        let address = 0x4dc6f5 + i * 20;
        let b = image.at(address, 20)?;
        if b[0] != 0x68
            || b[5] != 0x68
            || b[10] != 0xb9
            || b[15] != 0xe8
            || address
                .wrapping_add(20)
                .wrapping_add(u32::from_le_bytes(b[16..20].try_into().unwrap()))
                != 0x4d64a0
        {
            return Err("original level language registration shape changed".into());
        }
        let pointer = u32::from_le_bytes(b[1..5].try_into().unwrap());
        let name = image.at(pointer, 128)?;
        let name = &name[..name
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated level language constant")?];
        runtime.set_number(
            std::str::from_utf8(name).map_err(|e| e.to_string())?,
            f64::from(f32::from_le_bytes(b[6..10].try_into().unwrap())),
        )?;
    }
    Ok(())
}

pub fn load(
    image: &PeImage<'_>,
    source: &[u8],
    development: bool,
    directory: impl FnMut(&[u8]) -> Result<Option<i32>, String>,
) -> Result<Catalog, String> {
    load_sources(
        image,
        &[("GAMESTART/hub.lua", source)],
        development,
        directory,
    )
}

/// Execute buffers in caller order in one runtime and catalog. The host owns
/// discovery/containment; Lua receives only the existing registration callbacks.
pub fn load_sources(
    image: &PeImage<'_>,
    sources: &[(&str, &[u8])],
    development: bool,
    directory: impl FnMut(&[u8]) -> Result<Option<i32>, String>,
) -> Result<Catalog, String> {
    let mut runtime = Runtime::new()?;
    register_language(&mut runtime, image)?;
    register_sources(&mut runtime, sources, development, directory)
}

fn register_sources(
    runtime: &mut Runtime,
    sources: &[(&str, &[u8])],
    development: bool,
    mut directory: impl FnMut(&[u8]) -> Result<Option<i32>, String>,
) -> Result<Catalog, String> {
    runtime.register_coerced("openmt_level_literal", 0, "nsss")?;
    runtime.register_coerced("openmt_level_localized", 1, "nsnn")?;
    runtime.execute("level registration table",b"Frontend={RegisterLevel=openmt_level_literal, RegisterLevel_Id=openmt_level_localized}; openmt_level_literal=nil; openmt_level_localized=nil",&mut|_,_|Err("unexpected level binding callback".into()))?;
    let mut catalog = Catalog::default();
    let mut registrations = 0usize;
    for &(name, source) in sources {
        runtime
            .execute(name, source, &mut |id, args| {
                registrations += 1;
                if registrations > 16_384 {
                    return Err("level registration callback limit exceeded".into());
                }
                let number = |i: usize| -> Result<u32, String> {
                    match args.get(i) {
                        Some(Value::Number(n)) => Ok(ftol_low_word(*n) as u32),
                        _ => Err("level registration numeric coercion missing".into()),
                    }
                };
                let text = |i: usize| -> Result<Vec<u8>, String> {
                    match args.get(i) {
                        Some(Value::Bytes(s)) => Ok(s.clone()),
                        _ => Err("level registration string conversion failed".into()),
                    }
                };
                let level_id = number(0)? as i32;
                if !development && level_id / 100000 >= 7 {
                    return Ok(None);
                }
                let token = text(1)?;
                let Some(checksum) = directory(&token)? else {
                    return Ok(None);
                };
                let labels = match id {
                    0 => Labels::Literal {
                        title: text(2)?,
                        description: text(3)?,
                    },
                    1 => Labels::Localized {
                        title: number(2)?,
                        description: number(3)?,
                    },
                    _ => return Err("unknown level registration callback".into()),
                };
                catalog.register(
                    Registration {
                        id: level_id,
                        directory: token,
                        labels,
                        checksum,
                    },
                    development,
                    true,
                )?;
                Ok(None)
            })
            .map_err(|error| format!("{name}: {error}"))?;
    }
    Ok(catalog)
}

/// Read original SetLevelInfo records as data. Other profile settings are not
/// executed or imported by this narrowly scoped progression reader.
pub fn apply_profile(catalog: &mut Catalog, source: &str) -> Result<(), String> {
    for line in source.lines() {
        let line = line.split("--").next().unwrap_or("").trim();
        let Some(tail) = line.strip_prefix("SetLevelInfo") else {
            continue;
        };
        let values = tail
            .trim()
            .strip_prefix('(')
            .and_then(|s| s.trim_end_matches(';').trim_end().strip_suffix(')'))
            .ok_or("malformed profile level record")?;
        let values: Vec<_> = values.split(',').map(str::trim).collect();
        if values.len() != 6 {
            return Err("profile level record requires six arguments".into());
        }
        let number = |i: usize| -> Result<i32, String> {
            values[i]
                .parse::<f64>()
                .map(ftol_low_word)
                .map_err(|_| "profile level record is not numeric data".into())
        };
        catalog.set_level_info(
            number(0)? as u32,
            number(2)? as u32,
            [number(3)?, number(4)?, number(5)?],
        );
    }
    Ok(())
}
pub fn profile_text(catalog: &Catalog) -> String {
    let mut out = String::new();
    for (id, attempts, finished) in catalog.profile_records() {
        out.push_str(&format!(
            "SetLevelInfo({}, DEPRECATED, {}, {}, {}, {})\n",
            id as i32,
            attempts as i32,
            u8::from(finished[0]),
            u8::from(finished[1]),
            u8::from(finished[2])
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_buffers_share_globals_callbacks_and_catalog_order() {
        let mut runtime = Runtime::new().unwrap();
        let sources: &[(&str, &[u8])] = &[
            ("GAMESTART", b"custom_id=500000; Frontend.RegisterLevel(200001,'base','Base','Description')"),
            ("CUSTOM/first", b"Frontend.RegisterLevel(custom_id,'custom/first','Literal','Details'); Frontend.RegisterLevel_Id(500001,'custom/second',12,34)"),
            ("CUSTOM/last", b"Frontend.RegisterLevel(custom_id,'duplicate','Duplicate','D'); Frontend.RegisterLevel(700000,'dev','Dev','D'); Frontend.RegisterLevel(500002,'missing','Missing','D')"),
        ];
        let mut queried = Vec::new();
        let catalog = register_sources(&mut runtime, sources, false, |name| {
            queried.push(name.to_vec());
            Ok(if name == b"missing" { None } else { Some(123) })
        })
        .unwrap();
        assert_eq!(
            catalog.levels.iter().map(|l| l.id()).collect::<Vec<_>>(),
            [200001, 500000, 500001, 500000]
        );
        assert_eq!(catalog.find(500000), Some(1));
        assert!(!queried.iter().any(|name| name == b"dev"));
        assert_eq!(catalog.levels[2].directory(), b"custom/second");
        assert!(
            matches!(catalog.levels[1].labels(), Labels::Literal { title, description }
            if title == b"Literal" && description == b"Details")
        );
        assert!(matches!(
            catalog.levels[2].labels(),
            Labels::Localized {
                title: 12,
                description: 34
            }
        ));
        let empty = register_sources(&mut Runtime::new().unwrap(), &[], false, |_| {
            panic!("no callback expected")
        })
        .unwrap();
        assert!(empty.levels.is_empty());
    }
    #[test]
    #[ignore = "requires original executable and GAMESTART registration source"]
    fn original_registration_and_profile_data() {
        let executable = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let root = crate::fixtures::path("ORIGINAL_SCRIPTS");
        let source = std::fs::read(root.join("GAMESTART/hub.lua")).unwrap();
        let mut catalog = load(&image, &source, false, |_| Ok(Some(0))).unwrap();
        assert_eq!(catalog.levels.len(), 36);
        assert_eq!(catalog.levels.iter().filter(|l| l.tutorial()).count(), 13);
        let campaign: Vec<_> = catalog
            .levels
            .iter()
            .filter(|l| l.id() / 100000 == 2)
            .map(|l| l.id())
            .collect();
        assert_eq!(
            campaign,
            [
                200003, 200004, 200005, 200002, 200012, 200001, 200006, 200011, 200007, 200008,
                200009, 200010, 200013, 200014, 200015, 200016
            ]
        );
        catalog.reset_progress();
        catalog.unlock_first();
        assert_eq!(
            catalog
                .levels
                .iter()
                .filter(|l| l.unlocked())
                .map(|l| l.id())
                .collect::<Vec<_>>(),
            [200003]
        );
        // The original profile data form is parsed without evaluating any Lua.
        apply_profile(&mut catalog,"g_ProfileOptions = {}\nSetLevelInfo(200003, DEPRECATED, 7, 0, 1, -2)\nSetLevelInfo(400001, nil, 2, 0, 0, 0)\n").unwrap();
        let first = &catalog.levels[catalog.find(200003).unwrap()];
        assert_eq!(first.attempts(), 7);
        assert_eq!(first.completed(), [false, true, true]);
        assert!(catalog.levels[catalog.find(400001).unwrap()].unlocked());
        let serialized = profile_text(&catalog);
        catalog.reset_progress();
        apply_profile(&mut catalog, &serialized).unwrap();
        assert_eq!(catalog.levels[catalog.find(200003).unwrap()].attempts(), 7);
        assert!(catalog.levels[catalog.find(400001).unwrap()].unlocked());
        // The supplied v1.4 payload adds Sandbox and the seventh network map.
        let executable_path = crate::fixtures::path("EXE");
        let patched = std::fs::read(
            executable_path
                .parent()
                .unwrap()
                .join("scripts/GAMESTART/hub.lua"),
        )
        .unwrap();
        let patched = load(&image, &patched, false, |_| Ok(Some(0))).unwrap();
        assert_eq!(patched.levels.len(), 38);
        assert_eq!(
            patched.levels[patched.find(400002).unwrap()].directory(),
            b"sandbox"
        );
        assert_eq!(
            patched.levels[patched.find(300006).unwrap()].directory(),
            b"net6"
        );
    }
}
