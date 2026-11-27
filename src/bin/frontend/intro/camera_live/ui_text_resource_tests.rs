//! Supplied-original regressions are ignored and use real loaders
//! and retained CPU owners. No production provider or font fallback is faked.
use super::super::{
    case_zero_text::{ErrorWord, ScratchByte},
    main_panel,
};
use super::*;
fn path(key: &str) -> PathBuf {
    openmt2001::fixtures::path(key)
}
struct Inputs {
    exe: Vec<u8>,
    installer: PathBuf,
    language: PathBuf,
    index: ArchiveIndex,
    archive: Vec<u8>,
}
impl Inputs {
    fn supplied() -> Self {
        let exe = fs::read(path("OPENMT2001_EXE")).unwrap();
        let installer = path("OPENMT2001_INSTALLER");
        let language = path("OPENMT2001_LANGUAGE");
        let archive = fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(
            &fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap(),
            archive.len() as u64,
        )
        .unwrap();
        Self {
            exe,
            installer,
            language,
            index,
            archive,
        }
    }
    fn load(&self) -> Result<UiTextResources, String> {
        UiTextResources::load(
            &self.exe,
            &self.installer,
            &self.language,
            &self.index,
            &self.archive,
        )
    }
    fn navigator(&self) -> crate::navigator::Navigator {
        let board =
            openmt2001::board::Board::from_v14(&PeImage::parse(&self.exe).unwrap()).unwrap();
        crate::navigator::Navigator::load(&self.index, &self.archive, &board.blocks, &self.exe)
            .unwrap()
    }
}
#[test]
#[ignore = "supplied original PE, patched language, six TABs, archive"]
fn shared_text_actual_loader_six_tables_pages_and_unknown_error() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    assert_eq!(resources.tables.len(), 6);
    assert_eq!(resources.pages.records.len(), 12);
    for source in &resources.tables {
        assert!(source.owned);
        assert_eq!(
            source.count, 176,
            "inspected supplied tables, not a general parser constant"
        );
        assert_eq!(
            resources.table(source.slot).unwrap(),
            fs::read(&source.path).unwrap()
        );
        let font = &resources.fonts().fonts[&source.slot];
        assert_eq!((font.baseline, font.scale), (0, 1.));
        for page in 0..2 {
            let record = resources.pages.page(source.slot, page).unwrap();
            assert!(!record.texels.is_empty());
            let provenance = &resources.pages.records[(source.slot * 2) as usize + page];
            assert_eq!(provenance.record.len(), 56 + record.texels.len());
        }
    }
    let font = resources.fonts_mut();
    assert_eq!((font.current, font.gap, font.line_mode), (0, -1, 1));
    assert_eq!(font.pair, [2243, 2244]);
    assert_eq!(font.tail, Some(2245));
    assert_eq!(font.context, u32::MAX);
    assert_eq!(font.glyph_callback, 0x4554e0);
    assert_eq!((font.ready_callback, font.four_callback), (0, 0));
    assert_eq!(font.error, ErrorWord::Unknown);
    assert!(font.take_error().unwrap_err().contains("5b4"));
    let w = font.map(b"W").unwrap();
    font.width(&w).unwrap();
    font.height(&w).unwrap();
    assert_eq!(
        font.error,
        ErrorWord::Unknown,
        "successful metrics never consume/zero unwritten error"
    );
}
#[test]
#[ignore = "supplied original resources and retained native-font.bin"]
fn shared_text_tables_match_retained_native_font_fixture_without_regenerating_it() {
    let input = Inputs::supplied();
    let resources = input.load().unwrap();
    let fixture = fs::read(path("OPENMT2001_NATIVE_FONT")).unwrap();
    assert_eq!(&fixture[..8], b"MTFNT001");
    //The existing core matches_native_glyph_layout test consumes the cases.
    //Here only traverse its documented lengths to join each actual TAB owner.
    fn word(bytes: &[u8], at: &mut usize) -> usize {
        let n = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap()) as usize;
        *at += 4;
        n
    }
    let mut at = 8;
    assert_eq!(word(&fixture, &mut at), 6);
    for slot in 0..6 {
        let length = word(&fixture, &mut at);
        let cases = word(&fixture, &mut at);
        assert_eq!(&fixture[at..at + length], resources.table(slot).unwrap());
        at += length;
        assert_eq!(cases, 3);
        for _ in 0..cases {
            let length = word(&fixture, &mut at);
            at += length + 20;
            let low = word(&fixture, &mut at);
            let high = word(&fixture, &mut at);
            at += (low + high) * 80;
        }
    }
    assert_eq!(at, fixture.len());
}
#[test]
#[ignore = "supplied patched USA/board language and original resources"]
fn shared_text_board_operation_preserves_base_rows_ids_and_real_scratch_tail() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    let alphabet = resources.fonts().language.row(0).unwrap().to_vec();
    let prior = resources.fonts().language.row(2241).unwrap().to_vec();
    let count = resources.fonts().language.row_count();
    assert_eq!(count, 2449);
    let old_range = resources.fonts().language.row_range(2241).unwrap();
    resources
        .install_usa_board(&input.exe, &input.installer)
        .unwrap();
    let language = &resources.fonts().language;
    assert_eq!(language.row_count(), count);
    assert_eq!(language.row(0).unwrap(), alphabet);
    assert_eq!(language.row(2241).unwrap(), prior);
    assert_eq!(language.row_range(2241).unwrap(), old_range);
    let board =
        PackedLanguage::load(&input.installer.join("Standard/fonts/board/usa/tycoon")).unwrap();
    for id in 2242..count as u32 {
        assert_eq!(language.row(id).unwrap(), board.row(id - 2241).unwrap());
    }
    let native = expand_one(
        &ntbs(&PeImage::parse(&input.exe).unwrap(), 0x542324, 128).unwrap(),
        b"usa",
    )
    .unwrap();
    let expected = [native, b".off\0".to_vec()].concat();
    for (i, &byte) in expected.iter().enumerate() {
        assert_eq!(resources.fonts().scratch[i], ScratchByte::Byte(byte));
    }
    let scratch = resources.fonts().scratch.clone();
    let base = resources.fonts().language.bytes().as_ptr();
    //Same selected-board call must not reload/reallocate or rewrite scratch.
    resources
        .install_usa_board(&input.exe, &input.installer)
        .unwrap();
    assert_eq!(base, resources.fonts().language.bytes().as_ptr());
    assert_eq!(scratch, resources.fonts().scratch);
    let w = resources.fonts_mut().map(b"W").unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(
        &resources.fonts().scratch[1..],
        &scratch[1..],
        "map writes no NUL and retains filename suffix"
    );
    let scratch = resources.fonts().scratch.clone();
    resources.fonts_mut().map(b"").unwrap();
    assert_eq!(scratch, resources.fonts().scratch);
    let newline = resources.fonts_mut().map(b"\n").unwrap();
    assert_eq!(newline, [alphabet[60], alphabet[78]]);
}
#[test]
#[ignore = "supplied original resources; controlled edits through actual packed owner"]
fn shared_text_map_and_packed_consumers_observe_one_relocated_language_base() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    //Real row identity0 is re-resolved after replacement, never a saved slice.
    resources
        .install_usa_board(&input.exe, &input.installer)
        .unwrap();
    let at = b'W' as usize - 32;
    let before = resources.fonts().language.row(0).unwrap()[at];
    let replacement = before.wrapping_add(1);
    resources.fonts_mut().language.test_byte(at, replacement);
    assert_eq!(resources.fonts_mut().map(b"W").unwrap(), [replacement]);
    //row0 can contain the packed-format prefix; inspect direct owner range to
    //avoid pretending a no-varargs formatter admits that tokenized row.
    assert_eq!(resources.fonts().language.row(0).unwrap()[at], replacement);
    let id = 86;
    let range = resources.fonts().language.row_range(id).unwrap();
    let prefix = resources.fonts().language.bytes()[5];
    let glyph = if prefix == 1 { 2 } else { 1 };
    for at in range {
        resources.fonts_mut().language.test_byte(at, glyph);
    }
    assert!(
        resources
            .fonts()
            .packed(id)
            .unwrap()
            .iter()
            .all(|&g| g == glyph)
    );
}
#[test]
#[ignore = "supplied original resources and actual Navigator/Text CPU factory"]
fn shared_text_default_f_ret_stays_empty_then_explicit_cpu_calls_share_alias_and_manager() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    let mut navigator = input.navigator();
    let primitives = navigator.native_primitives_mut();
    let key = primitives.panels[&0x58cdb0].caption;
    let text = &primitives.texts[&key];
    assert_eq!(
        (text.group.count, text.font, text.pitch, text.visible),
        (0, 0, 10, 64)
    );
    assert!(primitives.require_text_layout(key).is_err());
    assert!(primitives.require_process_graph().is_err());
    let initial_scratch = resources.fonts().scratch.clone();
    let identity = primitives.text_mut(key).unwrap() as *mut _;
    //An explicit CPU layout is the admitted concrete operation in this test;
    //production default F RET never invokes it merely because fonts exist.
    {
        let text = primitives.text_mut(key).unwrap();
        text.literal(resources.fonts_mut(), &[b'W'], &[]).unwrap();
        text.measure(resources.fonts_mut()).unwrap();
        assert!(text.group.count > 0);
        assert_eq!(text.dirty, 1);
        assert_eq!(text.geometry_dirty, 0);
    }
    assert_eq!(identity, primitives.text_mut(key).unwrap() as *mut _);
    assert_ne!(initial_scratch, resources.fonts().scratch);
    let scratch = resources.fonts().scratch.clone();
    //A second real factory product uses exactly the same retained manager.
    let mut second = input.navigator();
    let other = second.native_primitives().panels[&0x58cdb0].caption;
    second
        .native_primitives_mut()
        .text_mut(other)
        .unwrap()
        .literal(resources.fonts_mut(), &[b'W'], &[])
        .unwrap();
    assert_eq!(scratch, resources.fonts().scratch);
    assert_eq!(resources.fonts().fonts[&0].scale, 1.);
    assert_eq!(resources.fonts().error, ErrorWord::Unknown);
}
#[test]
#[ignore = "supplied original resources; no native callback/batch execution"]
fn shared_text_selector_cache_hit_and_direct_index_are_distinct_and_effects_reject() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    let fonts = resources.fonts_mut();
    fonts.select(0).unwrap();
    let cached = fonts.selector.cached;
    assert_eq!(cached, 0);
    let other = (cached + 1) % 6;
    fonts.direct_index(other).unwrap();
    fonts.select(cached).unwrap();
    assert_eq!(fonts.current, other);
    fonts.selector.pending = [1, 0];
    fonts.select(cached).unwrap(); //Native hit skips pending branch.
    let before = fonts.current;
    assert!(fonts.select(other).unwrap_err().contains("pending"));
    assert_eq!(fonts.current, before);
    fonts.selector.pending = [0, 0];
    fonts.selector.notify = 0x1234;
    assert!(fonts.select(other).unwrap_err().contains("callback"));
    assert_eq!(fonts.selector.cached, cached);
    fonts.selector.notify = 0;
    fonts.select(other).unwrap();
    assert_eq!(fonts.selector.cached, other);
}
#[test]
fn shared_text_parser_rejects_partial_off_bad_coverage_and_short_tables() {
    assert!(
        PackedLanguage::parse(&[1], vec![1])
            .unwrap_err()
            .contains("incomplete OFF")
    );
    assert!(
        PackedLanguage::parse(&[2, 0], vec![1])
            .unwrap_err()
            .contains("outside BIN")
    );
    assert!(
        PackedLanguage::parse(&[1, 0], vec![1, 2])
            .unwrap_err()
            .contains("cover BIN")
    );
    for bytes in [vec![], vec![1, 0, 0], vec![1, 0, 0, 0]] {
        assert!(Font::parse(&bytes).is_err());
    }
}
#[test]
#[ignore = "supplied original resources; caller-owned writable scratch directory"]
fn shared_text_partial_failure_preserves_retained_owner_and_actual_error_writer_then_take() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    let fonts = resources.fonts_mut();
    let table = fonts.fonts[&0].table.clone();
    let scratch = fonts.scratch.clone();
    let missing = path("OPENMT2001_TEXT_SCRATCH").join("deliberately-absent-original-font.tab");
    assert!(!missing.exists());
    assert!(fonts.load_table(0, &missing).unwrap_err().contains("font0"));
    assert_eq!(fonts.fonts[&0].table, table);
    assert_eq!(fonts.scratch, scratch);
    assert_eq!(fonts.take_error().unwrap(), 1);
    assert_eq!(fonts.take_error().unwrap(), 0);
    let before = format!("{:?}", resources.fonts());
    assert!(
        UiTextResources::load(
            &input.exe,
            &missing,
            &input.language,
            &input.index,
            &input.archive
        )
        .is_err()
    );
    assert_eq!(
        format!("{:?}", resources.fonts()),
        before,
        "unreturned replacement cannot touch retained owner"
    );
    let records = &resources.pages.records;
    let end = records[10].record.end;
    let mut eleven = resources.pages.bytes[..end].to_vec();
    eleven[..4].copy_from_slice(&11u32.to_le_bytes());
    assert!(
        FontPages::parse(resources.pages.name.clone(), 0..eleven.len(), eleven)
            .unwrap_err()
            .contains("twelve")
    );
}
#[test]
#[ignore = "all actual Intro asset inputs/ffmpeg; root owns heavy execution lane"]
fn shared_text_real_intro_asset_load_retains_process_resource_and_default_ret() {
    //No abbreviated provider: this traverses the real application boundary.
    let paths = crate::intro::Paths {
        compatibility: crate::compatibility::Mode::StrictV14,
        installer: path("OPENMT2001_INSTALLER"),
        ffmpeg: path("OPENMT2001_FFMPEG"),
        scene: path("OPENMT2001_SCENE"),
        executable: path("OPENMT2001_EXE"),
        map: path("OPENMT2001_INTRO_MAP"),
        quality: "low".into(),
        language: path("OPENMT2001_LANGUAGE"),
        profile_root: path("OPENMT2001_TEXT_PROFILE"),
        save_root: path("OPENMT2001_TEXT_SAVES"),
    };
    let assets = crate::intro::Assets::load(paths).unwrap();
    let pointer = assets.ui_text.fonts() as *const _;
    let scratch = assets.ui_text.fonts().scratch.clone();
    assert_eq!(assets.ui_text.tables.len(), 6);
    assert!(assets.ui_text.board_stem.is_some());
    let key = assets.navigator.native_primitives().panels[&0x58cdb0].caption;
    let text = &assets.navigator.native_primitives().texts[&key];
    assert_eq!((text.pitch, text.visible, text.group.count), (10, 64, 0));
    assert!(
        assets
            .navigator
            .native_primitives()
            .require_text_layout(key)
            .is_err()
    );
    //Asset owner move into the same-process holder; no second Fonts is copied.
    let retained = assets;
    assert_eq!(pointer, retained.ui_text.fonts() as *const _);
    assert_eq!(scratch, retained.ui_text.fonts().scratch);
    assert_eq!(retained.ui_text.fonts().error, ErrorWord::Unknown);
}

///Same finite CPU Main/Panel/Text owners used by the existing rollback tests,
///with the real resource producer borrowed rather than a second installed font.
struct BorrowedMain<'a> {
    main: main_panel::State,
    panel: super::super::ui_graph::Panel,
    resources: &'a mut UiTextResources,
}
impl main_panel::Access for BorrowedMain<'_> {
    fn main(&self) -> Result<&main_panel::State, String> {
        Ok(&self.main)
    }
    fn main_mut(&mut self) -> Result<&mut main_panel::State, String> {
        Ok(&mut self.main)
    }
    fn fonts(&self) -> Result<&Fonts, String> {
        Ok(self.resources.fonts())
    }
    fn text_parts_mut(&mut self) -> Result<(&mut main_panel::State, &mut Fonts), String> {
        Ok((&mut self.main, self.resources.fonts_mut()))
    }
    fn panel(&self) -> Result<&super::super::ui_graph::Panel, String> {
        Ok(&self.panel)
    }
    fn panel_mut(&mut self) -> Result<&mut super::super::ui_graph::Panel, String> {
        Ok(&mut self.panel)
    }
}
#[test]
#[ignore = "supplied resources; existing real finite CPU Main/Group/Text owners"]
fn shared_text_real_main_transactions_commit_or_discard_one_common_manager() {
    let input = Inputs::supplied();
    let mut resources = input.load().unwrap();
    let main_panel::Prospective { main, panel, .. } = main_panel::regression_tests::fixture();
    let identity = resources.fonts() as *const _;
    let mut view = BorrowedMain {
        main,
        panel: panel.unwrap(),
        resources: &mut resources,
    };
    main_panel::transact(&mut view, None, |plan| {
        main_panel::clear_notice(plan)?;
        let (main, font) = main_panel::Access::text_parts_mut(plan)?;
        main.notice_text.literal(font, b"W", &[])?;
        main.notice_text.measure(font)
    })
    .unwrap();
    assert_eq!(identity, view.resources.fonts() as *const _);
    let before = format!("{:?}/{:?}", view.main, view.resources.fonts());
    let failure = main_panel::transact(&mut view, None, |plan| {
        main_panel::clear_notice(plan)?;
        plan.fonts.selector.pending = [1, 0];
        plan.main.notice_text.font = 1;
        main_panel::set_notice_identifier(plan, 86)
    })
    .unwrap_err();
    assert!(failure.contains("pending"));
    assert_eq!(
        format!("{:?}/{:?}", view.main, view.resources.fonts()),
        before
    );
    assert_eq!(view.resources.fonts().error, ErrorWord::Unknown);
}
