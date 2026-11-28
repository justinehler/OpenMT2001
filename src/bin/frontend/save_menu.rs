//! Host-only save destination and atomic file publication.
//! --save-dir is a read source; writes always use the selected V2 profile root.
#[path = "save_camera.rs"]
pub(crate) mod camera_source;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// V2 uses the campaign store's reversible profile key for save isolation.
/// The extra `saves` child is supplied by the existing slot publisher.
pub fn selected_profile_root(app_root: &Path, name: &[u8]) -> PathBuf {
    let campaign = crate::campaign_store::profile_path(app_root, name);
    profile_root_for_campaign(app_root, &campaign)
        .expect("campaign_store produced an invalid profile identity")
}

pub fn profile_root_for_campaign(app_root: &Path, campaign: &Path) -> Option<PathBuf> {
    let filename = campaign.file_name()?.to_str()?;
    let key = profile_key(filename)?;
    Some(app_root.join("profiles").join(key))
}

/// Recover only the menu's exact selected profile identity, never an arbitrary
/// save parent or a filename supplied by the saved game.
pub fn progress_for_profile_root(app_root: &Path, selected: &Path) -> Option<PathBuf> {
    let relative = selected.strip_prefix(app_root.join("profiles")).ok()?;
    let key = relative.to_str()?;
    let filename = format!("{key}.lua");
    profile_key(&filename)?;
    let progress = app_root.join("campaign").join(filename);
    (profile_root_for_campaign(app_root, &progress)?.as_path() == selected).then_some(progress)
}

fn profile_key(filename: &str) -> Option<&str> {
    let key = filename.strip_suffix(".lua")?;
    (key.len().is_multiple_of(2)
        && !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then_some(key)
}

/// Old V2 releases placed all saves in one directory. Attribute a legacy
/// row only when its intact extension record 1 identifies this profile.
/// These paths are load-only: overwrite and Delete operate on scoped slots.
pub fn matching_legacy_paths(
    app_root: &Path,
    campaign_filename: &str,
) -> Result<Vec<PathBuf>, String> {
    if profile_key(campaign_filename).is_none() {
        return Ok(Vec::new());
    }
    reject_reference(app_root)?;
    let directory = app_root.join("saves");
    if directory.exists() {
        let root = fs::canonicalize(app_root).map_err(|e| format!("Legacy profile root: {e}"))?;
        let actual =
            fs::canonicalize(&directory).map_err(|e| format!("Legacy save directory: {e}"))?;
        reject_reference(&actual)?;
        if !actual.starts_with(root) {
            return Err("Legacy V2 save directory escapes its profile root".into());
        }
    }
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("List legacy V2 saves: {error}")),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("List legacy V2 saves: {e}"))?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sav"))
        {
            continue;
        }
        let Some(owner) = fs::read(&path).ok().and_then(|bytes| {
            let save = openmt2001::save::SaveFile::parse(&bytes).ok()?;
            let extension = openmt2001::save::extension::Extension::parse(&save).ok()??;
            extension.validate_runtime_section_set().ok()?;
            std::str::from_utf8(extension.required(1).ok()?)
                .ok()
                .map(str::to_owned)
        }) else {
            continue;
        };
        if owner == campaign_filename {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Profile Delete (native 4c1fd0) removes saves belonging to that profile.
/// Re-registration repeats this cleanup so an interrupted earlier Delete
/// cannot reveal old slots. Every directory is resolved before removing its
/// known regular files; no recursive filesystem operation is used.
pub fn clear_profile_saves(app_root: &Path, name: &[u8]) -> Result<(), String> {
    reject_reference(app_root)?;
    let app = match fs::canonicalize(app_root) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Profile save root: {error}")),
    };
    reject_reference(&app)?;
    let profiles = app_root.join("profiles");
    match fs::symlink_metadata(&profiles) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() {
                return Err("V2 profiles directory is not a regular directory".into());
            }
            let actual_profiles =
                fs::canonicalize(&profiles).map_err(|e| format!("V2 profiles directory: {e}"))?;
            if actual_profiles.parent() != Some(app.as_path()) {
                return Err("V2 profiles directory escapes the application root".into());
            }
            let selected = selected_profile_root(app_root, name);
            match fs::symlink_metadata(&selected) {
                Ok(metadata) => {
                    if !metadata.file_type().is_dir() {
                        return Err("Selected V2 profile is not a regular directory".into());
                    }
                    let actual_selected = fs::canonicalize(&selected)
                        .map_err(|e| format!("Selected V2 profile: {e}"))?;
                    if actual_selected.parent() != Some(actual_profiles.as_path()) {
                        return Err("Selected V2 profile escapes its profile directory".into());
                    }
                    let saves = selected.join("saves");
                    match fs::symlink_metadata(&saves) {
                        Ok(metadata) => {
                            if !metadata.file_type().is_dir() {
                                return Err("Selected V2 saves is not a regular directory".into());
                            }
                            let actual_saves = fs::canonicalize(&saves)
                                .map_err(|e| format!("Selected V2 saves: {e}"))?;
                            if actual_saves.parent() != Some(actual_selected.as_path()) {
                                return Err("Selected V2 saves escapes its profile".into());
                            }
                            let mut files = Vec::new();
                            for entry in fs::read_dir(&saves)
                                .map_err(|e| format!("List profile saves: {e}"))?
                            {
                                let entry =
                                    entry.map_err(|e| format!("List profile saves: {e}"))?;
                                if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                                    return Err("Unexpected non-file in selected V2 saves".into());
                                }
                                let path = entry.path();
                                let is_save = path
                                    .extension()
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("sav"));
                                let is_stage = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .is_some_and(|s| s.starts_with(".V2-") && s.ends_with(".tmp"));
                                if !is_save && !is_stage {
                                    return Err("Unexpected file in selected V2 saves".into());
                                }
                                files.push(path);
                            }
                            for path in files {
                                fs::remove_file(path)
                                    .map_err(|e| format!("Delete profile save: {e}"))?;
                            }
                            fs::remove_dir(&saves)
                                .map_err(|e| format!("Remove selected V2 saves: {e}"))?;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                        Err(error) => return Err(format!("Inspect selected V2 saves: {error}")),
                    }
                    fs::remove_dir(&selected)
                        .map_err(|e| format!("Remove selected V2 profile: {e}"))?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(format!("Inspect selected V2 profile: {error}")),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(format!("Inspect V2 profiles directory: {error}")),
    }
    let filename = crate::campaign_store::profile_path(app_root, name)
        .file_name()
        .ok_or("Invalid V2 profile name")?
        .to_string_lossy()
        .into_owned();
    for path in matching_legacy_paths(app_root, &filename)? {
        if !fs::symlink_metadata(&path)
            .map_err(|e| format!("Inspect legacy profile save: {e}"))?
            .file_type()
            .is_file()
        {
            return Err("Legacy profile save is no longer a regular file".into());
        }
        fs::remove_file(path).map_err(|e| format!("Delete legacy profile save: {e}"))?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Slot {
    /// Basename including `.sav`; never a path supplied by the UI.
    pub id: String,
    pub name: String,
    pub modified: Option<SystemTime>,
    pub metadata: Result<[u32; 4], String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    NewName(String),
    ExistingSlot(String),
}

/// Windows/NTFS enumeration used by 497660 and 494ac0 compares upper-case
/// names ordinally. Use UTF-16 units, without locale collation or expanding
/// characters such as sharp S into multiple letters. Non-ASCII uses Rust's
/// single-character upper-case mappings as a deterministic portable fallback
/// for the original volume's NTFS upper-case table.
pub(crate) fn compare_save_names(left: &str, right: &str) -> std::cmp::Ordering {
    fn folded(name: &str) -> impl Iterator<Item = u16> + '_ {
        name.chars().flat_map(|character| {
            let mut uppercase = character.to_uppercase();
            let first = uppercase.next().unwrap();
            let simple = if uppercase.next().is_none() {
                first
            } else {
                character
            };
            let mut units = [0; 2];
            let len = simple.encode_utf16(&mut units).len();
            units.into_iter().take(len)
        })
    }
    folded(left)
        .cmp(folded(right))
        // Case-only duplicates cannot occur on NTFS, but can on other hosts.
        .then_with(|| left.as_bytes().cmp(right.as_bytes()))
}

/// Enumerate only writable V2-profile saves for the in-game Save Game panel.
/// Original import directories remain load-only, even when the Load Game
/// panel presents them alongside V2 slots.
pub fn list_slots(profile_root: &Path) -> Result<Vec<Slot>, String> {
    reject_reference(profile_root)?;
    if !profile_root.exists() {
        return Ok(Vec::new());
    }
    let root = fs::canonicalize(profile_root).map_err(|e| format!("Save profile root: {e}"))?;
    reject_reference(&root)?;
    let directory = root.join("saves");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let canonical = fs::canonicalize(directory).map_err(|e| format!("Save directory: {e}"))?;
    reject_reference(&canonical)?;
    if !canonical.starts_with(&root) {
        return Err("V2 save directory escapes its profile root".into());
    }
    let mut slots = Vec::new();
    for entry in fs::read_dir(&canonical).map_err(|e| format!("List V2 saves: {e}"))? {
        let entry = entry.map_err(|e| format!("List V2 saves: {e}"))?;
        // Refuse links and subdirectories, including links into original inputs.
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sav"))
        {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(name) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        let metadata = (|| {
            let bytes = fs::read(&path).map_err(|e| format!("Read saved game: {e}"))?;
            openmt2001::save::SaveFile::parse(&bytes)?.level_metadata()
        })();
        slots.push(Slot {
            id,
            name,
            modified,
            metadata,
        });
    }
    slots.sort_by(|left, right| compare_save_names(&left.id, &right.id));
    Ok(slots)
}

/// Original 497708 uses CRT localtime for the displayed hour/minute, then
/// FileTimeToSystemTime for the date, so the date remains UTC near a local
/// day boundary. GetDateFormat uses the user's short-date format.
pub fn format_modified_local(modified: Option<SystemTime>) -> Option<String> {
    let utc: chrono::DateTime<chrono::Utc> = modified?.into();
    Some(original_modified_label(
        utc.with_timezone(&chrono::Local),
        user_short_date,
    ))
}

fn original_modified_label<Tz: chrono::TimeZone>(
    local: chrono::DateTime<Tz>,
    format_date: impl FnOnce(chrono::NaiveDate) -> String,
) -> String {
    use chrono::Timelike;
    original_time_label(
        local.hour(),
        local.minute(),
        &format_date(local.naive_utc().date()),
    )
}

#[cfg(windows)]
fn user_short_date(date: chrono::NaiveDate) -> String {
    windows_short_date(date).unwrap_or_else(|| short_date_for_locale(date, "C"))
}

#[cfg(windows)]
fn windows_short_date(date: chrono::NaiveDate) -> Option<String> {
    use chrono::Datelike;
    use windows_sys::Win32::{
        Foundation::SYSTEMTIME,
        Globalization::{GetDateFormatW, LOCALE_USER_DEFAULT},
    };
    let system_time = SYSTEMTIME {
        wYear: date.year().try_into().ok()?,
        wMonth: date.month() as u16,
        wDay: date.day() as u16,
        ..Default::default()
    };
    // SAFETY: SYSTEMTIME is initialized and lives through both calls. A null
    // format selects the user's short date, including any system overrides.
    // With a null output and zero capacity, this call only queries its size.
    let size = unsafe {
        GetDateFormatW(
            LOCALE_USER_DEFAULT,
            0,
            &system_time,
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
        )
    };
    if size <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize];
    // SAFETY: The writable buffer has the requested number of UTF-16 units.
    let written = unsafe {
        GetDateFormatW(
            LOCALE_USER_DEFAULT,
            0,
            &system_time,
            std::ptr::null(),
            buffer.as_mut_ptr(),
            size,
        )
    };
    (written > 0).then(|| String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

#[cfg(not(windows))]
fn user_short_date(date: chrono::NaiveDate) -> String {
    let locale = date_locale(|key| std::env::var(key).ok());
    short_date_for_locale(date, locale.as_deref().unwrap_or("C"))
}

#[cfg(any(not(windows), test))]
fn date_locale(mut lookup: impl FnMut(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .filter_map(&mut lookup)
        .find(|locale| !locale.is_empty())
}

/// A small fallback for hosts without Windows' user-date API. Recognize common
/// locale orders/separators; C, POSIX and unknown locales keep the ISO fallback.
fn short_date_for_locale(date: chrono::NaiveDate, locale: &str) -> String {
    let locale = locale
        .split(['.', '@'])
        .next()
        .unwrap_or(locale)
        .replace('-', "_")
        .to_ascii_lowercase();
    let language = locale.split('_').next().unwrap_or(&locale);
    let pattern = match locale.as_str() {
        "en_gb" | "en_au" | "en_nz" | "en_ie" | "en_in" | "en_sg" => "%d/%m/%Y",
        "en_ca" | "fr_ca" => "%Y-%m-%d",
        "de_ch" | "fr_ch" | "it_ch" => "%d.%m.%Y",
        _ => match language {
            "en" => "%-m/%-d/%Y",
            "fr" | "es" | "it" | "pt" => "%d/%m/%Y",
            "de" | "ru" | "pl" => "%d.%m.%Y",
            "fi" => "%-d.%-m.%Y",
            "nl" => "%d-%m-%Y",
            "ja" | "zh" => "%Y/%m/%d",
            _ => "%Y-%m-%d",
        },
    };
    date.format(pattern).to_string()
}

fn original_time_label(hour: u32, minute: u32, date: &str) -> String {
    // 49772c subtracts 12 only when greater; 497783 changes zero to 12.
    // The original still gives noon the morning suffix.
    let (hour, suffix) = if hour > 12 {
        (hour - 12, "pm")
    } else {
        (hour, "am")
    };
    let hour = if hour == 0 { 12 } else { hour };
    format!("{hour}:{minute:02}{suffix} {date}")
}

fn reject_reference(path: &Path) -> Result<(), String> {
    if path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.eq_ignore_ascii_case("Original Distribution")
            || name.eq_ignore_ascii_case("Original Game Evidence")
    }) {
        return Err("Save destination is inside immutable original reference inputs".into());
    }
    Ok(())
}

fn writable_directory(profile_root: &Path) -> Result<PathBuf, String> {
    reject_reference(profile_root)?;
    let root = if profile_root.exists() {
        fs::canonicalize(profile_root).map_err(|e| format!("Save profile root: {e}"))?
    } else {
        let ancestor = profile_root
            .ancestors()
            .find(|path| path.exists())
            .ok_or("Save profile root has no existing ancestor")?;
        reject_reference(&fs::canonicalize(ancestor).map_err(|e| format!("Save ancestor: {e}"))?)?;
        fs::create_dir_all(profile_root).map_err(|e| format!("Create V2 profile root: {e}"))?;
        fs::canonicalize(profile_root).map_err(|e| format!("Save profile root: {e}"))?
    };
    reject_reference(&root)?;
    let directory = root.join("saves");
    if directory.exists() {
        reject_reference(
            &fs::canonicalize(&directory).map_err(|e| format!("Save directory: {e}"))?,
        )?;
    }
    fs::create_dir_all(&directory).map_err(|e| format!("Create V2 save directory: {e}"))?;
    let canonical = fs::canonicalize(&directory).map_err(|e| format!("Save directory: {e}"))?;
    reject_reference(&canonical)?;
    if !canonical.starts_with(&root) {
        return Err("V2 save directory escapes its profile root".into());
    }
    Ok(canonical)
}
/// Test and programmatic export helper: allocate a unique V2 slot without a
/// dialog name. Production Save Game uses `write_target` after confirmation.
#[cfg(test)]
pub fn write_new(profile_root: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    openmt2001::save::SaveFile::parse(bytes)?;
    let canonical = writable_directory(profile_root)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Save clock: {e}"))?
        .as_millis();
    for sequence in 0..1000u32 {
        let name = format!("V2-{stamp}-{sequence:03}");
        let temporary = canonical.join(format!(".{name}.tmp"));
        let destination = canonical.join(format!("{name}.sav"));
        if destination.exists() {
            continue;
        }
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Create V2 save: {e}")),
        };
        let result = (|| -> Result<(), String> {
            file.write_all(bytes)
                .map_err(|e| format!("Write V2 save: {e}"))?;
            file.sync_all().map_err(|e| format!("Flush V2 save: {e}"))?;
            drop(file);
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        match fs::hard_link(&temporary, &destination) {
            Ok(()) => {
                // The complete published slot is authoritative even if a
                // temporary-name cleanup fails after publication.
                if let Err(error) = fs::remove_file(&temporary) {
                    eprintln!("Published V2 save; temporary cleanup failed: {error}");
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    continue;
                }
                return Err(format!("Publish V2 save: {error}"));
            }
        }
        return Ok(destination);
    }
    Err("Unable to allocate a unique V2 save slot".into())
}

fn validate_stem(name: &str, max_bytes: usize) -> Result<(), String> {
    if name.is_empty()
        || name.len() > max_bytes
        || name == "."
        || name == ".."
        || name.ends_with(' ')
        || name.ends_with('.')
        || name
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
    {
        return Err("Save name contains unsupported filename characters".into());
    }
    let device = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    let device_bytes = device.as_bytes();
    if ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|reserved| device.eq_ignore_ascii_case(reserved))
        || (device_bytes.len() == 4
            && (device_bytes[..3].eq_ignore_ascii_case(b"COM")
                || device_bytes[..3].eq_ignore_ascii_case(b"LPT"))
            && device_bytes[3].is_ascii_digit()
            && device_bytes[3] != b'0')
    {
        return Err("Save name is reserved by the operating system".into());
    }
    Ok(())
}

/// Publish a dialog-selected name or replace one listed V2 slot. Native
/// 4bbe30 uses CreateFileA(CREATE_ALWAYS) after dialog acceptance; staging
/// first preserves the previous complete file when serialization or I/O fails.
pub fn write_target(profile_root: &Path, target: &Target, bytes: &[u8]) -> Result<PathBuf, String> {
    write_target_inner(profile_root, target, bytes, None)
}
pub(crate) fn write_target_from_snapshot(
    profile_root: &Path,
    target: &Target,
    bytes: &[u8],
    source: &camera_source::Reader,
    snapshot: &camera_source::Snapshot,
) -> Result<PathBuf, String> {
    write_target_inner(profile_root, target, bytes, Some((source, snapshot)))
}
fn write_target_inner(
    profile_root: &Path,
    target: &Target,
    bytes: &[u8],
    source: Option<(&camera_source::Reader, &camera_source::Snapshot)>,
) -> Result<PathBuf, String> {
    openmt2001::save::SaveFile::parse(bytes)?;
    let name = match target {
        Target::NewName(name) => name.as_str(),
        Target::ExistingSlot(id) => {
            let (stem, extension) = id
                .rsplit_once('.')
                .ok_or("Selected save slot has no .sav suffix")?;
            if !extension.eq_ignore_ascii_case("sav") {
                return Err("Selected save slot has no .sav suffix".into());
            }
            stem
        }
    };
    validate_stem(name, 120)?;
    let existing = list_slots(profile_root)?;
    let destination_id = match target {
        Target::NewName(_) => {
            if existing
                .iter()
                .any(|slot| slot.name.eq_ignore_ascii_case(name))
            {
                return Err("A saved game with this name already exists".into());
            }
            format!("{name}.sav")
        }
        Target::ExistingSlot(id) => {
            if !existing.iter().any(|slot| slot.id == id.as_str()) {
                return Err("Selected V2 save slot is no longer available".into());
            }
            id.clone()
        }
    };
    let directory = writable_directory(profile_root)?;
    let destination = directory.join(&destination_id);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Save clock: {e}"))?
        .as_nanos();
    for sequence in 0..100u32 {
        let temporary =
            directory.join(format!(".V2-{}-{nonce}-{sequence}.tmp", std::process::id()));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Create V2 save: {error}")),
        };
        let result = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Write V2 save: {error}"));
        }
        let publish = || {
            match target {
                Target::NewName(_) => fs::hard_link(&temporary, &destination),
                Target::ExistingSlot(_) => fs::rename(&temporary, &destination),
            }
            .map_err(|error| format!("Publish V2 save: {error}"))
        };
        let published = match source {
            Some((reader, snapshot)) => reader.with_live(snapshot, publish),
            None => publish(),
        };
        if let Err(error) = published {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if matches!(target, Target::NewName(_))
            && let Err(error) = fs::remove_file(&temporary)
        {
            eprintln!("Published V2 save; temporary cleanup failed: {error}");
        }
        return Ok(destination);
    }
    Err("Unable to allocate V2 save staging file".into())
}

/// Recoverable save attempts never become simulation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutosaveFailure {
    MissingSelectedProfile,
    UnsupportedCamera,
    UnsupportedState(String),
    Encoding(String),
    Storage(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutosaveStatus {
    Saved(PathBuf),
    Failed {
        destination: Option<PathBuf>,
        error: AutosaveFailure,
    },
}

/// The selected worker profile is bound before preparation. This temporary is
/// opened before native producer mutations and never truncates the old slot.
/// Only owned bytes and this destination cross the encode callback boundary.
pub struct PreparedAutosave {
    file: Option<fs::File>,
    temporary: PathBuf,
    destination: PathBuf,
    previous: Option<fs::Metadata>,
}
fn autosave_metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(metadata)),
        Ok(_) => Err("Autosave destination must be a regular file, not a link or directory".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Inspect autosave destination: {error}")),
    }
}
fn same_autosave_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev() || a.ino() != b.ino() {
            return false;
        }
    }
    a.len() == b.len() && a.modified().ok() == b.modified().ok()
}
impl PreparedAutosave {
    pub fn prepare(selected_profile_root: &Path) -> Result<Self, String> {
        // Reuse the manual publisher's reference/containment and directory
        // checks. Reject case collisions and nonregular fixed-slot entries.
        let directory = writable_directory(selected_profile_root)?;
        for entry in
            fs::read_dir(&directory).map_err(|e| format!("List autosave directory: {e}"))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            if let Some(name) = entry.file_name().to_str()
                && name.eq_ignore_ascii_case("autosave.sav")
                && name != "autosave.sav"
            {
                return Err("Autosave fixed slot has a case-colliding filename".into());
            }
        }
        let destination = directory.join("autosave.sav");
        let previous = autosave_metadata(&destination)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        for sequence in 0..100u32 {
            let temporary =
                directory.join(format!(".V2-{}-{nonce}-{sequence}.tmp", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(file),
                        temporary,
                        destination,
                        previous,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("Create V2 save: {error}")),
            }
        }
        Err("Unable to allocate V2 save staging file".into())
    }
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn publish(self, bytes: Vec<u8>) -> Result<PathBuf, String> {
        self.publish_inner(bytes, None)
    }
    pub(crate) fn publish_from_snapshot(
        self,
        bytes: Vec<u8>,
        source: &camera_source::Reader,
        snapshot: &camera_source::Snapshot,
    ) -> Result<PathBuf, String> {
        self.publish_inner(bytes, Some((source, snapshot)))
    }
    fn publish_inner(
        mut self,
        bytes: Vec<u8>,
        source: Option<(&camera_source::Reader, &camera_source::Snapshot)>,
    ) -> Result<PathBuf, String> {
        openmt2001::save::SaveFile::parse(&bytes)?;
        let mut file = self
            .file
            .take()
            .ok_or("Autosave staging file already closed")?;
        let write = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        write.map_err(|e| format!("Write V2 save: {e}"))?;
        let publish = || {
            let now = autosave_metadata(&self.destination)?;
            match (&self.previous, &now) {
                (None, None) => fs::hard_link(&self.temporary, &self.destination),
                (Some(before), Some(after)) if same_autosave_file(before, after) => {
                    fs::rename(&self.temporary, &self.destination)
                }
                _ => {
                    return Err("Autosave slot changed during capture; prior file preserved".into());
                }
            }
            .map_err(|e| format!("Publish V2 save: {e}"))
        };
        match source {
            Some((reader, snapshot)) => reader.with_live(snapshot, publish),
            None => publish(),
        }?;
        Ok(self.destination.clone())
    }
}
impl Drop for PreparedAutosave {
    fn drop(&mut self) {
        // Windows requires closing the writer before deleting the temporary.
        drop(self.file.take());
        match fs::remove_file(&self.temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("Autosave temporary cleanup failed: {error}"),
        }
    }
}

/// Called only after the Save Game panel's original 497c70 confirmation.
/// The opaque ID must still name a regular file in the writable V2 profile.
pub fn delete_slot(profile_root: &Path, id: &str) -> Result<(), String> {
    if !list_slots(profile_root)?.iter().any(|slot| slot.id == id) {
        return Err("Selected V2 save slot is no longer available".into());
    }
    let path = writable_directory(profile_root)?.join(id);
    if !fs::symlink_metadata(&path)
        .map_err(|e| format!("Inspect V2 save: {e}"))?
        .file_type()
        .is_file()
    {
        return Err("Selected V2 save slot is no longer a regular file".into());
    }
    fs::remove_file(&path).map_err(|e| format!("Delete V2 save: {e}"))
}

/// Preserve untouched native camera bytes when available; replace only the
/// recovered view triples 4c/58/64 with the live retained frontend pose.
pub fn cameras(
    prior: Option<[[u8; 196]; 2]>,
    camera: openmt2001::flyby::CameraSample,
) -> Result<[[u8; 196]; 2], String> {
    if camera
        .eye
        .into_iter()
        .chain(camera.target)
        .chain(camera.up)
        .any(|v| !v.is_finite())
        || camera.up == [0.; 3]
    {
        return Err("Cannot save an invalid camera pose".into());
    }
    let mut rows = prior.unwrap_or([[0; 196]; 2]);
    for row in &mut rows {
        for (at, values) in [(0x4c, camera.eye), (0x58, camera.target), (0x64, camera.up)] {
            for (i, value) in values.into_iter().enumerate() {
                row[at + i * 4..at + i * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_names_use_uppercase_ordinal_order_with_bytewise_ties() {
        let mut names = ["cherry", "apple", "Banana", "_x", "1a", "APPLE", "Apple"];
        names.sort_by(|a, b| compare_save_names(a, b));
        assert_eq!(
            names,
            ["1a", "APPLE", "Apple", "apple", "Banana", "cherry", "_x"]
        );
        assert!(compare_save_names("éclair", "Êclair").is_lt());
        assert!(compare_save_names("éclair", "Éclair").is_gt());
        assert!(
            compare_save_names("ß", "SS").is_gt(),
            "do not expand uppercase"
        );
        assert!(
            compare_save_names("\u{10000}", "\u{e000}").is_lt(),
            "UTF-16 ordinal order"
        );
    }

    #[test]
    fn selected_profile_progress_identity_roundtrips_without_save_path_authority() {
        let root = Path::new("work/identity-fixture");
        for name in [b"Alice".as_slice(), b"B ob", b"\xff"] {
            let selected = selected_profile_root(root, name);
            assert_eq!(
                progress_for_profile_root(root, &selected),
                Some(crate::campaign_store::profile_path(root, name))
            );
        }
        for invalid in [
            "profiles",
            "profiles/",
            "profiles/zz",
            "profiles/a",
            "profiles/41/saves",
            "profiles/41/../42",
            "elsewhere/41",
        ] {
            assert!(
                progress_for_profile_root(root, &root.join(invalid)).is_none(),
                "{invalid}"
            );
        }
        assert!(progress_for_profile_root(root, Path::new("work/other/profiles/41")).is_none());
    }
    const TERMINATOR: [u8; 8] = [255, 255, 255, 255, 0, 0, 0, 0];
    #[test]
    fn selected_profiles_isolate_save_overwrite_delete_and_legacy_rows() {
        let app = std::env::temp_dir().join(format!(
            "omt2-profile-saves-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&app).unwrap();
        let alice = selected_profile_root(&app, b"Alice");
        let bob = selected_profile_root(&app, b"Bob");
        assert_ne!(alice, bob);
        let native = |cash: u32| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&0x2000u32.to_le_bytes());
            bytes.extend_from_slice(&16u32.to_le_bytes());
            for word in [7u32, 12, cash, 3] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes.extend_from_slice(&TERMINATOR);
            bytes
        };
        let alice_path =
            write_target(&alice, &Target::NewName("Shared".into()), &native(10)).unwrap();
        let bob_path = write_target(&bob, &Target::NewName("Shared".into()), &native(20)).unwrap();
        assert_eq!(list_slots(&alice).unwrap()[0].metadata, Ok([7, 12, 10, 3]));
        assert_eq!(list_slots(&bob).unwrap()[0].metadata, Ok([7, 12, 20, 3]));
        write_target(
            &alice,
            &Target::ExistingSlot("Shared.sav".into()),
            &native(30),
        )
        .unwrap();
        assert_eq!(list_slots(&alice).unwrap()[0].metadata, Ok([7, 12, 30, 3]));
        assert_eq!(list_slots(&bob).unwrap()[0].metadata, Ok([7, 12, 20, 3]));
        delete_slot(&bob, "Shared.sav").unwrap();
        assert!(alice_path.exists() && !bob_path.exists());
        assert!(delete_slot(&bob, "Shared.sav").is_err());

        // A shared-directory save from an earlier V2 release remains intact,
        // visible only for the owner recorded in its extension and load-only.
        let legacy_directory = app.join("saves");
        fs::create_dir_all(&legacy_directory).unwrap();
        let mut legacy = native(40);
        let owner = crate::campaign_store::profile_path(&app, b"Alice")
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let records = (1..=13)
            .map(|tag| {
                (
                    tag,
                    if tag == 1 {
                        owner.as_bytes().to_vec()
                    } else {
                        Vec::new()
                    },
                )
            })
            .collect::<Vec<_>>();
        openmt2001::save::extension::append(&mut legacy, &records).unwrap();
        let legacy_path = legacy_directory.join("Earlier.sav");
        fs::write(&legacy_path, &legacy).unwrap();
        assert_eq!(
            matching_legacy_paths(&app, &owner).unwrap(),
            vec![legacy_path.clone()]
        );
        let bob_owner = crate::campaign_store::profile_path(&app, b"Bob")
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(matching_legacy_paths(&app, &bob_owner).unwrap().is_empty());
        assert!(delete_slot(&alice, "Earlier.sav").is_err());
        assert_eq!(fs::read(&legacy_path).unwrap(), legacy);
        let bob_again = write_target(&bob, &Target::NewName("Keep".into()), &native(50)).unwrap();
        let mut bob_legacy = native(60);
        let bob_records = (1..=13)
            .map(|tag| {
                (
                    tag,
                    if tag == 1 {
                        bob_owner.as_bytes().to_vec()
                    } else {
                        Vec::new()
                    },
                )
            })
            .collect::<Vec<_>>();
        openmt2001::save::extension::append(&mut bob_legacy, &bob_records).unwrap();
        let bob_legacy_path = legacy_directory.join("Bob earlier.sav");
        fs::write(&bob_legacy_path, &bob_legacy).unwrap();
        let unattributed = legacy_directory.join("Unattributed.sav");
        fs::write(&unattributed, native(70)).unwrap();
        clear_profile_saves(&app, b"Alice").unwrap();
        assert!(!alice_path.exists() && !legacy_path.exists());
        assert!(bob_again.exists() && bob_legacy_path.exists() && unattributed.exists());
        assert!(list_slots(&alice).unwrap().is_empty());
        assert!(matching_legacy_paths(&app, &owner).unwrap().is_empty());
        // Recreating the same name begins with no stale slot identity.
        write_target(&alice, &Target::NewName("New".into()), &native(80)).unwrap();
        assert_eq!(list_slots(&alice).unwrap().len(), 1);
        fs::remove_dir_all(app).unwrap();
    }
    #[test]
    fn reference_roots_are_rejected_before_creating_files() {
        for root in [
            "Original Distribution/saves",
            "original game evidence/nested",
        ] {
            assert!(
                write_new(Path::new(root), &TERMINATOR)
                    .unwrap_err()
                    .contains("immutable")
            );
        }
    }
    #[test]
    fn publication_produces_complete_distinct_loadable_slots() {
        let root = std::env::temp_dir().join(format!("omt2-save-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let first = write_new(&root, &TERMINATOR).unwrap();
        let second = write_new(&root, &TERMINATOR).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), TERMINATOR);
        assert_eq!(fs::read(&second).unwrap(), TERMINATOR);
        fs::remove_file(first).unwrap();
        fs::remove_file(second).unwrap();
        let _ = fs::remove_dir(root.join("saves"));
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn save_dialog_lists_only_profile_slots_with_metadata() {
        let root = std::env::temp_dir().join(format!(
            "omt2-save-list-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x2000u32.to_le_bytes());
        bytes.extend_from_slice(&16u32.to_le_bytes());
        for word in [7u32, 12, 120.5f32.to_bits(), 3] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&TERMINATOR);
        let path = write_new(&root, &bytes).unwrap();
        fs::write(root.join("saves").join("notes.txt"), b"not a save").unwrap();
        let slots = list_slots(&root).unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(
            slots[0].id,
            path.file_name().unwrap().to_string_lossy().to_string()
        );
        assert_eq!(
            slots[0].name,
            path.file_stem().unwrap().to_string_lossy().to_string()
        );
        assert_eq!(slots[0].metadata, Ok([7, 12, 120.5f32.to_bits(), 3]));
        assert!(slots[0].modified.is_some());
        assert!(
            list_slots(Path::new("Original Distribution/saves"))
                .unwrap_err()
                .contains("immutable")
        );
        assert!(
            delete_slot(Path::new("Original Distribution/saves"), &slots[0].id)
                .unwrap_err()
                .contains("immutable")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn named_save_requires_explicit_existing_slot_for_atomic_replace() {
        let root = std::env::temp_dir().join(format!(
            "omt2-save-named-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let with_cash = |cash: u32| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&0x2000u32.to_le_bytes());
            bytes.extend_from_slice(&16u32.to_le_bytes());
            for word in [7u32, 12, cash, 3] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes.extend_from_slice(&TERMINATOR);
            bytes
        };
        let first = with_cash(120.5f32.to_bits());
        let revised = with_cash(97.25f32.to_bits());
        let path = write_target(
            &root,
            &Target::NewName("test1-inprogressgame".into()),
            &first,
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), first);
        assert!(
            write_target(
                &root,
                &Target::NewName("test1-inprogressgame".into()),
                &revised
            )
            .unwrap_err()
            .contains("already exists")
        );
        assert_eq!(fs::read(&path).unwrap(), first);
        assert!(
            write_target(
                &root,
                &Target::ExistingSlot("../test1-inprogressgame.sav".into()),
                &revised
            )
            .is_err()
        );
        assert!(write_target(&root, &Target::NewName("CON".into()), &revised).is_err());
        assert!(write_target(&root, &Target::NewName("x".repeat(121)), &revised).is_err());
        assert_eq!(fs::read(&path).unwrap(), first);
        let id = list_slots(&root).unwrap().remove(0).id;
        assert_eq!(
            write_target(&root, &Target::ExistingSlot(id), &revised).unwrap(),
            path
        );
        assert_eq!(fs::read(&path).unwrap(), revised);
        assert_eq!(
            list_slots(&root).unwrap()[0].metadata,
            Ok([7, 12, 97.25f32.to_bits(), 3])
        );
        assert!(delete_slot(&root, "../test1-inprogressgame.sav").is_err());
        assert_eq!(fs::read(&path).unwrap(), revised);
        delete_slot(&root, "test1-inprogressgame.sav").unwrap();
        assert!(list_slots(&root).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn original_save_list_clock_changes_zero_to_twelve_but_keeps_noon_suffix() {
        assert_eq!(
            original_time_label(0, 5, "2026-06-21"),
            "12:05am 2026-06-21"
        );
        assert_eq!(
            original_time_label(12, 5, "2026-06-21"),
            "12:05am 2026-06-21"
        );
        assert_eq!(
            original_time_label(17, 21, "2026-06-21"),
            "5:21pm 2026-06-21"
        );
        assert_eq!(
            original_time_label(23, 59, "2026-06-21"),
            "11:59pm 2026-06-21"
        );
    }

    #[test]
    fn short_dates_follow_locale_order_separator_and_padding() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        for (locale, expected) in [
            ("en_US.UTF-8", "6/9/2026"),
            ("en-GB", "09/06/2026"),
            ("de_DE.utf8@euro", "09.06.2026"),
            ("fr_FR", "09/06/2026"),
            ("fr-CH", "09.06.2026"),
            ("nl_NL", "09-06-2026"),
            ("ja_JP", "2026/06/09"),
            ("fr_CA", "2026-06-09"),
            ("C", "2026-06-09"),
            ("C.UTF-8", "2026-06-09"),
            ("POSIX", "2026-06-09"),
            ("unknown", "2026-06-09"),
        ] {
            assert_eq!(short_date_for_locale(date, locale), expected, "{locale}");
        }
    }

    #[test]
    fn date_locale_obeys_environment_precedence_and_ignores_empty_values() {
        let resolve = |values: [Option<&str>; 3]| {
            date_locale(|key| {
                let index = ["LC_ALL", "LC_TIME", "LANG"]
                    .iter()
                    .position(|candidate| *candidate == key)
                    .unwrap();
                values[index].map(str::to_owned)
            })
        };
        assert_eq!(
            resolve([Some("C"), Some("de_DE"), Some("en_US")]),
            Some("C".into())
        );
        assert_eq!(
            resolve([Some(""), Some("de_DE"), Some("en_US")]),
            Some("de_DE".into())
        );
        assert_eq!(
            resolve([None, Some(""), Some("en_US")]),
            Some("en_US".into())
        );
        assert_eq!(resolve([None, None, None]), None);
    }

    #[test]
    fn save_timestamp_keeps_utc_date_when_local_time_is_on_the_previous_day() {
        let utc = chrono::DateTime::parse_from_rfc3339("2026-06-22T01:21:00Z").unwrap();
        let local = utc.with_timezone(&chrono::FixedOffset::west_opt(4 * 3600).unwrap());
        assert_eq!(
            original_modified_label(local, |date| short_date_for_locale(date, "en_US")),
            "9:21pm 6/22/2026"
        );
    }

    #[test]
    fn save_timestamp_keeps_utc_date_when_local_midnight_is_on_the_next_day() {
        let utc = chrono::DateTime::parse_from_rfc3339("2026-06-21T22:05:00Z").unwrap();
        let local = utc.with_timezone(&chrono::FixedOffset::east_opt(2 * 3600).unwrap());
        assert_eq!(
            original_modified_label(local, |date| short_date_for_locale(date, "de_DE")),
            "12:05am 21.06.2026"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_short_date_uses_the_system_formatter() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        assert!(windows_short_date(date).is_some_and(|label| !label.is_empty()));
    }
}

#[cfg(test)]
#[path = "session_autosave_writer_test.rs"]
mod autosave_writer_tests;
