//! CI coverage of the surfaces and input used by the live business panel.
use super::*;
use openmt2001::business_report::{Graph, Report};

fn ascii(text: &str) -> Vec<u8> {
    text.bytes().map(|b| b - 32).collect()
}
fn tagged(tag: u8) -> Surface {
    let mut surface = Surface::solid();
    surface.pixels[0] = tag;
    surface
}
fn assets() -> Assets {
    let mut strings = vec![vec![]; 2245];
    for (id, text) in [
        (9, "%s: %s"),
        (84, "Cost to Demolish:"),
        (85, "REFURBISH"),
        (87, "Cost to Refurbish:"),
        (88, "DEMOLISH"),
        (93, "Business Lease"),
        (128, "Bakery"),
        (129, "Book Store"),
        (130, "Clothes Store"),
        (1118, "Business Value"),
        (1413, "Selling Price"),
        (1416, "SELL"),
        (2243, "$"),
    ] {
        strings[id] = ascii(text);
    }
    // USA currency layout: the symbol leads the amount.
    strings[2244] = vec![b'L'];
    let mut tab = ui::synthetic_text_font(4, 8);
    // Encode each glyph in its U coordinate so tests can read the actual
    // submitted bold-font batch without invoking a GPU or duplicating layout.
    for glyph in 0..96 {
        for corner in 0..4 {
            let at = 4 + glyph * 44 + corner * 8;
            tab[at..at + 4].copy_from_slice(&(glyph as f32).to_le_bytes());
        }
    }
    let bounds = [32., 20., 196., 300.];
    Assets {
        base: vec![],
        title_backgrounds: vec![tagged(200)],
        textures: (0..300).map(|i| tagged(i as u8)).collect(),
        fonts: [
            vec![tagged(240); 2],
            vec![tagged(241); 2],
            vec![tagged(242); 2],
        ],
        tabs: [tab.clone(), tab.clone(), tab.clone()],
        title_fonts: vec![tagged(243); 2],
        title_tab: tab,
        strings,
        modes: [[0., 0., 16., 16.]; 6],
        bounds,
        title_text_bounds: [32., 0., 196., 18.],
        layout: Layout::new(bounds, [64, 16]),
        detail_buttons: [[0.; 4]; 2],
        information: ui::mpp_modes(bounds, [16, 16]).unwrap(),
        color: 0xffffffff,
        palette: vec![0xff6ed1e1; 17],
        report_line_height: 10,
    }
}
fn inspection() -> Inspection {
    let mut owner_name = [0; 96];
    owner_name[..3].copy_from_slice(b"ref");
    Inspection {
        building: 0,
        business_label: 128,
        demolition_label: 128,
        owned: true,
        owner_name,
        owner_character: None,
        report: Report {
            summary: vec![],
            income: vec![],
            expenses: vec![],
            highlight: [0; 3],
        },
        graph: Graph {
            points: [[0.; 2]; 4],
            rect: [0.; 4],
            plot: [0.; 4],
            domain: [0., 0., 1., 1.],
        },
        base_value: 1029.23,
        rent_age: 41,
        trade_price: Some(411.692),
        demolition_price: Some(252.),
        permissions: openmt2001::business_permissions::Permissions {
            modes: [true; 6],
            trade: Some(openmt2001::business_permissions::Trade::Sell),
            convert: true,
            demolish: true,
        },
        conversions: vec![
            crate::business_operations::Conversion {
                kind: 2,
                label: 129,
                price: 920.,
            },
            crate::business_operations::Conversion {
                kind: 3,
                label: 130,
                price: 1082.,
            },
        ],
    }
}
fn bold(surfaces: &[Surface]) -> &Surface {
    surfaces
        .iter()
        .find(|s| s.pixels[0] == 240 && !s.vertices.is_empty())
        .unwrap()
}
fn text(surface: &Surface) -> String {
    surface
        .vertices
        .as_chunks::<6>()
        .0
        .iter()
        .map(|q| (q[0][6] as u8 + 32) as char)
        .collect()
}
fn click(view: &mut View, assets: &Assets, rect: [f32; 4]) {
    let p = Some([(rect[0] + rect[2]) / 2., (rect[1] + rect[3]) / 2.]);
    view.mouse(assets, p, true);
    view.mouse(assets, p, false);
}
#[test]
fn sell_draws_three_bold_rows_with_live_values_and_submits_trade() {
    let assets = assets();
    let mut view = View {
        mode: 2,
        inspection: Some(inspection()),
        ..Default::default()
    };
    let surfaces = assets.surfaces(&view, 128).unwrap();
    let font = bold(&surfaces);
    assert_eq!(
        text(font),
        "Business Value$1029.23Business Lease41Selling Price$411.69"
    );
    let quads = font.vertices.as_chunks::<6>().0;
    for (offset, y) in [(0, 23.), (22, 33.), (38, 43.)] {
        let top = quads[offset]
            .iter()
            .map(|v| v[1])
            .fold(f32::INFINITY, f32::min);
        assert_eq!(top, y - 0.5);
    }
    assert!(
        surfaces
            .iter()
            .any(|s| s.pixels[0] == 76 && !s.vertices.is_empty())
    );
    click(&mut view, &assets, assets.layout.action);
    assert_eq!(
        view.action,
        Some(openmt2001::business_controls::Action::Trade)
    );
}
#[test]
fn refurbish_draws_centered_white_header_and_each_price_without_cost_row() {
    let assets = assets();
    let mut view = View {
        mode: 3,
        inspection: Some(inspection()),
        ..Default::default()
    };
    let surfaces = assets.surfaces(&view, 128).unwrap();
    let font = bold(&surfaces);
    assert_eq!(
        text(font),
        "ref: BakeryBook Store$920.00Clothes Store$1082.00"
    );
    let header = &font.vertices[..11 * 6];
    assert!(header.iter().all(|v| v[3..6] == [1., 1., 1.]));
    let left = header.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min);
    let right = header
        .iter()
        .map(|v| v[0])
        .fold(f32::NEG_INFINITY, f32::max);
    // Native centering uses width-minus-one advances and integer rounding.
    assert_eq!((left + right) / 2., 114.5);
    // The price's right edge stays in the list, left of the scrollbar.
    for index in [27, 48] {
        let q = &font.vertices[index * 6..(index + 1) * 6];
        assert_eq!(
            q.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max),
            179.5
        );
    }
    assert!(
        surfaces
            .iter()
            .any(|s| s.pixels[0] == 143 && !s.vertices.is_empty())
    );
    for arrow in [105, 93] {
        assert!(
            surfaces
                .iter()
                .any(|s| s.pixels[0] == arrow && !s.vertices.is_empty())
        );
    }
    let list = assets.conversion_list().unwrap();
    click(&mut view, &assets, list.highlight(1));
    assert_eq!(view.selected, Some(1));
    let surfaces = assets.surfaces(&view, 128).unwrap();
    let mut highlight = Surface::solid();
    highlight.rect(list.highlight(1), [0xff008ed0; 4]);
    assert!(surfaces.iter().any(|s| s.vertices == highlight.vertices));
    click(&mut view, &assets, assets.layout.action);
    assert_eq!(
        view.action,
        Some(openmt2001::business_controls::Action::Convert {
            selected_label: 130
        })
    );
}
#[test]
fn refurbish_scrollbar_changes_visible_rows_and_hit_selection() {
    let assets = assets();
    let mut data = inspection();
    data.conversions = (0..40)
        .map(|i| crate::business_operations::Conversion {
            kind: i,
            label: 129,
            price: 100. + i as f32,
        })
        .collect();
    let mut view = View {
        mode: 3,
        inspection: Some(data),
        ..Default::default()
    };
    let arrows = ui::slider_arrows(assets.conversion_scrollbar(), true);
    click(&mut view, &assets, arrows[1]);
    assert_eq!(view.scroll, 1);
    assert_eq!(view.selected, None);
    let list = assets.conversion_list().unwrap();
    click(&mut view, &assets, list.highlight(0));
    assert_eq!(view.selected, Some(1));
    let surfaces = assets.surfaces(&view, 128).unwrap();
    assert!(text(bold(&surfaces)).starts_with("ref: BakeryBook Store$101.00"));
    let track = ui::slider_track(assets.conversion_scrollbar(), true);
    view.mouse(&assets, Some([188., track[1]]), true);
    view.pointer(&assets, Some([188., track[3] + 30.]));
    view.mouse(&assets, Some([188., track[3] + 30.]), false);
    assert_eq!(view.scroll, 40 - list.visible_rows());
    click(&mut view, &assets, list.highlight(list.visible_rows() - 1));
    assert_eq!(view.selected, Some(39));
    click(&mut view, &assets, arrows[1]);
    assert_eq!(view.scroll, 40 - list.visible_rows());
    assert_eq!(view.selected, Some(39));
}
#[test]
fn demolition_draws_white_header_and_quote_then_submits_demolition() {
    let assets = assets();
    let mut view = View {
        mode: 4,
        inspection: Some(inspection()),
        ..Default::default()
    };
    let surfaces = assets.surfaces(&view, 128).unwrap();
    let font = bold(&surfaces);
    assert_eq!(text(font), "ref: BakeryCost to Demolish:$252.00");
    assert!(
        font.vertices[..11 * 6]
            .iter()
            .all(|v| v[3..6] == [1., 1., 1.])
    );
    assert!(
        surfaces
            .iter()
            .any(|s| s.pixels[0] == 76 && !s.vertices.is_empty())
    );
    click(&mut view, &assets, assets.layout.action);
    assert_eq!(
        view.action,
        Some(openmt2001::business_controls::Action::Demolish)
    );
}
#[test]
#[ignore = "requires supplied original panel/font/language assets"]
fn supplied_action_pages_build_through_production_surface_path() {
    let (assets, _) = super::tests::original_assets();
    for mode in [2, 3, 4] {
        let view = View {
            mode,
            inspection: Some(inspection()),
            ..Default::default()
        };
        let surfaces = assets.surfaces(&view, 128).unwrap();
        assert!(
            surfaces
                .iter()
                .flat_map(|s| &s.vertices)
                .all(|v| v.iter().all(|c| c.is_finite()))
        );
        assert!(surfaces.iter().map(|s| s.vertices.len()).sum::<usize>() > 100);
    }
}
