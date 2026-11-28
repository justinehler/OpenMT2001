//! Host I/O for the original level catalog and V2-owned progression records.
//! Private original profiles are never written or executed.
use crate::{
    level_catalog::{Catalog, script},
    pe::PeImage,
};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

/// Resolve original Windows filenames on case-sensitive hosts without allowing
/// registration strings to escape the supplied scripts directory.
const MAX_CUSTOM_DEPTH: usize = 32;
const MAX_SCAN_ENTRIES: usize = 16_384;
const MAX_REGISTRATION_FILES: usize = 1_024;
const MAX_REGISTRATION_BYTES: usize = 8 * 1024 * 1024;

fn contained(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let path = fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !path.starts_with(root) {
        return Err(format!(
            "script path escapes supplied root: {}",
            path.display()
        ));
    }
    Ok(path)
}

// Modern ordering policy, not a claim about Windows enumeration order.
// Reject case collisions even when the requested spelling exists exactly.
fn ordered_entries(path: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("non-UTF8 script filename in {}", path.display()))?;
        entries.push((name, entry.path()));
        if entries.len() > MAX_SCAN_ENTRIES {
            return Err(format!(
                "script directory entry limit exceeded: {}",
                path.display()
            ));
        }
    }
    entries.sort_by_key(|(name, _)| name.to_ascii_lowercase());
    for pair in entries.windows(2) {
        if pair[0].0.eq_ignore_ascii_case(&pair[1].0) {
            return Err(format!(
                "ambiguous original script filename case in {}: {} / {}",
                path.display(),
                pair[0].0,
                pair[1].0
            ));
        }
    }
    Ok(entries)
}

pub fn original_path(root: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let name = name.replace('\\', "/");
    if name.is_empty()
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains(':'))
    {
        return Err("level script path is not relative".into());
    }
    let mut path = root.clone();
    for component in Path::new(&name).components() {
        let Component::Normal(part) = component else {
            return Err("level script path is not relative".into());
        };
        let entries = ordered_entries(&path)?;
        let Some((_, next)) = entries
            .into_iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&part.to_string_lossy()))
        else {
            return Ok(None);
        };
        path = contained(&root, &next)?;
    }
    Ok(Some(path))
}

fn read_registration(path: &Path, remaining: &mut usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take((*remaining + 1) as u64).read_to_end(&mut bytes))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > *remaining {
        return Err(format!(
            "registration byte limit exceeded at {}",
            path.display()
        ));
    }
    *remaining -= bytes.len();
    Ok(bytes)
}

/// GAMESTART first, then CUSTOM descendants before each directory's register.
/// Missing CUSTOM is valid. No installer or filesystem callback is exposed to Lua.
fn registration_sources(scripts: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let scripts = fs::canonicalize(scripts).map_err(|e| e.to_string())?;
    let hub = original_path(&scripts, "GAMESTART/hub.lua")?
        .ok_or("original GAMESTART/hub.lua missing")?;
    let mut remaining = MAX_REGISTRATION_BYTES;
    let mut sources = vec![(
        "GAMESTART/hub.lua".into(),
        read_registration(&hub, &mut remaining)?,
    )];
    if let Some(custom) = original_path(&scripts, "CUSTOM")? {
        let mut seen = std::collections::BTreeSet::new();
        let mut entries = 0;
        discover_registrations(
            &scripts,
            &custom,
            0,
            &mut seen,
            &mut entries,
            &mut remaining,
            &mut sources,
        )?;
    }
    Ok(sources)
}

fn discover_registrations(
    root: &Path,
    directory: &Path,
    depth: usize,
    seen: &mut std::collections::BTreeSet<PathBuf>,
    entries_seen: &mut usize,
    remaining: &mut usize,
    sources: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    if depth > MAX_CUSTOM_DEPTH {
        return Err(format!(
            "custom registration depth limit exceeded: {}",
            directory.display()
        ));
    }
    let directory = contained(root, directory)?;
    if !seen.insert(directory.clone()) {
        return Err(format!(
            "custom registration directory cycle or alias: {}",
            directory.display()
        ));
    }
    let entries = ordered_entries(&directory)?;
    *entries_seen += entries.len();
    if *entries_seen > MAX_SCAN_ENTRIES {
        return Err("custom registration scan entry limit exceeded".into());
    }
    for (name, path) in &entries {
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        let resolved = contained(root, path)?;
        if resolved.is_dir() {
            discover_registrations(
                root,
                &resolved,
                depth + 1,
                seen,
                entries_seen,
                remaining,
                sources,
            )?;
        }
    }
    if let Some((_, path)) = entries
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("register.lua"))
    {
        if sources.len() >= MAX_REGISTRATION_FILES {
            return Err("registration file count limit exceeded".into());
        }
        let path = contained(root, path)?;
        if !path.is_file() {
            return Err(format!("registration is not a file: {}", path.display()));
        }
        sources.push((
            path.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            read_registration(&path, remaining)?,
        ));
    }
    Ok(())
}

fn directory_checksum(scripts: &Path, name: &[u8]) -> Result<Option<i32>, String> {
    let name = std::str::from_utf8(name).map_err(|_| "non-UTF8 level directory")?;
    let Some(directory) = original_path(scripts, name)? else {
        return Ok(None);
    };
    if !directory.is_dir() {
        return Ok(None);
    }
    let Some(hub) = original_path(&directory, "hub.lua")? else {
        return Ok(None);
    };
    if !hub.is_file() || fs::File::open(hub).is_err() {
        return Ok(None);
    }
    let mut checksum = 0i32;
    for (_, path) in ordered_entries(&directory)? {
        if !path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("lua"))
        {
            continue;
        }
        let path = contained(&directory, &path)?;
        //48d570 contributes -1 for a Lua file that cannot be opened/read.
        // Stream checksum inputs so large authored files cannot grow host memory.
        let value = (|| -> std::io::Result<i32> {
            let mut file = fs::File::open(path)?;
            let mut buffer = [0u8; 8192];
            let mut sum = 0i32;
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    return Ok(sum);
                }
                for &b in &buffer[..n] {
                    sum = sum.wrapping_add(i32::from(b));
                }
            }
        })()
        .unwrap_or(-1);
        checksum = checksum.wrapping_add(value);
    }
    Ok(Some(checksum))
}

pub fn load_catalog(image: &PeImage<'_>, scripts: &Path) -> Result<Catalog, String> {
    let scripts = fs::canonicalize(scripts).map_err(|e| e.to_string())?;
    let sources = registration_sources(&scripts)?;
    let buffers: Vec<_> = sources
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    script::load_sources(image, &buffers, false, |name| {
        directory_checksum(&scripts, name)
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
/// Test support for campaign continuations: a new V2 profile in which every
/// level before `level_id` in the original catalog's campaign order has been
/// won once on Easy, through the same `Catalog::finish` path a real Win takes.
/// Each call writes its own file, so tests never depend on another test's
/// output or share progress. Test support: panics on any failure.
#[doc(hidden)]
pub fn checkpoint_profile(image: &PeImage<'_>, scripts: &Path, level_id: u32) -> PathBuf {
    let mut catalog = load_catalog(image, scripts).unwrap();
    catalog.reset_progress();
    catalog.unlock_first();
    catalog.difficulty = 0;
    let target = catalog
        .find(level_id)
        .unwrap_or_else(|| panic!("checkpoint level {level_id} is not registered"));
    for _ in 0..catalog.levels.len() {
        if catalog.levels[target].unlocked() {
            break;
        }
        let frontier: Vec<u32> = catalog
            .levels
            .iter()
            .filter(|l| !l.tutorial() && l.single_player() && l.unlocked() && l.attempts() == 0)
            .map(|l| l.id())
            .collect();
        let [next] = frontier[..] else {
            panic!("campaign frontier is not a single level: {frontier:?}");
        };
        assert!(catalog.select(next, false));
        catalog.finish(true, 1).unwrap();
    }
    assert!(
        catalog.levels[target].unlocked(),
        "the campaign never unlocks {level_id}"
    );
    catalog.select(u32::MAX, false);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = crate::fixtures::scratch("CAMPAIGN_CHECKPOINTS").join(format!(
        "before-{level_id}-{}-{nonce}.lua",
        std::process::id()
    ));
    save_progress(&catalog, &path).unwrap();
    path
}
/// Apply original 4ba2a4 only once, even if writing the V2 profile fails and
/// the caller retries after fixing host storage. `applied` belongs to one
/// outcome presentation, never to the profile across launches. The authored
/// Game.End must match the sequence before either progression or I/O occurs.
pub fn commit_result(
    catalog: &mut Catalog,
    profile: Option<&Path>,
    applied: &mut bool,
    sequence: crate::outcome_notice::Sequence,
    end_requested: Option<u32>,
) -> Result<(), String> {
    let secondary = if sequence == crate::outcome_notice::Sequence::Lose {
        2
    } else {
        1
    };
    if end_requested != Some(secondary) {
        return Err("Original result script has not requested completion".into());
    }
    if !*applied {
        if catalog.selected_level().is_none() || catalog.difficulty >= 3 {
            return Err("Result catalog selection or difficulty invalid".into());
        }
        catalog.finish(true, secondary as i32)?;
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
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            // macOS clocks can repeat a nanosecond value across back-to-back
            // calls, so a per-process counter keeps parallel tests apart.
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = PathBuf::from(format!(
                "work/custom-import-{}-{nonce}-{serial}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("GAMESTART")).unwrap();
            fs::write(root.join("GAMESTART/hub.lua"), b"-- base").unwrap();
            Self(root)
        }
        fn write(&self, name: &str, bytes: &[u8]) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn result_profile_waits_for_result_sequence_end() {
        use crate::level_catalog::{Labels, Registration};
        use crate::outcome_notice::Sequence;
        let fixture = Fixture::new();
        for (sequence, secondary, awarded) in [
            (Sequence::Win, 1, true),
            (Sequence::Complete, 1, true),
            (Sequence::Lose, 2, false),
        ] {
            let mut catalog = Catalog::default();
            for id in [200003, 200004] {
                catalog
                    .register(
                        Registration {
                            id,
                            directory: b"fixture".to_vec(),
                            labels: Labels::Literal {
                                title: vec![],
                                description: vec![],
                            },
                            checksum: 0,
                        },
                        false,
                        true,
                    )
                    .unwrap();
            }
            catalog.unlock_first();
            catalog.difficulty = 1;
            let path = fixture.0.join(format!("{sequence:?}.lua"));
            let mut applied = false;
            // These are the same absent/end states read by the live Host.
            for end in [None, Some(0), Some(3 - secondary), Some(u32::MAX)] {
                assert!(
                    commit_result(&mut catalog, Some(&path), &mut applied, sequence, end).is_err()
                );
                assert!(!path.exists());
                assert!(!applied);
                assert_eq!(catalog.levels[0].attempts(), 0);
                assert_eq!(catalog.levels[0].completed(), [false; 3]);
                assert!(!catalog.levels[1].unlocked());
            }
            // The live UI dispatches CutSceneEnded only at the controller's
            // fade-out edge; the result hub then supplies Game.End's pair.
            let mut controller = crate::result_controller::Controller::new(true, false, 0);
            for tick in 1..=186 {
                let ended = controller.tick(tick == 101);
                assert_eq!(ended, tick == 186);
                let end = ended.then_some(secondary);
                let result = commit_result(&mut catalog, Some(&path), &mut applied, sequence, end);
                if !ended {
                    assert!(result.is_err());
                    assert!(!path.exists());
                    assert_eq!(catalog.levels[0].attempts(), 0);
                } else {
                    result.unwrap();
                }
            }
            let saved = fs::read(&path).unwrap();
            let mut reopened = catalog.clone();
            load_progress(&mut reopened, &path).unwrap();
            assert_eq!(reopened.levels[0].attempts(), 1);
            assert_eq!(reopened.levels[0].completed(), [false, awarded, false]);
            assert_eq!(reopened.levels[1].unlocked(), awarded);
            commit_result(
                &mut catalog,
                Some(&path),
                &mut applied,
                sequence,
                Some(secondary),
            )
            .unwrap();
            assert_eq!(fs::read(&path).unwrap(), saved);
            assert_eq!(catalog.levels[0].attempts(), 1);
        }
    }

    #[test]
    fn custom_result_records_share_profile_and_preserve_campaign_by_id() {
        use crate::level_catalog::{Labels, Registration};
        use crate::outcome_notice::Sequence;
        let fixture = Fixture::new();
        let path = profile_path(&fixture.0, b"Combined profile");
        let mut catalog = Catalog::default();
        for (id, directory) in [
            (200001, "SCENARIO1"),
            (200002, "SCENARIO2"),
            (500000, "custom/author/first"),
            (500001, "custom/author/second"),
        ] {
            catalog
                .register(
                    Registration {
                        id,
                        directory: directory.as_bytes().to_vec(),
                        labels: Labels::Literal {
                            title: vec![],
                            description: vec![],
                        },
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
        }
        catalog.reset_progress();
        catalog.set_level_info(200001, 3, [1, 0, 1]);
        catalog.set_level_info(200002, 2, [0, 1, 0]);
        let campaign_before: Vec<_> = catalog.levels[..2].to_vec();
        assert!(catalog.select(500000, false));
        catalog.difficulty = 1;
        let mut applied = false;
        // Exercise the persistence boundary, not a simulated gameplay victory.
        commit_result(
            &mut catalog,
            Some(&path),
            &mut applied,
            Sequence::Win,
            Some(1),
        )
        .unwrap();
        commit_result(
            &mut catalog,
            Some(&path),
            &mut applied,
            Sequence::Win,
            Some(1),
        )
        .unwrap();
        assert_eq!(catalog.levels[..2], campaign_before);
        let mut reopened = catalog.clone();
        load_progress(&mut reopened, &path).unwrap();
        let level = &reopened.levels[reopened.find(500000).unwrap()];
        assert_eq!(
            (level.attempts(), level.completed()),
            (1, [false, true, false])
        );
        assert_eq!(reopened.levels[..2], campaign_before);
        // Original unlock_next applies to the actual catalog successor too.
        assert!(reopened.levels[reopened.find(500001).unwrap()].unlocked());
        assert!(reopened.select(500000, false));
        reopened.difficulty = 2;
        commit_result(
            &mut reopened,
            Some(&path),
            &mut false,
            Sequence::Lose,
            Some(2),
        )
        .unwrap();
        load_progress(&mut catalog, &path).unwrap();
        let level = &catalog.levels[catalog.find(500000).unwrap()];
        assert_eq!(
            (level.attempts(), level.completed()),
            (2, [false, true, false])
        );
        assert_eq!(catalog.levels[..2], campaign_before);
    }
    #[test]
    fn custom_discovery_empty_order_and_exclusions() {
        let fixture = Fixture::new();
        assert_eq!(registration_sources(&fixture.0).unwrap().len(), 1);
        fixture.write("CuStOm/register.lua", b"parent");
        fixture.write("CuStOm/z/register.lua", b"z");
        fixture.write("CuStOm/A/register.lua", b"a");
        fixture.write("CuStOm/A/child/REGISTER.LUA", b"child");
        fixture.write("CuStOm/_hidden/register.lua", b"ignored");
        fixture.write("CuStOm/.hidden/register.lua", b"ignored");
        let sources = registration_sources(&fixture.0).unwrap();
        assert_eq!(
            sources
                .iter()
                .map(|(_, b)| b.as_slice())
                .collect::<Vec<_>>(),
            vec![b"-- base".as_slice(), b"child", b"a", b"z", b"parent"]
        );
        fixture.write("CuStOm/a/register.lua", b"collision");
        // Case-insensitive filesystems update A rather than creating a second
        // entry. Only an actual ambiguous directory pair must be rejected.
        let names = fs::read_dir(fixture.0.join("CuStOm"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        if names.iter().any(|name| name == "A") && names.iter().any(|name| name == "a") {
            assert!(
                registration_sources(&fixture.0)
                    .unwrap_err()
                    .contains("ambiguous")
            );
        } else {
            assert!(registration_sources(&fixture.0).is_ok());
        }
    }
    #[test]
    fn registration_limits_and_missing_hubs() {
        let fixture = Fixture::new();
        assert_eq!(directory_checksum(&fixture.0, b"missing").unwrap(), None);
        fixture.write("CUSTOM/author/register.lua", b"abc");
        assert_eq!(
            directory_checksum(&fixture.0, b"custom/author").unwrap(),
            None
        );
        fixture.write("CUSTOM/author/hub.lua", b"d");
        assert_eq!(
            directory_checksum(&fixture.0, b"CUSTOM/author").unwrap(),
            Some(394)
        );
        let mut remaining = 2;
        assert!(
            read_registration(
                &fixture.0.join("CUSTOM/author/register.lua"),
                &mut remaining
            )
            .is_err()
        );
        assert!(original_path(&fixture.0, "../escape").is_err());
        let mut path = String::from("CUSTOM");
        for _ in 0..=MAX_CUSTOM_DEPTH {
            path.push_str("/child");
        }
        fixture.write(&format!("{path}/register.lua"), b"");
        assert!(
            registration_sources(&fixture.0)
                .unwrap_err()
                .contains("depth limit")
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlink_containment_covers_discovery_hubs_and_checksum_inputs() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let outside_root = fs::canonicalize(&outside.0).unwrap();
        fs::create_dir_all(fixture.0.join("CUSTOM/author")).unwrap();
        symlink(&outside_root, fixture.0.join("CUSTOM/escape")).unwrap();
        assert!(
            registration_sources(&fixture.0)
                .unwrap_err()
                .contains("escapes")
        );
        fs::remove_file(fixture.0.join("CUSTOM/escape")).unwrap();
        symlink(
            outside_root.join("GAMESTART/hub.lua"),
            fixture.0.join("CUSTOM/author/hub.lua"),
        )
        .unwrap();
        assert!(directory_checksum(&fixture.0, b"custom/author").is_err());
        fs::remove_file(fixture.0.join("CUSTOM/author/hub.lua")).unwrap();
        fixture.write("CUSTOM/author/hub.lua", b"hub");
        symlink(
            outside_root.join("GAMESTART/hub.lua"),
            fixture.0.join("CUSTOM/author/other.lua"),
        )
        .unwrap();
        assert!(directory_checksum(&fixture.0, b"custom/author").is_err());
        fs::remove_file(fixture.0.join("CUSTOM/author/other.lua")).unwrap();
        symlink(
            fs::canonicalize(fixture.0.join("CUSTOM")).unwrap(),
            fixture.0.join("CUSTOM/author/cycle"),
        )
        .unwrap();
        assert!(
            registration_sources(&fixture.0)
                .unwrap_err()
                .contains("cycle or alias")
        );
    }
    #[test]
    #[ignore = "requires original executable and supplied patch Essential scripts"]
    fn supplied_eight_custom_registrations_append_to_gamestart() {
        let executable = fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let scripts = crate::fixtures::path("SCRIPTS");
        let sources = registration_sources(&scripts).unwrap();
        assert_eq!(sources.len(), 9);
        let base = script::load(&image, &sources[0].1, false, |name| {
            directory_checksum(&scripts, name)
        })
        .unwrap();
        let catalog = load_catalog(&image, &scripts).unwrap();
        assert_eq!(catalog.levels.len(), base.levels.len() + 8);
        assert_eq!(
            catalog.levels[..base.levels.len()]
                .iter()
                .map(|l| l.id())
                .collect::<Vec<_>>(),
            base.levels.iter().map(|l| l.id()).collect::<Vec<_>>()
        );
        assert_eq!(
            catalog.levels[base.levels.len()..]
                .iter()
                .map(|l| l.id())
                .collect::<Vec<_>>(),
            (500000..=500007).collect::<Vec<_>>()
        );
        assert_eq!(
            catalog.levels[catalog.find(500000).unwrap()].directory(),
            b"custom\\deepred\\Jon001"
        );
    }
    #[test]
    fn retrying_failed_profile_write_does_not_finish_twice() {
        use crate::level_catalog::{Labels, Registration};
        let mut catalog = Catalog::default();
        for (id, name) in [
            (200003, b"SCENARIO3".as_slice()),
            (200004, b"SCENARIO4".as_slice()),
        ] {
            catalog
                .register(
                    Registration {
                        id,
                        directory: name.to_vec(),
                        labels: Labels::Literal {
                            title: vec![],
                            description: vec![],
                        },
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
        }
        catalog.reset_progress();
        catalog.unlock_first();
        assert!(catalog.select(200003, false));
        catalog.difficulty = 2;
        let root = PathBuf::from(format!(
            "work/economy/campaign-write-retry-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let occupied = root.join("occupied");
        fs::write(&occupied, b"not a directory").unwrap();
        let mut applied = false;
        assert!(
            commit_result(
                &mut catalog,
                Some(&occupied.join("progress.lua")),
                &mut applied,
                crate::outcome_notice::Sequence::Win,
                Some(1)
            )
            .is_err()
        );
        assert!(applied);
        assert_eq!(catalog.selected_level().unwrap().attempts(), 1);
        let path = root.join("campaign.lua");
        commit_result(
            &mut catalog,
            Some(&path),
            &mut applied,
            crate::outcome_notice::Sequence::Win,
            Some(1),
        )
        .unwrap();
        assert_eq!(catalog.selected_level().unwrap().attempts(), 1);
        assert_eq!(
            catalog.selected_level().unwrap().completed(),
            [false, false, true]
        );
        assert!(catalog.levels[1].unlocked());
        let mut reopened = catalog.clone();
        load_progress(&mut reopened, &path).unwrap();
        assert_eq!(reopened.levels[0].attempts(), 1);
        assert_eq!(reopened.levels[0].completed(), [false, false, true]);
        assert!(reopened.levels[1].unlocked());
        let mut replay_applied = false;
        commit_result(
            &mut catalog,
            Some(&path),
            &mut replay_applied,
            crate::outcome_notice::Sequence::Lose,
            Some(2),
        )
        .unwrap();
        assert_eq!(catalog.levels[0].attempts(), 2);
        let mut replayed = catalog.clone();
        load_progress(&mut replayed, &path).unwrap();
        assert_eq!(replayed.levels[0].attempts(), 2);
        assert_eq!(replayed.levels[0].completed(), [false, false, true]);
        catalog.difficulty = 3;
        let mut invalid_applied = false;
        assert!(
            commit_result(
                &mut catalog,
                None,
                &mut invalid_applied,
                crate::outcome_notice::Sequence::Lose,
                Some(2)
            )
            .is_err()
        );
        assert!(!invalid_applied);
        assert_eq!(catalog.levels[0].attempts(), 2);
    }
    #[test]
    fn abandoned_launch_reopens_original_profile_state() {
        use crate::level_catalog::{Labels, Registration};
        let mut base = Catalog::default();
        for (id, name) in [
            (200003, b"SCENARIO3".as_slice()),
            (200004, b"SCENARIO4".as_slice()),
        ] {
            base.register(
                Registration {
                    id,
                    directory: name.to_vec(),
                    labels: Labels::Literal {
                        title: vec![],
                        description: vec![],
                    },
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
        }
        // A previous test process can leave a committed progress file under a
        // reused PID. Each run needs a fresh profile to test launch abandonment.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(format!(
            "work/economy/campaign-abort-{}-{nonce}",
            std::process::id()
        ));
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
        commit_result(
            &mut reopened,
            Some(&path),
            &mut applied,
            crate::outcome_notice::Sequence::Win,
            Some(1),
        )
        .unwrap();
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
        let executable = fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let scripts = crate::fixtures::path("ORIGINAL_SCRIPTS");
        let mut catalog = load_catalog(&image, &scripts).unwrap();
        let hub = original_path(&scripts, "GAMESTART/hub.lua")
            .unwrap()
            .unwrap();
        let base = script::load(&image, &fs::read(hub).unwrap(), false, |name| {
            directory_checksum(&scripts, name)
        })
        .unwrap();
        assert!(catalog.levels.len() >= base.levels.len());
        assert_eq!(
            catalog.levels[..base.levels.len()]
                .iter()
                .map(|level| level.id())
                .collect::<Vec<_>>(),
            base.levels
                .iter()
                .map(|level| level.id())
                .collect::<Vec<_>>()
        );
        let root = PathBuf::from("work/economy/catalog-profile-roundtrip");
        let path = profile_path(&root, b"Campaign / Test");
        assert_eq!(path.parent(), Some(root.join("campaign").as_path()));
        catalog.reset_progress();
        catalog.unlock_first();
        assert!(catalog.select(200003, false));
        catalog.difficulty = 1;
        catalog.finish(true, 1).unwrap();
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
