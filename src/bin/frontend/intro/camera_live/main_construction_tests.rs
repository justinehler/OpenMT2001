//! Main construction regressions with explicit supplied-input boundaries.
//! No canned successful owner/maps/providers.
use super::super::{
    case_zero_text::ErrorWord,
    primitive_factory::{Phase, Step},
    process_image::Cells,
};
use super::*;
use std::{fs, path::PathBuf};
fn path(key: &str) -> PathBuf {
    openmt2001::fixtures::path(key)
}
struct Inputs {
    exe: Vec<u8>,
    directory: Vec<u8>,
    archive: Vec<u8>,
    index: ArchiveIndex,
    installer: PathBuf,
    language: PathBuf,
}
impl Inputs {
    fn supplied() -> Self {
        let exe = fs::read(path("OPENMT2001_EXE")).unwrap();
        let directory = fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap();
        let archive = fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        Self {
            exe,
            directory,
            archive,
            index,
            installer: path("OPENMT2001_INSTALLER"),
            language: path("OPENMT2001_LANGUAGE"),
        }
    }
    fn text(&self) -> UiTextResources {
        let mut t = UiTextResources::load(
            &self.exe,
            &self.installer,
            &self.language,
            &self.index,
            &self.archive,
        )
        .unwrap();
        t.install_usa_board(&self.exe, &self.installer).unwrap();
        t
    }
    fn navigator(&self) -> crate::navigator::Navigator {
        let board =
            openmt2001::board::Board::from_v14(&PeImage::parse(&self.exe).unwrap()).unwrap();
        crate::navigator::Navigator::load(&self.index, &self.archive, &board.blocks, &self.exe)
            .unwrap()
    }
    fn images(&self) -> Images {
        Images::load(&self.exe, &self.index, &self.archive, [640, 480]).unwrap()
    }
    fn load(
        &self,
        t: &mut UiTextResources,
        n: &mut crate::navigator::Navigator,
        i: &mut Images,
    ) -> Result<AwaitingOtherMainChildren, String> {
        Prefix::load(
            &self.exe,
            &self.index,
            &self.archive,
            [640, 480],
            t,
            n.process_primitives_mut(),
            i,
        )
    }
    fn model(&self, id: PanelId) -> CpuModel {
        let c = Cells::process_allocation(&self.exe).unwrap();
        let ordinal = match id {
            PanelId::A => 4,
            PanelId::G => 12,
            _ => panic!("test catalog slot"),
        };
        let name = registration::asset(&c, Group::Primary, ordinal).unwrap();
        let e = self.index.find(&name).unwrap();
        let m = CachedMesh::parse(&self.archive[e.offset as usize..(e.offset + e.length) as usize])
            .unwrap();
        CpuModel::parsed(id, e, &m).unwrap()
    }
}
#[test]
#[ignore = "requires supplied original PE/DIR/BIN/installer/language"]
fn actual_prefix_products_topology_aliases_and_same_model_authority() {
    let input = Inputs::supplied();
    let mut text = input.text();
    let mut n = input.navigator();
    let mut images = input.images();
    let fonts = text.fonts() as *const Fonts;
    let pages = text
        .pages
        .records
        .iter()
        .map(|p| (p.ordinal, p.record.clone()))
        .collect::<Vec<_>>();
    let f = n
        .process_primitives_mut()
        .resources
        .model(PanelId::F)
        .unwrap()
        .frames[0]
        .as_ptr();
    let original_a = input.model(PanelId::A);
    let original_g = input.model(PanelId::G);
    let prefix = input.load(&mut text, &mut n, &mut images).unwrap();
    let p = n.process_primitives_mut();
    assert_eq!(p.main_phase(), MainPhase::AwaitingOtherMainChildren);
    assert_eq!(prefix.phase(), p.main_phase());
    assert_eq!(p.panels[&A].phase, Phase::AwaitingLayout);
    assert_eq!(
        p.panels[&A].descriptor,
        Some(crate::intro::camera_live::primitive_factory::DescriptorIdentity::Archive(PanelId::A))
    );
    assert_eq!(p.cells().byte(A + 0x184).unwrap(), 0);
    assert_eq!(
        p.cells().word(A + 0x37c).unwrap(),
        0,
        "no fabricated descriptor address"
    );
    assert_eq!(p.resources.model(PanelId::F).unwrap().frames[0].as_ptr(), f);
    assert_eq!(
        p.steps
            .iter()
            .filter(|s| **s == Step::NavigatorSuffix(0x58cdb0))
            .count(),
        1
    );
    assert_eq!(fonts, text.fonts() as *const Fonts);
    assert_eq!(text.fonts().error, ErrorWord::Unknown);
    assert_eq!(
        pages,
        text.pages
            .records
            .iter()
            .map(|p| (p.ordinal, p.record.clone()))
            .collect::<Vec<_>>()
    );
    let a = p.resources.model(PanelId::A).unwrap();
    assert_eq!(a.source, original_a.source);
    assert_eq!(a.definition, original_a.definition);
    assert_eq!(
        p.resources.descriptor(PanelId::A).unwrap().frame,
        0,
        "descriptor current frame is not preparation's final frame"
    );
    let pe = PeImage::parse(&input.exe).unwrap();
    let negative = float(&pe, 0x525264).unwrap();
    let offset = float(&pe, 0x525034).unwrap();
    for (before, after) in original_a
        .frames
        .iter()
        .flatten()
        .zip(a.frames.iter().flatten())
    {
        assert_eq!(&before[..2], &after[..2]);
        let mut z = sum(before[2]).unwrap();
        z.mul_f32(negative).unwrap();
        let rounded = z.to_f32();
        let mut z = sum(rounded).unwrap();
        z.add_f32(offset).unwrap();
        assert_eq!(
            after[2].to_bits(),
            z.to_f32().to_bits(),
            "two distinct source f32 stores"
        );
    }
    //Fresh A/G initial materializations are separately owned; equality of the
    //archive span or definition is never used to invent general model aliasing.
    let g_before = original_g.frames.clone();
    let c = Cells::process_allocation(&input.exe).unwrap();
    p.resources.insert_primary(&c, original_g).unwrap();
    let g = p.resources.model(PanelId::G).unwrap();
    assert_eq!(g.frames, g_before);
    assert_ne!(a_frames(p), g.frames[0].as_ptr());
    let root = prefix.node(prefix.root()).unwrap();
    assert_eq!(root.children.len(), 20);
    assert_eq!(prefix.nodes().count(), 25);
    assert_eq!(prefix.counter(), 23);
    assert_eq!(p.cells().word(A + 0x388).unwrap(), 23);
    let labels = root
        .children
        .iter()
        .map(|&id| prefix.node(id).unwrap().label)
        .collect::<Vec<_>>();
    let mut expected = vec![
        Label::Overlay,
        Label::Previous,
        Label::Next,
        Label::Notice,
        Label::Extra,
        Label::ParentSelector,
    ];
    expected.extend((0..6).map(Label::Mode));
    expected.extend((0..5).map(Label::Info));
    expected.extend([Label::SharedTitle, Label::Child0List, Label::Child0Selector]);
    assert_eq!(labels, expected);
    let numbers = root
        .children
        .iter()
        .map(|&id| prefix.node(id).unwrap().number.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        numbers,
        [
            0, 2, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22
        ]
    );
    for (i, meta) in prefix.mode_metadata().iter().enumerate() {
        assert_eq!(
            prefix.button(meta[3]),
            Some(prefix.alias(0x48c + i as u32 * 4).unwrap())
        );
    }
    for i in 0..5 {
        assert_eq!(
            prefix.info(i),
            Some(prefix.alias(0x4a4 + i as u32 * 4).unwrap())
        );
    }
    assert!(matches!(
        prefix.node(prefix.alias(0x4b8).unwrap()).unwrap().subtype,
        Subtype::Text(_)
    ));
    assert!(prefix.alias(0x4bc).is_err());
    assert!(prefix.alias(0x4c0).is_err());
    for node in prefix.nodes() {
        if node.id != prefix.root() {
            assert_eq!(node.root, Some(prefix.root()));
            assert!(node.parent.is_some());
            assert!(matches!(node.padding1e, Field::Unknown));
        }
        let events = prefix
            .journal()
            .iter()
            .filter(|e| e.node == node.id)
            .map(|e| &e.event)
            .collect::<Vec<_>>();
        if node.id != prefix.root() {
            let slot = events
                .iter()
                .position(|e| matches!(e, Event::Slot04(_)))
                .unwrap();
            let num = events
                .iter()
                .position(|e| matches!(e, Event::Number(_)))
                .unwrap();
            let append = events
                .iter()
                .position(|e| matches!(e, Event::Append(_)))
                .unwrap();
            assert!(slot < num && num < append);
        }
    }
    for offset in [0x34a, 0x34b, 0x358, 0x35c] {
        assert_eq!(
            p.cells().byte(A + offset).unwrap(),
            prefix.static_preimage()[offset as usize]
        );
    }
    assert!(
        prefix
            .require_full_main()
            .unwrap_err()
            .contains("child5+394/409ec0")
    );
    assert!(prefix.require_input_or_render().is_err());
    assert!(p.require_process_graph().is_err());
}
fn a_frames(p: &Primitives) -> *const [f32; 3] {
    p.resources.model(PanelId::A).unwrap().frames[0].as_ptr()
}
#[test]
#[ignore = "supplied originals; real list/selector products and shared font manager"]
fn reached_list_reset_configuration_growth_shared_fonts_and_repeated_selector_show() {
    let input = Inputs::supplied();
    let mut text = input.text();
    let mut n = input.navigator();
    let mut images = input.images();
    let fonts = text.fonts() as *const Fonts;
    let mut prefix = input.load(&mut text, &mut n, &mut images).unwrap();
    let child = prefix.child0();
    assert_eq!((child.state, child.native_return), (0, -1));
    assert_eq!(child.parent, prefix.root());
    let list = child.list;
    let selector = child.selector;
    {
        let node = prefix.node(list).unwrap();
        assert_eq!((node.shown, node.field18), (0, 0));
        assert_eq!(node.embedded_text_contexts24.len(), 3);
        assert!(
            node.embedded_text_contexts24
                .iter()
                .all(|v| matches!(v, Field::Unknown))
        );
        let Subtype::List3Texts(l) = &node.subtype else {
            panic!("actual list subtype")
        };
        assert_eq!(l.indicator, Some(selector));
        assert_eq!(l.list.key, list);
        assert_eq!((l.list.chosen, l.list.value), (-1, 0));
        assert!(l.list.flags.is_empty() && l.list.slots.is_empty() && l.list.entries.is_empty());
        assert_eq!(l.list.primary.bounds, l.list.bounds);
        assert_eq!(l.list.secondary.bounds, l.list.bounds);
        assert_eq!(l.list.scratch.bounds, l.list.bounds);
        //Native440650 first args0/0/3;4406a0 style0 Y=FTOL(top+3), low16.
        let first = &l.list.primary;
        assert_eq!(
            (first.style, first.inset, first.scroll, first.padding),
            (0, 0, 3, 1)
        );
        let mut native_y = Sum::from_f32(first.bounds[1]).unwrap();
        native_y.add(Sum::from_i32(3));
        assert_eq!(first.y, native_y.ftol_low_word() as i16);
        assert_eq!(
            (
                l.list.secondary.style,
                l.list.secondary.inset,
                l.list.secondary.scroll
            ),
            (2, 3, 3)
        );
        assert_eq!(
            (
                l.list.primary.gap,
                l.list.secondary.gap,
                l.list.primary.font,
                l.list.secondary.font
            ),
            (2, 2, 3, 3)
        );
        assert_eq!(
            l.list.scratch.font, 0,
            "scratch was not configured as a copied primary"
        );
        assert!(!std::ptr::eq(&l.list.primary, &l.list.secondary));
    }
    {
        let node = prefix.node(selector).unwrap();
        let Subtype::Selector2Arrows(s) = &node.subtype else {
            panic!()
        };
        assert_eq!(s.first_list, Some(list));
        assert_eq!((node.shown, node.disabled, s.admitted191), (0, 1, 0));
        for id in [s.previous.unwrap(), s.next.unwrap()] {
            let arrow = prefix.node(id).unwrap();
            assert_eq!(arrow.parent, Some(selector));
            let Subtype::Visual(v) = &arrow.subtype else {
                panic!()
            };
            assert_eq!((v.caption_mode, v.disabled333, v.down335), (4, 1, 0));
        }
    }
    let shows = prefix
        .journal()
        .iter()
        .filter(|e| e.node == selector && matches!(e.event, Event::Show(0)))
        .map(|e| e.source)
        .collect::<Vec<_>>();
    assert_eq!(shows, [0x41f977, 0x40a99d]);
    //Equal Text.font still invokes the selector; cached3/current1 divergence
    //is retained on a hit, so the actual active1 metrics are used.
    text.fonts_mut().direct_index(1).unwrap();
    text.fonts_mut().selector.cached = 3;
    let Subtype::List3Texts(l) = &mut prefix.inner.node_mut(list).unwrap().subtype else {
        panic!()
    };
    l.list.primary.set_font(text.fonts_mut(), 3).unwrap();
    assert_eq!(text.fonts().current, 1);
    l.list.primary.measure(text.fonts_mut()).unwrap();
    let w = text.fonts_mut().map(b"W").unwrap();
    assert_eq!(l.list.primary.pitch, text.fonts().height(&w).unwrap() + 2);
    l.list.primary.literal(text.fonts_mut(), b"W", &[]).unwrap();
    l.list.ensure().unwrap();
    assert!(!l.list.flags.is_empty());
    assert!(
        l.list
            .flags
            .iter()
            .all(|f| matches!(f, super::super::case_zero::FlagByte::Unwritten))
    );
    l.list.reset(text.fonts_mut(), None).unwrap();
    assert!(
        l.list
            .flags
            .iter()
            .all(|f| matches!(f, super::super::case_zero::FlagByte::Byte(0)))
    );
    assert_eq!(fonts, text.fonts() as *const Fonts);
    assert_eq!(text.fonts().error, ErrorWord::Unknown);
}
#[test]
#[ignore = "supplied originals; image registration/data provenance"]
fn actual_image_dimensions_ordinals_and_scale_stores_change_the_produced_bounds() {
    let input = Inputs::supplied();
    let mut text = input.text();
    let mut n = input.navigator();
    let mut images = input.images();
    let e = input.index.find(&images.path).unwrap();
    assert_eq!(
        images.archive_range,
        e.offset as usize..(e.offset + e.length) as usize
    );
    let parsed =
        openmt2001::texture::TextureSet::parse(&input.archive[images.archive_range.clone()])
            .unwrap();
    assert_eq!(parsed.textures.len(), images.records.len());
    for (i, (source, record)) in parsed.textures.iter().zip(&images.records).enumerate() {
        assert_eq!(record.ordinal, i);
        assert_eq!(record.registration, 0x68ae28 + i as u32 * 4);
        assert_eq!(record.dimensions, [source.width, source.height]);
        assert_eq!(record.range.len(), 56 + source.texels.len());
    }
    let prefix = input.load(&mut text, &mut n, &mut images).unwrap();
    let id = images.id(0x68afcc).unwrap();
    assert_eq!(
        prefix.scrollbar_width(),
        parsed.textures[(0x68afcc - 0x68ae28) / 4].width as f32
    );
    assert_eq!(images.record(id).unwrap().scale, Some(1.));
    let Subtype::Visual(overlay) = &prefix.node(prefix.alias(0x488).unwrap()).unwrap().subtype
    else {
        panic!()
    };
    assert_eq!(
        overlay.images,
        [
            Some(images.id(0x68af88).unwrap()),
            Some(images.id(0x68af90).unwrap()),
            None,
            Some(images.id(0x68af8c).unwrap()),
            None
        ]
    );
    assert_eq!(overlay.icon, Some(images.id(0x68af44).unwrap()));
    let mut bad_index = input.index.clone();
    bad_index.entries.retain(|e| {
        e.path.replace('\\', "/").to_ascii_lowercase()
            != images.path.replace('\\', "/").to_ascii_lowercase()
    });
    assert!(
        Images::load(&input.exe, &bad_index, &input.archive, [640, 480])
            .err()
            .unwrap()
            .contains("collection")
    );
    assert!(Images::load(&input.exe, &input.index, &input.archive, [800, 600]).is_err());
    let mut text = input.text();
    let mut n = input.navigator();
    let mut images = input.images();
    images.records.truncate(90);
    assert!(
        input
            .load(&mut text, &mut n, &mut images)
            .err()
            .unwrap()
            .contains("ordinal")
    );
}
#[test]
#[ignore = "supplied originals; production failure and duplicate phase retention"]
fn failed_new_candidate_and_duplicate_birth_never_replace_a_retained_product() {
    let input = Inputs::supplied();
    let mut text = input.text();
    let mut nav = input.navigator();
    let mut images = input.images();
    let mut retained = input.load(&mut text, &mut nav, &mut images).unwrap();
    let root = retained.root();
    let fonts = text.fonts() as *const Fonts;
    let model = a_frames(nav.process_primitives_mut());
    let notice = retained.alias(0x3a0).unwrap();
    retained
        .inner
        .text_mut(notice)
        .unwrap()
        .literal(text.fonts_mut(), b"W", &[])
        .unwrap();
    let count = retained.inner.text_mut(notice).unwrap().group.count;
    let scratch = text.fonts().scratch.clone();
    let error = input.load(&mut text, &mut nav, &mut images).err().unwrap();
    assert!(error.contains("phase"));
    assert_eq!(
        nav.process_primitives_mut().main_phase(),
        MainPhase::AwaitingOtherMainChildren
    );
    assert_eq!(model, a_frames(nav.process_primitives_mut()));
    assert_eq!(text.fonts().scratch, scratch);
    let mut candidate_text = input.text();
    let mut candidate_nav = input.navigator();
    let mut candidate_images = input.images();
    candidate_text.fonts_mut().selector.pending = [1, 0];
    candidate_text.fonts_mut().selector.cached = u32::MAX;
    assert!(
        input
            .load(
                &mut candidate_text,
                &mut candidate_nav,
                &mut candidate_images
            )
            .err()
            .unwrap()
            .contains("pending")
    );
    assert_eq!(
        candidate_nav.process_primitives_mut().main_phase(),
        MainPhase::Failed
    );
    assert!(
        input
            .load(
                &mut candidate_text,
                &mut candidate_nav,
                &mut candidate_images
            )
            .err()
            .unwrap()
            .contains("phase")
    );
    assert_eq!(retained.root(), root);
    assert_eq!(retained.inner.text_mut(notice).unwrap().group.count, count);
    assert_eq!(text.fonts() as *const Fonts, fonts);
    //Short real source archive fails before A model publication/font mutation.
    let mut t = input.text();
    let mut n = input.navigator();
    let mut i = input.images();
    let before = format!("{:?}", t.fonts());
    assert!(
        Prefix::load(
            &input.exe,
            &input.index,
            &input.archive[..1],
            [640, 480],
            &mut t,
            n.process_primitives_mut(),
            &mut i
        )
        .is_err()
    );
    assert_eq!(before, format!("{:?}", t.fonts()));
    assert!(
        n.process_primitives_mut()
            .resources
            .model(PanelId::A)
            .is_err()
    );
    let moved = retained;
    let mut parked = Some(moved);
    let reattached = parked.take().unwrap();
    assert_eq!(reattached.root(), root);
    assert_eq!(reattached.node(notice).unwrap().id, notice);
    assert!(parked.is_none());
    let mut t = input.text();
    let mut n = input.navigator();
    let mut i = input.images();
    let retry = input.load(&mut t, &mut n, &mut i).unwrap();
    assert_ne!(retry.root(), root);
}
#[test]
#[ignore = "requires supplied Assets::load inputs and ffmpeg"]
fn actual_assets_entry_retains_prefix_with_same_assets_across_retirement() {
    let assets = crate::intro::Assets::load(crate::intro::Paths {
        compatibility: crate::compatibility::Mode::StrictV14,
        installer: path("OPENMT2001_INSTALLER"),
        ffmpeg: path("OPENMT2001_FFMPEG"),
        scene: path("OPENMT2001_SCENE"),
        executable: path("OPENMT2001_EXE"),
        map: path("OPENMT2001_INTRO_MAP"),
        quality: Some("low".into()),
        language: path("OPENMT2001_LANGUAGE"),
        profile_root: openmt2001::fixtures::scratch("TEXT_PROFILE"),
        save_root: openmt2001::fixtures::scratch("TEXT_SAVES"),
    })
    .unwrap();
    assert_eq!(
        assets.reordered_main().unwrap().phase(),
        MainPhase::AwaitingBinding
    );
    assert!(assets.ui_text.board_stem.is_some());
    let root = assets.reordered_main().unwrap().root();
    let fonts = assets.ui_text.fonts() as *const Fonts;
    let list = assets.reordered_main().unwrap().child0().list;
    let list_pointer = assets.reordered_main().unwrap().node(list).unwrap() as *const Node;
    let mut process = Some(Box::new(assets));
    let mut retry = crate::intro::refresh::Retry::default();
    let mut refreshing = false;
    let mut loading = None;
    let mut parked = None;
    crate::intro::refresh::retire(&mut retry, &mut refreshing, &mut loading, None, &mut parked)
        .unwrap();
    let mut moved = process.take().unwrap();
    assert!(process.is_none());
    assert_eq!(root, moved.reordered_main().unwrap().root());
    assert_eq!(fonts, moved.ui_text.fonts() as *const Fonts);
    assert_eq!(
        list_pointer,
        moved.reordered_main().unwrap().node(list).unwrap() as *const Node
    );
    assert_eq!(
        moved
            .navigator
            .process_primitives_mut()
            .steps
            .iter()
            .filter(|s| **s == Step::Bind(A))
            .count(),
        1
    );
    assert!(moved.reordered_main().unwrap().require_full_main().is_err());
    //Intro::new retains this entire Assets value. Real endpoint attach and
    //fullMain activation remain deliberately gated by missing child2 and parent tail.
}
#[test]
#[ignore = "supplied PE for raw unknown-preimage control"]
fn normal_text_field_admission_does_not_relax_old_raw_factory_unknown_reads() {
    // Synthetic heap fixture outside the supplied PE; no native heap-address claim.
    const HEAP: u32 = 0x1000_0000;
    let input = Inputs::supplied();
    let mut cells = Cells::process_allocation(&input.exe).unwrap();
    cells.heap_allocation(HEAP, 0x388).unwrap();
    let mut b = super::super::primitive_factory::Builder::new(cells);
    assert!(b.panel(HEAP).err().unwrap().contains("producer"));
    let t = Text::normal_fields();
    assert_eq!((t.pitch, t.visible, t.font, t.dirty), (10, 64, 0, 0));
    assert!(t.group.bytes.is_empty());
    for pitch in [0, 13] {
        let mut c = Cells::process_allocation(&input.exe).unwrap();
        c.heap_allocation(HEAP, 0x388).unwrap();
        c.set_word(HEAP + 0x188 + 0x140, pitch).unwrap();
        let mut b = super::super::primitive_factory::Builder::new(c);
        b.panel(HEAP).unwrap();
        let products = b.finish().unwrap();
        let actual = &products.texts[&(HEAP + 0x188)];
        assert_eq!(
            (actual.pitch, actual.visible, actual.font, actual.dirty),
            (t.pitch, t.visible, t.font, t.dirty)
        );
    }

    //No native heap preimage is manufactured for normal_fields; dead old
    //pitch/status/traps are outside its explicit field-level contract.
}
#[test]
#[ignore = "actual supplied TM; finite name wrappers versus raw no-write query"]
fn actual_name_first_match_mutation_null_and_no_write_extents_stay_distinct() {
    let input = Inputs::supplied();
    let mut t = input.text();
    let mut n = input.navigator();
    let mut i = input.images();
    let prefix = input.load(&mut t, &mut n, &mut i).unwrap();
    let p = n.process_primitives_mut();
    assert_eq!(p.main_named(None, false).unwrap(), [0., 0., 0., 0., 0.5]);
    assert_eq!(
        p.main_named(Some(b"ABSENT_REQUIRED_NAME"), false).unwrap(),
        [0., 0., 0., 0., 0.5]
    );
    assert!(
        p.main_named(Some(b"ABSENT_REQUIRED_NAME"), true)
            .err()
            .unwrap()
            .contains("43b040")
    );
    let m = p.resources.models.get_mut(&PanelId::A).unwrap();
    let before = [1., 2., 3., 4.];
    let mut raw = before;
    if let Some(r) = m.region(u32::MAX, 0).unwrap() {
        raw = r;
    }
    assert_eq!(raw, before, "raw -1 query does no write");
    assert!(m.region(0, usize::MAX).is_err());
    assert!(m.region(u32::MAX - 1, 0).is_err());
    let mut duplicated = input.model(PanelId::A);
    let first = duplicated.records35[0];
    duplicated.records35.insert(1, first);
    let name = first[..33]
        .split(|&b| b == 0)
        .next()
        .unwrap()
        .to_ascii_uppercase();
    assert_eq!(duplicated.find(Some(&name)).unwrap(), u32::from(first[33]));
    assert_eq!(
        duplicated.records35[1], first,
        "first-match stop must not uppercase the unvisited duplicate"
    );
    assert!(
        t.fonts_mut().take_error().is_err(),
        "Unknown5b4 is not success0"
    );
    assert!(prefix.require_full_main().is_err());
}
#[test]
fn unexecuted_ftol_and_extent_store_boundaries() {
    for (input, expected) in [
        (0.9999999999999999, 0),
        (-0.9999999999999999, 0),
        (1.0000000000000002, 1),
        (-1.0000000000000002, -1),
        (2147483647.75, i32::MAX),
        (-2147483648.75, i32::MIN),
    ] {
        assert_eq!(ftol(Sum::from_f64(input).unwrap()).unwrap(), expected);
    }
    for input in [2147483648., -2147483649.] {
        assert!(ftol(Sum::from_f64(input).unwrap()).is_err());
    }
    assert!(Sum::from_f32(f32::NAN).is_err());
    let mut t = Text::normal_fields();
    t.extent440900([0., 0., 1., 9.999999]).unwrap();
    assert_eq!(t.visible, 0, "440900 has no minimum-one clamp");
    t.extent440900([0., 0., 1., 10.]).unwrap();
    assert_eq!(t.visible, 1);
    t.pitch = 0;
    assert!(t.extent440900([0.; 4]).is_err());
}

#[test]
#[ignore = "supplied resources; actual default Text reset dispatch and shared manager history"]
fn default_text_reset_retains_pitch_geometry_and_manager_without_metric_dispatch() {
    use super::super::case_zero_text::{Group, ResetTarget, ScratchByte};
    let input = Inputs::supplied();
    let mut resources = input.text();
    let f = resources.fonts_mut();
    f.select(0).unwrap();
    f.direct_index(1).unwrap();
    f.scratch[0] = ScratchByte::Byte(0x7b);
    let before = (
        f.current,
        f.selector.cached,
        f.scratch.clone(),
        f.error,
        f.selector.pending,
    );
    let mut text = Text::normal_fields();
    text.bounds = [4., 8., 100., 48.];
    text.pitch = 13;
    text.y = -12;
    text.dirty = 23;
    text.geometry_dirty = 29;
    text.extent = [1., 2., 3., 4.];
    text.colors = vec![9; 4];
    text.x = vec![-7; 4];
    text.blink_colors = vec![8; 4];
    text.blink = vec![6; 4];
    text.group = Group {
        bytes: vec![7, 8, 0],
        offsets: vec![0, 2, 3],
        count: 2,
    };
    text.reset(f).unwrap();
    assert_eq!(text.reset_target, ResetTarget::Ret43b370);
    assert_eq!(
        (
            text.pitch,
            text.y,
            text.dirty,
            text.geometry_dirty,
            text.visible
        ),
        (13, -12, 23, 29, 3)
    );
    assert_eq!(text.extent, [1., 2., 3., 4.]);
    assert_eq!(text.x, vec![-7; 4]);
    assert_eq!(text.colors, vec![9; 4]);
    assert_eq!(
        (
            text.group.bytes.as_slice(),
            text.group.offsets.as_slice(),
            text.group.count
        ),
        (&[0; 3][..], &[0; 3][..], 0)
    );
    assert_eq!(
        (
            f.current,
            f.selector.cached,
            f.scratch.clone(),
            f.error,
            f.selector.pending
        ),
        before
    );
    text.reset_target = ResetTarget::Unsupported(0x4406a0);
    assert!(
        text.reset(f)
            .unwrap_err()
            .contains("virtual+2c target004406a0 unsupported")
    );
    assert_eq!(
        (
            f.current,
            f.selector.cached,
            f.scratch.clone(),
            f.error,
            f.selector.pending
        ),
        before
    );
}

#[test]
#[ignore = "supplied resources; direct4406a0 active-font metrics without selector or measure tail"]
fn direct_layout_uses_active_font_without_selecting_text_font_or_measuring_extent() {
    let input = Inputs::supplied();
    let mut resources = input.text();
    let f = resources.fonts_mut();
    f.select(0).unwrap();
    f.direct_index(3).unwrap();
    f.selector.pending = [1, 2];
    let glyph = usize::from(f.language.bytes()[usize::from(b'W' - 32)]);
    let at = 4 + glyph * 44 + 36;
    let table = &f.fonts[&3].table;
    let height = i32::from_le_bytes(table[at..at + 4].try_into().unwrap());
    let mut text = Text::normal_fields();
    text.font = 1;
    text.bounds = [0., 10., 100., 80.];
    text.style = 3;
    text.gap = 2;
    text.extent = [11., 12., 13., 14.];
    text.layout(f).unwrap();
    assert_eq!(
        (f.current, f.selector.cached, f.selector.pending),
        (3, 0, [1, 2])
    );
    assert_eq!(text.font, 1);
    assert_eq!(text.pitch, height + 2);
    assert_eq!(
        text.y,
        (((70 - height) as f64 / 2.).trunc() as i32 + 10) as i16
    );
    assert_eq!((text.dirty, text.geometry_dirty), (1, 1));
    assert_eq!(text.extent, [11., 12., 13., 14.]);
}

#[test]
#[ignore = "supplied resources; real Main prefix default title/secondary pitch and direct primary layout"]
fn actual_prefix_retains_default_reset_pitches_and_direct_layout_extent() {
    let input = Inputs::supplied();
    let mut text = input.text();
    let mut nav = input.navigator();
    let mut images = input.images();
    let prefix = input.load(&mut text, &mut nav, &mut images).unwrap();
    let glyph = usize::from(text.fonts().language.bytes()[usize::from(b'W' - 32)]);
    let at = 4 + glyph * 44 + 36;
    let table = &text.fonts().fonts[&3].table;
    let height = i32::from_le_bytes(table[at..at + 4].try_into().unwrap());
    let Subtype::Text(title) = &prefix.node(prefix.alias(0x4b8).unwrap()).unwrap().subtype else {
        panic!("real shared title Text")
    };
    let Subtype::List3Texts(product) = &prefix.node(prefix.child0().list).unwrap().subtype else {
        panic!("real child0 List")
    };
    assert_eq!(title.pitch, 10);
    assert_eq!(product.list.secondary.pitch, 10);
    assert_eq!(product.list.primary.pitch, height);
    assert_eq!(product.list.primary.extent, [0.; 4]);
    assert_eq!(product.list.primary.geometry_dirty, 1);
    assert_eq!(product.list.secondary.geometry_dirty, 0);
    assert_eq!((text.fonts().current, text.fonts().selector.cached), (3, 3));
}

#[test]
fn default_ret_reset_rejects_nonfinite_ratio_before_minimum_row_clamp() {
    let mut fonts = super::super::main_panel::regression_tests::fonts();
    fonts.current = u32::MAX; //RET reset must not consult the font table.
    let scratch = fonts.scratch.clone();
    for top in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut text = Text::normal_fields();
        text.bounds = [0., top, 10., 40.];
        text.dirty = 23;
        assert!(text.reset(&mut fonts).unwrap_err().contains("non-finite"));
        assert_eq!((text.pitch, text.dirty), (10, 23));
        assert_eq!(fonts.current, u32::MAX);
        assert_eq!(fonts.scratch, scratch);
    }
}
