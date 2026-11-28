use super::*;

pub(super) fn synthetic_assets() -> Assets {
    let mut assets = Assets {
        boards: Vec::new(),
        board_pitch: 11.,
        panels: std::array::from_fn(|_| Panel {
            base: Vec::new(),
            regions: (0..35)
                .map(|i| [10., 20. + i as f32 * 25., 110., 40. + i as f32 * 25.])
                .collect(),
        }),
        fonts: vec![Surface::solid(); 2],
        font1: vec![Surface::solid(); 2],
        font3: vec![Surface::solid(); 2],
        slider_textures: vec![Surface::solid(); 105],
        radio_buttons: std::array::from_fn(|_| Surface::solid()),
        checkboxes: std::array::from_fn(|_| Surface::solid()),
        tab: crate::ui::synthetic_text_font(11, 14),
        tab1: crate::ui::synthetic_text_font(7, 10),
        tab3: crate::ui::synthetic_text_font(3, 6),
        strings: vec![vec![33]; 1800],
    };
    // Fractional origins, spans and deliberately different authored heights
    // distinguish factory layout from truncating the two endpoints separately.
    for (page, region, bounds) in [
        (Page::Sound, 13, [200.75, 150.75, 460.25, 179.75]),
        (Page::Sound, 21, [210.75, 200.75, 470.25, 229.75]),
        (Page::Graphics, 27, [220.75, 300.75, 420.25, 329.75]),
        (Page::Gameplay, 29, [230.75, 350.75, 430.25, 379.75]),
    ] {
        assets.panels[page.index()].regions[region] = bounds;
    }
    assets
}

fn state(page: Page) -> MenuState {
    MenuState {
        page,
        hovered: None,
        pressed: None,
        levels: Levels::default(),
        graphics: GraphicsLevels::default(),
        gameplay: GameplayLevels::default(),
        reset_enabled: false,
    }
}

#[test]
fn settings_credits_click_hides_settings_and_requests_one_presentation_only_outside_city() {
    let assets = synthetic_assets();
    let bounds = assets.bounds(Page::Main, Button::Credits).unwrap();
    let pointer = [(bounds[0] + bounds[2]) / 2., (bounds[1] + bounds[3]) / 2.];
    let mut view = View::default();
    let mut levels = Levels::default();
    let click = |view: &mut View, levels: &mut Levels| {
        view.mouse(&assets, Some(pointer), true, levels);
        view.mouse(&assets, Some(pointer), false, levels);
    };
    view.open_in_game();
    click(&mut view, &mut levels);
    assert_eq!(view.page, Some(Page::Main));
    assert!(!view.take_credits());
    view.open();
    assert!(assets.action_cursor_hover(Page::Main, Some(pointer), true));
    click(&mut view, &mut levels);
    assert_eq!(view.page, None);
    assert!(view.take_credits());
    assert!(!view.take_credits());
    assert_eq!(levels, Levels::default());
    assert_eq!(view.graphics, GraphicsLevels::default());
    assert_eq!(view.gameplay, GameplayLevels::default());
    // A press dragged off the action must not start a presentation.
    view.open();
    view.mouse(&assets, Some(pointer), true, &mut levels);
    view.mouse(&assets, None, false, &mut levels);
    assert!(!view.take_credits());
}

fn rect(vertices: &[[f32; 8]]) -> [f32; 4] {
    [
        vertices.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min),
        vertices.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min),
        vertices
            .iter()
            .map(|v| v[0])
            .fold(f32::NEG_INFINITY, f32::max),
        vertices
            .iter()
            .map(|v| v[1])
            .fold(f32::NEG_INFINITY, f32::max),
    ]
}

#[test]
fn settings_draw_headers_use_green_palette_font1_and_style9() {
    let assets = synthetic_assets();
    for (page, rows) in [
        (Page::Sound, &[9, 17][..]),
        (Page::Graphics, &[9, 18, 25][..]),
        (Page::Gameplay, &[9, 14, 19, 24][..]),
    ] {
        let surfaces = assets.surfaces(state(page)).unwrap();
        let green = surfaces
            .iter()
            .flat_map(|s| &s.vertices)
            .filter(|v| v[3..6] == [0., 1., 0.])
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(green.len(), rows.len() * 6, "{page:?} header glyphs");
        for (&region, glyph) in rows.iter().zip(green.as_chunks::<6>().0) {
            let top = assets.panel(page).regions[region][1];
            assert_eq!(
                rect(glyph),
                [56.5, top + 4.5, 63.5, top + 14.5],
                "{page:?} font 1 glyph centered with style 9"
            );
        }
        for surface in &surfaces {
            for (v, alpha) in surface.vertices.iter().zip(&surface.alpha) {
                if v[3..6] == [0., 1., 0.] {
                    assert_eq!(*alpha, 255);
                }
            }
        }
    }
}

fn assert_slider_draw(
    assets: &Assets,
    page: Page,
    controls: [Button; 3],
    expected: [f32; 4],
    icon_index: usize,
) {
    let [l, t, r, b] = expected;
    let surfaces = assets.surfaces(state(page)).unwrap();
    // Sound has only slider icons; Graphics has five radios; Gameplay has
    // three checkboxes before its slider. Surface zero contains solid art.
    assert_eq!(rect(&surfaces[icon_index].vertices), [l, t, l + 16., b]);
    assert_eq!(rect(&surfaces[icon_index + 1].vertices), [r - 16., t, r, b]);
    let tracks = surfaces[0]
        .vertices
        .as_chunks::<6>()
        .0
        .iter()
        .filter(|quad| quad[0][3..6] == [24. / 255., 48. / 255., 136. / 255.])
        .map(|quad| rect(quad))
        .collect::<Vec<_>>();
    assert!(
        tracks.contains(&[l + 11., t + 1.5, r - 10., b - 1.5]),
        "{page:?} track: {tracks:?}"
    );
    for (button, expected) in
        controls
            .into_iter()
            .zip([[l, t, l + 16., b], expected, [r - 16., t, r, b]])
    {
        assert_eq!(assets.bounds(page, button), Some(expected));
        let point = [(expected[0] + expected[2]) * 0.5, (t + b) * 0.5];
        assert_eq!(
            assets.button(page, point),
            Some(button),
            "{page:?} {button:?}"
        );
    }
    assert_eq!(
        View::track_value(assets, controls[1], [l + 16., t + 8.]),
        Some(0)
    );
    assert_eq!(
        View::track_value(assets, controls[1], [r - 16., t + 8.]),
        Some(100)
    );
}

#[test]
fn settings_draw_sliders_use_factory_regions_and_integer_origin_span() {
    let assets = synthetic_assets();
    for (page, controls, expected, icon) in [
        (
            Page::Sound,
            [Button::MusicLeft, Button::MusicTrack, Button::MusicRight],
            [200., 150., 459., 166.],
            1,
        ),
        (
            Page::Sound,
            [
                Button::EffectsLeft,
                Button::EffectsTrack,
                Button::EffectsRight,
            ],
            [210., 200., 469., 216.],
            3,
        ),
        (
            Page::Graphics,
            [
                Button::DistanceLeft,
                Button::DistanceTrack,
                Button::DistanceRight,
            ],
            [220., 300., 419., 316.],
            6,
        ),
        (
            Page::Gameplay,
            [
                Button::SensitivityLeft,
                Button::SensitivityTrack,
                Button::SensitivityRight,
            ],
            [230., 350., 429., 366.],
            4,
        ),
    ] {
        assert_slider_draw(&assets, page, controls, expected, icon);
    }
}

#[test]
#[ignore = "requires original v1.4 Settings meshes and localization"]
fn settings_draw_original_slider_extents() {
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
    let assets = Assets::load(&index, &archive, installer, &language).unwrap();
    for (page, controls, expected, icon) in [
        (
            Page::Sound,
            [Button::MusicLeft, Button::MusicTrack, Button::MusicRight],
            [215., 219., 430., 235.],
            1,
        ),
        (
            Page::Sound,
            [
                Button::EffectsLeft,
                Button::EffectsTrack,
                Button::EffectsRight,
            ],
            [215., 266., 430., 282.],
            3,
        ),
        (
            Page::Graphics,
            [
                Button::DistanceLeft,
                Button::DistanceTrack,
                Button::DistanceRight,
            ],
            [224., 298., 422., 314.],
            6,
        ),
        (
            Page::Gameplay,
            [
                Button::SensitivityLeft,
                Button::SensitivityTrack,
                Button::SensitivityRight,
            ],
            [224., 317., 422., 333.],
            4,
        ),
    ] {
        assert_slider_draw(&assets, page, controls, expected, icon);
    }
}
