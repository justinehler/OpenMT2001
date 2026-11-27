//! Synthetic mechanics regressions. These callbacks deliberately
//! mutate state at callback boundaries; they are not native callback models,
//! actual construction products, numeric or whole-program equivalence claims.
use super::super::ui_graph::{WidgetKind, group_index};
use super::*;
use openmt2001::native_input::{
    CharacterQueue, LogicalPointer, MousePacket, RetainedCells, WindowInputEffect,
};
use std::collections::VecDeque;
const P: Group = Group::Primary;
const S: Group = Group::Secondary;
const T: Group = Group::Tertiary;
const A: PanelId = PanelId::A;
const B: PanelId = PanelId::B;
const C: PanelId = PanelId::C;
fn w(id: PanelId) -> WidgetId {
    panel(id)
}
fn child(id: PanelId, n: u32) -> WidgetId {
    WidgetId {
        panel: id,
        kind: WidgetKind::Unported(n),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    Visible(PanelId),
    Hit(PanelId),
    Generic(WidgetId, u32),
    Retain(WidgetId),
    Override,
    HasChar,
    ResetKey(WidgetId),
    GPrelude,
    After,
    Draw(PanelId),
    DrawCursor,
}
#[derive(Clone, Debug)]
enum Action {
    Promote(Group, PanelId),
    Select(Group, Option<WidgetId>),
    Dwell(Group, Option<WidgetId>),
    Hover(Group, Option<WidgetId>),
    Key(Group, Option<WidgetId>),
    Override(Option<WidgetId>),
    Focus(Option<WidgetId>),
    Repeat(i32),
    GFlag(u8),
    Enable(Group, u8),
    Polls(u32),
    Fail,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Log {
    Point(Point),
    Event(WidgetId, u32, i32, i32),
    Focus(WidgetId),
    ClearFocus,
    Cursor,
    VisibleHead(PanelId, Option<PanelId>, u8),
    Repeat(i32),
    Prelude,
    Increment,
    After,
    Take(u8),
    Poll0,
    Poll1,
}
struct Synthetic {
    groups: [PointerGroup; 3],
    panels: BTreeMap<PanelId, Panel>,
    input: StagedInput,
    hits: BTreeMap<PanelId, Option<WidgetId>>,
    retain: u8,
    override_target: Option<WidgetId>,
    focus: Option<WidgetId>,
    script: VecDeque<(Point, Vec<Action>)>,
    log: Vec<Log>,
    repeat: i32,
    g_flag: u8,
    suppressed: u8,
    signed: i8,
    child_block: u8,
    key_byte: u8,
}
fn input(
    left: u8,
    prior_left: u8,
    right: u8,
    prior_right: u8,
    wheel: i32,
    prior_wheel: i32,
) -> StagedInput {
    let mut current = [0; 16];
    current[12] = left;
    current[13] = right;
    current[8..12].copy_from_slice(&wheel.to_le_bytes());
    let mut prior = [0; 16];
    prior[12] = prior_left;
    prior[13] = prior_right;
    prior[8..12].copy_from_slice(&prior_wheel.to_le_bytes());
    StagedInput::from_process_cells(
        RetainedCells {
            mouse_current: MousePacket(current),
            mouse_prior: MousePacket(prior),
            keyboard_current: [0; 256],
            keyboard_prior: [0; 256],
            logical_pointer: LogicalPointer { x: 7, y: 9 },
            left_override: u32::from(left),
            right_override: u32::from(right),
            double_click: 0,
            keyboard_poll_tick: 0,
            device_event_count: 16,
            input_enabled: 1,
            focus_transition: 0,
            recording: 0,
            space_recorded: 0,
        },
        CharacterQueue::initialize([0x9a; 16]),
    )
}
impl Synthetic {
    fn new(draw: &[PanelId]) -> Self {
        let groups = [
            PointerGroup::after_constructor(draw.to_vec(), None),
            PointerGroup::after_constructor(vec![PanelId::Frontend(0)], None),
            PointerGroup::after_constructor(vec![PanelId::Auction(0)], None),
        ];
        let mut panels = BTreeMap::new();
        for (i, g) in groups.iter().enumerate() {
            for id in g.links.identities() {
                let mut p = Panel::bound([P, S, T][i], 4, 0, [0., 0.], false, None, 255).unwrap();
                p.requested = 1;
                panels.insert(id, p);
            }
        }
        let mut out = Self {
            groups,
            panels,
            input: input(0, 0, 0, 0, 0, 0),
            hits: BTreeMap::new(),
            retain: 1,
            override_target: None,
            focus: None,
            script: VecDeque::new(),
            log: Vec::new(),
            repeat: 1,
            g_flag: 0,
            suppressed: 0,
            signed: 0,
            child_block: 0,
            key_byte: 1,
        };
        out.groups[0].enabled = 1;
        out
    }
    fn script(&mut self, point: Point, actions: Vec<Action>) {
        self.script.push_back((point, actions));
    }
    fn point(&mut self, point: Point) -> Result<()> {
        self.log.push(Log::Point(point));
        if self.script.front().is_some_and(|(at, _)| *at == point) {
            let (_, actions) = self.script.pop_front().unwrap();
            for a in actions {
                match a {
                    Action::Promote(g, id) => self.groups[group_index(g)].links.promote(id)?,
                    Action::Select(g, id) => self.groups[group_index(g)].selected = id,
                    Action::Dwell(g, id) => self.groups[group_index(g)].dwell = id,
                    Action::Hover(g, id) => self.groups[group_index(g)].hovered = id,
                    Action::Key(g, id) => self.groups[group_index(g)].key_target = id,
                    Action::Override(id) => self.override_target = id,
                    Action::Focus(id) => self.focus = id,
                    Action::Repeat(v) => self.repeat = v,
                    Action::GFlag(v) => self.g_flag = v,
                    Action::Enable(g, v) => self.groups[group_index(g)].enabled = v,
                    Action::Polls(wparam) => {
                        self.input
                            .window_input_effect(WindowInputEffect::ButtonMessage { wparam });
                        self.log.push(Log::Poll0);
                        self.input
                            .poll0_live(&mut SyntheticDevice)
                            .map_err(|e| format!("{e:?}"))?;
                        self.log.push(Log::Poll1);
                        self.input
                            .poll1_live(
                                openmt2001::native_input::KeyboardMode::Unbuffered256,
                                &mut SyntheticDevice,
                            )
                            .map_err(|e| format!("{e:?}"))?;
                    }
                    Action::Fail => return Err("synthetic callback boundary absent".into()),
                }
            }
        }
        Ok(())
    }
    fn visible(&self) -> Vec<PanelId> {
        self.log
            .iter()
            .filter_map(|l| match l {
                Log::Point(Point::Visible(id)) => Some(*id),
                _ => None,
            })
            .collect()
    }
    fn events(&self) -> Vec<(WidgetId, u32, i32, i32)> {
        self.log
            .iter()
            .filter_map(|l| match l {
                Log::Event(id, e, a, b) => Some((*id, *e, *a, *b)),
                _ => None,
            })
            .collect()
    }
    fn queue(&mut self, n: usize) {
        for _ in 0..n {
            self.input
                .window_input_effect(WindowInputEffect::Character { wparam: 65 });
        }
    }
}
impl Dispatch for Synthetic {
    fn group(&self, g: Group) -> Result<&PointerGroup> {
        Ok(&self.groups[group_index(g)])
    }
    fn group_mut(&mut self, g: Group) -> Result<&mut PointerGroup> {
        Ok(&mut self.groups[group_index(g)])
    }
    fn panel(&self, id: PanelId) -> Result<&Panel> {
        self.panels.get(&id).ok_or("synthetic panel absent".into())
    }
    fn panel_mut(&mut self, id: PanelId) -> Result<&mut Panel> {
        self.panels
            .get_mut(&id)
            .ok_or("synthetic panel absent".into())
    }
    fn input(&self) -> Result<&StagedInput> {
        Ok(&self.input)
    }
    fn override_target(&mut self) -> Result<Option<WidgetId>> {
        //Read result before scripted mutation: a subsequent load can differ.
        let result = self.override_target;
        self.point(Point::Override)?;
        Ok(result)
    }
    fn block34c(&mut self, id: WidgetId) -> Result<u8> {
        if id.kind == WidgetKind::Panel {
            Ok(self.panel(id.panel)?.block34c)
        } else {
            Ok(self.child_block)
        }
    }
    fn node1c(&mut self, _id: WidgetId) -> Result<u8> {
        Ok(self.key_byte)
    }
    fn slot20(&mut self, id: WidgetId) -> Result<u8> {
        self.point(Point::Retain(id))?;
        Ok(self.retain)
    }
    fn slot24(&mut self, id: PanelId) -> Result<Option<WidgetId>> {
        self.point(Point::Hit(id))?;
        Ok(self.hits.get(&id).copied().flatten())
    }
    fn generic14(&mut self, id: WidgetId, e: u32, a: i32, b: i32) -> Result<()> {
        self.log.push(Log::Event(id, e, a, b));
        self.point(Point::Generic(id, e))
    }
    fn direct18_visible1(&mut self, id: PanelId) -> Result<()> {
        let g = self.panel(id)?.group;
        self.log.push(Log::VisibleHead(
            id,
            self.group(g)?.links.head,
            self.panel(id)?.promote34b,
        ));
        self.point(Point::Visible(id))
    }
    fn cursor0_priority1(&mut self) -> Result<()> {
        self.log.push(Log::Cursor);
        Ok(())
    }
    fn focus_child(&self) -> Result<Option<WidgetId>> {
        Ok(self.focus)
    }
    fn show_focus43e310(&mut self, id: WidgetId) -> Result<()> {
        self.log.push(Log::Focus(id));
        self.focus = Some(id);
        Ok(())
    }
    fn clear_focus43e390(&mut self) -> Result<()> {
        self.log.push(Log::ClearFocus);
        self.focus = None;
        Ok(())
    }
    fn reset_old_key4439d0(&mut self, id: WidgetId) -> Result<()> {
        self.point(Point::ResetKey(id))
    }
    fn has_character(&mut self) -> Result<bool> {
        let available = self.input.character_available_byte() != 0;
        self.point(Point::HasChar)?;
        Ok(available)
    }
    fn take_character(&mut self) -> Result<u8> {
        let value = self.input.take_character();
        self.log.push(Log::Take(value));
        Ok(value)
    }
}
impl Central for Synthetic {
    fn increment_update822d38(&mut self) -> Result<()> {
        self.log.push(Log::Increment);
        Ok(())
    }
    fn suppressed8b0700(&mut self) -> Result<u8> {
        Ok(self.suppressed)
    }
    fn signed8b22d3(&mut self) -> Result<i8> {
        Ok(self.signed)
    }
    fn repeat541918(&mut self) -> Result<i32> {
        self.log.push(Log::Repeat(self.repeat));
        Ok(self.repeat)
    }
    fn g_prelude8b22dc(&mut self) -> Result<u8> {
        Ok(self.g_flag)
    }
    fn actual_g4207d0(&mut self) -> Result<()> {
        self.log.push(Log::Prelude);
        self.point(Point::GPrelude)
    }
    fn after_groups4ba5eb(
        &mut self,
        _prefix: PrefixComplete,
    ) -> Result<super::super::continuation::BeforeTail> {
        self.log.push(Log::After);
        self.point(Point::After)?;
        Ok(super::super::continuation::BeforeTail::synthetic())
    }
}
#[test]
fn live_visible_callback_rewrites_current_next_in_same_link_authority() {
    let mut c = Synthetic::new(&[C, B, A]);
    c.script(Point::Visible(A), vec![Action::Promote(P, B)]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.visible(), vec![A, C]);
    assert_eq!(c.groups[0].links.head, Some(B));
    assert_eq!(c.groups[0].links.next(A).unwrap(), Some(C));
    assert!(c.script.is_empty());
    c.groups[0].links.validate().unwrap();
}
#[test]
fn animation_then_promotion_then_direct_visible1_then_fresh_next() {
    let mut c = Synthetic::new(&[C, B, A]);
    {
        let p = c.panels.get_mut(&B).unwrap();
        p.seeking = 1;
        p.target = 1;
    }
    c.script(Point::Generic(w(B), 11), vec![Action::Promote(P, C)]);
    //Set B's promotion request before pass. Its animation callback promotes C,
    //then B consumes its own request and becomes the head before direct1.
    c.panels.get_mut(&B).unwrap().promote34b = 1;
    advance(&mut c, P).unwrap();
    let eleven = c
        .log
        .iter()
        .position(|l| *l == Log::Event(w(B), 11, 0, 0))
        .unwrap();
    let visible = c
        .log
        .iter()
        .position(|l| *l == Log::VisibleHead(B, Some(B), 0))
        .unwrap();
    assert!(eleven < visible);
    assert_eq!(c.visible(), vec![A, B, C, A]);
    assert_eq!(c.panels[&B].promote34b, 0);
    c.groups[0].links.validate().unwrap();
}
#[test]
fn direct_visible1_ignores_generic_block_disabled_moving_and_draw_gates() {
    let mut c = Synthetic::new(&[A]);
    {
        let p = c.panels.get_mut(&A).unwrap();
        p.disabled1d = 1;
        p.block34c = 1;
        p.draw_enabled = 0;
        p.moving = 1;
    }
    advance(&mut c, P).unwrap();
    assert_eq!(c.visible(), vec![A]);
    assert!(!c.events().iter().any(|(_, e, _, _)| *e == 1));
}
#[test]
fn hit_callback_changes_next_and_every_hit_result_is_published() {
    let mut c = Synthetic::new(&[C, B, A]);
    c.script(
        Point::Hit(A),
        vec![Action::Promote(P, B), Action::Select(P, Some(w(C)))],
    );
    c.hits.insert(C, Some(child(C, 7)));
    advance(&mut c, P).unwrap();
    let hits: Vec<_> = c
        .log
        .iter()
        .filter_map(|l| match l {
            Log::Point(Point::Hit(id)) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(hits, vec![A, C]);
    assert_eq!(c.groups[0].selected, Some(child(C, 7)));
}
#[test]
fn last_blocker_and_post_hit_override_win_with_search_panel_dwell_gate() {
    let mut c = Synthetic::new(&[C, B, A]);
    c.panels.get_mut(&A).unwrap().block34d = 1;
    c.panels.get_mut(&C).unwrap().block34d = 1;
    c.panels.get_mut(&C).unwrap().block34c = 1;
    c.groups[0].dwell = Some(w(B));
    c.hits.insert(C, Some(child(C, 1)));
    c.script(Point::Hit(C), vec![Action::Override(Some(child(A, 2)))]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.groups[0].blocks_input, 1);
    assert_eq!(c.groups[0].selected, Some(child(A, 2)));
    assert_eq!(c.groups[0].dwell, Some(w(B)));
    assert!(c.log.contains(&Log::Focus(w(B))));
    assert_eq!(
        c.log
            .iter()
            .filter(|l| matches!(l, Log::Point(Point::Hit(_))))
            .count(),
        1
    );
}
#[test]
fn override_only_uses_two_live_loads_and_child_search_gate() {
    let mut c = Synthetic::new(&[A]);
    let hit = child(A, 4);
    let search = child(A, 5);
    c.override_target = Some(hit);
    c.child_block = 1;
    c.groups[0].dwell = Some(w(A));
    c.script(Point::Override, vec![Action::Override(Some(search))]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.groups[0].selected, Some(hit));
    assert_eq!(c.groups[0].dwell, Some(w(A)));
    assert_eq!(
        c.log
            .iter()
            .filter(|l| **l == Log::Point(Point::Override))
            .count(),
        2
    );
    assert!(!c.log.iter().any(|l| matches!(l, Log::Point(Point::Hit(_)))));
}
#[test]
fn selected_is_reloaded_after_both_leave3_and_enter2() {
    let mut c = Synthetic::new(&[C, B, A]);
    c.retain = 0;
    c.groups[0].selected = Some(w(A));
    c.groups[0].hovered = Some(w(B));
    c.script(Point::Generic(w(B), 3), vec![Action::Select(P, Some(w(C)))]);
    c.script(Point::Generic(w(C), 2), vec![Action::Select(P, Some(w(B)))]);
    c.input = input(1, 1, 0, 0, 0, 0);
    advance(&mut c, P).unwrap();
    assert_eq!(c.groups[0].hovered, Some(w(B)));
    assert_eq!(
        c.events(),
        vec![(w(B), 3, 0, 0), (w(C), 2, 0, 0), (w(B), 5, 0, 0)]
    );
    //Retain branch has null search node; it cannot set dwell from selected.
    assert_eq!(c.groups[0].dwell, None);
}
#[test]
fn selected_child_dwell_uses_search_panel34c_not_child_or_selected_parent() {
    let mut c = Synthetic::new(&[B, A]);
    let hit = child(B, 9);
    c.hits.insert(A, Some(hit));
    c.panels.get_mut(&B).unwrap().block34c = 1;
    c.child_block = 1;
    advance(&mut c, P).unwrap();
    assert_eq!(c.groups[0].dwell, Some(hit));
    assert!(c.log.contains(&Log::Focus(hit)));
}
#[test]
fn falling_right_clears_hover_early_and_again_after_hit_callbacks() {
    let mut c = Synthetic::new(&[A]);
    c.input = input(0, 0, 0, 1, 0, 0);
    c.groups[0].hovered = Some(w(B));
    c.hits.insert(A, Some(w(A)));
    c.script(Point::Hit(A), vec![Action::Hover(P, Some(w(C)))]);
    advance(&mut c, P).unwrap();
    assert!(c.log.contains(&Log::Cursor));
    assert!(!c.events().iter().any(|(_, e, _, _)| *e == 3));
    assert!(c.events().contains(&(w(A), 2, 0, 0)));
    assert_eq!(c.groups[0].hovered, Some(w(A)));
}
#[test]
fn old_packet_edge_held_release_wheel_and_right_suppression() {
    for (left, prior, right, wheel, oldwheel, event) in [
        (1, 0, 0, 0, 0, Some(4)),
        (1, 1, 0, 0, 0, Some(5)),
        (0, 1, 0, 0, 0, Some(6)),
        (0, 0, 0, 2, 0, Some(8)),
        (0, 0, 0, -2, 0, Some(9)),
        (0, 0, 0, 2, 1, None),
        (1, 1, 1, 0, 0, None),
    ] {
        let mut c = Synthetic::new(&[A]);
        c.input = input(left, prior, right, 0, wheel, oldwheel);
        c.hits.insert(A, Some(w(A)));
        advance(&mut c, P).unwrap();
        let actual: Vec<_> = c
            .events()
            .into_iter()
            .filter(|(_, e, _, _)| [4, 5, 6, 8, 9, 10].contains(e))
            .collect();
        assert_eq!(
            actual,
            event.map(|e| vec![(w(A), e, 0, 0)]).unwrap_or_default()
        );
        assert_eq!(c.input.cells().mouse_current.left(), left); //No hidden poll.
    }
}
#[test]
fn selected_char_double_take_uses_single_public_queue_and_press_also_takes() {
    let mut c = Synthetic::new(&[A]);
    c.hits.insert(A, Some(w(A)));
    c.queue(2);
    advance(&mut c, P).unwrap();
    assert!(c.events().contains(&(w(A), 10, 0, 65)));
    assert_eq!(c.input.characters().controls().count, 0);
    assert_eq!(c.log.iter().filter(|l| **l == Log::Take(65)).count(), 2);
    let mut c = Synthetic::new(&[A]);
    c.hits.insert(A, Some(w(A)));
    c.input = input(1, 0, 0, 0, 0, 0);
    c.queue(1);
    advance(&mut c, P).unwrap();
    assert!(c.events().contains(&(w(A), 4, 0, 0)));
    assert_eq!(c.input.characters().controls().count, 0);
}
#[test]
fn post_event_key_reload_can_preserve_second_char_and_key_delivery_is_separate() {
    let mut c = Synthetic::new(&[A]);
    c.hits.insert(A, Some(w(A)));
    c.queue(2);
    c.script(
        Point::Generic(w(A), 10),
        vec![Action::Key(P, Some(child(A, 8)))],
    );
    advance(&mut c, P).unwrap();
    assert_eq!(c.input.characters().controls().count, 1);
    assert!(c.events().contains(&(child(A, 8), 10, 0, 0)));
    assert_eq!(
        c.log.iter().filter(|l| matches!(l, Log::Take(_))).count(),
        1
    );
}
#[test]
fn keytarget_reloads_after_haschar_and_right_does_not_skip_keyboard_tail() {
    let mut c = Synthetic::new(&[A]);
    c.input = input(0, 0, 1, 0, 0, 0);
    c.queue(1);
    c.groups[0].key_target = Some(w(A));
    c.script(Point::HasChar, vec![Action::Key(P, Some(child(A, 6)))]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.events(), vec![(child(A, 6), 10, 0, 0)]);
    assert_eq!(c.input.characters().controls().count, 1);
}
#[test]
fn null_selected_after_enter_does_not_repeat_callbacks_or_dereference_old_target() {
    let mut c = Synthetic::new(&[A]);
    c.hits.insert(A, Some(w(A)));
    c.input = input(1, 1, 0, 0, 0, 0);
    c.script(Point::Generic(w(A), 2), vec![Action::Select(P, None)]);
    let e = advance(&mut c, P).unwrap_err();
    assert!(e.contains("selected slot14 dereference became null"));
    assert_eq!(c.groups[0].hovered, None);
    assert_eq!(c.events(), vec![(w(A), 2, 0, 0)]);
}
#[test]
fn no_selected_dwell3_reloads_dwell_and_current_focus_before_clear() {
    let mut c = Synthetic::new(&[A]);
    c.groups[0].dwell = Some(w(A));
    c.script(
        Point::Generic(w(A), 3),
        vec![Action::Dwell(P, Some(w(B))), Action::Focus(Some(w(B)))],
    );
    advance(&mut c, P).unwrap();
    assert!(c.log.contains(&Log::ClearFocus));
    assert_eq!(c.groups[0].dwell, None);
}
#[test]
fn key_replacement_runs_old_reset_once_before_publish_and_failure_retains_old() {
    let mut c = Synthetic::new(&[A]);
    c.groups[0].key_target = Some(w(A));
    c.script(Point::ResetKey(w(A)), vec![Action::Key(P, Some(w(C)))]);
    replace_key_target(&mut c, P, Some(w(B))).unwrap();
    assert_eq!(c.groups[0].key_target, Some(w(B)));
    c.script(Point::ResetKey(w(B)), vec![Action::Fail]);
    assert!(replace_key_target(&mut c, P, None).is_err());
    assert_eq!(c.groups[0].key_target, Some(w(B)));
}
#[test]
fn disabled_group_is_no_work_and_enabled_othergroups_are_read_when_reached() {
    let mut c = Synthetic::new(&[A]);
    advance(&mut c, S).unwrap();
    assert!(c.log.is_empty());
    c.script(Point::Visible(A), vec![Action::Enable(S, 1)]);
    c.script(
        Point::Visible(PanelId::Frontend(0)),
        vec![Action::Enable(T, 1)],
    );
    central_source_boundary(&mut c).unwrap();
    assert_eq!(
        c.visible(),
        vec![A, PanelId::Frontend(0), PanelId::Auction(0)]
    );
    assert!(matches!(c.log.last(), Some(Log::Point(Point::After))));
}
#[test]
fn live_primary_repeat_increases_then_decreases_and_gflag_is_reread_each_pass() {
    let mut c = Synthetic::new(&[A]);
    c.repeat = 1;
    c.script(Point::Visible(A), vec![Action::Repeat(4), Action::GFlag(1)]);
    c.script(Point::GPrelude, vec![Action::Repeat(2), Action::GFlag(0)]);
    central_source_boundary(&mut c).unwrap();
    assert_eq!(c.visible(), vec![A, A]);
    let reads: Vec<_> = c
        .log
        .iter()
        .filter_map(|l| match l {
            Log::Repeat(n) => Some(*n),
            _ => None,
        })
        .collect();
    assert_eq!(reads, vec![1, 4, 2]);
    assert_eq!(c.log.iter().filter(|l| **l == Log::Prelude).count(), 1);
    assert_eq!(c.log.iter().filter(|l| **l == Log::Increment).count(), 1);
}
#[test]
fn zero_primary_repeat_still_runs_secondary_and_tertiary_once_and_suppression_skips_all() {
    let mut c = Synthetic::new(&[A]);
    c.repeat = 0;
    c.groups[1].enabled = 1;
    c.groups[2].enabled = 1;
    central_source_boundary(&mut c).unwrap();
    assert_eq!(c.visible(), vec![PanelId::Frontend(0), PanelId::Auction(0)]);
    let mut c = Synthetic::new(&[A]);
    c.suppressed = 1;
    central_source_boundary(&mut c).unwrap();
    assert!(c.visible().is_empty());
    assert!(c.log.contains(&Log::After));
}
#[test]
fn error_preserves_prior_callback_writes_and_never_preflights_twice() {
    let mut c = Synthetic::new(&[B, A]);
    c.script(
        Point::Visible(A),
        vec![Action::Select(P, Some(w(B))), Action::Fail],
    );
    let e = advance(&mut c, P).unwrap_err();
    assert!(e.contains("prior effects retained"));
    assert_eq!(c.groups[0].selected, Some(w(B)));
    assert_eq!(c.visible(), vec![A]);
    assert!(c.script.is_empty());
}
#[test]
fn typed_draw_direction_and_source_link_validation_are_not_registration_clones() {
    let mut links = Links::synthetic_draw_order(&[C, B, A]);
    links.promote(B).unwrap();
    let mut draw = Vec::new();
    let mut at = links.tail;
    while let Some(id) = at {
        draw.push(id);
        at = links.previous(id).unwrap();
    }
    assert_eq!(draw, vec![C, A, B]);
    let mut broken = BTreeMap::new();
    broken.insert(
        A,
        Link {
            next: Some(B),
            previous: None,
        },
    );
    assert!(Links::from_source(Some(A), Some(A), broken).is_err());
}

#[test]
fn disabled_startup_broadcast_uses_live_next_without_an_enabled_gate() {
    let mut c = Synthetic::new(&[C, B, A]);
    c.groups[0].enabled = 0;
    c.script(Point::Generic(w(A), 11), vec![Action::Promote(P, B)]);
    broadcast11(&mut c, P).unwrap();
    assert_eq!(c.events(), vec![(w(A), 11, 0, 0), (w(C), 11, 0, 0)]);
}
#[test]
fn null_second_override_load_is_not_replaced_with_first_selected_identity() {
    let mut c = Synthetic::new(&[A]);
    c.override_target = Some(w(A));
    c.groups[0].dwell = Some(w(B));
    c.script(Point::Override, vec![Action::Override(None)]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.groups[0].selected, Some(w(A)));
    assert_eq!(c.groups[0].dwell, Some(w(B)));
    assert!(c.log.contains(&Log::Focus(w(B))));
}
#[test]
fn hit_callback_cannot_freeze_an_old_selected_event_target() {
    let mut c = Synthetic::new(&[B, A]);
    c.input = input(1, 0, 0, 0, 0, 0);
    c.hits.insert(A, Some(w(A)));
    c.script(Point::Generic(w(A), 2), vec![Action::Select(P, Some(w(B)))]);
    advance(&mut c, P).unwrap();
    assert!(c.events().contains(&(w(B), 4, 0, 0)));
    assert!(!c.events().contains(&(w(A), 4, 0, 0)));
}
#[test]
fn group_gate_is_entry_only_not_a_new_midpass_abort() {
    let mut c = Synthetic::new(&[B, A]);
    c.script(Point::Visible(A), vec![Action::Enable(P, 0)]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.visible(), vec![A, B]);
    assert_eq!(c.groups[0].enabled, 0);
    let count = c.log.len();
    advance(&mut c, P).unwrap();
    assert_eq!(c.log.len(), count);
}
#[test]
fn signed_gate_and_live_negative_repeat_preserve_native_central_branching() {
    let mut c = Synthetic::new(&[A]);
    c.signed = 1;
    central_source_boundary(&mut c).unwrap();
    assert!(c.visible().is_empty());
    assert!(c.log.contains(&Log::Increment));
    assert!(c.log.contains(&Log::After));
    let mut c = Synthetic::new(&[A]);
    c.repeat = 4;
    c.script(Point::Visible(A), vec![Action::Repeat(-1)]);
    central_source_boundary(&mut c).unwrap();
    assert_eq!(c.visible(), vec![A]);
    assert!(c.log.contains(&Log::Repeat(-1)));
}
impl Presentation for Synthetic {
    fn draw08(&mut self, id: PanelId) -> Result<()> {
        self.point(Point::Draw(id))
    }
    fn finish_cursor9c(&mut self) -> Result<()> {
        self.point(Point::DrawCursor)
    }
}
#[test]
fn reverse_draw_rereads_previous_after_callback_and_finishes_cursor_once() {
    let mut c = Synthetic::new(&[C, B, A]);
    //Tail C first; promote its previous B to the head. C.previous becomes A.
    c.script(Point::Draw(C), vec![Action::Promote(P, B)]);
    draw_group(&mut c, P).unwrap();
    let draws: Vec<_> = c
        .log
        .iter()
        .filter_map(|l| match l {
            Log::Point(Point::Draw(id)) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(draws, vec![C, A, B]);
    assert_eq!(
        c.log
            .iter()
            .filter(|l| **l == Log::Point(Point::DrawCursor))
            .count(),
        1
    );
}

struct SyntheticDevice;
impl openmt2001::native_input::PortableDeviceAdapter for SyntheticDevice {
    fn mouse_state(
        &mut self,
        dst: &mut [u8; 16],
    ) -> std::result::Result<i32, openmt2001::native_input::BoundaryError> {
        *dst = [0; 16];
        Ok(0)
    }
    fn keyboard_state(
        &mut self,
        dst: &mut [u8; 256],
    ) -> std::result::Result<i32, openmt2001::native_input::BoundaryError> {
        *dst = [0; 256];
        Ok(0)
    }
    fn keyboard_events(
        &mut self,
        _requested: u32,
    ) -> std::result::Result<
        openmt2001::native_input::BufferedKeyboard,
        openmt2001::native_input::BoundaryError,
    > {
        Err(openmt2001::native_input::BoundaryError::AdapterUnavailable(
            "synthetic test never admits buffered branch",
        ))
    }
}
#[test]
fn all_primary_passes_use_old_packet_before_required_continuation_poll0_then_poll1() {
    let mut c = Synthetic::new(&[A]);
    c.input = input(1, 0, 0, 0, 0, 0);
    c.hits.insert(A, Some(w(A)));
    c.repeat = 2;
    c.script(Point::After, vec![Action::Polls(0)]);
    central_source_boundary(&mut c).unwrap();
    assert_eq!(c.events().iter().filter(|(_, e, _, _)| *e == 4).count(), 2);
    let after = c.log.iter().position(|l| *l == Log::After).unwrap();
    let poll0 = c.log.iter().position(|l| *l == Log::Poll0).unwrap();
    let poll1 = c.log.iter().position(|l| *l == Log::Poll1).unwrap();
    assert!(after < poll0 && poll0 < poll1);
    assert_eq!(c.input.cells().mouse_prior.left(), 1);
    assert_eq!(c.input.cells().mouse_current.left(), 0);
}
#[test]
fn haschar_can_change_selected_and_dispatch_reloads_after_first_take() {
    let mut c = Synthetic::new(&[A]);
    c.hits.insert(A, Some(w(A)));
    c.queue(2);
    c.script(Point::HasChar, vec![Action::Select(P, Some(child(A, 3)))]);
    advance(&mut c, P).unwrap();
    assert!(c.events().contains(&(child(A, 3), 10, 0, 65)));
    assert_eq!(c.input.characters().controls().count, 0);
}
#[test]
fn input_packet_is_read_again_after_hit_callback_before_right_clear() {
    let mut c = Synthetic::new(&[A]);
    c.hits.insert(A, Some(w(A)));
    c.script(Point::Hit(A), vec![Action::Polls(2)]);
    advance(&mut c, P).unwrap();
    assert_eq!(c.groups[0].selected, None);
    assert_eq!(c.input.cells().mouse_current.right(), 2);
    assert!(
        !c.events()
            .iter()
            .any(|(_, e, _, _)| [2, 4, 5, 6].contains(e))
    );
}
