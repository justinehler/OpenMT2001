//! Source-owned reusable node/Panel/caption and 43b770 primitive factory.
//! This is a construction stage, NOT a process UI graph or input activation.
//! Only the concrete F layout is supplied. Other real layouts reject; no
//! arbitrary callback, successful placeholder, heap pointer or model handle.
use super::{
    case_zero_text::Text,
    native_initializers as init,
    navigation::Group,
    panel_models::{CpuModel, CpuResources},
    process_image::Cells,
    registration,
    ui_graph::PanelId,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Node(u32),
    PanelBounds(u32),
    CaptionNode(u32),
    CaptionDefaults(u32),
    PanelSuffix(u32),
    NavigatorSuffix(u32),
    Bind(u32),
    Names(u32),
    ModelZ(u32),
    CaptionMode(u32),
    Frames(u32),
    Prepare(u32),
    BindFlags(u32),
    Layout(u32),
    Bound(u32),
    Currency,
    DescriptorCopy(u8),
    PrimaryReferences,
    LayoutReturned(u32),
    Registered(u32),
    PrimaryCount(u32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Constructed,
    Binding,
    AwaitingLayout,
    Bound,
    Parked,
}
/// Actual descriptor argument identity. Unbound is the outer Option::None.
/// EmbeddedD is non-null D+388, with a separately null model reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorIdentity {
    Archive(PanelId),
    EmbeddedD,
    NativeNull,
}
pub struct Panel {
    pub address: u32,
    pub caption: u32,
    pub phase: Phase,
    /// Actual +37c argument identity in the typed CPU owner. Outer None is
    /// unbound; NativeNull and the non-null EmbeddedD are distinct. Raw Cells
    /// never receive a fabricated modern pointer.
    pub descriptor: Option<DescriptorIdentity>,
}
#[derive(Clone, Copy)]
pub enum Layout {
    Navigator,
    Unsupplied,
}
pub struct Builder {
    cells: Cells,
    panels: BTreeMap<u32, Panel>,
    caption_keys: BTreeSet<u32>,
    spans: BTreeMap<u32, u32>,
    resources: CpuResources,
    steps: Vec<Step>,
}
fn rect(c: &mut Cells, at: u32, b: [f32; 4]) -> Result<(), String> {
    for (i, v) in b.into_iter().enumerate() {
        c.set_word(at + i as u32 * 4, v.to_bits())?
    }
    Ok(())
}
fn extent(c: &mut Cells, at: u32, b: [f32; 5]) -> Result<(), String> {
    for (i, v) in b.into_iter().enumerate() {
        c.set_word(at + i as u32 * 4, v.to_bits())?
    }
    Ok(())
}
/// Full default440160/440540 NORMAL field result. The525908+2c target is
/// actual43b370 RET (checked from supplied PE). Group constructor4e69b0 gives
/// count/capacity0, so clear4e6ac0 and resize440330 reach no dynamic storage.
/// No fonts are reached. Unwritten padding/context24 retain their actual preimages.
fn caption(c: &mut Cells, p: u32, steps: &mut Vec<Step>) -> Result<(), String> {
    //440540 reads this old divisor BEFORE its final pitch10 store. An unknown
    //heap preimage must fail even though its transient row result is dead.
    let _actual_divisor_preimage = c.word(p + 0x140)?;
    if c.word(0x525908 + 0x2c)? != 0x43b370 {
        return Err("Actual caption rebuild target differs from supplied RET family".into());
    }
    init::node43e700(c, p)?;
    steps.push(Step::CaptionNode(p));
    for o in [0x16c, 0x170, 0x174, 0x178] {
        c.set_word(p + o, 0)?
    }
    c.set_word(p + 0x17c, 0x3f000000)?;
    for o in [0x180, 0x184, 0x188, 0x18c, 0x190] {
        c.set_word(p + o, 0)?
    }
    c.set_word(p, 0x525908)?;
    c.write(p + 0x158, &0u16.to_le_bytes())?;
    for o in [0x15c, 0x160, 0x164, 0x168] {
        c.set_word(p + o, 0)?
    }
    c.set_byte(p + 0x1b0, 0)?;
    c.set_word(p + 0x1ac, 0)?;
    //RET rebuild cannot mutate count/capacity; no array allocation is reached.
    //The height/divisor->ftol transient148 is overwritten64 with no read by
    //440330 (which tests158/190). x87 exception/status parity is outside scope.
    c.set_word(p + 0x140, 10)?;
    c.set_byte(p + 0x144, 0)?;
    for o in [0x198, 0x19c, 0x1a0, 0x1a4, 0x13c, 0x154] {
        c.set_word(p + o, 0)?
    }
    c.set_byte(p + 0x1a8, 0)?;
    c.write(p + 0x194, &0u16.to_le_bytes())?;
    c.set_byte(p + 0x14c, 1)?;
    c.set_word(p + 0x150, 1f32.to_bits())?;
    c.set_word(p + 0x148, 64)?;
    c.set_word(p + 12, 0)?;
    c.set_word(p + 16, 0)?;
    c.set_byte(p + 0x1a9, 0)?;
    steps.push(Step::CaptionDefaults(p));
    Ok(())
}
impl Builder {
    /// Cells are moved from the actual PE process allocation or explicit
    /// supplied live storage. This factory never zero-initializes heap bytes.
    pub fn new(cells: Cells) -> Self {
        Self {
            cells,
            panels: BTreeMap::new(),
            caption_keys: BTreeSet::new(),
            spans: BTreeMap::new(),
            resources: CpuResources::new(),
            steps: Vec::new(),
        }
    }
    pub fn add_primary_model(&mut self, model: CpuModel) -> Result<(), String> {
        self.resources.insert_primary(&self.cells, model)
    }
    fn admit_span(&self, p: u32, len: u32) -> Result<(), String> {
        let end = p
            .checked_add(len)
            .ok_or("Primitive owner address overflow")?;
        if self.spans.iter().any(|(&at, &n)| at < end && p < at + n) {
            return Err("Primitive allocation aliases an existing retained subtype".into());
        }
        Ok(())
    }
    pub fn node(&mut self, p: u32) -> Result<(), String> {
        self.admit_span(p, 0x138)?;
        init::node43e700(&mut self.cells, p)?;
        self.steps.push(Step::Node(p));
        self.spans.insert(p, 0x138);
        Ok(())
    }
    pub fn panel(&mut self, p: u32) -> Result<(), String> {
        self.admit_span(p, 0x388)?;
        if self.panels.contains_key(&p)
            || self.caption_keys.contains(&p)
            || self.panels.values().any(|q| q.caption == p)
        {
            return Err("Panel/caption identity already constructed".into());
        }
        let cp = p
            .checked_add(0x188)
            .ok_or("Panel caption address overflow")?;
        if self.panels.contains_key(&cp) || self.caption_keys.contains(&cp) {
            return Err("Embedded caption aliases another retained subtype".into());
        }
        //Read-only preflight for the only consumed field absent from440160's
        //stores. Do not replace an unknown heap divisor with a default.
        self.cells.word(cp + 0x140)?;
        if self.cells.word(0x525908 + 0x2c)? != 0x43b370 {
            return Err("Actual caption rebuild target differs from supplied RET family".into());
        }
        init::panel43b380_prefix(&mut self.cells, p)?;
        self.steps.extend([Step::Node(p), Step::PanelBounds(p)]);
        caption(&mut self.cells, cp, &mut self.steps)?;
        init::panel43b380_suffix(&mut self.cells, p)?;
        self.steps.push(Step::PanelSuffix(p));
        self.panels.insert(
            p,
            Panel {
                address: p,
                caption: cp,
                phase: Phase::Constructed,
                descriptor: None,
            },
        );
        self.caption_keys.insert(cp);
        self.spans.insert(p, 0x388);
        Ok(())
    }
    pub fn navigator(&mut self) -> Result<(), String> {
        let p = registration::PRIMARY
            .iter()
            .find(|e| e.id == PanelId::F)
            .unwrap()
            .object;
        self.admit_span(p, 0x160c)?;
        self.panel(p)?;
        self.spans.insert(p, 0x160c);
        for at in [0x15c8, 0x15e4, 0x15f8] {
            extent(&mut self.cells, p + at, [0., 0., 0., 0., 0.5])?
        }
        self.cells.set_word(p, 0x525578)?;
        self.steps.push(Step::NavigatorSuffix(p));
        Ok(())
    }
    fn main_static(&mut self) -> Result<(), String> {
        self.admit_span(0x58e3c0, 0x50c)?;
        self.panel(0x58e3c0)?;
        self.spans.insert(0x58e3c0, 0x50c);
        for i in 0..6 {
            extent(
                &mut self.cells,
                0x58e3c0 + 0x3b0 + i * 0x24,
                [0., 0., 0., 0., 0.5],
            )?;
        }
        extent(&mut self.cells, 0x58e3c0 + 0x4d4, [0., 0., 0., 0., 0.5])?;
        self.cells.set_word(0x58e3c0, 0x525668)?;
        //40a770 is static birth; no40a950 body or reset is reached here.
        for o in [4, 0] {
            self.cells.set_word(0x560920 + o, 0)?;
        }
        Ok(())
    }
    ///43b770 prefix through the actual layout call boundary. DescriptorNone
    ///means the native NULL argument, NOT a failed resource lookup. A present
    ///descriptor with referenceNone takes its native independent branch.
    ///A missing map entry, wrong slot or definition alias is always an error.
    pub fn bind_prefix(
        &mut self,
        p: u32,
        descriptor: Option<PanelId>,
        names: Option<[Option<&[u8]>; 2]>,
        group: Group,
    ) -> Result<(), String> {
        self.bind_descriptor_prefix(
            p,
            descriptor.map_or(DescriptorIdentity::NativeNull, DescriptorIdentity::Archive),
            names,
            group,
        )
    }
    pub(super) fn bind_descriptor_prefix(
        &mut self,
        p: u32,
        descriptor: DescriptorIdentity,
        names: Option<[Option<&[u8]>; 2]>,
        group: Group,
    ) -> Result<(), String> {
        let panel = self
            .panels
            .get(&p)
            .ok_or("Bind requires completed actual Panel constructor")?;
        if panel.phase != Phase::Constructed {
            return Err("Panel binding cannot replay on a retained owner".into());
        }
        let model = match descriptor {
            DescriptorIdentity::Archive(id) => self.resources.resolve(id)?,
            DescriptorIdentity::NativeNull => None,
            DescriptorIdentity::EmbeddedD => {
                if p != 0x574488 || group != Group::Primary || self.cells.word(p + 0x388)? != 0 {
                    return Err(
                        "D bind requires SAME real non-null embedded descriptor with null model"
                            .into(),
                    );
                }
                None
            }
        };
        if let DescriptorIdentity::Archive(id) = descriptor {
            let actual = registration::entries(group)
                .into_iter()
                .find(|e| e.object == p)
                .ok_or("Bind panel/group identity mismatch")?;
            if actual.id != id {
                return Err("Bind descriptor aliases a different actual panel slot".into());
            }
        }
        self.panels.get_mut(&p).unwrap().phase = Phase::Binding;
        self.panels.get_mut(&p).unwrap().descriptor = Some(descriptor);
        self.cells
            .set_word(p + 0x138, registration::group_address(group))?;
        self.steps.push(Step::Bind(p));
        if let Some(key) = model {
            let m = self.resources.models.get_mut(&key).unwrap();
            if let Some(n) = names {
                let a = m.find(n[0])?;
                self.cells.set_word(p + 0x33c, a)?;
                let b = m.find(n[1])?;
                self.cells.set_word(p + 0x340, b)?;
            } else {
                self.cells.set_word(p + 0x340, u32::MAX)?;
                self.cells.set_word(p + 0x33c, u32::MAX)?;
            }
            self.steps.push(Step::Names(p));
            //Two separate native float stores; second reloads rounded Z.
            let negative = self.cells.float(0x525264)?;
            let offset = self.cells.float(0x525034)?;
            if !negative.is_finite() || !offset.is_finite() {
                return Err("Actual UI Z constants outside finite primitive domain".into());
            }
            let samples =
                u32::from(u16::from_le_bytes(m.header[..2].try_into().unwrap())) * m.frame_count();
            if samples as i32 > 0 {
                for frame in &mut m.frames {
                    for xyz in frame {
                        xyz[2] = (f64::from(xyz[2]) * f64::from(negative)) as f32;
                        xyz[2] = (f64::from(xyz[2]) + f64::from(offset)) as f32;
                        if !xyz[2].is_finite() {
                            return Err("Bound UI Z left finite primitive domain".into());
                        }
                    }
                }
            }
            self.steps.push(Step::ModelZ(p));
            //440650(4,-999,-999) preserves both optional fields.
            self.cells.set_word(p + 0x188 + 0x1a0, 4)?;
            self.cells.set_byte(p + 0x188 + 0x1a8, 0)?;
            self.steps.push(Step::CaptionMode(p));
        } else {
            self.cells.set_byte(p + 0x34e, 1)?;
        }
        let count = if self.cells.byte(p + 0x34e)? == 0 {
            self.resources
                .model(model.ok_or("Independent byte requires actual bound model")?)?
                .frame_count()
        } else {
            1
        };
        self.cells.set_word(p + 0x178, count)?;
        self.cells.set_word(p + 0x180, count)?;
        self.steps.push(Step::Frames(p));
        self.prepare_frames(p, model, count)?;
        self.steps.push(Step::Prepare(p));
        for o in [0x344, 0x345, 0x347, 0x185] {
            self.cells.set_byte(p + o, 0)?
        }
        self.steps.push(Step::BindFlags(p));
        self.panels.get_mut(&p).unwrap().phase = Phase::AwaitingLayout;
        Ok(())
    }
    fn prepare_frames(&mut self, p: u32, model: Option<PanelId>, count: u32) -> Result<(), String> {
        if self.cells.byte(p + 0x34e)? != 0 {
            return Ok(());
        }
        let m = self
            .resources
            .model(model.ok_or("Prepare needs actual model reference")?)?;
        let a = self.cells.word(p + 0x33c)?;
        let b = self.cells.word(p + 0x340)?;
        if self.cells.word(p + 0x178)? as i32 > 1 {
            if let Some(r) = m.region(a, 0)? {
                rect(&mut self.cells, p + 0x150, r)?
            }
        }
        let last = count.checked_sub(1).ok_or("Prepare frame underflow")? as usize;
        rect(&mut self.cells, p + 0x13c, m.parent(last)?)?;
        if let Some(r) = m.region(a, last)? {
            rect(&mut self.cells, p + 0x164, r)?
        }
        if let Some(r) = m.region(b, last)? {
            rect(&mut self.cells, p + 0x18c, r)?
        }
        self.cells.set_word(p + 0x17c, count)?;
        self.cells.set_byte(p + 0x347, 1)
    }
    /// Concrete synchronous layouts only. No generic callback/clone-and-commit
    /// mechanism. Unsupported layouts leave1840 and cannot finish installation.
    pub fn layout(&mut self, p: u32, layout: Layout) -> Result<(), String> {
        let panel = self.panels.get(&p).ok_or("Layout panel absent")?;
        if panel.phase != Phase::AwaitingLayout {
            return Err("Layout requires actual bind-prefix boundary".into());
        }
        match layout {
            Layout::Unsupplied => {
                return Err(
                    "Actual specialized panel layout/children/fonts remain unsupplied".into(),
                );
            }
            Layout::Navigator => {
                if p != 0x58cdb0
                    || self.cells.word(p)? != 0x525578
                    || panel.descriptor != Some(DescriptorIdentity::Archive(PanelId::F))
                {
                    return Err(
                        "43d010 requires actual Navigator constructor/descriptor identity".into(),
                    );
                }
                if self.cells.word(0x525578 + 4)? != 0x43d010 {
                    return Err("Actual Navigator slot04 differs from concrete43d010".into());
                }
                let key = self
                    .resources
                    .resolve(PanelId::F)?
                    .ok_or("43d010 cannot use a null model definition")?;
                let count = self.cells.word(p + 0x180)?;
                let last = count
                    .checked_sub(1)
                    .ok_or("Navigator named region frame underflow")?
                    as usize;
                self.steps.push(Step::Layout(p));
                init::navigator43d010_scalar(&mut self.cells, p)?;
                //Native43af30 starts an extent of zeros/depth. Missing NAME
                //thus gives that extent;43aca0's -1 remains a no-write query.
                for (name, at) in [(0x5364ac, 0x15e4), (0x5364ac, 0x15c8), (0x534ed0, 0x15f8)] {
                    let name = self.cells.ntbs(name, 256)?;
                    let m = self.resources.models.get_mut(&key).unwrap();
                    let part = m.find(Some(&name))?;
                    let mut e = [0., 0., 0., 0., 0.5];
                    if let Some(r) = m.region(part, last)? {
                        e[..4].copy_from_slice(&r)
                    }
                    extent(&mut self.cells, p + at, e)?;
                }
            }
        }
        self.cells.set_byte(p + 0x184, 1)?;
        self.steps.push(Step::Bound(p));
        self.panels.get_mut(&p).unwrap().phase = Phase::Bound;
        Ok(())
    }
    pub fn finish(self) -> Result<Primitives, String> {
        if self.panels.values().any(|p| p.phase == Phase::Binding) {
            return Err("Failed partial bind cannot publish a retained primitive owner".into());
        }
        let mut texts = BTreeMap::new();
        for &key in &self.caption_keys {
            //One native address, one actual Text subtype. It is NOT Visual;
            //+338/+336 are never read. A single eventual shared Fonts owner
            //must be supplied separately if text/layout reaches it.
            texts.insert(
                key,
                super::case_zero_factories::text_owner(&self.cells, key)?,
            );
        }
        Ok(Primitives {
            provenance: Provenance::Reordered,
            caption_keys: self.caption_keys,
            spans: self.spans,
            construction_identity: super::main_construction::ResourceIdentity::new(),
            primary_claimed: false,
            primary_currency_identity: None,
            primary_pool: None,
            primary_phase: None,
            main_phase: MainPhase::Unstarted,
            main_node_owner: None,
            cells: Some(self.cells),
            panels: self.panels,
            texts,
            resources: self.resources,
            steps: self.steps,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Provenance {
    Reordered,
    DeferredUnclaimed,
    DeferredClaimed,
}
pub struct Primitives {
    provenance: Provenance,
    caption_keys: BTreeSet<u32>,
    spans: BTreeMap<u32, u32>,
    //Frozen construction cells after transfer to typed Text. Not exposed for
    //mutation, reset/replay, or use by the old raw descriptor installation.
    pub(super) construction_identity: super::main_construction::ResourceIdentity,
    pub(super) primary_claimed: bool,
    pub(super) primary_currency_identity: Option<super::main_construction::ResourceIdentity>,
    pub(super) primary_pool: Option<[PanelId; 16]>,
    pub(super) primary_phase: Option<super::primary_d_construction::PrimaryPhase>,
    main_phase: MainPhase,
    main_node_owner: Option<u64>,
    cells: Option<Cells>,
    pub panels: BTreeMap<u32, Panel>,
    pub texts: BTreeMap<u32, Text>,
    pub resources: CpuResources,
    pub steps: Vec<Step>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainPhase {
    Unstarted,
    Constructing,
    AwaitingOtherMainChildren,
    ConstructingProperty,
    AwaitingChild5,
    ConstructingChild5,
    AwaitingChild2,
    ConstructingBusiness,
    AwaitingParentTail,
    ConstructingParentTail,
    AwaitingBinding,
    Failed,
}
#[path = "chat_primitives.rs"]
mod chat_primitives;
#[path = "deferred_primitives.rs"]
mod deferred_primitives;
#[path = "eventlist_primitives.rs"]
mod eventlist_primitives;
#[path = "primary_d_primitives.rs"]
mod primary_d_primitives;
#[path = "voting_primitives.rs"]
mod voting_primitives;
impl Primitives {
    ///Actual Navigator asset-load entry calls this with its successfully
    ///parsed retained model. This owner lives with that application's asset
    ///owner; no epoch/Pool lifecycle is reimplemented here.
    pub fn navigator(executable: &[u8], model: CpuModel) -> Result<Self, String> {
        let mut b = Builder::new(Cells::process_allocation(executable)?);
        b.add_primary_model(model)?;
        b.navigator()?;
        b.bind_prefix(0x58cdb0, Some(PanelId::F), None, Group::Primary)?;
        b.layout(0x58cdb0, Layout::Navigator)?;
        b.finish()
    }
    pub fn primary_pool(&self) -> Option<&[PanelId; 16]> {
        self.primary_pool.as_ref()
    }
    pub fn main_phase(&self) -> MainPhase {
        self.main_phase
    }
    pub(super) fn claim_main(&mut self) -> Result<(), String> {
        if self.provenance != Provenance::Reordered {
            return Err("Fresh Main must wait for the real Voting-through-Clock continuation; eager Prefix cannot consume it".into());
        }
        if self.main_phase != MainPhase::Unstarted {
            return Err("439ae0/41dcf0 A construction phase already claimed; no replay".into());
        }
        self.main_phase = MainPhase::Constructing;
        Ok(())
    }
    pub(super) fn main_failed(&mut self) {
        self.main_phase = MainPhase::Failed;
    }
    pub(super) fn retain_main_node_owner(&mut self, owner: u64) {
        self.main_node_owner = Some(owner);
    }
    pub(super) fn check_main_property_owner(&self, owner: u64) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingOtherMainChildren
            || self.main_node_owner != Some(owner)
        {
            return Err("4055f0 continuation phase/retained node owner mismatch; no replay".into());
        }
        Ok(())
    }
    pub(super) fn claim_main_property(&mut self, owner: u64) -> Result<(), String> {
        self.check_main_property_owner(owner)?;
        self.main_phase = MainPhase::ConstructingProperty;
        Ok(())
    }
    pub(super) fn main_complete_property(&mut self) {
        self.main_phase = MainPhase::AwaitingChild5;
    }
    pub(super) fn check_main_child5_owner(&self, owner: u64) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingChild5 || self.main_node_owner != Some(owner) {
            return Err("409ec0 continuation phase/retained node owner mismatch; no replay".into());
        }
        Ok(())
    }
    pub(super) fn claim_main_child5(&mut self, owner: u64) -> Result<(), String> {
        self.check_main_child5_owner(owner)?;
        self.main_phase = MainPhase::ConstructingChild5;
        Ok(())
    }
    pub(super) fn main_complete_child5(&mut self) {
        self.main_phase = MainPhase::AwaitingChild2;
    }
    pub(super) fn check_main_business_owner(&self, owner: u64) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingChild2 || self.main_node_owner != Some(owner) {
            return Err("408da0 continuation phase/retained node owner mismatch; no replay".into());
        }
        Ok(())
    }
    pub(super) fn claim_main_business(&mut self, owner: u64) -> Result<(), String> {
        self.check_main_business_owner(owner)?;
        self.main_phase = MainPhase::ConstructingBusiness;
        Ok(())
    }
    pub(super) fn main_complete_business(&mut self) {
        self.main_phase = MainPhase::AwaitingParentTail;
    }
    pub(super) fn check_main_tail_owner(&self, owner: u64) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingParentTail || self.main_node_owner != Some(owner) {
            return Err("41e759 continuation phase/retained node owner mismatch; no replay".into());
        }
        Ok(())
    }
    pub(super) fn claim_main_tail(&mut self, owner: u64) -> Result<(), String> {
        self.check_main_tail_owner(owner)?;
        self.main_phase = MainPhase::ConstructingParentTail;
        Ok(())
    }
    ///41dcf0 returned;43ba52 and the owning43cc60 group join have not run.
    pub(super) fn main_layout_returned(&mut self) {
        self.main_phase = MainPhase::AwaitingBinding;
    }
    pub(super) fn reset_retained_child5(&mut self, owner: u64) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingChild2 || self.main_node_owner != Some(owner) {
            return Err("409f00 requires SAME retained child5 owner at child2 boundary".into());
        }
        super::native_initializers::five409f00(
            self.cells.as_mut().ok_or("process cells are borrowed")?,
            0x5608e8,
        )
    }
    pub(super) fn main_complete_prefix(&mut self) {
        self.main_phase = MainPhase::AwaitingOtherMainChildren;
    }
    /// Temporarily move the SAME retained resources/cells into the existing
    /// constructor. F and its mutable model payload are never reconstructed.
    /// Restoration happens on every Result path, including partial A failure.
    pub(super) fn construct_main(&mut self, model: CpuModel) -> Result<(), String> {
        if self.main_phase != MainPhase::Constructing {
            return Err("A prefix phase not claimed".into());
        }
        let mut b = Builder {
            cells: self.cells.take().ok_or("nested process construction")?,
            panels: std::mem::take(&mut self.panels),
            caption_keys: std::mem::take(&mut self.caption_keys),
            spans: std::mem::take(&mut self.spans),
            resources: std::mem::replace(&mut self.resources, CpuResources::new()),
            steps: std::mem::take(&mut self.steps),
        };
        let result = (|| {
            b.add_primary_model(model)?;
            b.main_static()?;
            b.bind_prefix(0x58e3c0, Some(PanelId::A), None, Group::Primary)?;
            let key = 0x58e3c0 + 0x188;
            self.texts
                .insert(key, super::case_zero_factories::text_owner(&b.cells, key)?);
            Ok(())
        })();
        self.cells = Some(b.cells);
        self.panels = b.panels;
        self.caption_keys = b.caption_keys;
        self.spans = b.spans;
        self.resources = b.resources;
        self.steps = b.steps;
        result
    }
    pub(super) fn construction_cells(&mut self) -> Result<&mut Cells, String> {
        if !matches!(
            self.main_phase,
            MainPhase::Constructing
                | MainPhase::ConstructingProperty
                | MainPhase::ConstructingChild5
                | MainPhase::ConstructingBusiness
                | MainPhase::ConstructingParentTail
        ) {
            return Err("A source-field write outside construction phase".into());
        }
        self.cells
            .as_mut()
            .ok_or_else(|| "process cells are borrowed".into())
    }
    pub(super) fn main_named(
        &mut self,
        name: Option<&[u8]>,
        temporary: bool,
    ) -> Result<[f32; 5], String> {
        let m = self
            .resources
            .models
            .get_mut(&PanelId::A)
            .ok_or("43af30 A model missing")?;
        let part = m.find(name)?;
        if temporary && part == u32::MAX {
            return Err(
                "43b040 mandatory temporary Text name absent; no normal-field no-match admission"
                    .into(),
            );
        }
        let last = m.frame_count().checked_sub(1).ok_or("A frame underflow")? as usize;
        let mut e = [0., 0., 0., 0., 0.5];
        if let Some(r) = m.region(part, last)? {
            e[..4].copy_from_slice(&r)
        }
        Ok(e)
    }
    pub fn cells(&self) -> &Cells {
        self.cells.as_ref().expect("construction borrow restored")
    }
    pub fn text_mut(&mut self, key: u32) -> Result<&mut Text, String> {
        self.texts
            .get_mut(&key)
            .ok_or_else(|| "Actual Text alias not constructed".into())
    }
    pub fn require_text_layout(&self, key: u32) -> Result<(), String> {
        self.texts
            .get(&key)
            .ok_or("Actual Text alias not constructed")?;
        Err("Primitive caption has no supplied shared font manager/layout consumer; synchronous text layout remains closed".into())
    }
    pub fn require_process_graph(&self) -> Result<(), String> {
        Err("Primitive stage lacks containing 18/23/7 layouts, resets, child dispatch and live43c8e0; graph/general-F/native input remain closed".into())
    }
}
#[cfg(test)]
#[path = "primitive_factory_tests.rs"]
mod tests;

//Negative regression injection only. No production source/selection adapter.
#[cfg(test)]
impl Primitives {
    pub(super) fn test_property_selection_word(&mut self, value: u32) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingOtherMainChildren {
            return Err("fault injection outside prefix boundary".into());
        }
        self.cells
            .as_mut()
            .ok_or("source cells borrowed")?
            .set_word(0x822d50, value)
    }
}

#[cfg(test)]
impl Primitives {
    ///Reset regression canary, not a production selected-context producer.
    pub(super) fn test_child5_state_word(&mut self, value: u32) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingChild2 {
            return Err("child5 reset canary outside retained boundary".into());
        }
        self.cells
            .as_mut()
            .ok_or("source cells borrowed")?
            .set_word(0x5608e8 + 0x24, value)
    }
}

#[cfg(test)]
impl Primitives {
    ///Negative independent-authority regression only: cannot force NULL.
    pub(super) fn test_business_guard_nonnull(&mut self) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingChild2 {
            return Err("business guard canary outside child2 boundary".into());
        }
        self.cells
            .as_mut()
            .ok_or("source cells borrowed")?
            .set_word(0x590f60, 1)
    }
}

//Negative source-boundary injections only, never production selection/group producers.
#[cfg(test)]
impl Primitives {
    pub(super) fn test_tail_word(&mut self, address: u32, value: u32) -> Result<(), String> {
        if self.main_phase != MainPhase::AwaitingParentTail {
            return Err("tail injection outside retained41e759 boundary".into());
        }
        self.cells
            .as_mut()
            .ok_or("process cells borrowed")?
            .set_word(address, value)
    }
}
