//! Finite regressions over actual case0 transactions and
//! the shared production C checker; no Inspection/GPU, full graph activation,
//! synthetic successful family/phase callback, original or runtime execution.
use super::super::{
    case_zero,
    main_panel::{self, Access, Prospective, regression_tests},
    navigation::{self, Group},
    owner::Owner,
    property_child, strategic_panel,
    ui_graph::{self, Panel, PanelId, SelectionAccess, UiGraphState},
    ui_selection::{Selected, SelectionState},
};
use super::{PavementModel, Pool, ResetTransform};
use openmt2001::camera::bank::{Bank, Capture};
use std::cell::Cell;

struct Installed {
    owner: Owner,
    rows: [[u8; 380]; 52],
    model: PavementModel,
    reset: ResetTransform,
    _assets: crate::selection::Assets,
    stale: Selected,
    fresh: Selected,
}
impl Installed {
    fn new() -> Self {
        let definition = [0; 116]; // Authored fixture, no live manager claim.
        let assets = crate::selection::Assets::descriptor_test_assets(Vec::new(), definition);
        let model = PavementModel::bind(&assets).unwrap();
        let rows = [[0; 380]; 52];
        let reset = ResetTransform([0x10203040; 12]);
        let pool = Pool::from_rows(&rows, 1, &model, &reset).unwrap();
        let stale = pool.selected(pool.lookup(0).unwrap().unwrap()).unwrap();
        let mut owner = Owner::new(
            Bank::from_capture(Capture::from_records([[0; 196]; 2])),
            0x91,
        );
        owner
            .install_ui_selection(SelectionState::installed(
                Some(stale),
                Some(stale),
                Some(stale),
                None,
                Some(0x703000),
                None,
                12,
                13,
            ))
            .unwrap();
        owner.install_property_descriptors(pool).unwrap();
        owner
            .property_descriptors_mut()
            .unwrap()
            .rebuild_rows(&rows, 1, &model, &reset)
            .unwrap();
        let pool = owner.property_descriptors().unwrap();
        let fresh = pool.selected(pool.lookup(0).unwrap().unwrap()).unwrap();
        assert_ne!(stale.key, fresh.key);
        assert_eq!((stale.kind, stale.category), (fresh.kind, fresh.category));
        {
            let cells = owner.ui_selection_mut().unwrap();
            cells.published = Some(fresh);
            cells.transient = Some(fresh);
        }
        // The failure exists independently of the entry under test.
        assert!(
            owner
                .current_ui_selected()
                .unwrap_err()
                .contains("stale property descriptor allocation")
        );
        Self {
            owner,
            rows,
            model,
            reset,
            stale,
            fresh,
            _assets: assets,
        }
    }
    fn cells(&self) -> String {
        format!(
            "{:?} dirty={}",
            self.owner.ui_selection().unwrap(),
            self.owner.kind_dirty()
        )
    }
    fn still_stale(&self) {
        assert_eq!(self.owner.ui_selection().unwrap().current, Some(self.stale));
        assert_eq!(
            self.owner.ui_selection().unwrap().published,
            Some(self.fresh)
        );
        assert_eq!(
            self.owner.ui_selection().unwrap().transient,
            Some(self.fresh)
        );
        assert!(
            self.owner
                .current_ui_selected()
                .unwrap_err()
                .contains("stale property descriptor allocation")
        );
        let pool = self.owner.property_descriptors().unwrap();
        assert_eq!(
            pool.selected(pool.lookup(0).unwrap().unwrap()).unwrap(),
            self.fresh
        );
    }
}
type Check = fn(&Prospective, &Owner, Option<Selected>) -> Result<(), String>;
type Apply = fn(&mut Prospective, &mut Owner) -> Result<(), String>;
fn entries() -> [(&'static str, Check, Apply); 6] {
    [
        ("exit", case_zero::exit_check, case_zero::exit),
        ("enter", case_zero::enter_check, case_zero::enter),
        ("activate", case_zero::activate_check, case_zero::activate),
        ("reset", case_zero::reset_check, case_zero::reset),
        (
            "refresh",
            case_zero::refresh_check,
            case_zero::refresh_when_clear,
        ),
        ("show", case_zero::show_check, case_zero::show),
    ]
}
#[test]
fn all_six_public_case_zero_entries_allow_independently_stale_current_and_preserve_identity() {
    for (name, check, apply) in entries() {
        let mut f = Installed::new();
        let mut stale = regression_tests::fixture();
        // Exercise reset's state store; all other entries exercise their body.
        if name == "reset" {
            stale.main.case_zero.state = 7;
        }
        let mut live = stale.clone();
        let raw = f.owner.capture();
        let cells = f.cells();
        let before = format!("{stale:?}");
        check(&stale, &f.owner, Some(f.stale)).unwrap();
        assert_eq!(format!("{stale:?}"), before, "{name} checker mutated Main");
        apply(&mut stale, &mut f.owner).unwrap();
        assert_ne!(
            format!("{stale:?}"),
            before,
            "{name} did not reach its owned body"
        );
        assert_eq!(f.cells(), cells);
        assert_eq!(f.owner.capture(), raw);
        f.still_stale();
        // A live replacement of equal kind/category must have identical finite
        // Main/Text/Fonts results: these algorithms never consume those fields.
        f.owner.ui_selection_mut().unwrap().current = Some(f.fresh);
        check(&live, &f.owner, Some(f.fresh)).unwrap();
        apply(&mut live, &mut f.owner).unwrap();
        assert_eq!(format!("{stale:?}"), format!("{live:?}"), "{name}");
        assert_eq!(f.owner.ui_selection().unwrap().current, Some(f.fresh));
    }
}
#[test]
fn public_case_zero_null_tests_retain_the_original_skip_and_nonnull_show_behavior() {
    let mut f = Installed::new();
    let mut p = regression_tests::fixture();
    f.owner.ui_selection_mut().unwrap().current = None;
    let cells = f.cells();
    let raw = f.owner.capture();
    let before = format!("{p:?}");
    case_zero::activate_check(&p, &f.owner, None).unwrap();
    case_zero::activate(&mut p, &mut f.owner).unwrap();
    assert_eq!(format!("{p:?}"), before);
    assert_eq!(f.cells(), cells);
    // Unrelated stale current is nonnull, so activation really shows text.
    f.owner.ui_selection_mut().unwrap().current = Some(f.stale);
    case_zero::activate_check(&p, &f.owner, Some(f.stale)).unwrap();
    case_zero::activate(&mut p, &mut f.owner).unwrap();
    assert_eq!(p.main.notice_caption, Some(0x589));
    assert!(p.main.case_zero.list.as_ref().unwrap().primary.group.count > 0);
    assert_eq!(p.main.case_zero.indicator.as_ref().unwrap().shown, 1);
    f.still_stale();
    p.main.case_zero.state = 1;
    let before = format!("{p:?}");
    for (check, apply) in [
        (
            case_zero::activate_check as Check,
            case_zero::activate as Apply,
        ),
        (
            case_zero::refresh_check as Check,
            case_zero::refresh_when_clear as Apply,
        ),
    ] {
        check(&p, &f.owner, Some(f.stale)).unwrap();
        apply(&mut p, &mut f.owner).unwrap();
        assert_eq!(format!("{p:?}"), before);
        f.still_stale();
    }
    assert_eq!(f.owner.capture(), raw);
}
#[test]
fn public_case_zero_check_apply_resource_errors_agree_and_discard_every_owned_write() {
    for (name, check, apply) in entries() {
        let mut f = Installed::new();
        let mut p = regression_tests::fixture();
        // Real selector attachment error, shared by all six check/apply pairs.
        // No descriptor field read should precede this finite resource check.
        p.main.case_zero.list = None;
        let before = format!("{p:?}");
        let cells = f.cells();
        let raw = f.owner.capture();
        let admission = check(&p, &f.owner, Some(f.stale)).unwrap_err();
        let applied = apply(&mut p, &mut f.owner).unwrap_err();
        assert_eq!(admission, applied, "{name}");
        assert!(applied.contains("list"));
        assert_eq!(format!("{p:?}"), before, "{name}");
        assert_eq!(f.cells(), cells);
        assert_eq!(f.owner.capture(), raw);
        f.still_stale();
    }
}
#[test]
fn selection_free_case_zero_does_not_make_a_later_real_property_field_read_valid() {
    let mut f = Installed::new();
    let mut p = regression_tests::fixture();
    case_zero::show(&mut p, &mut f.owner).unwrap();
    f.still_stale();
    let before = format!("{p:?}");
    let cells = f.cells();
    let raw = f.owner.capture();
    assert!(
        property_child::activate(&mut p, &mut f.owner)
            .unwrap_err()
            .contains("stale property descriptor allocation")
    );
    assert_eq!(format!("{p:?}"), before);
    assert_eq!(f.cells(), cells);
    assert_eq!(f.owner.capture(), raw);
    // Refreshing the Pool is NOT an implicit current-cell repair either.
    f.owner
        .property_descriptors_mut()
        .unwrap()
        .rebuild_rows(&f.rows, 1, &f.model, &f.reset)
        .unwrap();
    assert_eq!(f.owner.ui_selection().unwrap().current, Some(f.stale));
}

/// Actual finite Graph/Main/Locator/navigation owners. Only the represented C
/// family delegates to production. Missing generic phase11 is an explicit
/// error, never a successful callback used to satisfy an admission assertion.
struct CGraph {
    fonts: super::super::case_zero_text::Fonts,
    graph: UiGraphState,
    locator: crate::locator::Locator,
    navigation: navigation::State,
    family_calls: Cell<u32>,
    phase_calls: Cell<u32>,
    passed: Cell<Option<Selected>>,
}
const PHASE_UNREPRESENTED: &str =
    "finite CPU fixture does not represent generic C phase11 dispatch";
impl CGraph {
    fn new(retained: Selected) -> Self {
        use crate::locator::strategic::{
            self, EnabledBufferProvenance, NormalAllocationPath, NormalLiveEnabledBuffer,
        };
        let witness = NormalLiveEnabledBuffer::from_live_unadjusted_684_allocation(
            0x940a38,
            4,
            16,
            NormalAllocationPath::Bump,
        )
        .unwrap();
        let mut s = strategic::selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::StrictV14,
            EnabledBufferProvenance::NormalLive(witness),
        );
        // Real ownership page avoids requiring an unrepresented People GPU
        // attachment. The finite show, active-page message and tree stay real.
        s.city_page = 3;
        s.cache = 123;
        let mut locator = crate::locator::refresh_ui_tests::fresh();
        locator.install_strategic(s).unwrap();
        let mut graph = ui_graph::refresh_fixture([0; 600]);
        graph.retained_c = Some(retained);
        strategic_panel::install(&mut graph.families);
        Self {
            fonts: regression_tests::fonts(),
            graph,
            locator,
            navigation: navigation::State::new(0, 7, 2, [0; 3], Some(ui_graph::consumers())),
            family_calls: Cell::new(0),
            phase_calls: Cell::new(0),
            passed: Cell::new(None),
        }
    }
    fn snapshot(&self) -> String {
        let g = &self.graph;
        let n = &self.navigation;
        format!(
            "fonts={:?} main={:?} panels={:?} groups={:?} retained={:?} world={:?} report={:?} \
            flags={:?} focus={:?} compass={:?} speed={:?} nav={:?} locator={:?}",
            self.fonts,
            g.main,
            g.panels,
            g.groups,
            g.retained_c,
            g.world,
            g.report_inputs,
            (g.human_enabled, g.dispatch_blocked),
            g.focus_child,
            (g.compass_literal, g.compass_identifier),
            (g.speed_disabled, g.speed_blink),
            (
                n.mode(),
                n.shadow(),
                n.previous(),
                n.saved_visibility(),
                n.compass_identifier()
            ),
            self.locator.strategic().unwrap()
        )
    }
}
impl Access for CGraph {
    fn fonts(&self) -> Result<&super::super::case_zero_text::Fonts, String> {
        Ok(&self.fonts)
    }
    fn text_parts_mut(
        &mut self,
    ) -> Result<
        (
            &mut main_panel::State,
            &mut super::super::case_zero_text::Fonts,
        ),
        String,
    > {
        Ok((&mut self.graph.main, &mut self.fonts))
    }
    fn main(&self) -> Result<&main_panel::State, String> {
        Ok(&self.graph.main)
    }
    fn main_mut(&mut self) -> Result<&mut main_panel::State, String> {
        Ok(&mut self.graph.main)
    }
    fn panel(&self) -> Result<&Panel, String> {
        self.graph.panel(PanelId::A)
    }
    fn panel_mut(&mut self) -> Result<&mut Panel, String> {
        self.graph.panel_mut(PanelId::A)
    }
}
impl SelectionAccess for CGraph {
    fn selection_graph(&self) -> Result<&UiGraphState, String> {
        Ok(&self.graph)
    }
    fn navigation_state(&self) -> Result<&navigation::State, String> {
        Ok(&self.navigation)
    }
    fn tutorial_session(&self) -> Result<bool, String> {
        panic!("unrepresented tutorial consumer")
    }
    fn check_group(&self, _: Group) -> Result<(), String> {
        panic!("unrepresented group consumer")
    }
    fn check_panel(&self, _: PanelId, _: bool, _: &Owner) -> Result<(), String> {
        panic!("unrepresented nested panel consumer")
    }
    fn check_family(
        &self,
        id: PanelId,
        shown: bool,
        owner: &Owner,
        current: Option<Selected>,
    ) -> Result<(), String> {
        assert_eq!((id, shown), (PanelId::C, true));
        assert!(self.graph.families.c_show.is_some());
        self.family_calls.set(self.family_calls.get() + 1);
        self.passed.set(current);
        // Exact production body used by direct show and the installed family.
        strategic_panel::check_show_owner(&self.graph, &self.locator, owner, current)
    }
    fn check_upper_tree(&self) -> Result<(), String> {
        panic!("unrepresented upper tree consumer")
    }
    fn check_shown_phase11(&self, id: PanelId, _: &Owner, _: bool) -> Result<(), String> {
        assert_eq!(id, PanelId::C);
        self.phase_calls.set(self.phase_calls.get() + 1);
        Err(PHASE_UNREPRESENTED.into())
    }
}
#[test]
fn direct_c_show_common_checker_accepts_stale_current_and_keeps_read_only_owners() {
    let f = Installed::new();
    let c = CGraph::new(f.fresh);
    let before = c.snapshot();
    let cells = f.cells();
    let raw = f.owner.capture();
    strategic_panel::check_show_owner(
        &c.graph,
        &c.locator,
        &f.owner,
        f.owner.ui_selection().unwrap().current,
    )
    .unwrap();
    assert_eq!(c.snapshot(), before);
    assert_eq!(f.cells(), cells);
    assert_eq!(f.owner.capture(), raw);
    f.still_stale();
}
#[test]
fn normal_generic_c_show_reaches_real_checker_with_raw_identity_before_missing_phase_error() {
    let f = Installed::new();
    let c = CGraph::new(f.fresh);
    let before = c.snapshot();
    let cells = f.cells();
    let raw = f.owner.capture();
    assert_eq!(
        ui_graph::check_visibility(&c, PanelId::C, true, &f.owner).unwrap_err(),
        PHASE_UNREPRESENTED
    );
    assert_eq!(c.family_calls.get(), 1);
    assert_eq!(c.phase_calls.get(), 1);
    assert_eq!(c.passed.get(), Some(f.stale));
    assert_eq!(c.snapshot(), before);
    assert_eq!(f.cells(), cells);
    assert_eq!(f.owner.capture(), raw);
    f.still_stale();
}
#[test]
fn direct_and_generic_c_show_still_reach_real_selector_and_tree_failures() {
    use crate::locator::strategic::{EnabledBufferProvenance, tree::Handler};
    for tree_failure in [false, true] {
        let f = Installed::new();
        let mut c = CGraph::new(f.fresh);
        if tree_failure {
            c.locator
                .strategic_mut()
                .unwrap()
                .tree
                .nodes
                .get_mut(&5)
                .unwrap()
                .handler = Handler::Unported(0xdeadbeef);
        } else {
            c.locator
                .strategic_mut()
                .unwrap()
                .commodity_enabled_provenance = EnabledBufferProvenance::Unrepresented;
        }
        let before = c.snapshot();
        let cells = f.cells();
        let raw = f.owner.capture();
        let error =
            strategic_panel::check_show_owner(&c.graph, &c.locator, &f.owner, Some(f.stale))
                .unwrap_err();
        if tree_failure {
            assert!(error.contains("deadbeef"), "{error}");
        } else {
            assert!(error.contains("UnsupportedOriginalMemoryRead"), "{error}");
        }
        assert_eq!(
            ui_graph::check_visibility(&c, PanelId::C, true, &f.owner).unwrap_err(),
            error
        );
        assert_eq!(c.family_calls.get(), 1);
        assert_eq!(c.phase_calls.get(), 0);
        assert_eq!(c.passed.get(), Some(f.stale));
        assert_eq!(c.snapshot(), before);
        assert_eq!(f.cells(), cells);
        assert_eq!(f.owner.capture(), raw);
        f.still_stale();
    }
}
