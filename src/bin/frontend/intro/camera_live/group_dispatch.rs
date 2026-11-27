//! Source-only43c8e0 core. Every borrow ends before a callback; no preflight,
//! cloned group, deferred commit or callback replay. Errors retain prior effects.
use super::{
    navigation::Group,
    ui_graph::{Panel, PanelId, PointerGroup, WidgetId, WidgetKind},
};
use openmt2001::native_input::StagedInput;
use std::collections::{BTreeMap, BTreeSet};
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub next: Option<PanelId>,
    pub previous: Option<PanelId>,
}
/// THE typed link authority after source-cell transfer. No retained draw vector.
/// Read tail->previous for43cb40; head->next for43c8e0/43c810/43c7c0.
#[derive(Clone, Debug)]
pub struct Links {
    pub head: Option<PanelId>,
    pub tail: Option<PanelId>,
    nodes: BTreeMap<PanelId, Link>,
}
impl Links {
    ///Actual+8/+c/+380/+384 producer supplies every operand. Validation is
    ///structural and read-only, before any arbitrary callback is admitted.
    pub fn from_source(
        head: Option<PanelId>,
        tail: Option<PanelId>,
        nodes: BTreeMap<PanelId, Link>,
    ) -> Result<Self> {
        let out = Self { head, tail, nodes };
        out.validate()?;
        Ok(out)
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn identities(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.nodes.keys().copied()
    }
    pub fn next(&self, id: PanelId) -> Result<Option<PanelId>> {
        Ok(self.link(id)?.next)
    }
    pub fn previous(&self, id: PanelId) -> Result<Option<PanelId>> {
        Ok(self.link(id)?.previous)
    }
    fn link(&self, id: PanelId) -> Result<&Link> {
        self.nodes
            .get(&id)
            .ok_or_else(|| format!("live link owner lacks {id:?}"))
    }
    fn link_mut(&mut self, id: PanelId) -> Result<&mut Link> {
        self.nodes
            .get_mut(&id)
            .ok_or_else(|| format!("live link owner lacks {id:?}"))
    }
    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        let mut current = self.head;
        let mut prior = None;
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err("source group link cycle".into());
            }
            let link = self.link(id)?;
            if link.previous != prior {
                return Err("source group next/previous disagree".into());
            }
            prior = Some(id);
            current = link.next;
        }
        if prior != self.tail || seen.len() != self.nodes.len() {
            return Err("source group head/tail/census disagree".into());
        }
        Ok(())
    }
    ///43c600 on an already registered node. No callbacks intervene between
    ///these stores. Missing identity rejects instead of allocating a fake node.
    pub fn promote(&mut self, id: PanelId) -> Result<()> {
        self.link(id)?;
        let Some(first) = self.head else {
            return Err("registered promotion has no head".into());
        };
        if let Some(previous) = self.link(id)?.previous {
            if let Some(next) = self.link(id)?.next {
                self.link_mut(previous)?.next = Some(next);
                let next = self.link(id)?.next.ok_or("promotion lost next")?;
                let previous = self.link(id)?.previous;
                self.link_mut(next)?.previous = previous;
            } else {
                self.link_mut(previous)?.next = None;
                self.tail = self.link(id)?.previous;
            }
        }
        if first != id {
            self.link_mut(first)?.previous = Some(id);
            let first = self.head; //Native reload after the preceding store.
            self.link_mut(id)?.next = first;
            self.head = Some(id);
            self.link_mut(id)?.previous = None;
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn synthetic_draw_order(draw: &[PanelId]) -> Self {
        let mut nodes = BTreeMap::new();
        for (i, &id) in draw.iter().enumerate() {
            nodes.insert(
                id,
                Link {
                    next: i.checked_sub(1).map(|j| draw[j]),
                    previous: draw.get(i + 1).copied(),
                },
            );
        }
        Self::from_source(draw.last().copied(), draw.first().copied(), nodes).unwrap()
    }
}
pub fn panel(id: PanelId) -> WidgetId {
    WidgetId {
        panel: id,
        kind: WidgetKind::Panel,
    }
}

///Required provider, with no successful default methods. Implementations must
///read the SAME retained graph, shared override, focus and public StagedInput.
///Callbacks can mutate all of them synchronously. Returned errors are partial
///boundaries, not a rollback promise. Synthetic implementations are test-only.
pub trait Dispatch {
    fn group(&self, g: Group) -> Result<&PointerGroup>;
    fn group_mut(&mut self, g: Group) -> Result<&mut PointerGroup>;
    fn panel(&self, id: PanelId) -> Result<&Panel>;
    fn panel_mut(&mut self, id: PanelId) -> Result<&mut Panel>;
    fn input(&self) -> Result<&StagedInput>;
    fn override_target(&mut self) -> Result<Option<WidgetId>>;
    fn block34c(&mut self, id: WidgetId) -> Result<u8>;
    fn node1c(&mut self, id: WidgetId) -> Result<u8>;
    fn slot20(&mut self, id: WidgetId) -> Result<u8>;
    fn slot24(&mut self, id: PanelId) -> Result<Option<WidgetId>>;
    fn generic14(&mut self, id: WidgetId, event: u32, a: i32, b: i32) -> Result<()>;
    fn direct18_visible1(&mut self, id: PanelId) -> Result<()>;
    fn cursor0_priority1(&mut self) -> Result<()>;
    fn focus_child(&self) -> Result<Option<WidgetId>>;
    fn show_focus43e310(&mut self, id: WidgetId) -> Result<()>;
    fn clear_focus43e390(&mut self) -> Result<()>;
    fn reset_old_key4439d0(&mut self, id: WidgetId) -> Result<()>;
    fn has_character(&mut self) -> Result<bool>;
    fn take_character(&mut self) -> Result<u8>;
}

///The exact retained PE43c8e0 sequence, including its unusual CHAR double take.
///On an unsupplied callback, effects before that boundary stay applied. Root
///must not retry the pass as though it were unstarted or certified atomic.
pub fn advance<C: Dispatch>(c: &mut C, g: Group) -> Result<()> {
    advance_inner(c, g)
        .map_err(|e| format!("43c8e0 {g:?} interrupted; prior effects retained: {e}"))
}
fn advance_inner<C: Dispatch>(c: &mut C, g: Group) -> Result<()> {
    if c.group(g)?.enabled == 0 {
        return Ok(());
    }
    if c.input()?.right_fell() {
        c.group_mut(g)?.hovered = None;
    }
    let mut node = c.group(g)?.links.head;
    let mut blocking = None;
    c.group_mut(g)?.blocks_input = 0;
    while let Some(id) = node {
        if c.panel(id)?.block34d != 0 {
            c.group_mut(g)?.blocks_input = 1;
            blocking = Some(id);
        }
        if c.panel_mut(id)?.advance() {
            c.generic14(panel(id), 11, 0, 0)?;
        }
        if c.panel(id)?.promote34b != 0 {
            c.panel_mut(id)?.promote34b = 0;
            c.group_mut(g)?.links.promote(id)?;
        }
        //DIRECT specialized18: no generic visibility/moving/disabled gates.
        if c.panel(id)?.requested != 0 {
            c.direct18_visible1(id)?;
        }
        node = c.group(g)?.links.next(id)?;
    }
    //ESI/node is null after the animation walk. If slot20 retains selection,
    //that null is preserved for the later search-node34c dwell gate.
    let recompute = match c.group(g)?.selected {
        None => true,
        Some(id) => c.slot20(id)? != 0,
    };
    let mut search = None;
    if recompute {
        c.group_mut(g)?.selected = None;
        if let Some(id) = blocking {
            search = Some(panel(id));
            let hit = c.slot24(id)?;
            c.group_mut(g)?.selected = hit;
            if let Some(replacement) = c.override_target()? {
                c.group_mut(g)?.selected = Some(replacement);
            }
        } else if let Some(replacement) = c.override_target()? {
            c.group_mut(g)?.selected = Some(replacement);
            search = c.override_target()?; //Native SECOND live590504 load.
        } else {
            let mut at = c.group(g)?.links.head;
            while let Some(id) = at {
                search = Some(panel(id));
                let hit = c.slot24(id)?;
                c.group_mut(g)?.selected = hit;
                if hit.is_some() {
                    break;
                }
                at = c.group(g)?.links.next(id)?;
                //ESI becomes null when the final hit misses.
                if at.is_none() {
                    search = None;
                }
            }
        }
    }
    if c.input()?.cells().mouse_current.right() != 0 {
        c.group_mut(g)?.selected = None;
    }
    if c.input()?.right_fell() {
        c.cursor0_priority1()?;
        c.group_mut(g)?.hovered = None;
    }
    if c.group(g)?.selected.is_some() {
        if c.group(g)?.selected != c.group(g)?.hovered {
            if let Some(prior) = c.group(g)?.hovered {
                c.generic14(prior, 3, 0, 0)?;
            }
            if let Some(fresh) = c.group(g)?.selected {
                c.generic14(fresh, 2, 0, 0)?;
            }
            //Native permits callbacks2/3 to clear selected. The branch stays
            //entered; null is legal until the actual later slot14 dereference.
            let fresh = c.group(g)?.selected;
            c.group_mut(g)?.hovered = fresh;
        }
        let selected = c.group(g)?.selected;
        //EAX is the entry selected if no enter/leave ran, otherwise its reload.
        //No callback intervenes before this current selected read.
        if selected != c.group(g)?.dwell {
            if let Some(search) = search {
                if c.block34c(search)? == 0 {
                    c.group_mut(g)?.dwell = selected;
                }
            }
            if let Some(target) = c.group(g)?.dwell {
                c.show_focus43e310(target)?;
            }
        }
        //Use the OLD current/prior packet owned by public StagedInput. Every
        //CHAR take below goes to that same owner, including the second take.
        let current = c.input()?.cells().mouse_current;
        if current.right() == 0 {
            let previous = c.input()?.cells().mouse_prior.left();
            let mut value = 0;
            let event = if current.left() != 0 {
                if previous != 0 { 5 } else { 4 }
            } else if previous != 0 {
                6
            } else {
                let wheel = c.input()?.cells().mouse_current.wheel();
                let prior = c.input()?.cells().mouse_prior.wheel();
                if wheel > 0 && prior == 0 {
                    8
                } else if wheel < 0 && prior == 0 {
                    9
                } else if c.group(g)?.key_target.is_none() && c.has_character()? {
                    value = i32::from(c.take_character()?);
                    10
                } else {
                    0
                }
            };
            if event != 0 {
                let active = c
                    .group(g)?
                    .selected
                    .ok_or("native selected slot14 dereference became null after callbacks")?;
                c.generic14(active, event, 0, value)?;
                if c.group(g)?.key_target.is_none() {
                    c.take_character()?;
                }
            }
        }
    } else {
        reset_dwell(c, g)?;
    }
    if let Some(key) = c.group(g)?.key_target {
        if c.node1c(key)? == 1 && c.has_character()? {
            let fresh = c
                .group(g)?
                .key_target
                .ok_or("native key-target slot14 dereference became null after has-CHAR")?;
            c.generic14(fresh, 10, 0, 0)?;
        }
    }
    Ok(())
}
///43c840 and the no-selected43cae8 branch share the same live dwell reload.
pub fn reset_dwell<C: Dispatch>(c: &mut C, g: Group) -> Result<()> {
    if let Some(target) = c.group(g)?.dwell {
        c.generic14(target, 3, 0, 0)?;
        let fresh = c.group(g)?.dwell;
        if fresh == c.focus_child()? {
            c.clear_focus43e390()?;
        }
        c.group_mut(g)?.dwell = None;
    }
    Ok(())
}
///43c880: old4439d0 returns BEFORE the supplied target is published. There is
///no invented warm key clear and no bypass for replacement with the same id.
pub fn replace_key_target<C: Dispatch>(c: &mut C, g: Group, new: Option<WidgetId>) -> Result<()> {
    if let Some(old) = c.group(g)?.key_target {
        c.reset_old_key4439d0(old)?;
    }
    c.group_mut(g)?.key_target = new;
    Ok(())
}
///43c810 startup/activity broadcast ignores enabled, with live next reload.
pub fn broadcast11<C: Dispatch>(c: &mut C, g: Group) -> Result<()> {
    let mut at = c.group(g)?.links.head;
    while let Some(id) = at {
        c.generic14(panel(id), 11, 0, 0)?;
        at = c.group(g)?.links.next(id)?;
    }
    Ok(())
}

///Linear source-phase token. Production construction occurs only after this
///module completes the group prefix; not a central/tail/save acknowledgment.
#[derive(Debug)]
#[must_use = "Pass this SAME operation to its continuation; prefix alone is incomplete"]
pub struct PrefixComplete {
    _private: (),
}
impl PrefixComplete {
    pub fn phase(&self) -> super::continuation::Phase {
        super::continuation::Phase::PrefixComplete
    }
    #[cfg(test)]
    pub(super) fn synthetic() -> Self {
        Self { _private: () }
    }
}

///Actual central source cells/provider, never a captured repeat parameter.
///Same context lets a callback change repeat, G-prelude flag and other gates.
///The continuation is REQUIRED: prefix completion alone cannot acknowledge a
///whole central callback or move groups after device polls/camera-tail.
pub trait Central: Dispatch {
    fn increment_update822d38(&mut self) -> Result<()>;
    fn suppressed8b0700(&mut self) -> Result<u8>;
    fn signed8b22d3(&mut self) -> Result<i8>;
    fn repeat541918(&mut self) -> Result<i32>;
    fn g_prelude8b22dc(&mut self) -> Result<u8>;
    fn actual_g4207d0(&mut self) -> Result<()>;
    ///Starts exactly at4ba5eb; must own live dispatch-block/input gates,
    ///busy48dbf0, ordered poll0/poll1 on SAME StagedInput, then controller/key/
    ///selection work. Does NOT include focus/camera tail or acknowledge it.
    fn after_groups4ba5eb(
        &mut self,
        prefix: PrefixComplete,
    ) -> Result<super::continuation::BeforeTail>;
}
///Exposed for a root-owned split acknowledgment. No polls/tail are implied.
pub fn central_prefix<C: Central>(c: &mut C) -> Result<()> {
    c.increment_update822d38()?;
    if c.suppressed8b0700()? == 0 && c.signed8b22d3()? <= 0 {
        if c.repeat541918()? > 0 {
            let mut completed = 0i32;
            loop {
                if c.g_prelude8b22dc()? != 0 {
                    c.actual_g4207d0()?;
                }
                advance(c, Group::Primary)?;
                let repeat = c.repeat541918()?; //4ba5cd, AFTER EACH pass.
                completed = completed.wrapping_add(1);
                if completed >= repeat {
                    break;
                }
            }
        }
        advance(c, Group::Secondary)?;
        advance(c, Group::Tertiary)?;
    }
    Ok(())
}
/// Typed before-tail receipt: cannot serve as a completed central/save ack.
pub fn central_source_boundary<C: Central>(c: &mut C) -> Result<super::continuation::BeforeTail> {
    central_prefix(c)?;
    c.after_groups4ba5eb(PrefixComplete { _private: () })
        .map_err(|e| {
            format!(
                "central4ba5eb interrupted after live group prefix; prior effects retained: {e}"
            )
        })
}
#[cfg(test)]
#[path = "group_dispatch_tests.rs"]
mod tests;

///43cb40 reverse draw traversal. A renderer must consume these SAME links,
///rereading previous only AFTER each concrete slot08 returns. Cursor finish
///is a required actual image-receiver call, never a successful default noop.
pub trait Presentation: Dispatch {
    fn draw08(&mut self, id: PanelId) -> Result<()>;
    fn finish_cursor9c(&mut self) -> Result<()>;
}
pub fn draw_group<C: Presentation>(c: &mut C, g: Group) -> Result<()> {
    if c.group(g)?.enabled == 0 {
        return Ok(());
    }
    let mut at = c.group(g)?.links.tail;
    while let Some(id) = at {
        c.draw08(id)?;
        at = c.group(g)?.links.previous(id)?;
    }
    c.finish_cursor9c()
}
