//! Controlled material identities, plus an ignored supplied Grocery descriptor join.
use super::*;

// Minimal initialized PE sections containing only the recovered selector tables.
// Synthetic transport bytes; this is not an executable or original payload.
fn selector_image() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x500];
    let mut put = |at: usize, value: u32| bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    put(0x3c, 0x80);
    put(0x80, 0x0000_4550);
    put(0x98 + 28, 0x400000);
    // Two sections, with compact raw data and their original preferred VAs.
    for (at, rva, size, raw) in [(0xb8, 0x13a254, 44, 0x200), (0xe0, 0x141af8, 400, 0x300)] {
        put(at + 12, rva); put(at + 16, size); put(at + 20, raw);
    }
    for (i, page) in [45, 46, 47, 48, 49, 54, 53, 50, 51, 52, 55].into_iter().enumerate() {
        put(0x200 + i * 4, page);
    }
    for (i, page) in [1, 0, 2, 3, 4, 5, 6, 7, 8, 9].into_iter().enumerate() {
        put(0x300 + i * 40, page);
    }
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    bytes
}

fn marked_assets() -> Assets {
    let pages = |bank: u8, count| (0..count).map(|index| TexturePage {
        width: 1, height: 1, rgba: Arc::from(vec![index as u8, bank, 0, 255]),
    }).collect();
    Assets { generic: pages(1, 16), generic640: pages(2, 287),
        building: pages(3, 26), signs: pages(4, 56), font4: Vec::new(),
        font4_tab: Vec::new(), business_value: Vec::new(), language_rows: Vec::new() }
}
fn controlled_banner() -> RendererTemplate {
    let mut row = [0u8; 52];
    for (at, value) in [(0, 1), (4, 0x0108_0202), (44, 10f32.to_bits()), (48, 5f32.to_bits())] {
        row[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    RendererTemplate { detail_slot: 2, descriptor: 0x0108_0202, row }
}
fn payload(selection: Option<BannerSelection>) -> RefreshLabels {
    RefreshLabels { owner: Vec::new(), business: Vec::new(), banner: selection }
}
fn assert_pages(draws: &[OwnedDraw], assets: &Assets, pages: (usize, usize, Option<usize>)) {
    let mut expected = vec![&assets.signs[pages.0], &assets.generic640[pages.1]];
    if let Some(page) = pages.2 { expected.push(&assets.signs[page]); }
    assert_eq!(draws.len(), expected.len());
    for (draw, page) in draws.iter().zip(expected) {
        assert_eq!((draw.width, draw.height), (page.width, page.height));
        assert_eq!(draw.rgba, page.rgba);
        assert_eq!(draw.vertices.len(), 12, "two-sided surface group");
        assert!(draw.vertices.iter().flatten().all(|value| value.is_finite()));
    }
}

#[test]
fn banner_registry_owner_changes_replace_materials_without_reusing_initial_selectors() {
    let bytes = selector_image(); let image = PeImage::parse(&bytes).unwrap();
    let assets = marked_assets(); let row = controlled_banner();
    let identity = ModelIdentity { descriptor: 814, renderer_candidates: Vec::new(),
        renderer_templates: vec![row] };
    let adapter = std::cell::RefCell::new(Adapter::default());
    let mut registry = RendererRegistry::prepare(&identity, |row|
        adapter.borrow_mut().construct(row, 10, Mat4::IDENTITY, &image, &assets)).unwrap();
    let handle = |registry: &RendererRegistry| u32::from_le_bytes(
        registry.rows[0].row[40..44].try_into().unwrap());
    assert_pages(adapter.borrow().draws(handle(&registry)).unwrap(), &assets, (45, 1, Some(0)));
    for (style, business, expected) in [
        (3, 5, (48, 3, Some(5))), // Actual supplied Grocery identity.
        (0, 44, (45, 1, Some(44))), // Last admitted business page.
        (4, 12, (49, 4, Some(12))), // Other owner, not a green override.
        (10, 5, (55, 286, Some(5))), // No owner has its own portrait fallback.
        (3, -1, (48, 3, None)), // Missing business omits the lower pair.
        (3, 5, (48, 3, Some(5))),
    ] {
        let old = handle(&registry);
        let labels = payload(Some(BannerSelection::refreshed(style, business).unwrap()));
        registry.refresh(|old| adapter.borrow_mut().release(old), |row|
            adapter.borrow_mut().construct_with_labels(row, style, Mat4::IDENTITY,
                &image, &assets, Some(&labels))).unwrap();
        assert!(adapter.borrow().draws(old).is_none(), "old banner must be released");
        assert_ne!(handle(&registry), old);
        assert_pages(adapter.borrow().draws(handle(&registry)).unwrap(), &assets, expected);
    }
}

#[test]
fn banner_refuses_unclaimed_or_out_of_domain_refresh_instead_of_guessing() {
    for (owner, business) in [(11, 5), (3, -2), (3, 45)] {
        assert!(BannerSelection::refreshed(owner, business).is_err());
    }
    assert!(BannerSelection::refreshed(0, 0).is_ok());
    assert!(BannerSelection::refreshed(10, 44).is_ok());
    let bytes = selector_image(); let image = PeImage::parse(&bytes).unwrap();
    let mut assets = marked_assets(); let mut adapter = Adapter::default();
    let labels = payload(None);
    assert!(adapter.construct_with_labels(&controlled_banner(), 3, Mat4::IDENTITY,
        &image, &assets, Some(&labels)).is_err());
    assets.generic640.truncate(286);
    let labels = payload(Some(BannerSelection::refreshed(10, -1).unwrap()));
    assert!(adapter.construct_with_labels(&controlled_banner(), 10, Mat4::IDENTITY,
        &image, &assets, Some(&labels)).is_err(), "missing no-owner page must refuse");
}

#[test]
#[ignore = "requires supplied v1.4 PE, SAVE_ONE, BuildFile and archive; no GPU"]
fn supplied_grocery_banner_uses_owner_and_business_pages_through_restored_registry() {
    use openmt2001::{board::Board, build_file::BuildFile, save::SaveFile};
    let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
    let exe = read("OPENMT2001_EXE"); let image = PeImage::parse(&exe).unwrap();
    let save_bytes = read("OPENMT2001_SAVE_ONE"); let save = SaveFile::parse(&save_bytes).unwrap();
    let buildings = save.buildings().unwrap(); let source = buildings.iter().find(|b| b.slot == 12).unwrap();
    assert_eq!(source.category, 36); assert_eq!(source.bytes[0x2d], 1);
    let metadata = save.players().unwrap()[0].metadata;
    assert_eq!(u32::from_le_bytes(metadata[..4].try_into().unwrap()), 3);
    assert_eq!(&metadata[8..15], b"Justin2");
    let economy = save.economy().unwrap(); let business = economy.businesses().nth(12).unwrap();
    assert_eq!(business.building_index(), 12); assert_eq!(business.business_type(), 5);
    let file_bytes = read("OPENMT2001_BUILD_FILE"); let file = BuildFile::parse(&file_bytes).unwrap();
    let board = Board::from_v14(&image).unwrap();
    let building = SceneBuilding::from_saved(source, &board, &file).unwrap();
    let mut identity = openmt2001::save::appearance::model_identity(&file,
        building.selected, source.height()).unwrap();
    assert_eq!(identity.descriptor, 814);
    identity.renderer_templates.retain(|row| row.descriptor == 0x0108_0202);
    assert_eq!(identity.renderer_templates.len(), 1);
    assert_eq!(identity.renderer_templates[0].detail_slot, 2);
    // Isolate the supplied roof descriptor; no synthetic text/font fixture.
    let mut buildings = BTreeMap::from([(12, (building, identity, 3))]);
    let assets = Assets::from_original(&read("OPENMT2001_ARCHIVE_DIR"),
        &read("OPENMT2001_ARCHIVE_BIN")).unwrap();
    let labels = BTreeMap::from([(12, payload(Some(BannerSelection::refreshed(3, 5).unwrap())))]);
    let (initial, replacements, _) = restored_draws_with_spans(
        &buildings, &[], &image, &assets, &labels).unwrap();
    assert_eq!(replacements, 0);
    assert_pages(&initial, &assets, (45, 1, Some(0)));
    let (refreshed, replacements, spans) = restored_draws_with_spans(
        &buildings, &[12], &image, &assets, &labels).unwrap();
    assert_eq!(replacements, 1); assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].0, 12); assert_eq!(spans[0].1, 0..3);
    assert_pages(&refreshed, &assets, (48, 3, Some(5)));
    assert_ne!(initial[0].rgba, refreshed[0].rgba);
    assert_ne!(initial[1].rgba, refreshed[1].rgba);
    assert_ne!(initial[2].rgba, refreshed[2].rgba);
    for (style, kind, expected) in [(10, 5, (55, 286, Some(5))), (3, -1, (48, 3, None))] {
        buildings.get_mut(&12).unwrap().2 = style;
        let labels = BTreeMap::from([(12, payload(Some(BannerSelection::refreshed(style, kind).unwrap())))]);
        let (draws, replacements, spans) = restored_draws_with_spans(
            &buildings, &[12], &image, &assets, &labels).unwrap();
        assert_eq!(replacements, 1);
        assert_eq!(spans[0].1.len(), if kind == -1 { 2 } else { 3 });
        assert_pages(&draws, &assets, expected);
    }
    assert_eq!(awning_material_indices(&image, 3).unwrap(), (7, 18));
}
