//! Concrete production focus-adapter regressions.
//! Authored fixture values; real Focus/UiGraphState/cache fields and the exact
//! field adapters called by Bridge and upper_f. Only font measurement fails
//! deliberately. No font substitute, GPU draw, actual cold48 or native proof.
use super::super::{
    focus::{ChildKey, Focus},
    upper_f,
};
use super::*;
use openmt2001::render::PreparedScene;

const PRIMARY: u32 = 42;
const CHILD: u32 = 43;
const CACHE: Option<(u32, [i32; 2])> = Some((CHILD, [7, 9]));
fn compass() -> WidgetId {
    WidgetId {
        panel: PanelId::E,
        kind: WidgetKind::Compass,
    }
}
fn f() -> WidgetId {
    group_dispatch::panel(PanelId::F)
}
struct Fixture {
    graph: ui_graph::UiGraphState,
    focus: Focus,
    tooltip: Option<PreparedScene>,
    tooltip_state: Option<(u32, [i32; 2])>,
}
impl Fixture {
    fn new(child: bool) -> Self {
        let mut graph = ui_graph::refresh_fixture([0; 600]);
        let mut focus = Focus::default();
        focus.show_category(0, PRIMARY, true);
        graph.focus_child = None;
        if child {
            focus.show_child(ChildKey::Compass, CHILD, true).unwrap();
            graph.focus_child = Some(compass());
        }
        Self {
            graph,
            focus,
            tooltip: Some(PreparedScene::default()),
            tooltip_state: CACHE,
        }
    }
    fn adapter(&mut self) -> FocusAdapter<'_> {
        FocusAdapter {
            graph: &mut self.graph,
            focus: &mut self.focus,
            tooltip: &mut self.tooltip,
            tooltip_state: &mut self.tooltip_state,
        }
    }
    fn assert_cache_retained(&self) {
        assert!(self.tooltip.is_some());
        assert_eq!(self.tooltip_state, CACHE);
    }
    fn assert_child_cleared(&mut self) {
        assert_eq!(self.graph.focus_child, None);
        assert_eq!(self.focus.retained_text_identifier(), Some(PRIMARY));
        //Removing primary must now leave no child text: inspect the concrete
        //owner's behavior, not merely the graph's identity publication.
        self.focus.clear_primary();
        assert_eq!(self.focus.retained_text_identifier(), None);
    }
}

#[test]
fn concrete_clear_null_child_skips_failing_measure_and_cache_invalidation() {
    let mut v = Fixture::new(false);
    let mut calls = 0;
    assert_eq!(v.focus.caption(), Some(PRIMARY));
    v.adapter()
        .clear_child(|_| {
            calls += 1;
            Err("controlled font failure".into())
        })
        .unwrap();
    assert_eq!(calls, 0);
    assert_eq!(
        v.focus.caption(),
        Some(PRIMARY),
        "no dwell refresh on immediate RET"
    );
    assert_eq!(v.graph.focus_child, None);
    v.assert_cache_retained();
}

#[test]
fn concrete_clear_nonnull_retains_both_clears_on_measure_error() {
    let mut v = Fixture::new(true);
    let mut measured = Vec::new();
    let error = v
        .adapter()
        .clear_child(|id| {
            measured.push(id);
            Err("controlled font failure".into())
        })
        .unwrap_err();
    assert_eq!(error, "controlled font failure");
    assert_eq!(measured, vec![PRIMARY]);
    assert_eq!(v.focus.caption(), None, "real refresh reset dwell");
    v.assert_cache_retained();
    v.assert_child_cleared();
}

#[test]
fn concrete_clear_success_measures_primary_once_then_invalidates_cache() {
    let mut v = Fixture::new(true);
    let mut measured = Vec::new();
    v.adapter()
        .clear_child(|id| {
            measured.push(id);
            Ok(())
        })
        .unwrap();
    assert_eq!(measured, vec![PRIMARY]);
    assert!(v.tooltip.is_none());
    assert_eq!(v.tooltip_state, None);
    v.assert_child_cleared();
}

#[test]
fn concrete_f_empty_child_error_keeps_cleared_identity_and_prior_group_effects() {
    let mut v = Fixture::new(true);
    let mut measured = Vec::new();
    {
        let g = v.graph.group_mut(Group::Primary);
        g.selected = Some(f());
        g.hovered = Some(f());
        g.dwell = Some(f());
        g.blocks_input = 1;
    }
    let error = upper_f::focus_dwell_panel_with_measure(v.adapter(), |id| {
        measured.push(id);
        Err("controlled unsupported font layout".into())
    })
    .unwrap_err();
    assert_eq!(error, "controlled unsupported font layout");
    assert_eq!(measured, vec![PRIMARY]);
    let g = v.graph.group(Group::Primary);
    assert_eq!(
        (g.selected, g.hovered, g.dwell, g.blocks_input),
        (Some(f()), Some(f()), Some(f()), 1)
    );
    assert_eq!(v.focus.caption(), None);
    v.assert_cache_retained();
    v.assert_child_cleared();
}

#[test]
fn concrete_f_empty_child_success_and_wrong_dwell_admission() {
    let mut v = Fixture::new(true);
    let mut calls = 0;
    v.graph.group_mut(Group::Primary).dwell = Some(compass());
    let error = upper_f::focus_dwell_panel_with_measure(v.adapter(), |_| {
        calls += 1;
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("actual pointer-group dwell"));
    assert_eq!(calls, 0);
    assert_eq!(v.graph.focus_child, Some(compass()));
    assert_eq!(v.focus.caption(), Some(CHILD));
    v.assert_cache_retained();
    v.graph.group_mut(Group::Primary).dwell = Some(f());
    upper_f::focus_dwell_panel_with_measure(v.adapter(), |id| {
        assert_eq!(id, PRIMARY);
        calls += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert!(v.tooltip.is_none());
    assert_eq!(v.tooltip_state, None);
    v.assert_child_cleared();
}

///Only drives43c840's admitted generic3 callback mutation. Every focus call
///is the SAME concrete production field adapter above, not a scripted clear.
///All operations outside this finite reset path panic instead of succeeding.
struct DwellReset<'a> {
    v: &'a mut Fixture,
    clear_dwell_in_leave: bool,
    leaves: usize,
    clears: usize,
    measures: usize,
}
impl Dispatch for DwellReset<'_> {
    fn group(&self, g: Group) -> Result<&PointerGroup> {
        Ok(self.v.graph.group(g))
    }
    fn group_mut(&mut self, g: Group) -> Result<&mut PointerGroup> {
        Ok(self.v.graph.group_mut(g))
    }
    fn panel(&self, _: PanelId) -> Result<&Panel> {
        unreachable!("reset does not visit panels")
    }
    fn panel_mut(&mut self, _: PanelId) -> Result<&mut Panel> {
        unreachable!("reset does not animate")
    }
    fn input(&self) -> Result<&StagedInput> {
        unreachable!("reset does not poll input")
    }
    fn override_target(&mut self) -> Result<Option<WidgetId>> {
        unreachable!("reset does not arbitrate")
    }
    fn block34c(&mut self, _: WidgetId) -> Result<u8> {
        unreachable!("reset does not hit")
    }
    fn node1c(&mut self, _: WidgetId) -> Result<u8> {
        unreachable!("reset does not dispatch keys")
    }
    fn slot20(&mut self, _: WidgetId) -> Result<u8> {
        unreachable!("reset does not retain")
    }
    fn slot24(&mut self, _: PanelId) -> Result<Option<WidgetId>> {
        unreachable!("reset does not search")
    }
    fn generic14(&mut self, id: WidgetId, event: u32, a: i32, b: i32) -> Result<()> {
        assert_eq!((id, event, a, b), (compass(), 3, 0, 0));
        self.leaves += 1;
        if self.clear_dwell_in_leave {
            self.v.graph.group_mut(Group::Primary).dwell = None;
        }
        Ok(())
    }
    fn direct18_visible1(&mut self, _: PanelId) -> Result<()> {
        unreachable!("reset does not show")
    }
    fn cursor0_priority1(&mut self) -> Result<()> {
        unreachable!("reset does not arbitrate cursor")
    }
    fn focus_child(&self) -> Result<Option<WidgetId>> {
        Ok(self.v.graph.focus_child)
    }
    fn show_focus43e310(&mut self, _: WidgetId) -> Result<()> {
        unreachable!("reset does not show focus")
    }
    fn clear_focus43e390(&mut self) -> Result<()> {
        self.clears += 1;
        let measures = &mut self.measures;
        self.v.adapter().clear_child(|_| {
            *measures += 1;
            Err("controlled font failure".into())
        })
    }
    fn reset_old_key4439d0(&mut self, _: WidgetId) -> Result<()> {
        unreachable!("reset does not replace keys")
    }
    fn has_character(&mut self) -> Result<bool> {
        unreachable!("reset does not read CHAR")
    }
    fn take_character(&mut self) -> Result<u8> {
        unreachable!("reset does not consume CHAR")
    }
}

#[test]
fn real_reset_dwell_leave_to_null_null_reaches_concrete_immediate_return() {
    let mut v = Fixture::new(false);
    v.graph.group_mut(Group::Primary).dwell = Some(compass());
    let mut c = DwellReset {
        v: &mut v,
        clear_dwell_in_leave: true,
        leaves: 0,
        clears: 0,
        measures: 0,
    };
    group_dispatch::reset_dwell(&mut c, Group::Primary).unwrap();
    assert_eq!(
        (c.leaves, c.clears, c.measures),
        (1, 1, 0),
        "null/null comparison still calls43e390"
    );
    assert_eq!(c.v.graph.group(Group::Primary).dwell, None);
    assert_eq!(c.v.focus.caption(), Some(PRIMARY));
    c.v.assert_cache_retained();
}

#[test]
fn real_reset_dwell_nonnull_font_error_keeps_leave_and_focus_clear_before_final_dwell_store() {
    let mut v = Fixture::new(true);
    v.graph.group_mut(Group::Primary).dwell = Some(compass());
    let mut c = DwellReset {
        v: &mut v,
        clear_dwell_in_leave: false,
        leaves: 0,
        clears: 0,
        measures: 0,
    };
    assert_eq!(
        group_dispatch::reset_dwell(&mut c, Group::Primary).unwrap_err(),
        "controlled font failure"
    );
    assert_eq!((c.leaves, c.clears, c.measures), (1, 1, 1));
    assert_eq!(
        c.v.graph.group(Group::Primary).dwell,
        Some(compass()),
        "final dwell store follows the failed call"
    );
    c.v.assert_cache_retained();
    c.v.assert_child_cleared();
}
