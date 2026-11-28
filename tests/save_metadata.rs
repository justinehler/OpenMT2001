#[path = "support/save_metadata.rs"]
mod fixtures;

#[cfg(feature = "native")]
#[test]
fn save_panel_entries_read_metadata_independently_of_world_payloads() {
    let root = openmt2001::fixtures::scratch("SAVE_METADATA_PANEL").join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("saves")).unwrap();
    let results = fixtures::cases()
        .into_iter()
        .map(|case| {
            case.check_full_parse();
            std::fs::write(root.join("saves/test.sav"), &case.bytes).unwrap();
            let slots = openmt2001::save_menu::list_slots(&root).unwrap();
            assert_eq!(slots.len(), 1);
            (case, slots[0].metadata.clone())
        })
        .collect::<Vec<_>>();
    std::fs::remove_dir_all(root).unwrap();
    for (case, metadata) in results {
        assert_eq!(
            metadata.is_ok(),
            case.readable,
            "{}: {metadata:?}",
            case.name
        );
        if case.readable {
            assert_eq!(metadata.unwrap(), fixtures::WORDS, "{}", case.name);
        }
    }
}
