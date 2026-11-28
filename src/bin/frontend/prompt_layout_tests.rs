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
    let rows = text_rows(&tab, std::slice::from_ref(&text), layout.body, 4).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], text);
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

/// Synthetic prompt panel: authored-style regions, a uniform glyph table and
/// distinguishable portrait textures, enough to build every prompt surface.
fn synthetic_assets() -> Assets {
    let regions = [
        [224., 139., 545., 282.],
        [225., 140., 544., 157.],
        [225., 158., 544., 282.],
        [234., 166., 304., 235.],
    ];
    let layout = PanelLayout::from_regions(&regions).unwrap();
    let mut tab = vec![0; 4 + 224 * 44];
    tab[..4].copy_from_slice(&224u32.to_le_bytes());
    for glyph in tab[4..].as_chunks_mut::<44>().0 {
        glyph[32..36].copy_from_slice(&8i32.to_le_bytes());
        glyph[36..40].copy_from_slice(&12i32.to_le_bytes());
    }
    let texture = |side: u32| Surface {
        width: side,
        height: side,
        pixels: vec![255; (side * side * 4) as usize],
        ..Surface::solid()
    };
    let mut portrait = texture(3);
    textured_rect(&mut portrait, layout.portrait);
    let card_portraits = (0..10)
        .map(|_| {
            let mut surface = texture(5);
            textured_rect(&mut surface, layout.portrait);
            surface
        })
        .collect();
    let mut strings = vec![vec![b'A' - 32; 4]; 1800];
    strings[1763] = vec![b'Q' - 32; 9];
    strings[1764] = vec![b'D' - 32; 30];
    Assets {
        main: layout.main,
        input_bounds: regions[0],
        base: vec![texture(7)],
        portrait,
        card_portraits,
        buttons: vec![texture(2), texture(2), texture(2)],
        fonts: vec![vec![Surface::solid()], vec![Surface::solid()]],
        tabs: vec![tab.clone(), tab],
        strings,
        title: layout.title,
        body: layout.body,
        button: layout.button,
    }
}

fn abandonment_state(portrait: u32) -> State {
    let dialog = crate::human_dialog::Dialog {
        portrait,
        ..crate::game_options::quit_dialog()
    };
    let mut state = State::default();
    state.submit(Request {
        script: None,
        dialog: Some(dialog),
        title: dialog.title,
        body: dialog.body,
        dismissal_id: 0,
        card: None,
    });
    state
}

fn portraits_drawn(surfaces: &[Surface]) -> Vec<u32> {
    surfaces
        .iter()
        .map(|s| s.width)
        .filter(|width| [3, 5].contains(width))
        .collect()
}

#[test]
fn abandonment_prompt_and_unknown_portraits_draw_without_portrait() {
    let assets = synthetic_assets();
    assert_eq!(
        crate::game_options::quit_dialog().portrait,
        crate::human_dialog::NO_PORTRAIT
    );
    // In-city Quit: the portrait-free 88c0d4 owner, shifted once by (-40, 0),
    // with the two solid colored Yes/No backgrounds and no portrait.
    let mut state = abandonment_state(crate::human_dialog::NO_PORTRAIT);
    let buttons = assets.frontend_dialog_buttons(2);
    assert_eq!(buttons.len(), 2);
    for (shifted, bounds) in buttons.iter().zip(assets.dialog_buttons(2)) {
        assert_eq!(
            *shifted,
            [bounds[0] - 40., bounds[1], bounds[2] - 40., bounds[3]]
        );
    }
    state.cursor([buttons[1][0] + 1., buttons[1][1] + 1.]);
    state.dialog_mouse(true, &buttons);
    let surfaces = assets.frontend_prompt_surfaces(&state).unwrap();
    assert!(portraits_drawn(&surfaces).is_empty());
    let title = &surfaces[assets.base.len()];
    assert_eq!(title.vertices[0][0], assets.title[0] - 40.);
    let colored = &surfaces[assets.base.len() + 1..assets.base.len() + 3];
    assert!(
        colored
            .iter()
            .all(|s| s.width == 1 && !s.vertices.is_empty())
    );
    state.dialog_mouse(false, &buttons);
    assert_eq!(
        state.take_dialog_replies(),
        vec![crate::human_dialog::Reply {
            callback: openmt2001::human_dialog::quit::CITY_CALLBACK,
            context: 0,
            button: 1,
        }]
    );

    // The ordinary owner never fails on an absent or out-of-range portrait.
    for portrait in [crate::human_dialog::NO_PORTRAIT, 0, 262, 273, 9999] {
        let state = abandonment_state(portrait);
        let surfaces = assets
            .message_surfaces(&state, true, [0.; 2], None)
            .unwrap();
        assert!(portraits_drawn(&surfaces).is_empty(), "portrait {portrait}");
        assert!(assets.frontend_prompt_surfaces(&state).is_ok());
    }
    // Known portraits are still drawn in the portrait column.
    for (portrait, width) in [(302, 3), (263, 5), (272, 5)] {
        let surfaces = assets
            .message_surfaces(&abandonment_state(portrait), true, [0.; 2], None)
            .unwrap();
        assert_eq!(portraits_drawn(&surfaces), [width], "portrait {portrait}");
    }
}

#[test]
fn save_and_load_confirmations_use_the_portrait_free_owner() {
    // Overwrite (497f60), Delete Saved Game (4981d0) and Load's Delete
    // (495191) raise the 88c0d4 owner as Quit does: no portrait, the panel
    // shifted by (-40, 0) and solid colored Yes/No buttons.
    let assets = synthetic_assets();
    let buttons = assets.frontend_dialog_buttons(2);
    for (title, body) in [(0x661, 0x65f), (0x65e, 0x660)] {
        // Not even the default portrait word draws Mr. Monopoly here.
        for portrait in [crate::human_dialog::NO_PORTRAIT, 302] {
            let dialog = crate::human_dialog::Dialog {
                portrait,
                title,
                body,
                buttons: [0x664, 0x665],
                button_count: 2,
                timeout: false,
                ..crate::human_dialog::Dialog::default()
            };
            let [l, t, r, b] = buttons[0];
            let surfaces = assets
                .screen_confirmation_surfaces(dialog, [(l + r) * 0.5, (t + b) * 0.5], Some(0))
                .unwrap();
            assert!(portraits_drawn(&surfaces).is_empty(), "portrait {portrait}");
            let title_bar = &surfaces[assets.base.len()];
            assert_eq!(title_bar.vertices[0][0], assets.title[0] - 40.);
            let colored = &surfaces[assets.base.len() + 1..assets.base.len() + 3];
            for (button, bounds) in colored.iter().zip(&buttons) {
                assert_eq!(button.width, 1, "solid button background");
                let xs = button.vertices.iter().map(|v| v[0]);
                let ys = button.vertices.iter().map(|v| v[1]);
                let drawn = [
                    xs.clone().fold(f32::MAX, f32::min),
                    ys.clone().fold(f32::MAX, f32::min),
                    xs.fold(f32::MIN, f32::max),
                    ys.fold(f32::MIN, f32::max),
                ];
                for (d, e) in drawn.into_iter().zip(*bounds) {
                    assert!((d - e).abs() <= 1., "{drawn:?} != {bounds:?}");
                }
            }
        }
    }
}
