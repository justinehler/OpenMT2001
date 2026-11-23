//! Supplied custom content stays in the ordinary authored single-player VM.
//! Files are buffered before execution; Lua receives no filesystem callback.
use openmt2001::{level_catalog::Level, lua4::Runtime};
use std::{fs, path::{Component, Path, PathBuf}};

pub fn valid_level(level: &Level) -> bool {
    level.bytes[2] == 1 && level.single_player() && !level.tutorial()
        && level.id() / 100000 == 5
        && std::str::from_utf8(level.directory()).is_ok_and(|name| {
            let name = name.replace('\\', "/");
            let parts = Path::new(&name).components().collect::<Vec<_>>();
            parts.len() >= 3 && parts.iter().all(|p| matches!(p, Component::Normal(_)))
                && parts.first().is_some_and(|p| p.as_os_str().eq_ignore_ascii_case("custom"))
        })
}

pub fn same_directory(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(&a, &b)| {
        let normalize = |c: u8| if c == b'\\' { b'/' } else { c.to_ascii_lowercase() };
        normalize(a) == normalize(b)
    })
}

fn contained(root: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let Some(path) = crate::campaign_store::original_path(root, name)? else { return Ok(None) };
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("custom script leaves the supplied scripts directory".into());
    }
    Ok(Some(path))
}

pub fn read_optional(root: &Path, relative: &str) -> Result<Option<Vec<u8>>, String> {
    contained(root, relative)?.map(|p| fs::read(&p).map_err(|e| format!("{}: {e}", p.display())))
        .transpose()
}

pub fn read(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    read_optional(root, relative)?.ok_or_else(|| format!("missing supplied custom script {relative}"))
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
        let mut entries = fs::read_dir(&folder).map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        entries.retain(|p| p.extension().is_some_and(|s| s.eq_ignore_ascii_case("lua")));
        if entries.len() > maximum { return Err("custom script bundle exceeds file bound".into()); }
        entries.sort_by_key(|p| p.file_name().unwrap().to_string_lossy().to_ascii_lowercase());
        for pair in entries.windows(2) {
            if pair[0].file_name().unwrap().eq_ignore_ascii_case(pair[1].file_name().unwrap()) {
                return Err("ambiguous custom script filename case".into());
            }
        }
        for path in entries {
            let relative = path.strip_prefix(root).map_err(|_| "custom bundle path outside scripts")?
                .to_str().ok_or("custom script path is not UTF-8")?.replace('\\', "/");
            let checked = contained(root, &relative)?.ok_or("custom script disappeared")?;
            if !checked.is_file() { return Err("custom script entry is not a file".into()); }
            let size = fs::metadata(&checked).map_err(|e| e.to_string())?.len();
            if size > 2 * 1024 * 1024 { return Err("custom script exceeds source bound".into()); }
            let bytes = fs::read(&checked).map_err(|e| e.to_string())?;
            total = total.checked_add(bytes.len()).ok_or("custom bundle size overflow")?;
            if total > 8 * 1024 * 1024 { return Err("custom script bundle exceeds byte bound".into()); }
            runtime.script_buffer(&format!("scripts/{relative}"), &bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::level_catalog::{Catalog, Labels, Registration};

    #[test]
    fn custom_identity_requires_registered_family_and_nested_relative_directory() {
        let make = |id, name: &[u8]| {
            let mut c = Catalog::default();
            c.register(Registration { id, directory: name.to_vec(), checksum: 0,
                labels: Labels::Literal { title: vec![], description: vec![] } }, false, true).unwrap();
            c.levels.remove(0)
        };
        assert!(valid_level(&make(500000, b"custom\\deepred\\Jon001")));
        for (id, name) in [(200003, b"custom/deepred/Jon001".as_slice()),
            (500000, b"custom/../Jon001"), (500000, b"custom/Jon001"),
            (500000, b"/custom/deepred/Jon001"), (500000, b"other/deepred/Jon001")] {
            assert!(!valid_level(&make(id, name)));
        }
        assert!(same_directory(b"custom\\deepred\\Jon001", b"CUSTOM/deepred/JON001"));
        assert!(!same_directory(b"custom/x", b"custom/xy"));
    }

    #[test]
    #[ignore = "requires supplied originals; authored custom500000 and cold Host continuation"]
    fn original_custom500000_cold_reload_continuation() {
        std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
            use super::super::{Inputs, Policy, Session, CommandAssets, character_names, load_era_styles};
            let data = |key| fs::read(std::env::var(key).unwrap()).unwrap();
            let executable = data("OPENMT2001_EXE");
            let directory = data("OPENMT2001_ARCHIVE_DIR");
            let archive = data("OPENMT2001_ARCHIVE_BIN");
            let build = data("OPENMT2001_BUILD_FILE");
            let parameters = data("OPENMT2001_PARAMETERS");
            let timetable = data("OPENMT2001_TIMETABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let default = PathBuf::from(std::env::var("OPENMT2001_DEFAULT_SETTINGS").unwrap());
            let scripts = default.parent().unwrap().parent().unwrap();
            let exe = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let language = exe.parent().unwrap().parent().unwrap().join("US/language/usa");
            let names = character_names(&image, &language).unwrap();
            let parameter_path = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let inputs = || Inputs {
                tree_locators: None, image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: data("OPENMT2001_ROAD_ROUTE_TABLE"), scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                era_styles: load_era_styles(parameter_path.parent().unwrap()).unwrap(),
                scenario: "CUSTOM/deepred/Jon001", difficulty: 1, selected_character: 4,
                profile_name: b"Custom Fixture", character_names: &names,
            };
            let policy = || Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] };
            let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
            let style = openmt2001::board_style::Style::from_v14(&image).unwrap();
            let mut session = Session::initialize_custom_with_style(inputs(), policy(), catalog,
                &style, crate::compatibility::Mode::Modern).unwrap();
            assert_eq!(session.catalog.selected_level().unwrap().id(), 500000);
            assert_eq!(session.world.objective.kind, 4);
            assert_eq!(session.world.objective.target, 2000.);
            assert_eq!(session.world.player_money, 6500.);
            assert_eq!(session.roster.manager.players.len(), 6);
            assert_eq!(session.roster.status.ai_count, 5);
            assert!(session.campaign_profile.is_none());
            assert!(session.network_command_timer.is_none());
            assert!(session.world.ledgers.residents.active_count > 0);
            let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
            let mut tokens = std::collections::BTreeSet::new();
            for (slot, row) in session.saved_player_metadata[..6].iter().enumerate() {
                assert!(tokens.insert(word(row, 0)), "authored token collision unresolved");
                assert_eq!(word(row, 4), u32::from(slot != 0));
                if slot == 0 { assert_eq!(word(row, 0), 4); }
                else { assert_eq!(word(row, 44), [1, 1, 2, 1, 2][slot-1]); }
            }
            // Original Medium preferences address these map-created business slots.
            for (business, owner) in [(1,4), (6,4), (12,4), (38,5), (39,5), (43,6),
                (44,6), (58,3), (60,3), (63,3), (88,2), (89,2), (93,2)] {
                let record = &session.world.ledgers.businesses[business];
                let building = i16::from_le_bytes(record[8..10].try_into().unwrap()) as usize;
                assert_eq!(session.world.buildings[building][0x2d], owner);
            }
            let assets = CommandAssets { image: &image, board: &board, file: &file };
            let camera = openmt2001::flyby::CameraSample { eye: [100.,120.,100.],
                target: [0.,0.,0.], up: [0.,1.,0.], vertical_fov: 0.7 };
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            let mut host = crate::live_host::Host::new(camera.eye); host.running = true;
            let initial = session.world.clock.timing[0];
            for _ in 0..30 {
                host.tick(&mut session, &assets);
                assert!(host.failure.is_none(), "{:?}", host.failure);
            }
            assert!(session.world.clock.timing[0] > initial);
            let bytes = session.save_v2(&image, cameras, 1).unwrap();
            let saved = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            assert_eq!(saved.level_metadata().unwrap()[0], 500000);
            assert_eq!(openmt2001::save::extension::Extension::parse(&saved).unwrap().unwrap().version, 4);
            let mut resumed = Session::load_original(inputs(), policy(), &bytes).unwrap();
            let metadata = resumed.restore_v2_extension_with_image(&saved, &image).unwrap().unwrap();
            assert!(metadata.profile_filename.is_none());
            assert!(resumed.campaign_profile.is_none());
            assert_eq!(&resumed.saved_player_metadata[..6], &session.saved_player_metadata[..6]);
            host = crate::live_host::Host::new(camera.eye); host.running = true;
            let mut other = crate::live_host::Host::new(camera.eye); other.running = true;
            let before = resumed.world.clock.timing[0];
            for _ in 0..60 {
                host.tick(&mut session, &assets); other.tick(&mut resumed, &assets);
                assert!(host.failure.is_none(), "{:?}", host.failure);
                assert!(other.failure.is_none(), "{:?}", other.failure);
                assert_eq!(session.world.clock, resumed.world.clock);
                assert_eq!(session.world.ledgers.businesses, resumed.world.ledgers.businesses);
                assert_eq!(session.world.pending_world.to_bytes(), resumed.world.pending_world.to_bytes());
                assert_eq!(session.roster.profiles, resumed.roster.profiles);
                assert_eq!(session.script_random_state, resumed.script_random_state);
            }
            assert!(resumed.world.clock.timing[0] > before);
            assert_eq!(session.save_v2(&image, cameras, 1).unwrap(), resumed.save_v2(&image, cameras, 1).unwrap());
        }).unwrap().join().unwrap();
    }
}
