//! Supplied-original component regressions. This is not a native
//! startup, allocation/unwind, raster, election, input or timer acceptance.
use super::super::super::voting_construction::{self as voting, Effect, Label};
use super::*;
const V: u32 = 0x573b38;
fn controlled(input: &Inputs) -> openmt2001::level_startup::World {
    let mut w = input.world();
    for i in 0..52 {
        row(&mut w, i, 0, 1, 46);
    }
    row(&mut w, 41, 0, 1, 0);
    w
}
fn words(p: &Primitives, offset: u32) -> [u32; 5] {
    std::array::from_fn(|i| p.cells().word(V + offset + 4 * i as u32).unwrap())
}
///Independent source asset reader: no CpuModel::find/region or candidate
///lookup, phase, child factory or stored Cells extent generates expectations.
///The original records associate named part IDs with indexed wire vertices.
fn original_region(input: &Inputs, name: &[u8]) -> [f32; 4] {
    let e = input
        .index
        .find("gamedata2/brand_new_panels/voting_panel.tm")
        .unwrap();
    assert_eq!((e.offset, e.length), (3918431, 4263));
    let mesh = CachedMesh::parse(&input.archive[e.offset as usize..(e.offset + e.length) as usize])
        .unwrap();
    assert_eq!((mesh.frames, mesh.parts.len()), (1, 18));
    let record = mesh
        .opaque_records_35
        .chunks_exact(35)
        .find(|r| {
            let end = r[..33].iter().position(|&b| b == 0).unwrap();
            r[..end].eq_ignore_ascii_case(name)
        })
        .unwrap();
    let positions = mesh.positions(0).unwrap();
    let part = &mesh.parts[usize::from(record[33])];
    let mut x = Vec::new();
    let mut y = Vec::new();
    for index in part.vertex_indices() {
        x.push(positions[index][0]);
        y.push(-positions[index][1]);
    }
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    y.sort_by(|a, b| a.partial_cmp(b).unwrap());
    [x[0], y[0], *x.last().unwrap(), *y.last().unwrap()]
}
fn check_static_nonwriters(primary: &DeferredPrimary, p: &Primitives) {
    let pre = &primary.voting().static_preimage;
    for offset in [0x4e0, 0x5a0, 0x5bc] {
        assert_eq!(
            p.cells().read(V + offset, 4).unwrap(),
            &pre[offset as usize..offset as usize + 4]
        );
    }
    for offset in [0x428, 0x4b4, 0x570] {
        assert_eq!(words(p, offset), [0, 0, 0, 0, 0x3f000000]);
    }
}
#[test]
#[ignore = "supplied PE/static constructor write set and move-stable owners"]
fn birth_has_seven_records_preserves_nonwriters_and_one_root_caption() {
    let input = Inputs::read();
    let (primary, p, text, images) = input.fresh();
    let v = primary.voting();
    assert_eq!(v.phase(), voting::Phase::Unbound);
    assert!(v.nodes().next().is_none());
    assert!(v.children().is_empty());
    for base in [0x3b0, 0x43c, 0x4f8] {
        for i in 0..7 {
            assert_eq!(words(&p, base + 20 * i), [0, 0, 0, 0, 0x3f000000]);
        }
    }
    for offset in [0x388, 0x39c, 0x4c8, 0x4e4] {
        assert_eq!(words(&p, offset), [0, 0, 0, 0, 0x3f000000]);
    }
    let loader = super::super::super::process_image::Cells::process_allocation(&input.exe).unwrap();
    //Generic PE file-copy/virtual-tail allocation, independent of439df0.
    for offset in [
        0x4dc, 0x4e0, 0x584, 0x588, 0x5a0, 0x5a4, 0x5bc, 0x5c0, 0x5c4,
    ] {
        assert_eq!(
            p.cells().read(V + offset, 4).unwrap(),
            loader.read(V + offset, 4).unwrap()
        );
    }
    assert_eq!(p.panels[&V].caption, V + 0x188);
    assert_eq!(v.caption_source, V + 0x188);
    let root_caption = &p.texts[&(V + 0x188)] as *const _;
    let product = v as *const _;
    let moved = Box::new((primary, p, text, images));
    assert_eq!(moved.0.voting() as *const _, product);
    assert_eq!(&moved.1.texts[&(V + 0x188)] as *const _, root_caption);
    assert_unbound(&moved.1);
}
#[test]
#[ignore = "supplied v1.4 language/model/Images and actual protected Install"]
fn complete_layout_uses_original_asset_geometry_and_actual_retained_children() {
    let input = Inputs::read();
    let (primary, p, text, mut images) = input.fresh();
    let w = controlled(&input);
    images.records[79].scale = Some(0.75);
    images.records[143].scale = Some(1.25);
    let currency_neighbor = p.cells().word(0x57f734).unwrap();
    let product = primary.voting() as *const _;
    let root_caption = &p.texts[&(V + 0x188)] as *const _;
    let fonts = text.fonts() as *const _;
    let image_owner = &images.records[0] as *const _;
    let image_scales = images.records.iter().map(|r| r.scale).collect::<Vec<_>>();
    let model = p
        .resources
        .model(PanelId::OtherPrimary(1))
        .unwrap()
        .frames
        .as_ptr();
    let (primary, p, text, images, result) = run(primary, p, text, images, &w);
    assert_eq!(
        result.unwrap(),
        InstallDisposition::CompletedChat {
            epoch: w.topology.epoch().serial()
        }
    );
    let v = primary.voting();
    assert_eq!(v as *const _, product);
    assert_eq!(v.phase(), voting::Phase::Registered);
    assert_eq!(primary.phase(), PrimaryPhase::AwaitingG);
    assert_unbound(&p);
    assert_eq!(&p.texts[&(V + 0x188)] as *const _, root_caption);
    assert_eq!(p.texts[&(V + 0x188)].style, 4);
    assert_eq!(text.fonts() as *const _, fonts);
    assert_eq!(&images.records[0] as *const _, image_owner);
    assert_eq!(
        images.records.iter().map(|r| r.scale).collect::<Vec<_>>(),
        image_scales
    );
    assert_eq!(
        p.resources
            .model(PanelId::OtherPrimary(1))
            .unwrap()
            .frames
            .as_ptr(),
        model
    );
    assert_eq!(
        (
            p.cells().word(G + 8).unwrap(),
            p.cells().word(G + 12).unwrap(),
            p.cells().word(G + 0x24).unwrap()
        ),
        (0x57ef28, D, 17)
    );
    assert_eq!(
        (
            p.cells().word(V + 0x380).unwrap(),
            p.cells().word(V + 0x384).unwrap(),
            p.cells().word(D + 0x384).unwrap()
        ),
        (D, 0x571378, V)
    );
    assert_eq!(p.cells().byte(V + 0x184).unwrap(), 1);
    assert_eq!(p.cells().word(V + 0x180).unwrap(), 1);
    assert_eq!(v.children().len(), 15);
    assert_eq!(v.nodes().count(), 15);
    //Frozen original factory/key order, not an expected vector made by the
    //candidate's factory or inferred from the number of existing aliases.
    let labels = [
        Label::Ok,
        Label::Percentage(0),
        Label::Count(0),
        Label::Percentage(1),
        Label::Count(1),
        Label::Percentage(2),
        Label::Count(2),
        Label::Percentage(3),
        Label::Count(3),
        Label::Percentage(4),
        Label::Count(4),
        Label::Percentage(5),
        Label::Count(5),
        Label::TitleLabel,
        Label::Title,
    ];
    for (key, id) in v.children().iter().copied().enumerate() {
        let n = v.node(id).unwrap();
        assert_eq!(n.label, labels[key]);
        assert_eq!(n.number, Some(key as u32));
        assert_eq!(n.event_sink, V);
        assert_eq!(n.frame, V);
        assert_eq!(n.depth(), 0.5);
        assert_eq!(n.next, v.children().get(key + 1).copied());
        assert_eq!(n.allocation_size, if key == 0 { 0x374 } else { 0x1b4 });
    }
    let mut expected_names = vec![
        b"VOTING_TITLE_AREA".to_vec(),
        b"VOTING_MAIN_BACKING".to_vec(),
    ];
    for i in 1..=6 {
        expected_names.push(format!("VOTING_TOKEN{i}").into_bytes());
        expected_names.push(format!("VOTING_ROSETTE{i}").into_bytes());
    }
    expected_names.extend([
        b"VOTING_GRAPH_AREA".to_vec(),
        b"VOTING_OK_BUTTON".to_vec(),
        b"VOTING_TITLE_LABEL".to_vec(),
        b"VOTING_TITLE_AREA".to_vec(),
    ]);
    let actual = v
        .effects()
        .iter()
        .filter_map(|e| match e {
            Effect::Query { name, .. } => Some(p.cells().ntbs(*name, 256).unwrap()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_names);
    assert_eq!(actual.len(), 18);
    let graph = original_region(&input, b"Voting_Graph_Area");
    for i in 0..6usize {
        let token = original_region(&input, format!("Voting_Token{}", i + 1).as_bytes());
        let rosette = original_region(&input, format!("Voting_Rosette{}", i + 1).as_bytes());
        assert_eq!(
            &words(&p, 0x3b0 + 20 * i as u32)[..4],
            &token.map(f32::to_bits)
        );
        assert_eq!(
            &words(&p, 0x43c + 20 * i as u32)[..4],
            &rosette.map(f32::to_bits)
        );
        let percent = v.alias(0x588 + 4 * i as u32).unwrap();
        let count = v.alias(0x5a4 + 4 * i as u32).unwrap();
        assert_eq!(percent, v.children()[1 + 2 * i]);
        assert_eq!(count, v.children()[2 + 2 * i]);
        assert_eq!(
            v.text(percent).unwrap().bounds,
            [rosette[0] + 3., graph[3], rosette[2], graph[3] + 15.]
        );
        assert_eq!(
            v.text(count).unwrap().bounds,
            [rosette[0], rosette[1] - 4., rosette[2], rosette[3] - 4.]
        );
        assert_eq!(v.text(count).unwrap().visible, 64); //direct store, not extent setter
        assert_eq!(v.text(count).unwrap().font, 1);
        assert_eq!(v.text(percent).unwrap().font, 0);
        let alphabet = text.fonts().language.row(0).unwrap();
        assert_eq!(
            v.text(percent).unwrap().group.row(0).unwrap(),
            &[
                alphabet[usize::from(b'0' - 32)],
                alphabet[usize::from(b'%' - 32)]
            ]
        );
        assert_eq!(
            v.text(count).unwrap().group.row(0).unwrap(),
            &[alphabet[usize::from(b'0' - 32)]]
        );
        assert_eq!(
            words(&p, 0x4f8 + 20 * i as u32),
            [
                rosette[0].to_bits(),
                graph[3].to_bits(),
                rosette[2].to_bits(),
                graph[3].to_bits(),
                0x3f000000
            ]
        );
    }
    let ok = v.alias(0x4dc).unwrap();
    let visual = v.visual(ok).unwrap();
    assert_eq!(
        v.node(ok).unwrap().bounds(),
        original_region(&input, b"Voting_OK_Button")
    );
    for (offset, ordinal) in [
        (0x300, 79),
        (0x304, 81),
        (0x308, 143),
        (0x30c, 80),
        (0x348, 79),
    ] {
        assert_eq!(
            images
                .record(match visual.images[&offset] {
                    super::super::super::visual_construction::ImageRef::Image(id) => id,
                    _ => panic!("actual image reference missing"),
                })
                .unwrap()
                .ordinal,
            ordinal
        );
    }
    assert_eq!(
        visual.images[&0x310],
        super::super::super::visual_construction::ImageRef::Invalid
    );
    assert_eq!(visual.scalars[&0x13c], 3);
    assert_eq!(visual.scalars[&0x33c], 0);
    assert!(!visual.scalars.contains_key(&0x2f4));
    for offset in (0x350..=0x364).step_by(4) {
        assert!(!visual.scalars.contains_key(&offset));
    }
    assert_eq!(v.text(ok).unwrap().font, 2);
    assert_eq!(v.text(ok).unwrap().style, 4);
    let label = v.alias(0x5c0).unwrap();
    assert_eq!(v.text(label).unwrap().group.count, 0);
    assert_eq!(v.text(label).unwrap().font, 3);
    let title = v.alias(0x5c4).unwrap();
    assert_eq!(v.text(title).unwrap().font, 1);
    assert!(v.text(title).unwrap().group.count > 0);
    assert_eq!(p.cells().word(V + 0x584).unwrap(), u32::MAX);
    assert_eq!(p.cells().word(V + 0x350).unwrap(), 1);
    assert_eq!(text.fonts().selector.cached, 3);
    assert_eq!(
        p.cells()
            .read(0x57e6ec, primary.currency().symbol.len())
            .unwrap(),
        primary.currency().symbol
    );
    assert_eq!(p.cells().word(0x59050c).unwrap(), primary.currency().suffix);
    assert_eq!(p.cells().word(0x57f734).unwrap(), currency_neighbor);
    assert_eq!(p.steps.iter().filter(|s| **s == Step::Currency).count(), 1);
    for record in &p
        .resources
        .model(PanelId::OtherPrimary(1))
        .unwrap()
        .records35
    {
        let n = record[..33].iter().position(|&b| b == 0).unwrap();
        assert!(!record[..n].iter().any(u8::is_ascii_lowercase));
    }
    assert_eq!(
        v.text(label).unwrap().bounds,
        original_region(&input, b"Voting_Title_Label")
    );
    assert_eq!(
        v.text(title).unwrap().bounds,
        original_region(&input, b"Voting_Title_Area")
    );
    check_static_nonwriters(&primary, &p);
    assert_eq!(v.alias(0x5a0), None);
    assert_eq!(v.alias(0x5bc), None);
    assert!(primary.require_supplier().is_err());
    assert!(primary.require_ordered_main().is_err());
}
fn failed_cut(
    input: &Inputs,
    missing_font: u32,
    expected_children: usize,
) -> (DeferredPrimary, Primitives, UiTextResources, Images) {
    let (primary, p, mut text, images) = input.fresh();
    let font = text.fonts_mut().fonts.remove(&missing_font).unwrap();
    let w = controlled(input);
    let (primary, p, mut text, images, result) = run(primary, p, text, images, &w);
    assert!(result.unwrap_err().contains("not registered"));
    assert_eq!(primary.phase(), PrimaryPhase::Failed);
    assert_eq!(primary.voting().phase(), voting::Phase::Failed);
    assert_eq!(primary.voting().children().len(), expected_children);
    assert_eq!(p.cells().byte(V + 0x184).unwrap(), 0);
    assert_eq!(
        (
            p.cells().word(G + 8).unwrap(),
            p.cells().word(G + 0x24).unwrap()
        ),
        (D, 17)
    );
    assert!(!p.steps.contains(&Step::Registered(V)));
    assert!(!p.steps.contains(&Step::LayoutReturned(V)));
    assert_unbound(&p);
    assert_eq!(
        p.cells().word(V + 0x584).unwrap(),
        u32::from_le_bytes(
            primary.voting().static_preimage[0x584..0x588]
                .try_into()
                .unwrap()
        )
    );
    assert_eq!(p.cells().word(V + 0x350).unwrap(), 2);
    check_static_nonwriters(&primary, &p);
    let children = primary.voting().children().to_vec();
    let steps = p.steps.clone();
    let effects = primary.voting().effects().to_vec();
    let scratch = text.fonts().scratch.clone();
    let model_records = p
        .resources
        .model(PanelId::OtherPrimary(1))
        .unwrap()
        .records35
        .clone();
    text.fonts_mut().fonts.insert(missing_font, font);
    let (primary, p, text, images, retry) = run(primary, p, text, images, &w);
    assert!(retry.is_err());
    assert_eq!(primary.voting().children(), children);
    assert_eq!(primary.voting().effects(), effects);
    assert_eq!(p.steps, steps);
    assert_eq!(text.fonts().scratch, scratch);
    assert_eq!(
        p.resources
            .model(PanelId::OtherPrimary(1))
            .unwrap()
            .records35,
        model_records
    );
    (primary, p, text, images)
}
#[test]
#[ignore = "actual missing-font2 failure after OK alias"]
fn failed_ok_font_retains_visual_caption_before_any_row() {
    let input = Inputs::read();
    let (primary, _, _, _) = failed_cut(&input, 2, 1);
    let v = primary.voting();
    let ok = v.alias(0x4dc).unwrap();
    assert_eq!(v.text(ok).unwrap().font, 2);
    assert_eq!(v.text(ok).unwrap().group.count, 0);
    assert_eq!(v.visual(ok).unwrap().scalars[&0x13c], 3);
    assert_eq!(v.alias(0x588), None);
}
#[test]
#[ignore = "actual missing-font1 after count literal, before direct shift"]
fn failed_first_count_font_keeps_literal_unshifted_and_same_live_fonts() {
    let input = Inputs::read();
    let (primary, _, text, _) = failed_cut(&input, 1, 3);
    let v = primary.voting();
    let count = v.alias(0x5a4).unwrap();
    assert_eq!(
        v.text(count).unwrap().bounds,
        original_region(&input, b"Voting_Rosette1")
    );
    assert_eq!(v.text(count).unwrap().font, 1);
    assert_eq!(v.text(count).unwrap().style, 4);
    assert_eq!(
        v.text(count).unwrap().group.row(0).unwrap(),
        &[text.fonts().language.row(0).unwrap()[usize::from(b'0' - 32)]]
    );
    assert_eq!(text.fonts().current, 0);
    assert_eq!(text.fonts().selector.cached, 0);
    assert_eq!(v.alias(0x58c), None);
}
#[test]
#[ignore = "actual missing-font3 retains all six rows and empty title-label"]
fn failed_title_label_font_does_not_reach_title_or_selected_mode_writes() {
    let input = Inputs::read();
    let (primary, _, _, _) = failed_cut(&input, 3, 14);
    let v = primary.voting();
    let label = v.alias(0x5c0).unwrap();
    assert_eq!(v.text(label).unwrap().font, 3);
    assert_eq!(v.text(label).unwrap().style, 4);
    assert_eq!(v.text(label).unwrap().group.count, 0);
    assert_eq!(v.alias(0x5c4), None);
}
///Controlled original-backed language fixture, before any Primary allocation.
///Old-disc base has2428rows while the selected208-row board needs IDs through
///2448. Complete ONLY the21 missing allocation rows from that actual board;
///preserve every old row, then run the genuine once-only board operation.
///No v1.4 base language, empty padding, synthetic caption or varargs are used.
fn old_language_with_complete_usa_board(input: &Inputs) -> UiTextResources {
    use super::super::super::ui_text_resources::PackedLanguage;
    let installer = path("OPENMT2001_INSTALLER");
    let mut text = UiTextResources::load(
        &input.exe,
        &installer,
        &path("OPENMT2001_VOTING_OLD_LANGUAGE"),
        &input.index,
        &input.archive,
    )
    .unwrap();
    let manager = text.fonts() as *const _;
    let old = text
        .fonts()
        .language
        .rows()
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    let board = PackedLanguage::load(&installer.join("Standard/fonts/board/usa/tycoon")).unwrap();
    //These counts pin the supplied fixture shape, not production parser limits.
    assert_eq!(old.len(), 2428);
    assert_eq!(board.row_count(), 208);
    assert_eq!(old[0].as_slice(), board.row(0).unwrap());
    assert!(
        old[0x603]
            .windows(2)
            .any(|w| w == [old[0][5], old[0][0x53]]),
        "actual old603 must retain its string token"
    );
    let end = 2242 + board.row_count() - 1;
    assert_eq!(end, 2449);
    let mut complete = old.clone();
    for id in old.len()..end {
        complete.push(board.row((id - 2242 + 1) as u32).unwrap().to_vec());
    }
    let off = complete
        .iter()
        .flat_map(|row| u16::try_from(row.len()).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let packed = PackedLanguage::parse(&off, complete.concat()).unwrap();
    for (id, row) in old.iter().enumerate() {
        assert_eq!(
            packed.row(id as u32).unwrap(),
            row.as_slice(),
            "existing old-disc row must not change before board selection"
        );
    }
    text.fonts_mut().language = packed;
    text.install_usa_board(&input.exe, &installer).unwrap();
    assert_eq!(text.fonts() as *const _, manager);
    assert_eq!(text.fonts().language.row_count(), end);
    for (id, row) in old.iter().take(2242).enumerate() {
        assert_eq!(
            text.fonts().language.row(id as u32).unwrap(),
            row.as_slice(),
            "all old base-prefix bytes stay selected"
        );
    }
    for id in 2242..end {
        assert_eq!(
            text.fonts().language.row(id as u32).unwrap(),
            board.row((id - 2242 + 1) as u32).unwrap()
        );
    }
    assert_eq!(
        text.fonts().language.row(0x603).unwrap(),
        old[0x603].as_slice()
    );
    assert_eq!(
        text.board_stem.as_ref(),
        Some(&installer.join("Standard/fonts/board/usa/tycoon"))
    );
    text
}
#[test]
#[ignore = "original-disc603 has unsupported%s, no invented name/fallback"]
fn old_language_failure_retains_title_alias_and_all_prior_children() {
    let input = Inputs::read();
    let images = input.images();
    let text = old_language_with_complete_usa_board(&input);
    //A separate fresh owner must bind this selected language identity. Never
    //substitute resources into an already allocated production controller.
    let mut p = Primitives::deferred_allocation(&input.exe, input.model_f()).unwrap();
    let primary = DeferredPrimary::allocate(
        &input.exe,
        &input.index,
        &input.archive,
        &mut p,
        &text,
        &images,
    )
    .unwrap();
    let w = controlled(&input);
    let (primary, p, text, images, result) = run(primary, p, text, images, &w);
    assert!(
        result
            .unwrap_err()
            .contains("unsupported packed formatting")
    );
    let v = primary.voting();
    assert_eq!(v.children().len(), 15);
    let title = v.alias(0x5c4).unwrap();
    let alias_offsets = [
        0x4dc, 0x588, 0x5a4, 0x58c, 0x5a8, 0x590, 0x5ac, 0x594, 0x5b0, 0x598, 0x5b4, 0x59c, 0x5b8,
        0x5c0, 0x5c4,
    ];
    let alias_ids = alias_offsets.map(|offset| {
        v.alias(offset)
            .expect("all fifteen native aliases precede603 refusal")
    });
    assert_eq!(alias_ids.as_slice(), v.children());
    assert_eq!(v.alias(0x5a0), None);
    assert_eq!(v.alias(0x5bc), None);
    assert_eq!(v.text(title).unwrap().font, 0);
    assert_eq!(v.text(title).unwrap().style, 0);
    assert_eq!(v.text(title).unwrap().group.count, 0);
    assert_eq!(p.cells().byte(V + 0x184).unwrap(), 0);
    assert_eq!(p.cells().word(V + 0x350).unwrap(), 2);
    assert_eq!(primary.phase(), PrimaryPhase::Failed);
    assert!(!p.steps.contains(&Step::Registered(V)));
    //Independent native44124d precedes4412ab. Title is born font0 after
    //the preceding label selected3; refusal must retain current/cache0.
    assert_eq!(text.fonts().current, 0);
    assert_eq!(text.fonts().selector.cached, 0);
    assert_eq!(text.fonts().line_mode, v.text(title).unwrap().line_mode);
    assert_eq!(
        text.fonts().fonts[&0].scale.to_bits(),
        v.text(title).unwrap().scale.to_bits()
    );
    let children = v.children().to_vec();
    let effects = v.effects().to_vec();
    let steps = p.steps.clone();
    let scratch = text.fonts().scratch.clone();
    let formatted = text.fonts().formatted.bytes.clone();
    let records = p
        .resources
        .model(PanelId::OtherPrimary(1))
        .unwrap()
        .records35
        .clone();
    let (primary, p, text, _, retry) = run(primary, p, text, images, &w);
    assert!(retry.is_err());
    assert_eq!(primary.phase(), PrimaryPhase::Failed);
    assert_eq!(primary.voting().children(), children);
    assert_eq!(primary.voting().effects(), effects);
    assert_eq!(p.steps, steps);
    assert_eq!(primary.voting().alias(0x5c4), Some(title));
    assert_eq!(
        alias_offsets.map(|offset| primary.voting().alias(offset).unwrap()),
        alias_ids
    );
    assert!(!p.steps.contains(&Step::LayoutReturned(V)));
    assert_eq!(text.fonts().current, 0);
    assert_eq!(text.fonts().selector.cached, 0);
    assert_eq!(text.fonts().scratch, scratch);
    assert_eq!(text.fonts().formatted.bytes, formatted);
    assert_eq!(
        p.resources
            .model(PanelId::OtherPrimary(1))
            .unwrap()
            .records35,
        records
    );
    assert_eq!(p.cells().byte(V + 0x184).unwrap(), 0);
    assert!(!p.steps.contains(&Step::Registered(V)));
    assert_unbound(&p);
}
#[test]
#[ignore = "actual shared pending/notify failures at first Voting selector miss"]
fn pending_or_notify_never_becomes_a_successful_missing_callback() {
    let input = Inputs::read();
    for pending in [true, false] {
        let (primary, p, mut text, images) = input.fresh();
        //D uses cached font0. Voting's explicit2 is the first miss and must
        //preserve the existing pending/notify owner instead of flushing it.
        text.fonts_mut().selector.cached = 0;
        text.fonts_mut().current = 0;
        if pending {
            text.fonts_mut().selector.pending = [1, 0];
        } else {
            text.fonts_mut().selector.notify = 7;
        }
        let w = controlled(&input);
        let (primary, p, text, _, result) = run(primary, p, text, images, &w);
        assert!(result.unwrap_err().contains(if pending {
            "pending page-batch"
        } else {
            "notify callback"
        }));
        let v = primary.voting();
        assert_eq!(v.children().len(), 1);
        assert_eq!(v.text(v.alias(0x4dc).unwrap()).unwrap().font, 2);
        assert_eq!(text.fonts().selector.cached, 0);
        assert_eq!(text.fonts().current, 0);
        assert_eq!(
            text.fonts().selector.pending,
            if pending { [1, 0] } else { [0, 0] }
        );
        assert_eq!(text.fonts().selector.notify, if pending { 0 } else { 7 });
        assert_eq!(p.cells().byte(V + 0x184).unwrap(), 0);
        assert!(!p.steps.contains(&Step::Registered(V)));
    }
}
#[test]
#[ignore = "invalid original-model part must fail, distinct from native missing-name extent"]
fn corrupt_part_reference_retains_mutated_model_and_unbound_partial_root() {
    let input = Inputs::read();
    let (primary, mut p, text, images) = input.fresh();
    let m = p
        .resources
        .models
        .get_mut(&PanelId::OtherPrimary(1))
        .unwrap();
    let record = m
        .records35
        .iter_mut()
        .find(|r| {
            let n = r[..33].iter().position(|&b| b == 0).unwrap();
            r[..n].eq_ignore_ascii_case(b"Voting_Title_Area")
        })
        .unwrap();
    record[33] = 255;
    let w = controlled(&input);
    let (primary, p, text, images, result) = run(primary, p, text, images, &w);
    assert!(result.unwrap_err().contains("part outside actual model"));
    assert_eq!(primary.phase(), PrimaryPhase::Failed);
    assert!(primary.voting().children().is_empty());
    assert_eq!(primary.voting().nodes().count(), 0);
    assert_eq!(p.cells().byte(V + 0x184).unwrap(), 0);
    assert_eq!(p.cells().word(G + 8).unwrap(), D);
    let m = p.resources.model(PanelId::OtherPrimary(1)).unwrap();
    assert!(
        m.records35
            .iter()
            .any(|r| r.starts_with(b"VOTING_TITLE_AREA") && r[33] == 255)
    );
    let steps = p.steps.clone();
    let (primary, p, _, _, result) = run(primary, p, text, images, &w);
    assert!(result.is_err());
    assert_eq!(p.steps, steps);
    assert!(primary.voting().children().is_empty());
}
#[test]
#[ignore = "actual factory nonnull inherited-frame branch, no context fallback"]
fn all_direct_children_inherit_actual_parent_frame_without_changing_event_sink() {
    let input = Inputs::read();
    let (primary, mut p, text, images) = input.fresh();
    //An existing real static object address, not a manufactured heap pointer.
    p.test_voting_inherited_frame(0x58e3c0).unwrap();
    let w = controlled(&input);
    let (primary, p, _, _, result) = run(primary, p, text, images, &w);
    result.unwrap();
    for node in primary.voting().nodes() {
        assert_eq!(node.event_sink, V);
        assert_eq!(node.frame, 0x58e3c0);
    }
    assert_eq!(p.cells().word(V + 0x24).unwrap(), 0x58e3c0);
    assert_unbound(&p);
}
