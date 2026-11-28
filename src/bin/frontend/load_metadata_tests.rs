use super::*;

#[path = "../../../tests/support/save_metadata.rs"]
mod fixtures;

#[test]
fn load_panel_entries_read_metadata_independently_of_world_payloads() {
    let root = openmt2001::fixtures::scratch("LOAD_METADATA_PANEL").join(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("saves")).unwrap();
    let results = fixtures::cases()
        .into_iter()
        .map(|case| {
            case.check_full_parse();
            let path = root.join("saves/test.sav");
            fs::write(&path, &case.bytes).unwrap();
            let save_entries = openmt2001::save_menu::list_slots(&root).unwrap();
            assert_eq!(save_entries.len(), 1);
            let entry = Entry::read(path, "test".into(), None, None);
            (case, save_entries[0].metadata.clone(), entry.metadata)
        })
        .collect::<Vec<_>>();
    fs::remove_dir_all(root).unwrap();
    for (case, save_metadata, metadata) in results {
        assert_eq!(
            (save_metadata.is_ok(), metadata.is_ok()),
            (case.readable, case.readable),
            "{}: Save {save_metadata:?}, Load {metadata:?}",
            case.name
        );
        if case.readable {
            assert_eq!(metadata.unwrap(), fixtures::WORDS, "{}", case.name);
            assert_eq!(save_metadata.unwrap(), fixtures::WORDS, "{}", case.name);
        }
    }
}
