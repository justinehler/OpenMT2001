#[path = "support/save_metadata.rs"]
mod fixtures;

use openmt2001::{pe::PeImage, save::SaveFile};

#[test]
fn metadata_reader_accepts_intact_metadata_without_admitting_damaged_loads() {
    // A minimal PE with the supplied executable's compatibility bytes at the
    // address used by full-load admission. No original files are required.
    let mut executable = vec![0u8; 0x128];
    executable[..2].copy_from_slice(b"MZ");
    executable[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    executable[0x80..0x84].copy_from_slice(b"PE\0\0");
    executable[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    executable[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    executable[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    executable[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    executable[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
    executable[0xc4..0xc8].copy_from_slice(&0x145d1cu32.to_le_bytes());
    executable[0xc8..0xcc].copy_from_slice(&40u32.to_le_bytes());
    executable[0xcc..0xd0].copy_from_slice(&0x100u32.to_le_bytes());
    executable[0x100..].fill(0x5a);
    let image = PeImage::parse(&executable).unwrap();

    for case in fixtures::cases() {
        case.check_full_parse();
        let metadata = openmt2001::save::read_level_metadata(&case.bytes);
        assert_eq!(
            metadata.is_ok(),
            case.readable,
            "{}: {metadata:?}",
            case.name
        );
        if case.readable {
            assert_eq!(metadata.unwrap(), fixtures::WORDS, "{}", case.name);
        }
        if let Ok(save) = SaveFile::parse(&case.bytes) {
            let compatible = save.validate_compatibility(&image);
            if case.name.ends_with("compatibility-mismatch") {
                assert_eq!(
                    compatible.unwrap_err(),
                    "save compatibility record differs from the supplied executable"
                );
            } else {
                compatible.unwrap();
            }
        }
    }
}

#[test]
fn metadata_reader_requires_framing_and_exact_metadata_length_before_stopping() {
    let words = |values: &[u32]| {
        values
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    };
    for invalid in [
        vec![],
        words(&[0x2000]),                    // truncated metadata header
        words(&[0x2000, 12, 1, 2, 3]),       // undersized metadata
        words(&[0x2000, 20, 1, 2, 3, 4, 5]), // oversized metadata
        words(&[0x2000, u32::MAX, 1, 2, 3, 4]),
        words(&[1, 100, 0x2000, 16, 1, 2, 3, 4]), // metadata inside another record
        words(&[u32::MAX, 0, 0x2000, 16, 1, 2, 3, 4]), // after terminator
        words(&[1, 0, u32::MAX, 0]),              // absent metadata
    ] {
        assert!(openmt2001::save::read_level_metadata(&invalid).is_err());
    }
    // Return without inspecting any later bytes, even another metadata record.
    let prefix = words(&[0x2000, 16, 200001, 1950, 120.5f32.to_bits(), 3]);
    for suffix in [
        vec![],
        vec![0xff],
        words(&[4, u32::MAX]),
        words(&[0x2000, 0, u32::MAX, 0]),
        words(&[u32::MAX, 0, 123]),
    ] {
        let mut bytes = prefix.clone();
        bytes.extend_from_slice(&suffix);
        assert_eq!(
            openmt2001::save::read_level_metadata(&bytes),
            Ok(fixtures::WORDS)
        );
    }
}

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
