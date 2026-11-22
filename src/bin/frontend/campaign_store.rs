//! Host I/O for the original level catalog and V2-owned progression records.
//! Private original profiles are never written or executed.
use openmt2001::{
    level_catalog::{Catalog, script},
    pe::PeImage,
};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Resolve original Windows filenames on case-sensitive hosts without allowing
/// registration strings to escape the supplied scripts directory.
fn original_path(root: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let name = name.replace('\\', "/");
    let mut path = root.to_path_buf();
    for component in Path::new(&name).components() {
        let Component::Normal(part) = component else {
            return Err("level script path is not relative".into());
        };
        let direct = path.join(part);
        if direct.exists() {
            path = direct;
            continue;
        }
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut found = None;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(&part.to_string_lossy())
            {
                if found.is_some() {
                    return Err("ambiguous original script filename case".into());
                }
                found = Some(entry.path());
            }
        }
        let Some(next) = found else {
            return Ok(None);
        };
        path = next;
    }
    Ok(Some(path))
}
pub fn load_catalog(image: &PeImage<'_>, scripts: &Path) -> Result<Catalog, String> {
    let hub =
        original_path(scripts, "GAMESTART/hub.lua")?.ok_or("original GAMESTART/hub.lua missing")?;
    let source = fs::read(&hub).map_err(|e| format!("{}: {e}", hub.display()))?;
    script::load(image, &source, false, |name| {
        let name = std::str::from_utf8(name).map_err(|_| "non-UTF8 level directory")?;
        let Some(directory) = original_path(scripts, name)? else {
            return Ok(None);
        };
        let Some(hub) = original_path(&directory, "hub.lua")? else {
            return Ok(None);
        };
        if fs::File::open(hub).is_err() {
            return Ok(None);
        }
        let mut checksum = 0i32;
        for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry
                .path()
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("lua"))
            {
                continue;
            }
            //48d570 contributes -1 for a Lua file that cannot be opened.
            let value = fs::read(entry.path())
                .map(|bytes| {
                    bytes
                        .into_iter()
                        .fold(0i32, |sum, b| sum.wrapping_add(i32::from(b)))
                })
                .unwrap_or(-1);
            checksum = checksum.wrapping_add(value);
        }
        Ok(Some(checksum))
    })
}
/// A reversible encoding prevents profile names becoming filesystem paths.
pub fn profile_path(root: &Path, name: &[u8]) -> PathBuf {
    let key: String = name.iter().map(|b| format!("{b:02x}")).collect();
    root.join("campaign").join(format!("{key}.lua"))
}
pub fn load_progress(catalog: &mut Catalog, path: &Path) -> Result<(), String> {
    catalog.reset_progress();
    match fs::read_to_string(path) {
        Ok(source) => script::apply_profile(catalog, &source)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", path.display())),
    }
    catalog.unlock_first();
    Ok(())
}
pub fn save_progress(catalog: &Catalog, path: &Path) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("campaign profile parent missing")?)
        .map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, script::profile_text(catalog)).map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}
/// Apply original 4ba2a4 only once, even if writing the V2 profile fails and
/// the caller retries after fixing host storage. `applied` belongs to one
/// outcome presentation, never to the profile across launches.
pub fn commit_result(
    catalog: &mut Catalog,
    profile: Option<&Path>,
    applied: &mut bool,
    sequence: openmt2001::outcome_notice::Sequence,
) -> Result<(), String> {
    if !*applied {
        if catalog.selected_level().is_none() || catalog.difficulty >= 3 {
            return Err("Result catalog selection or difficulty invalid".into());
        }
        catalog.finish(true, sequence != openmt2001::outcome_notice::Sequence::Lose)?;
        *applied = true;
    }
    if let Some(path) = profile {
        save_progress(catalog, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retrying_failed_profile_write_does_not_finish_twice() {
        use openmt2001::level_catalog::{Labels, Registration};
        let mut catalog = Catalog::default();
        for (id, name) in [(200003, b"SCENARIO3".as_slice()), (200004, b"SCENARIO4".as_slice())] {
            catalog.register(Registration {
                id,
                directory: name.to_vec(),
                labels: Labels::Literal { title: vec![], description: vec![] },
                checksum: 0,
            }, false, true).unwrap();
        }
        catalog.reset_progress();
        catalog.unlock_first();
        assert!(catalog.select(200003, false));
        catalog.difficulty = 2;
        let root = PathBuf::from(format!("work/economy/campaign-write-retry-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let occupied = root.join("occupied");
        fs::write(&occupied, b"not a directory").unwrap();
        let mut applied = false;
        assert!(commit_result(&mut catalog, Some(&occupied.join("progress.lua")), &mut applied,
            openmt2001::outcome_notice::Sequence::Win).is_err());
        assert!(applied);
        assert_eq!(catalog.selected_level().unwrap().attempts(), 1);
        let path = root.join("campaign.lua");
        commit_result(&mut catalog, Some(&path), &mut applied,
            openmt2001::outcome_notice::Sequence::Win).unwrap();
        assert_eq!(catalog.selected_level().unwrap().attempts(), 1);
        assert_eq!(catalog.selected_level().unwrap().completed(), [false, false, true]);
        assert!(catalog.levels[1].unlocked());
        let mut reopened = catalog.clone();
        load_progress(&mut reopened, &path).unwrap();
        assert_eq!(reopened.levels[0].attempts(), 1);
        assert_eq!(reopened.levels[0].completed(), [false, false, true]);
        assert!(reopened.levels[1].unlocked());
        let mut replay_applied = false;
        commit_result(&mut catalog, Some(&path), &mut replay_applied,
            openmt2001::outcome_notice::Sequence::Lose).unwrap();
        assert_eq!(catalog.levels[0].attempts(), 2);
        let mut replayed = catalog.clone();
        load_progress(&mut replayed, &path).unwrap();
        assert_eq!(replayed.levels[0].attempts(), 2);
        assert_eq!(replayed.levels[0].completed(), [false, false, true]);
        catalog.difficulty = 3;
        let mut invalid_applied = false;
        assert!(commit_result(&mut catalog, None, &mut invalid_applied,
            openmt2001::outcome_notice::Sequence::Lose).is_err());
        assert!(!invalid_applied);
        assert_eq!(catalog.levels[0].attempts(), 2);
    }
    #[test]
    fn abandoned_launch_reopens_original_profile_state() {
        use openmt2001::level_catalog::{Labels, Registration};
        let mut base = Catalog::default();
        for (id, name) in [(200003, b"SCENARIO3".as_slice()), (200004, b"SCENARIO4".as_slice())] {
            base.register(Registration { id, directory: name.to_vec(),
                labels: Labels::Literal { title: vec![], description: vec![] }, checksum: 0 },
                false, true).unwrap();
        }
        // A previous test process can leave a committed progress file under a
        // reused PID. Each run needs a fresh profile to test launch abandonment.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = PathBuf::from(format!("work/economy/campaign-abort-{}-{nonce}", std::process::id()));
        let path = root.join("progress.lua");
        let mut active = base.clone();
        load_progress(&mut active, &path).unwrap();
        assert!(active.select(200003, false));
        active.difficulty = 1;
        assert_eq!(active.selected_level().unwrap().attempts(), 0);
        drop(active); // user exits before original result Game.End
        assert!(!path.exists());
        let mut reopened = base.clone();
        load_progress(&mut reopened, &path).unwrap();
        assert_eq!(reopened.levels[0].attempts(), 0);
        assert!(!reopened.levels[1].unlocked());
        assert!(reopened.select(200003, false));
        reopened.difficulty = 1;
        let mut applied = false;
        commit_result(&mut reopened, Some(&path), &mut applied,
            openmt2001::outcome_notice::Sequence::Win).unwrap();
        let mut next_launch = base;
        load_progress(&mut next_launch, &path).unwrap();
        assert_eq!(next_launch.levels[0].attempts(), 1);
        assert_eq!(next_launch.levels[0].completed(), [false, true, false]);
        assert!(next_launch.levels[1].unlocked());
        assert!(next_launch.select(200004, false));
        assert_eq!(next_launch.selected_level().unwrap().id(), 200004);
    }
    #[test]
    #[ignore = "requires original executable and installed level scripts"]
    fn original_catalog_and_retained_profile_roundtrip() {
        let executable = fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let scripts = PathBuf::from(std::env::var("OPENMT2001_ORIGINAL_SCRIPTS").unwrap());
        let mut catalog = load_catalog(&image, &scripts).unwrap();
        assert_eq!(catalog.levels.len(), 36);
        let root = PathBuf::from("work/economy/catalog-profile-roundtrip");
        let path = profile_path(&root, b"Campaign / Test");
        assert_eq!(path.parent(), Some(root.join("campaign").as_path()));
        catalog.reset_progress();
        catalog.unlock_first();
        assert!(catalog.select(200003, false));
        catalog.difficulty = 1;
        catalog.finish(true, true).unwrap();
        catalog.unlock(400001);
        save_progress(&catalog, &path).unwrap();
        let expected = script::profile_text(&catalog);
        load_progress(&mut catalog, &path).unwrap();
        assert_eq!(script::profile_text(&catalog), expected);
        assert_eq!(catalog.selected_level().unwrap().attempts(), 1);
        assert_eq!(
            catalog.selected_level().unwrap().completed(),
            [false, true, false]
        );
        assert!(catalog.levels[catalog.find(200004).unwrap()].unlocked());
        assert!(catalog.levels[catalog.find(400001).unwrap()].unlocked());
    }
}
