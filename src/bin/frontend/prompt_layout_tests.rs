use super::*;

fn screen_rect(rect: [f32; 4], size: [f32; 2]) -> [f32; 4] {
    [
        (rect[0] * PROJECTION[0] + PROJECTION[12] + 1.) * size[0] / 2.,
        (1. - rect[1] * PROJECTION[5] - PROJECTION[13]) * size[1] / 2.,
        (rect[2] * PROJECTION[0] + PROJECTION[12] + 1.) * size[0] / 2.,
        (1. - rect[3] * PROJECTION[5] - PROJECTION[13]) * size[1] / 2.,
    ]
}

fn near(actual: [f32; 4], expected: [f32; 4]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!((a - e).abs() < 0.001, "{actual:?} != {expected:?}");
    }
}

#[test]
fn portrait_layout_width_anchor_and_wrap_use_shared_geometry() {
    // Synthetic mesh regions exercise the same rectangle derivation as loading.
    let regions = [
        [224., 139., 545., 282.],
        [225., 140., 544., 157.],
        [225., 158., 544., 282.],
        [234., 166., 304., 235.],
    ];
    let layout = PanelLayout::from_regions(&regions).unwrap();
    assert_eq!(layout.body, [304., 158., 544., 253.]);
    assert_eq!(layout.button, [354.5, 253., 414.5, 277.]);
    near(
        screen_rect(regions[0], [1024., 768.]),
        [358.4, 222.4, 872., 451.2],
    );
    near(screen_rect(regions[0], [640., 480.]), regions[0]);
    near(
        screen_rect(regions[0], [1280., 960.]),
        [448., 278., 1090., 564.],
    );

    let mut tab = vec![0; 4 + 224 * 44];
    tab[..4].copy_from_slice(&224u32.to_le_bytes());
    for glyph in tab[4..].as_chunks_mut::<44>().0 {
        glyph[32..36].copy_from_slice(&8i32.to_le_bytes());
        glyph[36..40].copy_from_slice(&12i32.to_le_bytes());
    }
    let text = vec![b'A' - 32; 33]; // 33 advances of 7 fit the 232-pixel wrap.
    assert_eq!(
        text_rows(&tab, std::slice::from_ref(&text), layout.body, 4).unwrap(),
        [text.clone()]
    );
    let mut longer = vec![b'A' - 32; 32];
    longer.extend_from_slice(&[0, b'A' - 32]);
    assert_eq!(
        text_rows(&tab, &[longer], layout.body, 4).unwrap(),
        [vec![b'A' - 32; 32], vec![b'A' - 32]]
    );
    let quads = text_quads(&tab, &[text], layout.body, false, 4, 4, [0., -100.]).unwrap();
    assert_eq!(quads.len(), 33);
    assert_eq!(quads[0].positions[0][0], 309.5);
    assert_eq!(quads[32].positions[0][0], 533.5);
    assert_eq!(quads[0].positions[0][1], 114.5);
    assert!(PanelLayout::from_regions(&regions[..3]).is_err());
}

#[test]
#[ignore = "requires supplied original prompt meshes, USA language and font metrics"]
fn original_portrait_layout_and_tutorial_one_three_lines() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("ARCHIVE_DIR");
    let archive = read("ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe = openmt2001::fixtures::path("EXE");
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let offsets = fs::read(language.join("tycoon.off")).unwrap();
    let bytes = fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = offsets
        .as_chunks::<2>()
        .0
        .iter()
        .map(|n| {
            let end = at + usize::from(u16::from_le_bytes(*n));
            let row = &bytes[at..end];
            at = end;
            row
        })
        .collect::<Vec<_>>();
    let executable = read("EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let assets = Assets::load_portrait(&index, &archive, installer, &strings, &image).unwrap();
    near(
        screen_rect(assets.input_bounds, [1024., 768.]),
        [358.4072, 222.1193, 872.3531, 450.4156],
    );
    near(
        screen_rect(assets.title, [1024., 768.]),
        [359.68, 223.3993, 870.813, 250.2793],
    );
    near(assets.body, [304.15472, 157.22456, 544.2581, 251.83847]);
    near(
        [
            assets.portrait.vertices[0][0],
            assets.portrait.vertices[0][1],
            assets.portrait.vertices[2][0],
            assets.portrait.vertices[2][1],
        ],
        [233.75471, 165.2928, 304.15472, 234.8128],
    );
    let [x, y] = assets.arrow_origin();
    near([x, y, x, y], [384.52905, 218.96716, 384.52905, 218.96716]);
    let expected = [
        "Welcome to Monopoly Tycoon's tutorials. In this first one, you",
        "will be introduced to the buttons and panels you will need to",
        "play the game.",
    ];
    let text: Vec<_> = expected.join(" ").bytes().map(|b| b - 32).collect();
    let body_id = assets.strings.iter().position(|row| row == &text).unwrap();
    let rows = text_rows(
        &assets.tabs[0],
        std::slice::from_ref(&assets.strings[body_id]),
        assets.body,
        4,
    )
    .unwrap();
    let decoded: Vec<_> = rows
        .iter()
        .map(|r| String::from_utf8(r.iter().map(|b| b + 32).collect()).unwrap())
        .collect();
    assert_eq!(decoded, expected);
    let quads = text_quads(&assets.tabs[0], &[text], assets.body, false, 4, 4, [0.; 2]).unwrap();
    // Compare the same glyph: different glyphs have different baselines/heights.
    let mut start = 0;
    let a_y: Vec<_> = rows
        .iter()
        .map(|row| {
            let a = start + row.iter().position(|&glyph| glyph == b'a' - 32).unwrap();
            start += row.len();
            quads[a].positions[0][1]
        })
        .collect();
    assert_eq!(a_y[1] - a_y[0], 8.);
    assert_eq!(a_y[2] - a_y[0], 16.);
    let mut state = State::default();
    state.submit(Request {
        script: None,
        dialog: None,
        title: 0,
        body: body_id as u32,
        dismissal_id: 1,
        card: None,
    });
    let surfaces = assets
        .message_surfaces(&state, false, [10., -100.], None)
        .unwrap();
    // Body pages are first, then title pages, through the production draw path.
    let body_page = &surfaces[assets.base.len() + 2];
    let actual = &body_page.vertices[0];
    assert_eq!(actual[0], quads[0].positions[0][0] + 10.);
    assert_eq!(actual[1], quads[0].positions[0][1] - 100.);
    assert_eq!(assets.button_at([10., -100.])[1], assets.button[1] - 100.);
    let frontend = Assets::load(&index, &archive, installer, &strings, &image).unwrap();
    assert!(frontend.input_bounds[2] < assets.input_bounds[2]);
    assert!(frontend.title[1] > assets.title[1]);
}
