use super::*;
#[test]
fn hotel_admission_requires_a_player_landlord() {
    let members = [27, 23, 33, -1];
    let mut owners = [0; 52];
    assert!(special_residential_group_owned(&members, 0, &owners).unwrap());
    assert!(!hotel_allowed(&members, 0, &owners).unwrap());
    for id in [27, 23, 33] {
        owners[id] = 2;
    }
    assert!(hotel_allowed(&members, 2, &owners).unwrap());
    for owner in [0, 1] {
        owners[23] = owner;
        assert!(!hotel_allowed(&members, 2, &owners).unwrap());
    }
}
#[test]
#[ignore = "requires pinned native424010 completion observations"]
fn matches_native_placement_choices() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_TUTORIAL_BUILD_DRAG")).unwrap();
    assert_eq!(&bytes[..8], b"MTBDRG01");
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(word(8), 0x037f);
    assert_eq!(bytes.len(), 16 + word(12) as usize * 56);
    for row in bytes[16..].chunks_exact(56) {
        let w = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
        let mut state = w(0);
        let counts = [w(1), w(2), w(3)].map(|value| value as usize);
        if w(4) == 1 {
            let choices = placement_choices(&mut state, counts).unwrap();
            assert_eq!(choices.map(|v| v as u32), [w(6), w(7), w(8)]);
            assert_eq!([w(11), w(12), w(13)], [2, 88, 3]);
        } else {
            assert_eq!(w(11), 0);
        }
        assert_eq!(state, w(5));
    }
}
#[test]
#[ignore = "requires private original v1.4 executable"]
fn original_special_residential_group() {
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = crate::pe::PeImage::parse(&exe).unwrap();
    let members = special_residential_group(&image, 33).unwrap();
    assert_eq!(members, [27, 23, 33, -1]);
    let mut owners = [0u8; 52];
    for id in [27, 23, 33] {
        owners[id] = 1;
    }
    assert!(special_residential_group_owned(&members, 1, &owners).unwrap());
    owners[23] = 2;
    assert!(!special_residential_group_owned(&members, 1, &owners).unwrap());
    owners[23] = 0;
    assert!(!special_residential_group_owned(&members, 1, &owners).unwrap());
    owners[27] = 0;
    owners[33] = 0;
    assert!(
        special_residential_group_owned(&members, 0, &owners).unwrap(),
        "48a770 compares the null city-owner reference too"
    );
}
#[test]
#[ignore = "private original planning grid oracle"]
fn original_grid_points() {
    let path = crate::fixtures::path("NATIVE_BUILD_UI");
    let b = std::fs::read(path.with_file_name("native-build-ui-grid.bin")).unwrap();
    assert_eq!(&b[..8], b"MTBLDGRD");
    assert_eq!(
        b.len(),
        12 + u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize * 864
    );
    for row in b[12..].chunks_exact(864) {
        let word = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
        let bounds =
            std::array::from_fn(|i| std::array::from_fn(|a| f32::from_bits(word(i * 3 + a))));
        for (i, p) in grid_points(&bounds).unwrap().iter().enumerate() {
            assert_eq!(
                p.map(f32::to_bits),
                std::array::from_fn(|a| word(24 + i * 3 + a)),
                "point {i}"
            );
        }
    }
}

#[test]
#[ignore = "private original projected grid oracle"]
fn original_cell_centers() {
    let path = crate::fixtures::path("NATIVE_BUILD_UI");
    let b = std::fs::read(path.with_file_name("native-build-ui-centers.bin")).unwrap();
    assert_eq!(&b[..8], b"MTBLDCEN");
    assert_eq!(
        b.len(),
        12 + u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize * 904
    );
    for row in b[12..].chunks_exact(904) {
        let word = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
        let grid =
            std::array::from_fn(|i| [f32::from_bits(word(i * 2)), f32::from_bits(word(i * 2 + 1))]);
        let actual = cell_centers(&grid);
        for (i, c) in actual.iter().enumerate() {
            assert_eq!(c.map(f32::to_bits), [word(128 + i * 2), word(129 + i * 2)]);
        }
    }
}

#[test]
#[ignore = "private original projected lot selection oracle"]
fn original_cell_picking() {
    let path = crate::fixtures::path("NATIVE_BUILD_UI");
    let b = std::fs::read(path.with_file_name("native-build-ui-picking.bin")).unwrap();
    assert_eq!(&b[..8], b"MTBLDPCK");
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    assert_eq!(b.len(), 12 + n * 404);
    for row in b[12..].chunks_exact(404) {
        let word = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
        let centers =
            std::array::from_fn(|i| [f32::from_bits(word(i * 2)), f32::from_bits(word(i * 2 + 1))]);
        let expected = (word(100) != u32::MAX).then_some(word(100) as u8);
        assert_eq!(
            nearest_cell(&centers, [word(98) as i32, word(99) as i32]).unwrap(),
            expected
        );
    }
}

#[test]
#[ignore = "private original planning entry oracle"]
fn original_planning_entry() {
    let path = crate::fixtures::path("NATIVE_BUILD_UI");
    let b = std::fs::read(path.with_file_name("native-build-ui-entry.bin")).unwrap();
    assert_eq!(&b[..8], b"MTBLDENT");
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    assert_eq!(b.len(), 12 + n * 20);
    for row in b[12..].chunks_exact(20) {
        let v: [u32; 5] =
            std::array::from_fn(|i| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap()));
        let expected = if v[3] != 0 {
            Entry::Open
        } else {
            Entry::Message {
                text: v[4],
                show_rights_action: v[4] == 1202,
            }
        };
        assert_eq!(entry(v[0] as u8, v[1] != 0, v[2]), expected);
    }
}

#[test]
#[ignore = "private original construction UI request oracle"]
fn original_build_button_requests() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILD_UI")).unwrap();
    assert_eq!(&bytes[..8], b"MTBLDUI1");
    let mut at = 8;
    let word = |at: &mut usize| {
        let v = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
        *at += 4;
        v
    };
    for _ in 0..word(&mut at) {
        let quality = word(&mut at) as usize;
        let columns = word(&mut at);
        let choice = word(&mut at) as i32;
        let gate = word(&mut at);
        let present = word(&mut at) != 0;
        let form = Form {
            category: 7,
            rectangle: [2, 3, 5, 7],
            height: 4,
            quality,
            choices: [choice; 3],
            variants: (0..columns).map(|i| 17 + i * 3).collect(),
            kind: 2,
            business: 11,
            displayed_quote: 1000.,
        };
        let result = form.request(gate == 1, gate != 2, gate != 3).unwrap();
        assert_eq!(result.is_ok(), present);
        if present {
            let expected: [u32; 11] = std::array::from_fn(|_| word(&mut at));
            let r = result.unwrap();
            assert_eq!(
                [
                    r.category as u32,
                    r.origin[0] as u32,
                    r.origin[1] as u32,
                    r.size[0] as u32,
                    r.size[1] as u32,
                    r.height as u32,
                    r.packed_model,
                    r.kind,
                    u32::from(r.quality),
                    r.displayed_quote.to_bits(),
                    r.business
                ],
                expected
            );
        }
    }
    assert_eq!(at, bytes.len());
}
#[test]
#[ignore = "private original construction rectangle oracle"]
fn original_drag_rectangles() {
    let path = crate::fixtures::path("NATIVE_BUILD_UI");
    let b = std::fs::read(path.with_file_name("native-build-ui-rectangles.bin")).unwrap();
    assert_eq!(&b[..8], b"MTBLDREC");
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    assert_eq!(b.len(), 12 + n * 24);
    for row in b[12..].chunks_exact(24) {
        let values: [u32; 6] =
            std::array::from_fn(|i| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap()));
        assert_eq!(
            rectangle_from_cells(values[0] as u8, values[1] as u8).unwrap(),
            values[2..]
                .iter()
                .map(|v| *v as i32)
                .collect::<Vec<_>>()
                .as_slice()
        );
    }
}
