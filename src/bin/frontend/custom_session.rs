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

    #[test]
    #[ignore = "requires supplied patch Essential originals; bounded 8 by 3 startup matrix"]
    fn original_custom_all_eight_three_difficulties_startup_matrix() {
        std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
            use super::super::{Inputs, Policy, Session, character_names, load_era_styles};
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
            // Require the patched supplied root, never the installer scripts tree.
            assert!(scripts.join("CUSTOM/deepred/Jon001/hub.lua").is_file());
            assert!(scripts.join("MAPS/jonmap002.lua").is_file());
            let exe = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let language = exe.parent().unwrap().parent().unwrap().join("US/language/usa");
            let names = character_names(&image, &language).unwrap();
            let parameter_path = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let inputs = |scenario, difficulty, selected_character| Inputs {
                tree_locators: None, image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                road_routes: data("OPENMT2001_ROAD_ROUTE_TABLE"), scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                era_styles: load_era_styles(parameter_path.parent().unwrap()).unwrap(),
                scenario, difficulty, selected_character,
                profile_name: b"Custom Fixture", character_names: &names,
            };
            // Fixed injected test seed; no claim this is the native UI's sampled seed.
            let style = openmt2001::board_style::Style::from_v14(&image).unwrap();
            // Literal expectations transcribed from pinned supplied scripts, not VM output.
            type Case = (u32, &'static str, u32, i32, f32, u32, f32,
                &'static [(usize, u8)], &'static [(u32, u32)]);
            let cases: &[Case] = &[
                (500000, "custom/deepred/Jon001", 0, 4, 2000.0, 1930, 8000.0, &[(1,4),(38,5),(39,5),(43,6),(44,6),(58,3),(88,2)], &[(2,0),(1,0),(8,1),(7,0),(9,1)]),
                (500000, "custom/deepred/Jon001", 1, 4, 2000.0, 1930, 6500.0, &[(1,4),(6,4),(12,4),(38,5),(39,5),(43,6),(44,6),(58,3),(60,3),(63,3),(88,2),(89,2),(93,2)], &[(2,1),(1,1),(8,2),(7,1),(5,2)]),
                (500000, "custom/deepred/Jon001", 2, 4, 2000.0, 1930, 5000.0, &[(1,4),(6,4),(12,4),(13,4),(17,4),(38,5),(39,5),(40,5),(41,5),(42,5),(43,6),(44,6),(45,6),(58,3),(60,3),(63,3),(66,3),(67,3),(88,2),(89,2),(93,2),(96,2),(102,2)], &[(2,2),(1,2),(8,2),(7,2),(3,2)]),
                (500001, "custom/deepred/Jon002", 0, 5, 40000.0, 1930, 8000.0, &[(0,2),(3,6),(11,3),(13,5),(16,5),(19,3),(26,4),(29,4),(40,2),(45,6)], &[(2,0),(1,0),(8,1),(7,0),(9,1)]),
                (500001, "custom/deepred/Jon002", 1, 5, 40000.0, 1930, 6500.0, &[(0,2),(1,2),(3,6),(4,6),(11,3),(12,3),(13,5),(14,5),(16,5),(17,5),(19,3),(20,3),(26,4),(27,4),(29,4),(31,4),(40,2),(41,2),(45,6),(46,6)], &[(2,1),(1,1),(8,2),(7,1),(5,2)]),
                (500001, "custom/deepred/Jon002", 2, 5, 40000.0, 1930, 5000.0, &[(0,2),(1,2),(2,2),(3,6),(4,6),(5,6),(11,3),(12,3),(13,5),(14,5),(15,5),(16,5),(17,5),(18,5),(19,3),(20,3),(23,3),(25,3),(26,4),(27,4),(28,4),(29,4),(31,4),(33,4),(40,2),(41,2),(42,2),(45,6),(46,6),(47,6)], &[(2,2),(1,2),(8,2),(7,2),(3,2)]),
                (500002, "custom/deepred/Jon003", 0, 5, 70000.0, 1980, 8000.0, &[], &[(2,0),(1,0),(8,1),(7,0),(9,1)]),
                (500002, "custom/deepred/Jon003", 1, 5, 70000.0, 1980, 6500.0, &[], &[(2,1),(1,1),(8,2),(7,1),(5,2)]),
                (500002, "custom/deepred/Jon003", 2, 5, 70000.0, 1980, 5000.0, &[], &[(2,2),(1,2),(8,2),(7,2),(3,2)]),
                (500003, "custom/deepred/Jon004", 0, 4, 2000.0, 1960, 8000.0, &[], &[(4,0),(1,0),(8,1),(7,0),(9,1)]),
                (500003, "custom/deepred/Jon004", 1, 4, 2000.0, 1960, 6500.0, &[], &[(4,1),(1,1),(8,2),(7,1),(5,2)]),
                (500003, "custom/deepred/Jon004", 2, 4, 2000.0, 1960, 5000.0, &[], &[(4,2),(1,2),(8,2),(7,2),(3,2)]),
                (500004, "custom/deepred/Jon005", 0, 4, 2000.0, 1930, 8000.0, &[(0,2),(2,2),(3,3),(6,3),(14,4),(21,5),(22,5),(29,6),(30,6)], &[(4,0),(1,0),(8,1),(7,0),(9,1)]),
                (500004, "custom/deepred/Jon005", 1, 4, 2000.0, 1930, 6500.0, &[(0,2),(1,3),(2,2),(3,3),(4,2),(5,3),(6,3),(7,2),(14,4),(15,4),(16,4),(21,5),(22,5),(23,5),(29,6),(30,6),(32,6),(33,6)], &[(4,1),(1,1),(8,2),(7,1),(5,2)]),
                (500004, "custom/deepred/Jon005", 2, 4, 2000.0, 1930, 5000.0, &[(0,2),(1,3),(2,2),(3,3),(4,2),(5,3),(6,3),(7,2),(8,2),(9,3),(10,2),(11,3),(12,2),(13,2),(14,4),(15,4),(16,4),(17,4),(18,4),(19,4),(20,4),(21,5),(22,5),(23,5),(24,5),(25,5),(29,6),(30,6),(32,6),(33,6),(35,6),(36,6),(45,6)], &[(4,2),(1,2),(8,2),(7,2),(3,2)]),
                (500005, "custom/deepred/Jon006", 0, 4, 2000.0, 1930, 8000.0, &[], &[(4,0),(1,0),(8,1),(7,0),(9,1)]),
                (500005, "custom/deepred/Jon006", 1, 4, 2000.0, 1930, 6500.0, &[(0,2),(2,2),(3,3),(6,3),(14,4),(21,5),(22,5),(29,6),(30,6)], &[(5,1),(1,1),(8,2),(7,1),(3,2)]),
                (500005, "custom/deepred/Jon006", 2, 4, 2000.0, 1930, 5000.0, &[(0,2),(1,3),(2,2),(3,3),(4,2),(5,3),(6,3),(7,2),(14,4),(15,4),(16,4),(21,5),(22,5),(23,5),(29,6),(30,6),(32,6),(33,6)], &[(5,2),(1,2),(8,2),(0,2),(3,2)]),
                (500006, "custom/deepred/Jon007", 0, 1, 3.0, 1930, 8000.0, &[], &[(4,0),(1,0),(8,1),(7,0),(9,1)]),
                (500006, "custom/deepred/Jon007", 1, 1, 3.0, 1930, 6500.0, &[], &[(5,1),(1,1),(8,2),(7,1),(3,2)]),
                (500006, "custom/deepred/Jon007", 2, 1, 3.0, 1930, 5000.0, &[], &[(5,2),(1,2),(8,2),(0,2),(3,2)]),
                (500007, "custom/deepred/Jon008", 0, 1, 4.0, 1930, 8000.0, &[(31,1),(11,6),(26,3),(21,4),(38,5)], &[(4,0),(6,0),(8,1),(5,0),(9,1)]),
                (500007, "custom/deepred/Jon008", 1, 1, 4.0, 1930, 6500.0, &[(36,1),(13,2),(12,2),(34,3),(20,3),(27,4),(29,4),(15,5),(16,5),(17,6),(18,6)], &[(4,1),(1,1),(8,2),(7,1),(5,2)]),
                (500007, "custom/deepred/Jon008", 2, 1, 4.0, 1930, 5000.0, &[(58,2),(59,2),(60,2),(61,2),(62,2),(63,2),(64,2),(65,2),(66,2),(67,2),(68,2),(69,2),(70,2),(15,3),(16,3),(17,3),(18,3),(19,3),(20,3),(21,3),(22,3),(23,3),(24,3),(75,4),(76,4),(77,4),(78,4),(79,4),(80,4),(81,4),(82,4),(83,4),(84,4),(85,4),(86,4),(50,5),(51,5),(52,5),(53,5),(54,5),(36,6),(37,6),(38,6),(39,6),(40,6),(41,6),(42,6),(43,6),(44,6)], &[(5,2),(1,2),(8,2),(7,2),(3,2)]),
            ];
            assert_eq!(cases.len(), 24);
            for &(id, scenario, difficulty, kind, target, year, cash, owners, ai) in cases {
                eprintln!("CUSTOM_STARTUP_BEGIN id={id} difficulty={difficulty}");
                let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
                // 500007 Easy already assigns Iron to an AI. Choose a free human
                // token for this matrix; the separate500000 continuation covers collisions.
                let selected_character = if id == 500007 && difficulty == 0 { 0 } else { 6 };
                let session = Session::initialize_custom_with_style(inputs(scenario, i32::try_from(difficulty).unwrap(), selected_character),
                    Policy { seed: 0x12345678, auxiliary_prior_bytes: [0; 0x11c] },
                    catalog, &style, crate::compatibility::Mode::Modern)
                    .unwrap_or_else(|error| panic!("{id}/{difficulty}: {error}"));
                let selected = session.catalog.selected_level().unwrap();
                assert!(valid_level(selected));
                assert_eq!(selected.id(), id);
                assert!(same_directory(selected.directory(), scenario.as_bytes()));
                let (title, description_len, description_fingerprint) = match id {
                    500000 => ("Empire Catch Up!".as_bytes(), 245usize, 0x8eb9c849e6210fc3u64),
                    500001 => ("State Run Utilities.".as_bytes(), 248usize, 0x9e55909d915e51ceu64),
                    500002 => ("You Got It All".as_bytes(), 199usize, 0x9de77d0db7c50407u64),
                    500003 => ("Swinging Sixties".as_bytes(), 128usize, 0x005df3f03198b59du64),
                    500004 => ("Land of Riches".as_bytes(), 251usize, 0x1a97a5720478e282u64),
                    500005 => ("Block Release".as_bytes(), 329usize, 0x535a007c31bffa31u64),
                    500006 => ("The Campaign Trail".as_bytes(), 236usize, 0x8616c716d566208eu64),
                    500007 => ("Reign of the Outsider".as_bytes(), 396usize, 0x19a037c278e5a89du64),
                    _ => unreachable!(),
                };
                match selected.labels() {
                    Labels::Literal { title: actual_title, description: actual_description } => {
                        assert_eq!(actual_title.as_slice(), title);
                        assert_eq!(actual_description.len(), description_len);
                        let fingerprint = actual_description.iter().fold(0xcbf29ce484222325u64,
                            |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3));
                        assert_eq!(fingerprint, description_fingerprint);
                    }
                    _ => panic!("{id}: expected authored literal catalog labels"),
                }
                assert_eq!(session.catalog.difficulty, difficulty);
                assert_eq!(session.world.objective.kind, kind);
                assert_eq!(session.world.objective.target, target);
                assert_eq!(1930 + 10 * session.world.clock.calendar[2] + session.world.clock.calendar[1], year);
                assert_eq!(session.world.clock.hour(), 9);
                assert_eq!(session.world.clock.timing[0], 0);
                assert!(session.world.player_money.is_finite());
                assert_eq!(session.world.player_money, cash);
                assert!(session.world.ledgers.residents.active_count > 0);
                assert_eq!(session.roster.manager.players.len(), 6);
                assert_eq!(session.roster.status.ai_count, 5);
                let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
                let mut unique = std::collections::BTreeSet::new();
                for (slot, row) in session.saved_player_metadata[..6].iter().enumerate() {
                    assert!(unique.insert(word(row, 0)), "{id}/{difficulty}: duplicate token");
                    assert_eq!(word(row, 4), u32::from(slot != 0));
                    if slot == 0 { assert_eq!(word(row, 0), selected_character as u32); }
                    else {
                        assert_eq!(word(row, 0), ai[slot-1].0);
                        assert_eq!(word(row, 44), ai[slot-1].1);
                    }
                }
                for &(business, owner) in owners {
                    let record = &session.world.ledgers.businesses[business];
                    let building = i16::from_le_bytes(record[8..10].try_into().unwrap());
                    assert!(building >= 0, "{id}/{difficulty}: business {business} missing building");
                    assert_eq!(session.world.buildings[building as usize][0x2d], owner,
                        "{id}/{difficulty}: authored business {business}");
                }
                assert!(session.campaign_profile.is_none());
                assert!(session.network_command_timer.is_none());
                drop(session); // Exactly one live Session; no advancing ticks or outcome calls.
                eprintln!("CUSTOM_STARTUP_END id={id} difficulty={difficulty}");
            }
        }).unwrap().join().unwrap();
    }
}
