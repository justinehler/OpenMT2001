//! Original asset geometry and same-target property controls; numeric reports have a separate native fixture.
use super::*;
#[test]
#[ignore = "requires original font0 and pinned native441220/4419f0 caption oracle"]
fn original_rights_caption_layout() {
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let tab = fs::read(
        parameter
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Standard/fonts/cust640p.tab"),
    )
    .unwrap();
    fn word(bytes: &mut &[u8]) -> u32 {
        let (head, tail) = bytes.split_at(4);
        *bytes = tail;
        u32::from_le_bytes(head.try_into().unwrap())
    }
    let native = fs::read("work/frontend/native-rights-caption-layout.bin").unwrap();
    assert_eq!(&native[..8], b"MTRCPT01");
    let mut bytes = &native[8..];
    assert_eq!(word(&mut bytes), 0x037f);
    let count = word(&mut bytes) as usize;
    let caption = &bytes[..count];
    bytes = &bytes[count..];
    let (offsets, _) =
        crate::prompt::wrap_with_gap(&tab, caption.strip_suffix(&[0]).unwrap_or(caption), 88, -1)
            .unwrap();
    assert_eq!(offsets, [0, 18, 25]);
    let mut pages = [Surface::solid(), Surface::solid()];
    paint_rights_caption(&mut pages, &tab, caption, [0., 0., 88., 25.], 0xffffffff).unwrap();
    assert_eq!(word(&mut bytes) as usize, pages.len());
    for page in &pages {
        let count = word(&mut bytes);
        let positions = (0..count).map(|_| word(&mut bytes)).collect::<Vec<_>>();
        let count = word(&mut bytes);
        let uvs = (0..count).map(|_| word(&mut bytes)).collect::<Vec<_>>();
        assert_eq!(page.vertices.len(), positions.len() / 12 * 6);
        for (quad, vertices) in page.vertices.chunks_exact(6).enumerate() {
            for (&corner, actual) in [0, 1, 2, 0, 2, 3].iter().zip(vertices) {
                for (axis, value) in actual[..3].iter().enumerate() {
                    let expected = positions[(quad * 4 + corner) * 3 + axis];
                    assert_eq!(
                        value.to_bits(),
                        expected,
                        "native position quad{quad} axis{axis}"
                    );
                }
                for (axis, value) in actual[6..].iter().enumerate() {
                    let expected = uvs[(quad * 4 + corner) * 2 + axis];
                    assert_eq!(value.to_bits(), expected, "native UV quad{quad} axis{axis}");
                }
            }
        }
    }
    assert!(bytes.is_empty());
}

#[test]
#[ignore = "private original property GPU/input capture"]
fn property_reports_and_offer_input() {
    use openmt2001::render_scene::{OriginalAssets, Request, Source, assemble};
    let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
    let exe = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build = read("OPENMT2001_BUILD_FILE");
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let off = fs::read(language.join("tycoon.off")).unwrap();
    let bin = fs::read(language.join("tycoon.bin")).unwrap();
    let mut offset = 0;
    let strings = off
        .chunks_exact(2)
        .map(|n| {
            let len = u16::from_le_bytes(n.try_into().unwrap()) as usize;
            let text = &bin[offset..offset + len];
            offset += len;
            text
        })
        .collect::<Vec<_>>();
    let sun = openmt2001::fixtures::path("SUNCOLS");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let assets = Assets::load(
        &index,
        &archive,
        sun.parent().unwrap().parent().unwrap(),
        &strings,
        &exe,
    )
    .unwrap();

    let image = PeImage::parse(&exe).unwrap();
    let save = read("OPENMT2001_SAVE_ONE");
    let original = openmt2001::save::SaveFile::parse(&save).unwrap();
    let mut report = crate::block_information::Report::from_save(&original, &image)
        .unwrap()
        .unwrap();
    let fixture = read("OPENMT2001_NATIVE_PROPERTY_REPORTS");
    let mut at = 24;
    fn records<const N: usize>(b: &[u8], at: &mut usize, n: usize) -> Vec<[u8; N]> {
        (0..n)
            .map(|_| {
                let r = b[*at..*at + N].try_into().unwrap();
                *at += N;
                r
            })
            .collect()
    }
    let categories: [[u8; 380]; 52] = records(&fixture, &mut at, 52).try_into().unwrap();
    let buildings = records(&fixture, &mut at, 600);
    let businesses = records(&fixture, &mut at, 600);
    let info = records(&fixture, &mut at, 45);
    let products = std::array::from_fn(|_| std::array::from_fn(|i| [i as u32, 0]));
    report.businesses = Some(
        crate::block_information::business::Report::from_saved_indices(
            &crate::block_information::business::SavedInputs {
                categories: &categories,
                buildings: &buildings,
                businesses: &businesses,
                info: &info,
                products: &products,
                hour: 6,
                tick: 0,
            },
            report.names.len(),
            &image,
        )
        .unwrap(),
    );
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let mut fresh_categories = [[0; 380]; 52];
    openmt2001::ai_neighbors::bind_geometry(&mut fresh_categories, &board);
    for block in &board.blocks {
        let id = block.id;
        let actual = u32::from_le_bytes(fresh_categories[id][0x94..0x98].try_into().unwrap());
        let native = u32::from_le_bytes(categories[id][0x94..0x98].try_into().unwrap());
        assert_ne!(actual, 0);
        assert_eq!(actual, native);
        report.title_colors[id] = actual;
    }
    // Saved native74 is disabled here. The offer capture uses an explicit
    // permitted native display input; real legal Host admission is a separate gate.
    report.lease_available[36] = true;
    assert!(
        !assets.currency_suffix,
        "supplied USA board declares prefix currency"
    );
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(target_os = "windows") {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let mut scene = assemble(Request {
        build_file: &build,
        source: Source::Save(&save),
        original: Some(OriginalAssets {
            executable: &exe,
            directory: &directory,
            archive: &archive,
        }),
        suncols: None,
        phase: None,
        camera: Some(
            crate::navigation::Navigation::load(&index, &archive, &exe)
                .unwrap()
                .camera,
        ),
        width: 1280,
        height: 960,
        animated: false,
    })
    .unwrap();
    // Production picker returns a compact geometry-vector index. Native report
    // identities remain52-wide with holes40/45. Suppress building hit targets
    // only for this pavement geometry/ID-boundary check; rendered scene unchanged.
    let selection = crate::selection::Assets::load(&exe, &index, &archive).unwrap();
    let camera = crate::navigation::Navigation::load(&index, &archive, &exe)
        .unwrap()
        .camera;
    let original_buildings = std::mem::take(&mut scene.inspection_buildings);
    let matrix = openmt2001::camera::scene_view_with_up(camera.eye, camera.target, camera.up);
    let [near, far] = openmt2001::camera::city_clip(camera.eye[1]).unwrap();
    let projection =
        openmt2001::picking::QueryProjection::new(1280., 960., 45., near, far).unwrap();
    let (hit, native) = selection
        .blocks
        .iter()
        .enumerate()
        .filter(|(index, b)| {
            b.id > 40
                && *index != b.id
                && b.flags & 1 == 0
                && assets.property_lease_available[b.id]
                && (report.categories[b.id].is_some()
                    || report.utility_categories.contains(&b.id)
                    || report.railroad_categories.contains(&b.id))
        })
        .find_map(|(index, b)| {
            let bounds = openmt2001::picking::category_bounds(
                scene.category_model_heights.get(&b.id).copied(),
            )
            .unwrap();
            let point = openmt2001::camera::transform_scene_point(
                matrix,
                [b.center[0], bounds[2][1] * 0.5, b.center[1]],
            );
            let screen = projection
                .project_coordinates(point)
                .map(|v| v.round() as i32);
            let hit = selection.pick(camera, &scene, screen, None)?;
            (hit.category == index && hit.building.is_none()).then_some((hit, b.id))
        })
        .expect("original source camera can pick an income property beyond native holes");
    assert_ne!(hit.category, native);
    let category =
        crate::intro::utility_report_category(Some(hit), &selection.blocks, Some(&report)).unwrap();
    assert_eq!(category, native);
    let mut mapped = crate::utility_report::View::default();
    mapped.select(Some(category));
    let b = assets.property_side_bounds[1];
    assets.property_pointer(
        &mut mapped,
        Some([(b[0] + b[2]) as i32, (b[1] + b[3]) as i32]),
        &report,
        category,
    );
    assert!(mapped.mouse(true));
    assert!(mapped.mouse(false));
    assert_eq!(
        mapped.take_property_request().unwrap().category,
        native as u32
    );
    scene.inspection_buildings = original_buildings;
    let category = 36;
    let mut view = crate::utility_report::View::default();
    view.select(Some(category));
    let center = |b: [f32; 4]| [(b[0] + b[2]) as i32, (b[1] + b[3]) as i32];
    let click = |view: &mut crate::utility_report::View, b: [f32; 4], id| {
        assets.property_pointer(view, Some(center(b)), &report, category);
        assert_eq!(
            assets.property_tutorial_control_at(view, Some(center(b)), &report, category),
            Some(id)
        );
        assert!(view.mouse(true));
        assert!(view.mouse(false));
        assert_eq!(view.take_tutorial_control(), Some(id));
    };
    for tab in 0..5 {
        click(&mut view, assets.city_layout.buttons[tab], 15 + tab as u32);
        assert_eq!(view.property_tab, tab);
        let (_panel, _) = assets
            .property_panel(&renderer, &report, category, &view)
            .unwrap();
        #[cfg(feature = "capture")]
        crate::capture_test::capture(
            &renderer,
            Some(&_panel),
            &scene,
            &format!("work/frontend/property-tab-{tab}.png"),
        );
        if matches!(tab, 2 | 3) {
            for i in 0..2 {
                click(&mut view, assets.period_bounds[i], 33 + i as u32);
                assert_eq!(
                    if tab == 2 {
                        view.previous_profit
                    } else {
                        view.previous_efficiency
                    },
                    i == 0
                );
            }
        }
    }
    click(&mut view, assets.property_side_bounds[1], 10);
    let request = view.take_property_request().unwrap();
    assert!(!request.submit);
    assert!(request.initialize_amount);
    let panel = openmt2001::property_offer::Panel {
        visible: 1,
        amount: 1000.,
        minimum: 1000.,
        maximum: 100000.,
        step: 100.,
        amount_visible: 1,
        action_visible: 1,
        ..Default::default()
    };
    view.apply_property_inspection(
        category as u32,
        Ok(crate::fresh_city::property::Inspection {
            category: category as u32,
            owned: false,
            panel,
            submitted: false,
        }),
    );
    assets.property_pointer(
        &mut view,
        Some(center(assets.amount_arrow_bounds[1])),
        &report,
        category,
    );
    assert!(view.mouse(true));
    assert!(view.mouse(false));
    assert!(view.take_tutorial_control().is_none());
    let changed = view.take_property_request().unwrap();
    assert!(!changed.initialize_amount);
    assert_eq!(changed.panel.amount, 1100.);
    for pressed in [false, true] {
        assets.property_pointer(
            &mut view,
            Some(center(assets.offer_button_bounds)),
            &report,
            category,
        );
        if pressed {
            assert!(view.mouse(true));
        }
        let (_panel, _) = assets
            .property_panel(&renderer, &report, category, &view)
            .unwrap();
        #[cfg(feature = "capture")]
        crate::capture_test::capture(
            &renderer,
            Some(&_panel),
            &scene,
            &format!(
                "work/frontend/property-offer-{}.png",
                if pressed { "pressed" } else { "released" }
            ),
        );
    }
    assets.property_pointer(&mut view, None, &report, category);
    view.mouse(false);
    assert!(view.take_property_request().is_none());
    assert!(view.take_tutorial_control().is_none());
    click(&mut view, assets.offer_button_bounds, 35);
    view.refresh_property_offer();
    let submit = view.take_property_request().unwrap();
    assert!(submit.submit);
    assert_eq!(submit.panel.amount, 1100.);
    let mut owned = panel;
    owned.amount = 500.;
    owned.amount_visible = 0;
    view.apply_property_inspection(
        category as u32,
        Ok(crate::fresh_city::property::Inspection {
            category: category as u32,
            owned: true,
            panel: owned,
            submitted: false,
        }),
    );
    let mut owned_report = report.clone();
    owned_report.owners[category] = 1;
    let (_owned_panel, _) = assets
        .property_panel(&renderer, &owned_report, category, &view)
        .unwrap();
    #[cfg(feature = "capture")]
    crate::capture_test::capture(
        &renderer,
        Some(&_owned_panel),
        &scene,
        "work/frontend/property-sell-lease.png",
    );
    assets.property_pointer(
        &mut view,
        Some(center(assets.amount_arrow_bounds[1])),
        &report,
        category,
    );
    assert!(view.property_control_hover.is_none());

    click(&mut view, assets.property_side_bounds[0], 9);
    assert!(!view.property_offer_open());
    click(&mut view, assets.property_side_bounds[2], 11);
    let request = view.take_rights_request().unwrap();
    assert!(!request.activate);
    assert_eq!(request.panel_mode, 0);
    let gate = openmt2001::property_rights::inspect_values(0, true, 0, 0);
    assert_eq!(gate.message, Some(1202));
    view.apply_rights_inspection(category as u32, Ok(gate));
    for pressed in [false, true] {
        assets.property_pointer(
            &mut view,
            Some(center(assets.rights_button_bounds)),
            &report,
            category,
        );
        assert_eq!(view.property_control_hover, Some(102));
        if pressed {
            assert!(view.mouse(true));
        }
        let (_panel, _) = assets
            .property_panel(&renderer, &report, category, &view)
            .unwrap();
        #[cfg(feature = "capture")]
        crate::capture_test::capture(
            &renderer,
            Some(&_panel),
            &scene,
            &format!(
                "work/frontend/property-rights-{}.png",
                if pressed { "pressed" } else { "released" }
            ),
        );
    }
    assets.property_pointer(&mut view, None, &report, category);
    view.mouse(false);
    assert!(view.take_rights_request().is_none());
    assets.property_pointer(
        &mut view,
        Some(center(assets.rights_button_bounds)),
        &report,
        category,
    );
    view.mouse(true);
    view.mouse(false);
    view.refresh_property_offer();
    assert!(view.take_rights_request().unwrap().activate);
    assert!(view.take_tutorial_control().is_none());
    view.apply_rights_inspection(
        category as u32,
        Ok(openmt2001::property_rights::inspect_values(1, true, 0, 0)),
    );
    assets.property_pointer(
        &mut view,
        Some(center(assets.rights_button_bounds)),
        &report,
        category,
    );
    assert!(view.property_control_hover.is_none());
    view.apply_rights_inspection(
        category as u32,
        Ok(openmt2001::property_rights::inspect_values(0, true, 1, 0)),
    );
    assert_eq!(view.take_rights_open(), Some(category));
    assert!(!view.property_rights_open());
}
