//! Concrete439ce0/43ce0a EventList storage in the SAME process allocation.
use super::super::primary_d_construction::PrimaryPhase;
use super::*;
const E: u32 = 0x571378;
const V: u32 = 0x573b38;
const D: u32 = 0x574488;
const G: u32 = 0x57e710;
const ID: PanelId = PanelId::OtherPrimary(2);
impl Builder {
    pub(super) fn eventlist_static(&mut self) -> Result<(), String> {
        self.admit_span(E, 0x490)?;
        self.panel(E)?;
        self.spans.insert(E, 0x490);
        let born = |c: &mut Cells, offset: u32| -> Result<(), String> {
            for o in [12, 8, 4, 0] {
                c.set_word(E + offset + o, 0)?;
            }
            c.set_word(E + offset + 16, 0.5f32.to_bits())
        };
        born(&mut self.cells, 0x388)?;
        for base in [0x39c, 0x400] {
            for i in 0..5 {
                born(&mut self.cells, base + 20 * i)?;
            }
        }
        //Alias words464..48c remain their actual loader preimages.
        self.cells.set_word(E, 0x5256e0)
    }
}
impl Primitives {
    pub(in crate::intro::camera_live) fn check_eventlist_ready(&self) -> Result<(), String> {
        if self.provenance != Provenance::DeferredClaimed
            || self.primary_phase != Some(PrimaryPhase::AwaitingEventList)
            || self.primary_pool.as_ref().and_then(|p| p.get(14)) != Some(&ID)
            || self.cells().word(G + 0x5c)? != E
            || self.cells().word(G + 0x24)? != 17
            || self.cells().word(G + 8)? != V
            || self.cells().word(G + 12)? != D
            || self.cells().word(V + 0x380)? != D
            || self.cells().word(V + 0x384)? != 0
            || self.cells().word(D + 0x384)? != V
            || self.cells().byte(V + 0x184)? != 1
        {
            return Err("EventList requires SAME actually returned/registered Voting before descriptor14; no replay".into());
        }
        if self.panels.get(&V).is_none_or(|v| {
            v.phase != Phase::Bound
                || v.descriptor != Some(DescriptorIdentity::Archive(PanelId::OtherPrimary(1)))
        }) {
            return Err("EventList prior Voting bind owner differs".into());
        }
        let panel = self.panels.get(&E).ok_or("EventList static Panel absent")?;
        if panel.phase != Phase::Constructed
            || panel.descriptor.is_some()
            || self.cells().byte(E + 0x184)? != 0
            || self.cells().word(E)? != 0x5256e0
            || self.cells().word(0x5256e4)? != 0x41cdc0
        {
            return Err("EventList static owner/actual41cdc0 dispatch differs".into());
        }
        if self.resources.resolve(ID)? != Some(ID) {
            return Err("EventList descriptor14 has missing/null/different model".into());
        }
        self.texts
            .get(&(E + 0x188))
            .ok_or("EventList SAME embedded caption absent")?;
        for (at, target) in [(0x52590c, 0x42f140), (0x525934, 0x43b370)] {
            if self.cells().word(at)? != target {
                return Err("EventList concrete Text dispatch differs".into());
            }
        }
        Ok(())
    }
    pub(in crate::intro::camera_live) fn eventlist_bind(&mut self) -> Result<(), String> {
        self.check_eventlist_ready()?;
        self.primary_phase = Some(PrimaryPhase::LayingOutEventList);
        let mut b = Builder {
            cells: self.cells.take().ok_or("process cells borrowed")?,
            panels: std::mem::take(&mut self.panels),
            caption_keys: std::mem::take(&mut self.caption_keys),
            spans: std::mem::take(&mut self.spans),
            resources: std::mem::replace(&mut self.resources, CpuResources::new()),
            steps: std::mem::take(&mut self.steps),
        };
        let result = b.bind_prefix(E, Some(ID), None, Group::Primary);
        self.cells = Some(b.cells);
        self.panels = b.panels;
        self.caption_keys = b.caption_keys;
        self.spans = b.spans;
        self.resources = b.resources;
        self.steps = b.steps;
        //SAME caption remains in Primitives, including after partial binding.
        let style = self.cells().word(E + 0x188 + 0x1a0)?;
        let dirty = self.cells().byte(E + 0x188 + 0x1a8)?;
        let caption = self.text_mut(E + 0x188)?;
        caption.style = style;
        caption.dirty = dirty;
        result
    }
    pub(in crate::intro::camera_live) fn eventlist_cells(&mut self) -> Result<&mut Cells, String> {
        if self.primary_phase != Some(PrimaryPhase::LayingOutEventList) {
            return Err("EventList storage outside reached body".into());
        }
        self.cells
            .as_mut()
            .ok_or_else(|| "process cells borrowed".into())
    }
    pub(in crate::intro::camera_live) fn eventlist_named(
        &mut self,
        name: u32,
    ) -> Result<[f32; 5], String> {
        if self.primary_phase != Some(PrimaryPhase::LayingOutEventList) {
            return Err("EventList query outside reached body".into());
        }
        let panel = self.panels.get(&E).ok_or("EventList actual Panel absent")?;
        if panel.phase != Phase::AwaitingLayout
            || panel.descriptor != Some(DescriptorIdentity::Archive(ID))
        {
            return Err("EventList query lacks SAME bound descriptor".into());
        }
        let frame = self
            .cells()
            .word(E + 0x180)?
            .checked_sub(1)
            .ok_or("EventList live frame underflow")? as usize;
        let name = self.cells().ntbs(name, 256)?;
        let key = self
            .resources
            .resolve(ID)?
            .ok_or("EventList actual model reference is null")?;
        let model = self
            .resources
            .models
            .get_mut(&key)
            .ok_or("EventList model absent")?;
        if frame >= model.frames.len() {
            return Err("EventList live frame leaves SAME model".into());
        }
        let part = model.find(Some(&name))?;
        let mut e = [0., 0., 0., 0., 0.5];
        if let Some(r) = model.region(part, frame)? {
            e[..4].copy_from_slice(&r)
        }
        Ok(e) //43af30's defined missing-name fallback; never a missing model
    }
    pub(in crate::intro::camera_live) fn eventlist_bind_return_and_register(
        &mut self,
        product: &super::super::eventlist_construction::EventListProduct,
    ) -> Result<(), String> {
        if self.primary_phase != Some(PrimaryPhase::LayingOutEventList)
            || product.phase() != super::super::eventlist_construction::Phase::Returned
            || !product.same_process(self)
        {
            return Err("EventList suffix outside SAME actual41cfa1 return".into());
        }
        let panel = self
            .panels
            .get_mut(&E)
            .ok_or("EventList actual Panel absent")?;
        if panel.phase != Phase::AwaitingLayout
            || panel.descriptor != Some(DescriptorIdentity::Archive(ID))
        {
            return Err("EventList suffix lacks actual retained bind".into());
        }
        let c = self.cells.as_mut().ok_or("process cells borrowed")?;
        self.steps.push(Step::LayoutReturned(E));
        c.set_byte(E + 0x184, 1)?;
        panel.phase = Phase::Bound;
        self.steps.push(Step::Bound(E));
        registration::promote(c, Group::Primary, E)?;
        self.steps.push(Step::Registered(E));
        self.primary_phase = Some(PrimaryPhase::AwaitingChat);
        Ok(())
    }
}
#[cfg(test)]
impl Primitives {
    ///Negative reached-identifier injection only, never production language.
    pub(in crate::intro::camera_live) fn test_eventlist_first_identifier(
        &mut self,
        id: u32,
    ) -> Result<(), String> {
        if self.provenance != Provenance::DeferredClaimed
            || self.primary_phase != Some(PrimaryPhase::Allocated)
        {
            return Err("EventList identifier canary outside untouched allocation".into());
        }
        self.cells
            .as_mut()
            .ok_or("process cells borrowed")?
            .set_word(0x534a30, id)
    }
    ///Existing static root canary only; not a production context supplier.
    pub(in crate::intro::camera_live) fn test_eventlist_inherited_root(
        &mut self,
        root: u32,
    ) -> Result<(), String> {
        if self.provenance != Provenance::DeferredClaimed
            || self.primary_phase != Some(PrimaryPhase::Allocated)
        {
            return Err("EventList root canary outside untouched allocation".into());
        }
        self.cells
            .as_mut()
            .ok_or("process cells borrowed")?
            .set_word(E + 0x24, root)
    }
}
