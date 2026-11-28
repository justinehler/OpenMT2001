//! Synthetic draw-output coverage; no original game files or GPU required.
use super::*;
use crate::ui::Surface;

fn synthetic_assets() -> Assets {
    // Independent rectangles and winding exercise the real mesh loader,
    // including layout-only faces and four authored strips around each bar.
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u16>::new();
    let mut headers = Vec::new();
    let mut regions = Vec::new();
    for part in 0usize..68 {
        let first = positions.len();
        let is_ring = (12..54).contains(&part) && (part - 12).is_multiple_of(7);
        let slot = part.saturating_sub(12) / 7;
        let top = 330. + slot as f32 * 24.;
        let rects = if is_ring {
            vec![
                [40., top, 180., top + 1.25],
                [40., top + 6.75, 180., top + 8.],
                [40., top + 1.25, 41.25, top + 6.75],
                [178.75, top + 1.25, 180., top + 6.75],
            ]
        } else {
            vec![match part {
                4 => [30., 302., 190., 328.],
                5 => [30., 328., 190., 472.],
                54 => [5., 302., 29., 472.],
                55 => [42., 306., 178., 324.],
                12..54 => match (part - 12) % 7 {
                    1 => [40., top - 12., 180., top],
                    2 => [40., top + 8., 180., top + 10.],
                    3 => [180., top - 12., 182., top + 10.],
                    4 => [38., top - 12., 40., top + 10.],
                    5 => [41.25, top + 1.25, 178.75, top + 6.75],
                    _ => [38., top - 12., 182., top + 10.],
                },
                _ => [part as f32, 304., part as f32 + 4., 326.],
            }]
        };
        let visible = part < 12 && part != 4 && part != 5 || is_ring || part == 55;
        let start = indices.len();
        for [l, t, r, b] in rects {
            let vertex = (positions.len() - first) as u16;
            positions.extend([[l, -t, 0.], [l, -b, 0.], [r, -b, 0.], [r, -t, 0.]]);
            let triangles = if visible {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            indices.extend(triangles.map(|i| vertex + i));
        }
        let mut header = [0u8; 20];
        header[4..6].copy_from_slice(&u16::from(part == 55).to_le_bytes());
        header[6..8].copy_from_slice(&((indices.len() - start) as u16).to_le_bytes());
        header[12..16].copy_from_slice(&(first as u32).to_le_bytes());
        header[16..20].copy_from_slice(&((positions.len() - 1) as u32).to_le_bytes());
        headers.extend(header);
        let mut region = [0u8; 35];
        let name = format!("REGION{part}");
        region[..name.len()].copy_from_slice(name.as_bytes());
        region[33] = part as u8;
        regions.extend(region);
    }
    let names = b"grey.tt\0navy.tt\0";
    let mut mesh = vec![0u8; 164];
    for (offset, value) in [
        (0, positions.len() as u16),
        (2, 68),
        (4, 10),
        (6, names.len() as u16),
    ] {
        mesh[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    mesh[8..12].copy_from_slice(&68u32.to_le_bytes());
    for _ in 0..10 {
        for point in &positions {
            for value in point {
                mesh.extend(value.to_le_bytes());
            }
        }
    }
    mesh.extend(vec![0u8; positions.len() * 20]); // UVs and normals.
    mesh.extend(regions);
    mesh.extend(headers);
    mesh.extend((indices.len() as u32).to_le_bytes());
    for index in indices {
        mesh.extend(index.to_le_bytes());
    }
    mesh.extend(names);
    let mut archive = Vec::new();
    let mut entries = Vec::new();
    let texture = |rgba: [u8; 4]| {
        let mut bytes = vec![0u8; 60];
        for (at, word) in [
            (0, 1u32),
            (8, 1),
            (12, 1),
            (16, 1f32.to_bits()),
            (20, 1f32.to_bits()),
        ] {
            bytes[at..at + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes.extend(rgba);
        bytes
    };
    for (path, bytes) in [
        ("gamedata2/brand_new_panels/stats_panel.tm", mesh),
        ("grey.tt", texture([136, 136, 136, 255])),
        ("navy.tt", texture([0, 23, 111, 255])),
    ] {
        entries.push(openmt2001::archive::ArchiveEntry {
            path: path.into(),
            offset: archive.len() as u64,
            length: bytes.len() as u64,
            metadata: [0; 13],
        });
        archive.extend(bytes);
    }
    let index = openmt2001::archive::ArchiveIndex {
        header_words: [0; 8],
        version: 3,
        header_flags: 0,
        entries,
    };
    let mut base = Vec::new();
    let mut bounds = Vec::new();
    for frame in [0, 9] {
        let (mut surfaces, regions) = panel_frame(&index, &archive, frame).unwrap();
        add_background(&mut surfaces, &regions, frame, 0xbb000a28).unwrap();
        base.push(surfaces);
        bounds.push(regions);
    }
    let mut tab = 95u32.to_le_bytes().to_vec();
    for _ in 0..95 {
        let mut record = [0u8; 44];
        record[32..36].copy_from_slice(&4i32.to_le_bytes());
        record[36..40].copy_from_slice(&6i32.to_le_bytes());
        tab.extend(record);
    }
    Assets {
        base: base.try_into().ok().unwrap(),
        regions: bounds.try_into().ok().unwrap(),
        generic: vec![Surface::solid(); 300],
        fonts: vec![Surface::solid(); 2],
        tab,
        strings: vec![vec![]; 2245],
        chat_art: crate::chat_panel::Art {
            base: vec![],
            body: 0xff000a4b,
            layout: crate::chat_panel::Layout {
                title: [4., 302., 250., 323.],
                backing: [4., 302., 250., 388.],
                to: [8., 330., 26., 346.],
                input: [8., 354., 238., 380.],
                all: [180., 327., 208., 349.],
                close: [210., 327., 248., 349.],
                icons: std::array::from_fn(|i| {
                    [27. + i as f32 * 25., 329., 50. + i as f32 * 25., 347.]
                }),
            },
        },
        colors: [0x009900; 10],
        icons: [0; 10],
    }
}

#[test]
fn comparison_draw_retains_opaque_title_and_six_authored_bar_outlines() {
    let assets = synthetic_assets();
    for mode in [0, 3, 5] {
        // Cash, Sales, Today's Profit.
        for count in [1, 3, 6] {
            let mut state = State::new(0);
            state.mode = mode;
            let mut players = vec![[0u8; 440]; count];
            for player in &mut players {
                player[0x154] = b'A';
            }
            let rows: Vec<_> = (0..count).map(|i| (i, 100. - i as f32 * 75.)).collect();
            let surfaces = assets
                .surfaces(&state, &players, &rows, &[true; 6], None, None, true)
                .unwrap();
            // Backings, ten panel borders, six rings, title, side, widgets,
            // then dynamic rows: every empty slot keeps its own mesh ring.
            assert_eq!(surfaces[0].alpha, [187; 12]);
            assert_eq!(surfaces[0].vertices[0][3..6], [0., 10. / 255., 40. / 255.]);
            for slot in 0..6 {
                let ring = &surfaces[11 + slot];
                assert_eq!(ring.pixels, [136, 136, 136, 255]);
                assert_eq!(ring.alpha, [255; 24]);
                let top = 330. + slot as f32 * 24.;
                for (quad, expected) in ring.vertices.as_chunks::<6>().0.iter().zip([
                    [40., top, 180., top + 1.25],
                    [40., top + 6.75, 180., top + 8.],
                    [40., top + 1.25, 41.25, top + 6.75],
                    [178.75, top + 1.25, 180., top + 6.75],
                ]) {
                    assert_eq!([quad[0][0], quad[0][1], quad[2][0], quad[2][1]], expected);
                }
            }
            let title = &surfaces[17];
            assert_eq!(title.alpha, [255; 6]);
            assert_eq!(title.pixels, [0, 22, 111, 255]);
            assert!(title.vertices.iter().all(|v| v[3..6] == [1.; 3]));
            assert_eq!(
                [
                    title.vertices[0][0],
                    title.vertices[0][1],
                    title.vertices[2][0],
                    title.vertices[2][1]
                ],
                [42., 306., 178., 324.]
            );
            // Five widgets and one portrait per player precede row fills.
            let solid = &surfaces[24 + count];
            assert_eq!(solid.vertices.len(), (count + 6 * 4) * 6);
            assert_eq!(solid.alpha[0], 253);
            assert_eq!(solid.vertices[0][0..2], [41.25, 331.25]);
            assert_eq!(solid.vertices[2][0..2], [178.75, 336.75]);
        }
    }
    let mut closed = State::new(0);
    closed.open = false;
    let surfaces = assets
        .surfaces(&closed, &[], &[], &[true; 6], None, None, true)
        .unwrap();
    assert!(surfaces.last().unwrap().vertices.is_empty());
    assert_eq!(surfaces[0].alpha, [255; 6]); // No translucent body when closed.
}

#[test]
fn chat_lines_use_sender_colour_and_stack_up_from_city_bottom() {
    use openmt2001::network::{chat, lobby};
    let mut assets = synthetic_assets();
    assets.colors[3] = 0xff0d00f1;
    let snapshot = lobby::Snapshot::host(
        lobby::Settings {
            name: b"Session".to_vec(),
            level: 300001,
            parameter_bits: 10000f32.to_bits(),
            difficulty: 0,
            map: b"Map".to_vec(),
        },
        b"Host".to_vec(),
        0,
    )
    .unwrap();
    let chat = chat::View {
        local_peer: 0,
        snapshot,
        lines: vec![
            chat::Line {
                row: 0,
                token: 0,
                text: b"Host: first".to_vec(),
            },
            chat::Line {
                row: 1,
                token: 3,
                text: b"Blue: second".to_vec(),
            },
        ],
    };
    let out = assets
        .chat_surfaces(&chat, &crate::chat_panel::Panel::default())
        .unwrap();
    let vertices: Vec<_> = out.iter().flat_map(|s| &s.vertices).collect();
    let blue: Vec<_> = vertices
        .iter()
        .copied()
        .filter(|v| {
            (v[3] - 13. / 255.).abs() < 0.001 && v[4] == 0. && (v[5] - 241. / 255.).abs() < 0.001
        })
        .collect();
    let green: Vec<_> = vertices
        .iter()
        .copied()
        .filter(|v| v[3] == 0. && (v[4] - 153. / 255.).abs() < 0.001 && v[5] == 0.)
        .collect();
    assert!(!blue.is_empty() && !green.is_empty());
    // Glyph quads sit on half-pixel centres, so each 14-pixel line band starts 0.5 early.
    assert!(
        blue.iter()
            .all(|v| v[0] >= 193.5 && v[1] >= 463.5 && v[1] <= 478.)
    );
    assert!(
        green
            .iter()
            .all(|v| v[0] >= 193.5 && v[1] >= 449.5 && v[1] < 463.5)
    );
}

#[test]
fn city_view_holds_the_panel_open_without_its_toggle() {
    let mut assets = synthetic_assets();
    // Tag row 8's button (88..=90) and contour (71/72) textures.
    for texture in [71, 72, 88, 89, 90] {
        assets.generic[texture].pixels = vec![texture as u8, 0, 0, 255];
    }
    let toggles = |surfaces: &[Surface]| {
        surfaces
            .iter()
            .filter(|s| [71, 72, 88, 89, 90].contains(&s.pixels[0]) && !s.vertices.is_empty())
            .count()
    };
    let mut view = View::new(Data {
        objective: 0,
        rows: vec![],
    });
    view.state.open = false;
    let shown = |view: &View| {
        assets
            .surfaces(
                &view.state,
                &[],
                &[],
                &[true; 6],
                None,
                None,
                view.toggle_visible(),
            )
            .unwrap()
    };
    assert_eq!(toggles(&shown(&view)), 2, "3D views draw the toggle");
    // 427210: interface mode 3 opens the closed panel and hides row 8.
    view.set_city_view(true);
    assert!(view.state.open && !view.toggle_visible());
    assert_eq!(toggles(&shown(&view)), 0, "City View has no toggle");
    // The toggle cannot be hovered or pressed while hidden.
    let [l, t, r, b] = assets.regions[1][57];
    let point = [(l + r) as i32, (t + b) as i32];
    assert_eq!(assets.button_at(&view.state, point), Some(0));
    view.pointer(&assets, Some(point));
    view.mouse(true);
    view.mouse(false);
    assert!(view.state.open);
    // Leaving City View closes the panel it opened and shows the toggle.
    view.set_city_view(false);
    assert!(!view.state.open && view.toggle_visible());
    assert_eq!(toggles(&shown(&view)), 2);
    // A panel the player left open stays open on both sides of City View.
    view.state.open = true;
    view.set_city_view(true);
    view.set_city_view(false);
    assert!(view.state.open);
}
