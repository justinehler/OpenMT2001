//! Supplied-source and resource tests for the complete Main layout return.
use super::*;
use crate::intro::camera_live::{
    case_zero_text::ResetTarget,
    primitive_factory::{Phase, Step},
    process_image::Cells,
};
use std::{fs, path::PathBuf};
fn path(key: &str) -> PathBuf {
    openmt2001::fixtures::path(key)
}
struct Supplied {
    exe: Vec<u8>,
    archive: Vec<u8>,
    index: ArchiveIndex,
    installer: PathBuf,
    language: PathBuf,
}
impl Supplied {
    fn read() -> Self {
        let exe = fs::read(path("OPENMT2001_EXE")).unwrap();
        let dir = fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap();
        let archive = fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(&dir, archive.len() as u64).unwrap();
        Self {
            exe,
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
    fn images(&self) -> Images {
        Images::load(&self.exe, &self.index, &self.archive, [640, 480]).unwrap()
    }
    fn navigator(&self) -> crate::navigator::Navigator {
        let board =
            openmt2001::board::Board::from_v14(&PeImage::parse(&self.exe).unwrap()).unwrap();
        crate::navigator::Navigator::load(&self.index, &self.archive, &board.blocks, &self.exe)
            .unwrap()
    }
    fn tail(
        &self,
        t: &mut UiTextResources,
        n: &mut crate::navigator::Navigator,
        i: &mut Images,
    ) -> AwaitingParentTail {
        let mut prefix = Some(
            Prefix::load(
                &self.exe,
                &self.index,
                &self.archive,
                [640, 480],
                t,
                n.process_primitives_mut(),
                i,
            )
            .unwrap(),
        );
        let mut one = Some(
            AwaitingChild5::continue_child1(
                &mut prefix,
                &self.exe,
                t,
                n.process_primitives_mut(),
                i,
            )
            .unwrap(),
        );
        let mut five = Some(
            AwaitingChild2::continue_child5(&mut one, &self.exe, t, n.process_primitives_mut(), i)
                .unwrap(),
        );
        AwaitingParentTail::continue_child2(&mut five, &self.exe, t, n.process_primitives_mut(), i)
            .unwrap()
    }
}
fn product(v: &AwaitingParentTail) -> &Product {
    &v.prefix.prefix.prefix.inner
}
fn product_mut(v: &mut AwaitingParentTail) -> &mut Product {
    &mut v.prefix.prefix.prefix.inner
}
fn paths() -> crate::intro::Paths {
    crate::intro::Paths {
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
    }
}
#[test]
#[ignore = "supplied originals;41dcf0 full parent tail retains every owner"]
fn parent_tail_returns_with_two_real_texts_and_same_product_tree_resources_and_models() {
    let s = Supplied::read();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let before = s.tail(&mut t, &mut n, &mut i);
    assert_eq!(
        (
            before.counter(),
            before.nodes().count(),
            before.node(before.root()).unwrap().children.len()
        ),
        (69, 83, 66)
    );
    let box_ptr = product(&before) as *const Product;
    let root = before.root();
    let children = before.node(root).unwrap().children.clone();
    let old = before
        .nodes()
        .map(|n| (n.id, n as *const Node))
        .collect::<Vec<_>>();
    let trace_start = before.journal().len();
    let fields = [
        before.child0().list,
        before.child1().field(0xb0).unwrap(),
        before.child5().list().unwrap(),
        before.child2().field(0xa4).unwrap(),
    ];
    let fonts = t.fonts() as *const Fonts;
    let language = t.fonts().language.bytes().as_ptr();
    let images = i.records.as_ptr();
    let tables = t
        .fonts()
        .fonts
        .iter()
        .map(|(&id, f)| (id, f.table.as_ptr()))
        .collect::<Vec<_>>();
    let a = n
        .process_primitives_mut()
        .resources
        .model(PanelId::A)
        .unwrap()
        .frames[0]
        .as_ptr();
    let f = n
        .process_primitives_mut()
        .resources
        .model(PanelId::F)
        .unwrap()
        .frames[0]
        .as_ptr();
    let title = before.alias(0x4b8).unwrap();
    let mut slot = Some(before);
    let done = CompletedMainLayout::continue_parent_tail(
        &mut slot,
        &s.exe,
        &mut t,
        n.process_primitives_mut(),
        &mut i,
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(product(&done.prefix) as *const Product, box_ptr);
    assert_eq!(done.root(), root);
    assert_eq!(done.phase(), MainPhase::AwaitingBinding);
    assert_eq!((done.counter(), done.nodes().count()), (71, 85));
    for (id, ptr) in old {
        assert_eq!(done.node(id).unwrap() as *const Node, ptr)
    }
    for id in fields {
        assert!(done.node(id).is_ok())
    }
    assert_eq!(done.alias(0x4b8).unwrap(), title);
    let first = done.alias(0x4bc).unwrap();
    let second = done.alias(0x4c0).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        &done.node(root).unwrap().children[..66],
        children.as_slice()
    );
    assert_eq!(&done.node(root).unwrap().children[66..], &[first, second]);
    for (id, number, offset) in [(first, 69, 0x4bc), (second, 70, 0x4c0)] {
        let node = done.node(id).unwrap();
        assert_eq!(node.family, Family::Text);
        assert_eq!(node.label, Label::Additional(offset));
        assert_eq!(
            (node.parent, node.root, node.number),
            (Some(root), Some(root), Some(number))
        );
        let Subtype::Text(text) = &node.subtype else {
            panic!("real Text, not Visual")
        };
        assert_eq!(
            (text.font, text.style, text.inset, text.scroll),
            (0, 4, 0, 0)
        );
        assert_eq!(text.reset_target, ResetTarget::Ret43b370);
        let events = done.journal()[trace_start..]
            .iter()
            .filter(|j| j.node == id)
            .map(|j| &j.event)
            .collect::<Vec<_>>();
        let at = |e: Event| events.iter().position(|x| **x == e).unwrap();
        assert!(at(Event::Slot04(0x42f140)) < at(Event::Append(root)));
        assert!(at(Event::TextCall(0x441770)) < at(Event::TextCall(0x440650)));
        assert!(at(Event::TextCall(0x440650)) < at(Event::TextCall(0x440b70)));
    }
    assert_eq!(t.fonts() as *const Fonts, fonts);
    assert_eq!(t.fonts().language.bytes().as_ptr(), language);
    assert_eq!(i.records.as_ptr(), images);
    for (id, ptr) in tables {
        assert_eq!(t.fonts().fonts[&id].table.as_ptr(), ptr)
    }
    let p = n.process_primitives_mut();
    assert_eq!(p.resources.model(PanelId::A).unwrap().frames[0].as_ptr(), a);
    assert_eq!(p.resources.model(PanelId::F).unwrap().frames[0].as_ptr(), f);
    assert_eq!(p.main_phase(), MainPhase::AwaitingBinding);
    assert_eq!(p.panels[&A].phase, Phase::AwaitingLayout);
    assert_eq!(p.cells().byte(A + 0x184).unwrap(), 0);
    assert_eq!(p.cells().word(A + 0x388).unwrap(), 71);
    assert_eq!(p.cells().word(A + 0x4bc).unwrap(), 0);
    assert_eq!(
        p.cells().word(A + 0x4c0).unwrap(),
        0,
        "typed aliases never forge native pointers"
    );
    assert_eq!(p.steps.iter().filter(|s| **s == Step::Bind(A)).count(), 1);
    assert!(!p.steps.contains(&Step::Bound(A)));
    assert!(done.require_full_main().unwrap_err().contains("Clock"));
    assert!(done.require_input_or_render().is_err());
    assert!(p.require_process_graph().is_err());
    let trace = &done.journal()[trace_start..];
    assert_eq!(
        trace
            .iter()
            .filter(|j| matches!(j.event, Event::Allocate(_)))
            .count(),
        2
    );
    assert!(matches!(
        trace.last().unwrap().event,
        Event::Returned(0x41dcf0)
    ));
    assert!(
        !trace
            .iter()
            .any(|j| matches!(j.event, Event::TextCall(0x4409c0)))
    );
}
#[test]
#[ignore = "supplied originals; actual old mode, lifecycle writes, hide guard and ordering"]
fn parent_tail_executes_child0_exit_six_reset_child1_entry_activation_then_actual_hide() {
    let s = Supplied::read();
    let pe = PeImage::parse(&s.exe).unwrap();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let before = s.tail(&mut t, &mut n, &mut i);
    let birth = Cells::process_allocation(&s.exe).unwrap();
    assert_eq!(birth.word(0x590f60).unwrap(), 0);
    assert_eq!(birth.word(0x822d50).unwrap(), 0);
    assert_eq!(birth.word(A + 0x39c).unwrap(), 0);
    let start = before.journal().len();
    let original = n
        .process_primitives_mut()
        .cells()
        .read(A + 0x344, 12)
        .unwrap()
        .to_vec();
    assert_eq!(
        n.process_primitives_mut().cells().word(A + 0x39c).unwrap(),
        0,
        "genuine loader preimage old mode"
    );
    let mut slot = Some(before);
    let done = CompletedMainLayout::continue_parent_tail(
        &mut slot,
        &s.exe,
        &mut t,
        n.process_primitives_mut(),
        &mut i,
    )
    .unwrap();
    let c = n.process_primitives_mut().cells();
    assert_eq!(c.word(A + 0x39c).unwrap(), 1);
    assert_eq!(c.byte(A + 0x508).unwrap(), 1);
    assert_eq!(c.byte(A + 0x509).unwrap(), 0);
    assert_eq!(c.word(A + 0x4f4).unwrap(), u32::MAX);
    assert_eq!(c.word(0x822d50).unwrap(), 0);
    assert_eq!(c.word(0x590f60).unwrap(), 0);
    assert_eq!(
        c.read(A + 0x344, 12).unwrap(),
        original.as_slice(),
        "hidden branch skips animation/specialized/group cleanup"
    );
    let group = registration::group_address(Group::Primary);
    assert_eq!(c.word(group + 8).unwrap(), 0);
    assert_eq!(c.word(group + 12).unwrap(), 0);
    for k in 0..4u32 {
        assert_eq!(
            c.word(A + 0x40c + k * 4).unwrap(),
            word(&pe, 0x534ce8 + k * 4).unwrap()
        )
    }
    let zero = done.child0();
    assert_eq!(done.node(zero.list).unwrap().shown, 0);
    assert_eq!(done.node(zero.selector).unwrap().shown, 0);
    let selector = done.alias(0x4cc).unwrap();
    assert_eq!(done.node(selector).unwrap().shown, 0);
    for o in [0x4c4, 0x4c8] {
        assert_eq!(done.node(done.alias(o).unwrap()).unwrap().shown, 0)
    }
    let notice = done.node(done.alias(0x3a0).unwrap()).unwrap();
    let Subtype::Text(notice) = &notice.subtype else {
        panic!()
    };
    assert_eq!(notice.group.count, 0);
    let title = done.node(done.alias(0x4b8).unwrap()).unwrap();
    let Subtype::Text(text) = &title.subtype else {
        panic!()
    };
    assert_eq!((title.shown, text.font), (1, 3));
    assert!(text.group.count > 0);
    assert!(
        text.colors[..text.group.count as usize]
            .iter()
            .all(|&v| v == 16)
    );
    assert_eq!(c.word(0x560820 + 0x20).unwrap(), 0);
    assert_eq!(c.word(0x560820 + 0x24).unwrap(), 0);
    for o in [
        0x2c, 0x30, 0x64, 0x74, 0x80, 0xb0, 0xac, 0xbc, 0x8c, 0x6c, 0x38,
    ] {
        assert_eq!(done.node(done.child1().field(o).unwrap()).unwrap().shown, 0)
    }
    for k in 0..6u32 {
        let node = done.node(done.alias(0x48c + k * 4).unwrap()).unwrap();
        let Subtype::Visual(v) = &node.subtype else {
            panic!()
        };
        assert_eq!(node.shown, u8::from(k < 3));
        assert_eq!(v.selected336, u8::from(k == 0));
        assert_eq!(v.disabled333, u8::from(k == 1 || k == 2));
        if k < 3 {
            assert_eq!(
                v.icon,
                Some(
                    i.id(0x68ae28 + word(&pe, 0x533244 + k * 4).unwrap() * 4)
                        .unwrap()
                )
            );
            assert_eq!(node.animation34, word(&pe, 0x533238 + k * 4).unwrap());
            assert_eq!(node.completion38, 0)
        }
    }
    for k in 0..5u32 {
        let node = done.node(done.alias(0x4a4 + k * 4).unwrap()).unwrap();
        let Subtype::Visual(v) = &node.subtype else {
            panic!()
        };
        assert_eq!(node.shown, 1);
        assert_eq!(v.selected336, u8::from(k == 0));
        assert_eq!(v.disabled333, 0);
        assert_eq!(
            v.icon,
            Some(
                i.id(0x68ae28 + word(&pe, 0x533250 + k * 8).unwrap() * 4)
                    .unwrap()
            )
        );
        assert_eq!(node.animation34, word(&pe, 0x533254 + k * 8).unwrap());
    }
    let trace = &done.journal()[start..];
    let at = |source| trace.iter().position(|j| j.source == source).unwrap();
    assert!(at(0x41e946) < at(0x41e963));
    assert!(at(0x41e963) < at(0x41e98f));
    assert!(at(0x41e98f) < at(0x41e9e3));
    assert_eq!(
        trace
            .iter()
            .filter(|j| j.source == 0x41e9aa && j.event == Event::Geometry(0x41f120))
            .count(),
        6
    );
    assert!(at(0x41e9e3) < at(0x41ea5a));
    assert!(at(0x41ea5a) < at(0x41ea61));
    assert!(at(0x41ea61) < at(0x41ea70));
    assert!(
        !trace
            .iter()
            .any(|j| matches!(j.event, Event::Callback(0x40a850 | 0x40a7c0 | 0x41ed90)))
    );
}
#[test]
#[ignore = "supplied originals; stack-derived two Text extents and distinct PC64 stores"]
fn additional_texts_use_first_temporary_extent_and_preserved_st0_for_second_bottom() {
    let s = Supplied::read();
    let pe = PeImage::parse(&s.exe).unwrap();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let mut slot = Some(s.tail(&mut t, &mut n, &mut i));
    let original = n
        .process_primitives_mut()
        .main_named(Some(&ntbs(&pe, 0x53385c).unwrap()), true)
        .unwrap();
    let mut expected_first = original;
    expected_first[0] = add(original[0], float(&pe, 0x5250a0).unwrap()).unwrap();
    expected_first[2] = add(original[2], float(&pe, 0x5250a0).unwrap()).unwrap();
    expected_first[3] = add(original[1], float(&pe, 0x525030).unwrap()).unwrap();
    let mut top = Sum::from_f32(original[1]).unwrap();
    top.add_f32(float(&pe, 0x5250a0).unwrap()).unwrap();
    let top_word = top.to_f32();
    top.add_f32(float(&pe, 0x525030).unwrap()).unwrap();
    let bottom_word = top.to_f32();
    let done = CompletedMainLayout::continue_parent_tail(
        &mut slot,
        &s.exe,
        &mut t,
        n.process_primitives_mut(),
        &mut i,
    )
    .unwrap();
    let first = done.node(done.alias(0x4bc).unwrap()).unwrap();
    let second = done.node(done.alias(0x4c0).unwrap()).unwrap();
    assert_eq!(first.bounds, expected_first[..4]);
    assert_eq!(
        second.bounds,
        [expected_first[0], top_word, expected_first[2], bottom_word]
    );
    assert_eq!(first.depth, 0.5);
    assert_eq!(second.depth, 0.5);
    let calls = done
        .journal()
        .iter()
        .filter(|j| [0x41e7bc, 0x41e801].contains(&j.source))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|j| j.event == Event::TextCall(0x43b040)));
    for node in [first, second] {
        let Subtype::Text(text) = &node.subtype else {
            panic!()
        };
        assert_eq!(text.pitch, 10);
        assert_eq!(text.style, 4);
        assert_eq!(text.reset_target, ResetTarget::Ret43b370);
    }
}
#[test]
#[ignore = "supplied originals; foreign source/resources/process and independent selection rejection before moves"]
fn tail_rejects_foreign_or_selected_inputs_before_claim_or_resource_mutation() {
    let s = Supplied::read();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let mut slot = Some(s.tail(&mut t, &mut n, &mut i));
    let ptr = product(slot.as_ref().unwrap()) as *const Product;
    let journal = slot.as_ref().unwrap().journal().len();
    let scratch = t.fonts().scratch.clone();
    let mut other_t = s.text();
    let mut other_i = s.images();
    let mut other_n = s.navigator();
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &s.exe,
            &mut other_t,
            n.process_primitives_mut(),
            &mut i
        )
        .is_err()
    );
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &s.exe,
            &mut t,
            n.process_primitives_mut(),
            &mut other_i
        )
        .is_err()
    );
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &s.exe,
            &mut t,
            other_n.process_primitives_mut(),
            &mut i
        )
        .is_err()
    );
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            b"bad PE",
            &mut t,
            n.process_primitives_mut(),
            &mut i
        )
        .is_err()
    );
    let pe = PeImage::parse(&s.exe).unwrap();
    let slice = pe.at(0x41e759, 1).unwrap();
    let offset = slice.as_ptr() as usize - s.exe.as_ptr() as usize;
    let mut changed = s.exe.clone();
    changed[offset] ^= 1;
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &changed,
            &mut t,
            n.process_primitives_mut(),
            &mut i
        )
        .unwrap_err_text()
        .contains("source41dcf0")
    );
    n.process_primitives_mut()
        .test_tail_word(0x590f60, 1)
        .unwrap();
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &s.exe,
            &mut t,
            n.process_primitives_mut(),
            &mut i
        )
        .err()
        .unwrap()
        .contains("590f60")
    );
    assert_eq!(
        n.process_primitives_mut().cells().word(0x822d50).unwrap(),
        0
    );
    //A separate new source-born product tests822d50; never force either NULL.
    assert_eq!(product(slot.as_ref().unwrap()) as *const Product, ptr);
    assert_eq!(slot.as_ref().unwrap().journal().len(), journal);
    assert_eq!(t.fonts().scratch, scratch);
    assert_eq!(
        slot.as_ref().unwrap().phase(),
        MainPhase::AwaitingParentTail
    );
    assert_eq!(
        n.process_primitives_mut().main_phase(),
        MainPhase::AwaitingParentTail
    );
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let mut slot = Some(s.tail(&mut t, &mut n, &mut i));
    n.process_primitives_mut()
        .test_tail_word(0x822d50, 1)
        .unwrap();
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &s.exe,
            &mut t,
            n.process_primitives_mut(),
            &mut i
        )
        .err()
        .unwrap()
        .contains("822d50")
    );
    assert_eq!(
        n.process_primitives_mut().cells().word(0x590f60).unwrap(),
        0
    );
    assert_eq!(
        slot.as_ref().unwrap().phase(),
        MainPhase::AwaitingParentTail
    );
}
#[test]
#[ignore = "supplied originals; reached Text failure retains partial arena and forbids replay"]
fn failed_notice_after_additional_texts_retains_partial_owners_and_fault_without_replay() {
    let s = Supplied::read();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let mut before = s.tail(&mut t, &mut n, &mut i);
    let id = before.alias(0x3a0).unwrap();
    let retained = product(&before) as *const Product;
    product_mut(&mut before).text_mut(id).unwrap().reset_target = ResetTarget::Unsupported(0x1234);
    let mut slot = Some(before);
    let error = CompletedMainLayout::continue_parent_tail(
        &mut slot,
        &s.exe,
        &mut t,
        n.process_primitives_mut(),
        &mut i,
    )
    .err()
    .unwrap();
    assert!(error.contains("41e946"));
    let partial = slot.as_ref().unwrap();
    assert_eq!(product(partial) as *const Product, retained);
    assert_eq!(partial.phase(), MainPhase::Failed);
    assert_eq!((partial.counter(), partial.nodes().count()), (71, 85));
    assert!(partial.alias(0x4bc).is_ok());
    assert!(partial.alias(0x4c0).is_ok());
    assert!(matches!(
        partial.journal().last().unwrap().event,
        Event::Fault(_)
    ));
    assert!(
        !partial
            .journal()
            .iter()
            .any(|j| j.event == Event::Returned(0x41dcf0))
    );
    assert_eq!(
        n.process_primitives_mut().cells().byte(A + 0x184).unwrap(),
        0
    );
    let count = partial.journal().len();
    product_mut(slot.as_mut().unwrap())
        .text_mut(id)
        .unwrap()
        .reset_target = ResetTarget::Ret43b370;
    assert!(
        CompletedMainLayout::continue_parent_tail(
            &mut slot,
            &s.exe,
            &mut t,
            n.process_primitives_mut(),
            &mut i
        )
        .is_err()
    );
    assert_eq!(slot.as_ref().unwrap().journal().len(), count);
    assert_eq!(n.process_primitives_mut().main_phase(), MainPhase::Failed);
}
#[test]
#[ignore = "supplied originals; real43bed0 reached motion effects cannot bypass missing actual group"]
fn nonbirth_hide_keeps_animation_and_specialized_ret_effects_then_faults_at_group_owner() {
    let s = Supplied::read();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let before = s.tail(&mut t, &mut n, &mut i);
    n.process_primitives_mut()
        .test_tail_word(A + 0x344, 0x100)
        .unwrap(); //345=1, other real bytes preserved zero
    let mut slot = Some(before);
    let error = CompletedMainLayout::continue_parent_tail(
        &mut slot,
        &s.exe,
        &mut t,
        n.process_primitives_mut(),
        &mut i,
    )
    .err()
    .unwrap();
    assert!(error.contains("43bf29/43bf37"));
    let partial = slot.as_ref().unwrap();
    assert_eq!(partial.phase(), MainPhase::Failed);
    let c = n.process_primitives_mut().cells();
    assert_eq!(c.byte(A + 0x344).unwrap(), 1);
    assert_eq!(c.byte(A + 0x345).unwrap(), 0);
    assert_eq!(c.byte(A + 0x184).unwrap(), 0);
    assert_eq!(
        c.word(A + 0x138).unwrap(),
        registration::group_address(Group::Primary)
    );
    assert!(
        partial
            .journal()
            .iter()
            .any(|j| j.source == 0x43bf26 && j.event == Event::Callback(0x41ed90))
    );
    assert!(
        !partial
            .journal()
            .iter()
            .any(|j| j.event == Event::Returned(0x41dcf0))
    );
    assert_eq!(
        c.word(registration::group_address(Group::Primary) + 0x14)
            .unwrap(),
        0,
        "raw zero never converted into synthetic group success"
    );
}
#[test]
#[ignore = "supplied originals; actual Assets reaches completed layout once and failed replacement retains owners"]
fn actual_assets_retains_layout_and_process_runtime_activity_then_rejects_failed_replacement() {
    let mut published = Box::new(crate::intro::Assets::load(paths()).unwrap());
    assert_eq!(
        published.reordered_main().unwrap().phase(),
        MainPhase::AwaitingBinding
    );
    assert_eq!(
        (
            published.reordered_main().unwrap().counter(),
            published.reordered_main().unwrap().nodes().count()
        ),
        (71, 85)
    );
    let root = published.reordered_main().unwrap().root();
    let font = published.ui_text.fonts() as *const Fonts;
    let id = published.reordered_main().unwrap().alias(0x4bc).unwrap();
    let node = published.reordered_main().unwrap().node(id).unwrap() as *const Node;
    let runtime = &published.process_runtime as *const _;
    let activity = &published.activity as *const _;
    let mut bad = paths();
    bad.executable = openmt2001::fixtures::scratch("TEXT_PROFILE").join("absent-tail-original.exe");
    if let Ok(replacement) = crate::intro::Assets::load(bad) {
        published = Box::new(replacement);
        panic!("missing original published")
    }
    assert_eq!(published.reordered_main().unwrap().root(), root);
    assert_eq!(published.ui_text.fonts() as *const Fonts, font);
    assert_eq!(
        published.reordered_main().unwrap().node(id).unwrap() as *const Node,
        node
    );
    assert_eq!(&published.process_runtime as *const _, runtime);
    assert_eq!(&published.activity as *const _, activity);
    assert_eq!(
        published.navigator.process_primitives_mut().main_phase(),
        MainPhase::AwaitingBinding
    );
    assert_eq!(
        published
            .reordered_main()
            .unwrap()
            .journal()
            .iter()
            .filter(|j| j.event == Event::Returned(0x41dcf0))
            .count(),
        1
    );
    assert_eq!(
        published
            .navigator
            .process_primitives_mut()
            .cells()
            .byte(A + 0x184)
            .unwrap(),
        0
    );
    let moved = published;
    assert_eq!(
        moved.reordered_main().unwrap().node(id).unwrap() as *const Node,
        node
    );
    assert!(moved.reordered_main().unwrap().require_full_main().is_err());
}
//Avoid Debug bounds on move-only successful products while asserting errors.
trait ErrorText {
    fn unwrap_err_text(self) -> String;
}
impl<T> ErrorText for Result<T, String> {
    fn unwrap_err_text(self) -> String {
        match self {
            Err(e) => e,
            Ok(_) => panic!("expected source rejection"),
        }
    }
}
#[test]
#[ignore = "supplied originals; stored old mode dispatch and conditional selection obey actual owner fields"]
fn old_mode_five_exit_and_conditional_mode_store_read_actual_fields() {
    let s = Supplied::read();
    let mut t = s.text();
    let mut n = s.navigator();
    let mut i = s.images();
    let mut before = s.tail(&mut t, &mut n, &mut i);
    n.process_primitives_mut()
        .test_tail_word(A + 0x39c, 5)
        .unwrap();
    let id = before.alias(0x48c).unwrap();
    {
        let visual = product_mut(&mut before).visual_mut(id).unwrap();
        visual.policy338 = 0;
        visual.selected336 = 7;
    }
    let start = before.journal().len();
    let mut slot = Some(before);
    let done = CompletedMainLayout::continue_parent_tail(
        &mut slot,
        &s.exe,
        &mut t,
        n.process_primitives_mut(),
        &mut i,
    )
    .unwrap();
    let trace = &done.journal()[start..];
    assert!(
        trace
            .iter()
            .any(|j| j.source == 0x41e98a && j.event == Event::Callback(0x409e90))
    );
    assert!(
        !trace
            .iter()
            .any(|j| j.source == 0x41e963 && j.event == Event::Callback(0x40a910))
    );
    let Subtype::Visual(visual) = &done.node(id).unwrap().subtype else {
        panic!()
    };
    assert_eq!(
        visual.selected336, 7,
        "442af0 calls retain selection when actual policy338 is not1"
    );
    let child = done.child5();
    assert_eq!(done.node(child.list().unwrap()).unwrap().shown, 0);
    assert_eq!(
        n.process_primitives_mut().cells().word(A + 0x39c).unwrap(),
        1
    );
}
