//! Successful normal-field A construction through41e70b, never FullMain.
//! Source VAs label provenance. Modern node identities are allocated here;
//! neither a Cells heap nor a legacy NativeObject key represents these nodes.
use super::{
    case_zero::{List, ListBirth},
    case_zero_text::{Fonts, Text},
    navigation::Group,
    panel_models::CpuModel,
    primitive_factory::{MainPhase, Primitives},
    process_images::{ImageId, Images},
    property_child::{ControlEffect, selector_range_controls},
    registration,
    ui_graph::PanelId,
    ui_text_resources::UiTextResources,
};
pub(super) use crate::ui_text::ResourceIdentity;
use openmt2001::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage, x87::Sum};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};
const A: u32 = 0x58e3c0;
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    owner: u64,
    slot: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Field<T> {
    Unknown,
    Known(T),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Panel,
    Text,
    Visual,
    List3Texts,
    SpecialList3Texts,
    Amount2Visuals,
    Selector2Arrows,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Main,
    Overlay,
    Previous,
    Next,
    Notice,
    Extra,
    ParentSelector,
    Additional(u32),
    Mode(usize),
    Info(usize),
    SharedTitle,
    Child0List,
    Child0Selector,
    Child5List,
    ArrowPrevious,
    ArrowNext,
    Property(u32),
    Business(u32),
    AmountFirst150,
    AmountSecond154,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Allocate(Family),
    Constructor(u32),
    Slot04(u32),
    Number(u32),
    Append(NodeId),
    Counter(u32),
    Alias(u32),
    TextCall(u32),
    CaptionStore(u32),
    Geometry(u32),
    Show(u8),
    Attach(NodeId),
    ScalarStore { field: u32, value: u32 },
    Callback(u32),
    Returned(u32),
    Fault(String),
    Stop,
}
#[derive(Clone, Debug)]
pub struct Journal {
    pub source: u32,
    pub node: NodeId,
    pub event: Event,
}
pub struct Visual {
    pub caption: Text,
    pub images: [Option<ImageId>; 5],
    pub current: Option<ImageId>,
    pub icon_bounds: [f32; 5],
    pub icon: Option<ImageId>,
    pub caption_mode: u32,
    pub policy338: u32,
    pub selected336: u8,
    pub disabled333: u8,
    pub down335: u8,
    pub hover330: u8,
    pub flags331_332: [u8; 2],
    pub flag314: u8,
    pub flag2fc: u8,
    pub flag34c: u8,
    pub flag370: u8,
    pub state33c: u32,
    pub caption_identifier2f4: Field<u32>,
    /// Remaining subclass bytes have no retained first writer in this port.
    pub extra: Field<Vec<u8>>,
}
pub struct ListProduct {
    pub list: List<NodeId>,
    pub indicator: Option<NodeId>,
}
pub struct SelectorProduct {
    pub body: [f32; 5],
    pub slider: [f32; 5],
    pub position150: i32,
    pub extent154: f32,
    pub vertical168: u8,
    pub flag174: u8,
    pub dragging190: u8,
    pub admitted191: u8,
    pub drag_position18c: f32,
    pub first_list: Option<NodeId>,
    pub last_list: Option<NodeId>,
    pub previous: Option<NodeId>,
    pub next: Option<NodeId>,
}
pub enum Subtype {
    SpecialList3Texts(property::SpecialList),
    Amount2Visuals(property::Amount),
    ///Internal candidate phase; a successful prefix never exposes this.
    ConstructingList,
    ///Caption stays in Navigator's SAME primitive owner; this is an alias.
    Panel {
        caption_source: u32,
    },
    Text(Text),
    Visual(Visual),
    List3Texts(ListProduct),
    Selector2Arrows(SelectorProduct),
}
pub struct Node {
    pub id: NodeId,
    pub label: Label,
    pub family: Family,
    pub bounds: [f32; 4],
    pub depth: f32,
    pub field18: u32,
    pub shown: u8,
    pub disabled: u8,
    pub parent: Option<NodeId>,
    pub root: Option<NodeId>,
    pub number: Option<u32>,
    pub children: Vec<NodeId>,
    pub animation34: u32,
    pub completion38: u8,
    pub padding1e: Field<[u8; 2]>,
    ///Embedded Text node contexts are untouched by their containing extent setter.
    ///The outer node root24 is held separately by the typed root reference.
    pub embedded_text_contexts24: Vec<Field<u32>>,
    pub subtype: Subtype,
}
pub struct ChildZero {
    pub source: u32,
    pub parent: NodeId,
    pub state: u32,
    pub list: NodeId,
    pub selector: NodeId,
    pub native_return: i32,
}
struct Product {
    text_identity: ResourceIdentity,
    image_identity: ResourceIdentity,
    child1: Option<property::ChildOne>,
    child5: Option<property::ChildFive>,
    child2: Option<property::ChildTwo>,
    phase: MainPhase,
    owner: u64,
    nodes: Vec<Box<Node>>,
    journal: Vec<Journal>,
    root: NodeId,
    aliases: BTreeMap<u32, NodeId>,
    buttons: BTreeMap<u32, NodeId>,
    infos: [Option<NodeId>; 5],
    counter: u32,
    regions: Vec<([f32; 5], [u32; 4])>,
    mode_metadata: Vec<[u32; 5]>,
    main_window: [f32; 5],
    scrollbar_width: f32,
    child0: Option<ChildZero>,
    source_static_preimage: Vec<u8>,
    child0_preimage: Vec<u8>,
}
///Move-only retained incomplete product. It has no path to graph/register or
///positive input. Later full construction must consume it, never replay it.
pub struct AwaitingOtherMainChildren {
    inner: Box<Product>,
}
impl AwaitingOtherMainChildren {
    pub fn phase(&self) -> MainPhase {
        self.inner.phase
    }
    pub fn root(&self) -> NodeId {
        self.inner.root
    }
    pub fn node(&self, id: NodeId) -> Result<&Node, String> {
        self.inner.node(id)
    }
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.inner.nodes.iter().map(Box::as_ref)
    }
    pub fn journal(&self) -> &[Journal] {
        &self.inner.journal
    }
    pub fn alias(&self, parent_offset: u32) -> Result<NodeId, String> {
        self.inner
            .aliases
            .get(&parent_offset)
            .copied()
            .ok_or_else(|| format!("A+{parent_offset:x}: not produced by41e70b"))
    }
    pub fn button(&self, index: u32) -> Option<NodeId> {
        self.inner.buttons.get(&index).copied()
    }
    pub fn info(&self, index: usize) -> Option<NodeId> {
        self.inner.infos.get(index).copied().flatten()
    }
    pub fn child0(&self) -> &ChildZero {
        self.inner
            .child0
            .as_ref()
            .expect("successful prefix includes child0")
    }
    pub fn counter(&self) -> u32 {
        self.inner.counter
    }
    pub fn mode_metadata(&self) -> &[[u32; 5]] {
        &self.inner.mode_metadata
    }
    pub fn regions(&self) -> &[([f32; 5], [u32; 4])] {
        &self.inner.regions
    }
    pub fn main_window(&self) -> [f32; 5] {
        self.inner.main_window
    }
    pub fn scrollbar_width(&self) -> f32 {
        self.inner.scrollbar_width
    }
    pub fn static_preimage(&self) -> &[u8] {
        &self.inner.source_static_preimage
    }
    pub fn child0_preimage(&self) -> &[u8] {
        &self.inner.child0_preimage
    }
    pub fn require_full_main(&self) -> Result<(), String> {
        Err("41e70b incomplete: child1+390/4055f0, child5+394/409ec0, child2+398/408da0, parent tail,43bed0/184/43c600/41eb40 remain unconstructed".into())
    }
    pub fn require_input_or_render(&self) -> Result<(), String> {
        Err("Incomplete A has no graph/input/render activation".into())
    }
}
pub struct Prefix;
impl Prefix {
    pub fn load(
        executable: &[u8],
        index: &ArchiveIndex,
        archive: &[u8],
        ui: [u32; 2],
        text: &mut UiTextResources,
        primitives: &mut Primitives,
        images: &mut Images,
    ) -> Result<AwaitingOtherMainChildren, String> {
        //Duplicate/retry rejection precedes PE/model/font/image mutation.
        primitives.claim_main()?;
        let result = Self::load_claimed(executable, index, archive, ui, text, primitives, images)
            .map_err(|e| format!("41dcf0/40a950 A construction through41e70b: {e}"));
        if result.is_ok() {
            primitives.main_complete_prefix()
        } else {
            primitives.main_failed()
        }
        result
    }
    fn load_claimed(
        executable: &[u8],
        index: &ArchiveIndex,
        archive: &[u8],
        ui: [u32; 2],
        text: &mut UiTextResources,
        p: &mut Primitives,
        images: &mut Images,
    ) -> Result<AwaitingOtherMainChildren, String> {
        if ui != [640, 480] || images.ui_dimensions != ui {
            return Err("41dcf0 requires same actual640 image/display producer".into());
        }
        let pe = PeImage::parse(executable)?;
        for (table, slot, target) in [
            (0x525668, 4, 0x41dcf0),
            (0x525668, 0x30, 0x41eb40),
            (0x525908, 4, 0x42f140),
            (0x525908, 0, 0x440900),
            (0x525908, 0x2c, 0x43b370),
            (0x5259a4, 4, 0x442600),
            (0x5252e0, 4, 0x42fdf0),
            (0x525330, 4, 0x42f140),
        ] {
            if word(&pe, table + slot)? != target {
                return Err(format!(
                    "A construction concrete subtype dispatch {table:x}+{slot:x} differs"
                ));
            }
        }
        let path = registration::asset(p.cells(), Group::Primary, 4)?;
        let e = index
            .find(&path)
            .ok_or_else(|| format!("43b770 A: actual Primary4 model {path} absent"))?;
        let first = usize::try_from(e.offset).map_err(|_| "A mesh offset")?;
        let last = usize::try_from(
            e.offset
                .checked_add(e.length)
                .ok_or("A mesh span overflow")?,
        )
        .map_err(|_| "A mesh end")?;
        let bytes = archive.get(first..last).ok_or_else(|| {
            format!("43b770 A model {path}: archive span {first}..{last} missing")
        })?;
        let mesh = CachedMesh::parse(bytes).map_err(|e| format!("43b770 A model {path}: {e}"))?;
        let model = CpuModel::parsed(PanelId::A, e, &mesh)
            .map_err(|e| format!("43b770 A model {path}: {e}"))?;
        let preimage = p.cells().read(A, 0x50c)?.to_vec();
        let child0_preimage = p.cells().read(0x560920, 16)?.to_vec();
        p.construct_main(model)?;
        let owner = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| "typed node owner identity exhausted")?;
        let root = NodeId { owner, slot: 0 };
        p.retain_main_node_owner(owner);
        let mut product = Product {
            text_identity: text.construction_identity.clone(),
            image_identity: images.construction_identity.clone(),
            child1: None,
            child5: None,
            child2: None,
            phase: MainPhase::Constructing,
            owner,
            nodes: Vec::new(),
            journal: Vec::new(),
            root,
            aliases: BTreeMap::new(),
            buttons: BTreeMap::new(),
            infos: [None; 5],
            counter: 0,
            regions: Vec::new(),
            mode_metadata: Vec::new(),
            main_window: [0., 0., 0., 0., 0.5],
            scrollbar_width: 0.,
            child0: None,
            source_static_preimage: preimage,
            child0_preimage,
        };
        let id = product.allocate(
            Label::Main,
            Family::Panel,
            [0.; 4],
            Subtype::Panel {
                caption_source: A + 0x188,
            },
            0x439ae0,
        );
        assert_eq!(id, root);
        {
            let root = product.node_mut(root)?;
            root.padding1e = Field::Known(p.cells().read(A + 0x1e, 2)?.try_into().unwrap());
            root.embedded_text_contexts24 = vec![Field::Known(p.cells().word(A + 0x188 + 0x24)?)];
        }
        product.record(0x439ae0, root, Event::Constructor(0x439ae0));
        product.record(0x40a770, root, Event::Constructor(0x40a770));
        product.record(0x43b770, root, Event::Constructor(0x43b770));
        product.parent(&pe, p, text.fonts_mut(), images)?;
        product.child_zero(&pe, p, text.fonts_mut(), images)?;
        if p.cells().byte(A + 0x184)? != 0 {
            return Err("A184 must remain incomplete through41e70b".into());
        }
        if product
            .nodes
            .iter()
            .any(|n| matches!(n.subtype, Subtype::ConstructingList))
        {
            return Err("41e70b contains an unfinished admitted List constructor".into());
        }
        product.record(0x41e70b, root, Event::Stop);
        product.phase = MainPhase::AwaitingOtherMainChildren;
        Ok(AwaitingOtherMainChildren {
            inner: Box::new(product),
        })
    }
}
fn word(pe: &PeImage<'_>, at: u32) -> Result<u32, String> {
    Ok(u32::from_le_bytes(pe.at(at, 4)?.try_into().unwrap()))
}
fn float(pe: &PeImage<'_>, at: u32) -> Result<f32, String> {
    let v = f32::from_bits(word(pe, at)?);
    if !v.is_finite() {
        return Err(format!("A constant{at:x} is nonfinite"));
    }
    Ok(v)
}
fn double(pe: &PeImage<'_>, at: u32) -> Result<f64, String> {
    let v = f64::from_le_bytes(pe.at(at, 8)?.try_into().unwrap());
    if !v.is_finite() {
        return Err(format!("A constant{at:x} is nonfinite"));
    }
    Ok(v)
}
fn ntbs(pe: &PeImage<'_>, at: u32) -> Result<Vec<u8>, String> {
    let b = pe.at(at, 256)?;
    let n = b
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("A name{at:x} unterminated"))?;
    Ok(b[..n].to_vec())
}
fn sum(v: f32) -> Result<Sum, String> {
    Sum::from_f32(v).map_err(str::to_owned)
}
fn add(a: f32, b: f32) -> Result<f32, String> {
    let mut s = sum(a)?;
    s.add_f32(b).map_err(str::to_owned)?;
    store(s)
}
fn sub(a: f32, b: f32) -> Result<Sum, String> {
    let mut s = sum(a)?;
    s.add(sum(b)?.negated());
    Ok(s)
}
fn store(s: Sum) -> Result<f32, String> {
    let v = s.to_f32();
    if !v.is_finite() {
        return Err("A layout f32 store leaves finite domain".into());
    }
    Ok(v)
}
fn ftol(s: Sum) -> Result<i32, String> {
    super::case_zero_text::ftol_layout(s)
}

fn bounds(e: [f32; 5]) -> [f32; 4] {
    e[..4].try_into().unwrap()
}
fn write_extent(p: &mut Primitives, offset: u32, e: [f32; 5]) -> Result<(), String> {
    let c = p.construction_cells()?;
    for (i, v) in e.into_iter().enumerate() {
        c.set_word(A + offset + i as u32 * 4, v.to_bits())?
    }
    Ok(())
}
impl Product {
    fn record(&mut self, source: u32, node: NodeId, event: Event) {
        self.journal.push(Journal {
            source,
            node,
            event,
        });
    }
    fn node(&self, id: NodeId) -> Result<&Node, String> {
        if id.owner != self.owner {
            return Err("typed node belongs to another retained allocation owner".into());
        }
        self.nodes
            .get(id.slot)
            .map(Box::as_ref)
            .ok_or_else(|| "typed node identity not allocated".into())
    }
    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, String> {
        if id.owner != self.owner {
            return Err("typed mutable node belongs to another owner".into());
        }
        self.nodes
            .get_mut(id.slot)
            .map(Box::as_mut)
            .ok_or_else(|| "typed node identity not allocated".into())
    }
    fn allocate(
        &mut self,
        label: Label,
        family: Family,
        b: [f32; 4],
        subtype: Subtype,
        source: u32,
    ) -> NodeId {
        let id = NodeId {
            owner: self.owner,
            slot: self.nodes.len(),
        };
        let contexts = match family {
            Family::Panel | Family::Visual => 1,
            Family::List3Texts | Family::SpecialList3Texts => 3,
            _ => 0,
        };
        self.nodes.push(Box::new(Node {
            id,
            label,
            family,
            bounds: b,
            depth: 0.5,
            field18: 1,
            shown: 1,
            disabled: 0,
            parent: None,
            root: None,
            number: None,
            children: Vec::new(),
            animation34: 0,
            completion38: 0,
            padding1e: Field::Unknown,
            embedded_text_contexts24: vec![Field::Unknown; contexts],
            subtype,
        }));
        self.record(source, id, Event::Allocate(family));
        id
    }
    fn bind_node(&mut self, id: NodeId, parent: NodeId) -> Result<(), String> {
        let root = self.node(parent)?.root.unwrap_or(parent);
        let n = self.node_mut(id)?;
        n.parent = Some(parent);
        n.root = Some(root);
        Ok(())
    }
    fn append(
        &mut self,
        id: NodeId,
        parent: NodeId,
        number: u32,
        slot: u32,
        source: u32,
    ) -> Result<(), String> {
        self.bind_node(id, parent)?;
        self.append_bound(id, parent, number, slot, source)
    }
    fn append_bound(
        &mut self,
        id: NodeId,
        parent: NodeId,
        number: u32,
        slot: u32,
        source: u32,
    ) -> Result<(), String> {
        if self.node(id)?.parent != Some(parent) {
            return Err("wrapper slot04 completed without actual typed parent binding".into());
        }
        self.record(source, id, Event::Slot04(slot));
        self.node_mut(id)?.number = Some(number);
        self.record(source, id, Event::Number(number));
        self.node_mut(parent)?.children.push(id);
        self.record(source, id, Event::Append(parent));
        Ok(())
    }
    fn increment(&mut self, p: &mut Primitives, source: u32) -> Result<(), String> {
        self.counter = self
            .counter
            .checked_add(1)
            .ok_or("A child counter overflow")?;
        p.construction_cells()?.set_word(A + 0x388, self.counter)?;
        self.record(source, self.root, Event::Counter(self.counter));
        Ok(())
    }
    fn alias(&mut self, offset: u32, id: NodeId, source: u32) -> Result<(), String> {
        self.node(id)?;
        if self.aliases.insert(offset, id).is_some() {
            return Err("A typed field published twice".into());
        }
        self.record(source, id, Event::Alias(offset));
        Ok(())
    }
    fn text_mut(&mut self, id: NodeId) -> Result<&mut Text, String> {
        match &mut self.node_mut(id)?.subtype {
            Subtype::Text(t) => Ok(t),
            _ => Err("A text operation requires actual Text subtype".into()),
        }
    }
    fn visual_mut(&mut self, id: NodeId) -> Result<&mut Visual, String> {
        match &mut self.node_mut(id)?.subtype {
            Subtype::Visual(v) => Ok(v),
            _ => Err("A visual operation requires actual Visual subtype".into()),
        }
    }
    fn make_text(
        &mut self,
        label: Label,
        e: [f32; 5],
        number: u32,
        source: u32,
    ) -> Result<NodeId, String> {
        let mut t = Text::normal_fields();
        t.bounds = bounds(e); //slot04 is actual RET
        let id = self.allocate(label, Family::Text, bounds(e), Subtype::Text(t), source);
        self.record(source, id, Event::Constructor(0x440160));
        self.append(id, self.root, number, 0x42f140, source)?;
        Ok(id)
    }
    fn make_visual(
        &mut self,
        label: Label,
        e: [f32; 5],
        parent: NodeId,
        number: u32,
        source: u32,
    ) -> Result<NodeId, String> {
        let caption = Text::normal_fields();
        let v = Visual {
            caption,
            images: [None; 5],
            current: None,
            icon_bounds: [0., 0., 0., 0., 0.5],
            icon: None,
            caption_mode: 2,
            policy338: 0,
            selected336: 0,
            disabled333: 0,
            down335: 0,
            hover330: 0,
            flags331_332: [0; 2],
            flag314: 0,
            flag2fc: 0,
            flag34c: 0,
            flag370: 0,
            state33c: 0,
            caption_identifier2f4: Field::Unknown,
            extra: Field::Unknown,
        };
        let id = self.allocate(label, Family::Visual, bounds(e), Subtype::Visual(v), source);
        self.record(source, id, Event::Constructor(0x43ed20));
        self.bind_node(id, parent)?;
        self.visual_mut(id)?
            .caption
            .extent440900(bounds(e))
            .map_err(|error| format!("{source:x} {label:?} Visual/440900: {error}"))?;
        self.record(source, id, Event::TextCall(0x440900));
        self.append_bound(id, parent, number, 0x442600, source)?;
        Ok(id)
    }
    fn images(
        &mut self,
        id: NodeId,
        images: &Images,
        labels: [Option<u32>; 5],
    ) -> Result<(), String> {
        let tuple = labels
            .map(|l| l.map(|l| images.id(l)).transpose())
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let tuple: [Option<ImageId>; 5] =
            tuple.try_into().map_err(|_| "Visual image tuple length")?;
        let v = self.visual_mut(id)?;
        v.images = tuple;
        v.current = tuple[0];
        Ok(())
    }
    fn caption_mode(&mut self, id: NodeId, mode: u32) -> Result<(), String> {
        let v = self.visual_mut(id)?;
        v.caption_mode = mode;
        v.state33c = 0;
        Ok(())
    }
    fn named(
        pe: &PeImage<'_>,
        p: &mut Primitives,
        name: u32,
        temp: bool,
    ) -> Result<[f32; 5], String> {
        let name = if name == 0 {
            None
        } else {
            Some(ntbs(pe, name)?)
        };
        p.main_named(name.as_deref(), temp)
    }
    fn temporary(
        &mut self,
        pe: &PeImage<'_>,
        p: &mut Primitives,
        name: u32,
        source: u32,
    ) -> Result<[f32; 5], String> {
        //43b040 creates/destroys a normal Text temporary and copies its five
        //extent words. Missing required name is a stated finite-domain error.
        let mut t = Text::normal_fields();
        let e = Self::named(pe, p, name, true)?;
        t.bounds = bounds(e);
        self.record(source, self.root, Event::TextCall(0x43b040));
        Ok([t.bounds[0], t.bounds[1], t.bounds[2], t.bounds[3], t.depth])
    }
    fn parent(
        &mut self,
        pe: &PeImage<'_>,
        p: &mut Primitives,
        fonts: &mut Fonts,
        images: &mut Images,
    ) -> Result<(), String> {
        {
            let c = p.construction_cells()?;
            for (o, v) in [
                (0x38c, 0x560920),
                (0x390, 0x560820),
                (0x394, 0x5608e8),
                (0x398, 0x560930),
                (0x388, 0),
                (0x504, 0x44),
            ] {
                c.set_word(A + o, v)?;
            }
        }
        for i in 0..6 {
            let at = 0x534c70 + i * 20;
            let e = Self::named(pe, p, word(pe, at)?, false)?;
            let meta = [
                word(pe, at + 4)?,
                word(pe, at + 8)?,
                word(pe, at + 12)?,
                word(pe, at + 16)?,
            ];
            write_extent(p, 0x3b0 + i * 0x24, e)?;
            for (j, v) in meta.into_iter().enumerate() {
                p.construction_cells()?
                    .set_word(A + 0x3c4 + i * 0x24 + j as u32 * 4, v)?;
            }
            self.regions.push((e, meta));
        }
        let e = Self::named(pe, p, 0x534dac, false)?;
        let overlay = self.make_visual(Label::Overlay, e, self.root, self.counter, 0x41ddd9)?;
        self.alias(0x488, overlay, 0x41dde4)?;
        p.construction_cells()?.set_word(A + 0x4f0, self.counter)?;
        self.increment(p, 0x41ddf1)?;
        self.images(
            overlay,
            images,
            [Some(0x68af88), Some(0x68af90), None, Some(0x68af8c), None],
        )?;
        self.caption_mode(overlay, 3)?;
        {
            let n = self.node_mut(overlay)?;
            n.animation34 = 2;
            n.completion38 = 0;
        }
        let icon = images.id(0x68af44)?;
        let extent = Self::named(pe, p, 0x534dac, false)?;
        {
            let v = self.visual_mut(overlay)?;
            v.icon = Some(icon);
            v.icon_bounds = extent;
        }
        self.increment(p, 0x41deb9)?;
        let e = Self::named(pe, p, 0x5338dc, false)?;
        let prev = self.make_visual(Label::Previous, e, self.root, self.counter, 0x41deea)?;
        self.alias(0x4c4, prev, 0x41def5)?;
        self.increment(p, 0x41defc)?;
        self.images(
            prev,
            images,
            [Some(0x68afcc), Some(0x68afd4), None, Some(0x68afd0), None],
        )?;
        self.caption_mode(prev, 3)?;
        self.increment(p, 0x41df58)?;
        let e = Self::named(pe, p, 0x5338c0, false)?;
        let next = self.make_visual(Label::Next, e, self.root, self.counter, 0x41df87)?;
        self.alias(0x4c8, next, 0x41df92)?;
        self.increment(p, 0x41df99)?;
        self.images(
            next,
            images,
            [Some(0x68af9c), Some(0x68afa4), None, Some(0x68afa0), None],
        )?;
        self.caption_mode(next, 3)?;
        self.increment(p, 0x41dfe9)?;
        self.scrollbar_width = store(
            Sum::from_f64(f64::from(images.dimension(0x68afcc, 0)?)).map_err(str::to_owned)?,
        )?;
        p.construction_cells()?
            .set_word(A + 0x4d0, self.scrollbar_width.to_bits())?;
        let title1 = self.temporary(pe, p, 0x53385c, 0x41e04b)?;
        let title2 = self.temporary(pe, p, 0x533848, 0x41e090)?;
        let offset = float(pe, 0x5250a0)?;
        let notice_extent = [
            add(title1[0], offset)?,
            title1[1],
            add(title1[2], offset)?,
            title2[3],
            0.5,
        ];
        let notice = self.make_text(Label::Notice, notice_extent, self.counter, 0x41e10a)?;
        self.alias(0x3a0, notice, 0x41e11c)?;
        self.increment(p, 0x41e122)?;
        let space = ntbs(pe, 0x533278)?;
        self.text_mut(notice)?
            .literal(fonts, &space, &[])
            .map_err(|e| format!("41e128 Notice Text/441770: {e}"))?;
        self.record(0x41e128, notice, Event::TextCall(0x441770));
        self.text_mut(notice)?.style440650(4, -999, -999);
        self.text_mut(notice)?
            .set_font(fonts, 1)
            .map_err(|e| format!("41e14f Notice Text/440b70: {e}"))?;
        self.record(0x41e14f, notice, Event::TextCall(0x440b70));
        p.construction_cells()?.set_word(A + 0x500, 100)?;
        let t1 = self.temporary(pe, p, 0x53385c, 0x41e179)?;
        let t2 = self.temporary(pe, p, 0x533848, 0x41e1be)?;
        let extra = self.make_visual(
            Label::Extra,
            [t1[0], t1[1], t1[2], t2[3], 0.5],
            self.root,
            self.counter,
            0x41e22c,
        )?;
        self.alias(0x3a8, extra, 0x41e231)?;
        p.construction_cells()?.set_word(A + 0x3ac, self.counter)?;
        self.increment(p, 0x41e24e)?;
        self.main_window = Self::named(pe, p, 0x53381c, false)?;
        write_extent(p, 0x4d4, self.main_window)?;
        let x = ftol(sub(self.main_window[2], self.scrollbar_width)?)?;
        let mut y = Sum::from_i32(100);
        y.add_f32(self.main_window[1]).map_err(str::to_owned)?;
        let y = ftol(y)?;
        let mut h = sub(self.main_window[3], self.main_window[1])?;
        h.add(Sum::from_i32(100).negated());
        let h = ftol(h)?;
        let selector = self.make_selector(
            Label::ParentSelector,
            x,
            y,
            h,
            self.counter,
            pe,
            images,
            0x41e2c4,
        )?;
        self.alias(0x4cc, selector, 0x41e2c9)?;
        if let Subtype::Selector2Arrows(s) = &mut self.node_mut(selector)?.subtype {
            s.extent154 = -1.;
        }
        self.increment(p, 0x41e2ea)?;
        p.construction_cells()?.set_word(A + 0x4e8, self.counter)?;
        for i in 0..6 {
            let at = 0x534cfc + i * 20;
            self.mode_metadata.push([
                word(pe, at)?,
                word(pe, at + 4)?,
                word(pe, at + 8)?,
                word(pe, at + 12)?,
                word(pe, at + 16)?,
            ]);
            let e = Self::named(pe, p, word(pe, at)?, false)?;
            let id = self.make_visual(
                Label::Mode(i as usize),
                e,
                self.root,
                self.counter,
                0x41e32e,
            )?;
            self.alias(0x48c + i * 4, id, 0x41e337)?;
            let index = word(pe, at + 12)?;
            if self.buttons.insert(index, id).is_some() {
                return Err("A mode table duplicate published index".into());
            }
            self.increment(p, 0x41e349)?;
            let base = if i < 5 { 0x68b124 } else { 0x68afe4 };
            self.images(
                id,
                images,
                [
                    Some(base),
                    Some(base + 8),
                    Some(0x68ae6c),
                    Some(base + 4),
                    Some(base + 12),
                ],
            )?;
            self.caption_mode(id, 3)?;
            let v = self.visual_mut(id)?;
            v.policy338 = 1;
            v.flag370 = 1;
        }
        let _width_b134 = images.dimension(0x68b134, 0)?;
        let height_b134 = images.dimension(0x68b134, 1)?;
        let width = images.dimension(0x68ae5c, 0)?;
        let height = images.dimension(0x68ae5c, 1)?;
        let mut xx = sum(self.main_window[0])?;
        xx.add_f32(float(pe, 0x525034)?).map_err(str::to_owned)?;
        let mut xx = ftol(xx)?.wrapping_add(1);
        let mut yy = sum(self.main_window[1])?;
        yy.add_f32(float(pe, 0x525028)?).map_err(str::to_owned)?;
        let yy = ftol(yy)?.wrapping_add(1);
        //Native FILDs below are signed32, unlike dimension return storage's
        //earlier zero-extended conversion for scrollbar width.
        let factor = Sum::from_f64(double(pe, 0x5250b8)?).map_err(str::to_owned)?;
        let mut width_scaled = Sum::from_i32(width as i32);
        width_scaled.mul(factor);
        //FSTP QWORD at41e53a, then FLD QWORD at41e56d.
        let width_scaled = Sum::from_f64(width_scaled.to_f64()).map_err(str::to_owned)?;
        let top = store(Sum::from_i32(yy))?;
        let mut bottom = Sum::from_i32(height as i32);
        bottom.mul(factor);
        bottom.add_f32(top).map_err(str::to_owned)?;
        let bottom = store(bottom)?;
        p.construction_cells()?.set_word(A + 0x4ec, self.counter)?;
        for i in 0..5 {
            let mut right = Sum::from_i32(xx);
            right.add(width_scaled);
            let id = self.make_visual(
                Label::Info(i),
                [store(Sum::from_i32(xx))?, top, store(right)?, bottom, 0.5],
                self.root,
                self.counter,
                0x41e57f,
            )?;
            self.alias(0x4a4 + i as u32 * 4, id, 0x41e584)?;
            self.infos[i] = Some(id);
            self.increment(p, 0x41e58f)?;
            self.images(
                id,
                images,
                [
                    Some(0x68ae5c),
                    Some(0x68ae64),
                    Some(0x68ae6c),
                    Some(0x68ae60),
                    Some(0x68ae68),
                ],
            )?;
            self.caption_mode(id, 3)?;
            self.visual_mut(id)?.policy338 = 1;
            xx = xx.wrapping_add(_width_b134 as i32);
        }
        let title_offset = (height_b134 as i32).wrapping_add(2);
        let mut title_y = Sum::from_i32(title_offset);
        title_y
            .add_f32(self.main_window[1])
            .map_err(str::to_owned)?;
        let top = store(title_y)?;
        title_y
            .add_f32(float(pe, 0x5250b0)?)
            .map_err(str::to_owned)?;
        let bottom = store(title_y)?;
        let title = self.make_text(
            Label::SharedTitle,
            [
                self.main_window[0],
                top,
                self.main_window[2],
                bottom,
                self.main_window[4],
            ],
            self.counter,
            0x41e699,
        )?;
        self.alias(0x4b8, title, 0x41e6a0)?;
        self.text_mut(title)?
            .reset(fonts)
            .map_err(|e| format!("41e6a6 SharedTitle Text/441940: {e}"))?;
        self.record(0x41e6a6, title, Event::TextCall(0x441940));
        self.text_mut(title)?
            .set_font(fonts, 3)
            .map_err(|e| format!("41e6b3 SharedTitle Text/440b70: {e}"))?;
        self.record(0x41e6b3, title, Event::TextCall(0x440b70));
        {
            let t = self.text_mut(title)?;
            t.background_color = word(pe, 0x5362cc)?;
            t.background = 1;
            t.style440650(4, -999, -999);
        }
        self.increment(p, 0x41e6f1)?;
        Ok(())
    }
    ///43f510 vertical successful domain, including both concrete431f50
    ///arrow Visual allocations. Range/show is shared with existing Selector.
    fn make_selector(
        &mut self,
        label: Label,
        x: i32,
        y: i32,
        height: i32,
        number: u32,
        pe: &PeImage<'_>,
        images: &mut Images,
        source: u32,
    ) -> Result<NodeId, String> {
        let w = images.dimension(0x68afcc, 0)? as i32;
        let h = images.dimension(0x68afcc, 1)? as i32;
        let left = store(Sum::from_i32(x))?;
        let right = store(Sum::from_i32(x.wrapping_add(w)))?;
        let top = store(Sum::from_i32(y.wrapping_add(h).wrapping_add(2)))?;
        let bottom = store(Sum::from_i32(
            height.wrapping_add(y.wrapping_sub(h)).wrapping_sub(2),
        ))?;
        let body = [left, top, right, bottom, 0.5];
        let sel = SelectorProduct {
            body: [0., 0., 0., 0., 0.5],
            slider: [0., 0., 0., 0., 0.5],
            position150: 0,
            extent154: 10.,
            vertical168: 1,
            flag174: 0,
            dragging190: 0,
            admitted191: 1,
            drag_position18c: 0.,
            first_list: None,
            last_list: None,
            previous: None,
            next: None,
        };
        let id = self.allocate(
            label,
            Family::Selector2Arrows,
            bounds(body),
            Subtype::Selector2Arrows(sel),
            source,
        );
        self.record(source, id, Event::Constructor(0x431dd0));
        self.append(id, self.root, number, 0x42f140, source)?;
        if let Subtype::Selector2Arrows(s) = &mut self.node_mut(id)?.subtype {
            s.body = body;
        }
        let first = [
            left,
            store(Sum::from_i32(y))?,
            right,
            store(Sum::from_i32(y.wrapping_add(h)))?,
            0.5,
        ];
        let second = [
            left,
            store(Sum::from_i32(y.wrapping_add(height).wrapping_sub(h)))?,
            right,
            store(Sum::from_i32(y.wrapping_add(height)))?,
            0.5,
        ];
        //431f50 reads105's actual dimensions for BOTH supplied arrow extents.
        self.record(0x43f7d7, id, Event::Geometry(0x431f50));
        let aw = images.dimension(0x68ae28 + 105 * 4, 0)? as i32;
        let ah = images.dimension(0x68ae28 + 105 * 4, 1)? as i32;
        let arrow_extent = |e: [f32; 5]| -> Result<[f32; 5], String> {
            let mut r = sum(e[0])?;
            r.add(Sum::from_i32(aw));
            let mut b = sum(e[1])?;
            b.add(Sum::from_i32(ah));
            Ok([e[0], e[1], store(r)?, store(b)?, e[4]])
        };
        let previous =
            self.make_visual(Label::ArrowPrevious, arrow_extent(first)?, id, 0, 0x432021)?;
        self.caption_mode(previous, 4)?;
        let a = 0x68ae28 + 105 * 4;
        self.images(
            previous,
            images,
            [Some(a), Some(a + 8), Some(a + 12), Some(a + 4), None],
        )?;
        let next = self.make_visual(Label::ArrowNext, arrow_extent(second)?, id, 1, 0x4320b8)?;
        self.caption_mode(next, 4)?;
        let a = 0x68ae28 + 93 * 4;
        self.images(
            next,
            images,
            [Some(a), Some(a + 8), Some(a + 12), Some(a + 4), None],
        )?;
        let mut adjusted = body;
        let difference = sub(body[3], body[1])?;
        let minimum = float(pe, 0x5250a4)?;
        if difference.compare(sum(minimum)?) == std::cmp::Ordering::Less {
            let mut expansion = sum(minimum)?;
            expansion.add(difference.negated());
            expansion
                .mul_f32(float(pe, 0x52502c)?)
                .map_err(str::to_owned)?;
            let mut t = sum(body[1])?;
            t.add(expansion.negated());
            adjusted[1] = store(t)?;
            let mut b = sum(body[3])?;
            b.add(expansion);
            adjusted[3] = store(b)?;
        }
        {
            let n = self.node_mut(id)?;
            n.bounds = bounds(adjusted);
            if let Subtype::Selector2Arrows(s) = &mut n.subtype {
                s.previous = Some(previous);
                s.next = Some(next);
                s.body = adjusted;
            }
        }
        Ok(id)
    }
    fn show_selector(
        &mut self,
        id: NodeId,
        value: u8,
        zero: f32,
        source: u32,
    ) -> Result<(), String> {
        let (initial, first, last, previous, next) = {
            let n = self.node(id)?;
            let Subtype::Selector2Arrows(s) = &n.subtype else {
                return Err("4326b0 requires selector subtype".into());
            };
            (
                s.extent154,
                s.first_list,
                s.last_list,
                s.previous.ok_or("selector previous arrow missing")?,
                s.next.ok_or("selector next arrow missing")?,
            )
        };
        //431e80 binds ordinary List at15c;431ea0 binds special at160.
        //4326d0 resamples these DISTINCT slots in native overwrite order.
        let first = first
            .map(|list| -> Result<_, String> {
                match &self.node(list)?.subtype {
                    Subtype::List3Texts(l) => Ok(l.list.range()),
                    _ => Err("selector15c lacks actual ordinary List subtype".into()),
                }
            })
            .transpose()?;
        let last = last
            .map(|list| -> Result<_, String> {
                match &self.node(list)?.subtype {
                    Subtype::SpecialList3Texts(l) => Ok(l.range()),
                    _ => Err("selector160 lacks actual special List subtype".into()),
                }
            })
            .transpose()?;
        let (extent, effect) = selector_range_controls(
            initial,
            value,
            zero,
            None,
            first.as_ref(),
            None,
            last.as_ref(),
        );
        {
            let n = self.node_mut(id)?;
            n.shown = value;
            if let Subtype::Selector2Arrows(s) = &mut n.subtype {
                s.admitted191 = value;
                s.extent154 = extent;
            }
        }
        match effect {
            ControlEffect::Disable => {
                self.node_mut(id)?.disabled = 1;
                for arrow in [previous, next] {
                    self.node_mut(arrow)?.disabled = 1;
                    let v = self.visual_mut(arrow)?;
                    v.disabled333 = 1;
                    v.down335 = 0;
                }
            }
            ControlEffect::DisableThenEnable => {
                self.node_mut(id)?.disabled = 0;
                for arrow in [previous, next] {
                    self.node_mut(arrow)?.disabled = 0;
                    let v = self.visual_mut(arrow)?;
                    v.disabled333 = 0;
                    v.down335 = 0;
                }
            }
            ControlEffect::Enable => {
                self.node_mut(id)?.disabled = 0;
                for arrow in [previous, next] {
                    self.node_mut(arrow)?.disabled = 0;
                    self.visual_mut(arrow)?.disabled333 = 0;
                }
            }
            ControlEffect::Retain => (),
        }
        self.record(source, id, Event::Show(value));
        Ok(())
    }
    ///Actual43eb40/42f3d0/42fdf0 ordinary List path shared by BOTH
    ///41f750 no-title callers. All three embedded Text owners survive here.
    fn make_plain_list(
        &mut self,
        label: Label,
        extent: [f32; 5],
        fonts: &mut Fonts,
        source: u32,
    ) -> Result<NodeId, String> {
        let id = self.allocate(
            label,
            Family::List3Texts,
            bounds(extent),
            Subtype::ConstructingList,
            source,
        );
        let birth = ListBirth::new(id);
        self.record(source, id, Event::Constructor(0x42f3d0));
        for _ in 0..3 {
            self.record(0x42f3d0, id, Event::Constructor(0x440160));
        }
        self.bind_node(id, self.root)?;
        let list = {
            let node = self.node_mut(id)?;
            birth.layout(bounds(extent), fonts, &mut node.field18)
        }
        .map_err(|e| format!("41f88c {label:?} List3Texts/42fdf0: {e}"))?;
        for _ in 0..3 {
            self.record(0x42fdf0, id, Event::TextCall(0x440900));
        }
        self.record(0x42fe7d, id, Event::TextCall(0x42f510));
        self.node_mut(id)?.subtype = Subtype::List3Texts(ListProduct {
            list,
            indicator: None,
        });
        self.append_bound(id, self.root, self.counter, 0x42fdf0, source)?;
        Ok(id)
    }
    ///41f8a3..41f904: direct4406a0 precedes the two font3 setters.
    ///There is no4409c0 measure, no selector binding and no extra Text.
    fn configure_plain_list(&mut self, id: NodeId, fonts: &mut Fonts) -> Result<(), String> {
        let Subtype::List3Texts(l) = &mut self.node_mut(id)?.subtype else {
            return Err("41f750 requires ordinary List subtype".into());
        };
        l.list.primary.style440650(0, 0, 3);
        l.list.primary.padding = 1;
        l.list.primary.layout(fonts)?;
        l.list.secondary.style440650(2, 3, 3);
        l.list.primary.gap = 2;
        l.list.secondary.gap = 2;
        l.list.primary.set_font(fonts, 3)?;
        l.list.secondary.set_font(fonts, 3)?;
        l.list.shown = 0;
        for (source, call) in [
            (0x41f8a3, 0x440650),
            (0x41f8ba, 0x4406a0),
            (0x41f8cd, 0x440650),
            (0x41f8ee, 0x440b70),
            (0x41f8fd, 0x440b70),
        ] {
            self.record(source, id, Event::TextCall(call));
        }
        self.node_mut(id)?.shown = 0;
        Ok(())
    }
    fn child_zero(
        &mut self,
        pe: &PeImage<'_>,
        p: &mut Primitives,
        fonts: &mut Fonts,
        images: &mut Images,
    ) -> Result<(), String> {
        //40a950: child0 parent store occurs before41f750, state after it.
        p.construction_cells()?.set_word(0x560920, A)?;
        let mut y = sum(self.main_window[1])?;
        y.add_f32(float(pe, 0x525020)?).map_err(str::to_owned)?;
        let y = ftol(y)?;
        //41f750 no-title branch, selector output present.
        let right = store(sub(self.main_window[2], self.scrollbar_width)?)?;
        //FILD QWORD uses zero high word: negative low values become unsigned.
        let top = store(Sum::from_f64(f64::from(y as u32)).map_err(str::to_owned)?)?;
        let bottom = store(sub(self.main_window[3], float(pe, 0x525028)?)?)?;
        let extent = [self.main_window[0], top, right, bottom, 0.5];
        let id = self.make_plain_list(Label::Child0List, extent, fonts, 0x41f88c)?;
        self.configure_plain_list(id, fonts)?;
        self.increment(p, 0x41f911)?;
        let h = ftol(sub(self.main_window[3], top)?)?;
        let x = ftol(sub(self.main_window[2], self.scrollbar_width)?)?;
        let selector = self.make_selector(
            Label::Child0Selector,
            x,
            y,
            h,
            self.counter,
            pe,
            images,
            0x41f947,
        )?;
        self.increment(p, 0x41f955)?;
        {
            let Subtype::List3Texts(l) = &mut self.node_mut(id)?.subtype else {
                unreachable!()
            };
            l.indicator = Some(selector);
        }
        {
            let Subtype::Selector2Arrows(s) = &mut self.node_mut(selector)?.subtype else {
                unreachable!()
            };
            s.first_list = Some(id);
        }
        self.record(0x431e80, selector, Event::Attach(id));
        let zero = float(pe, 0x525040)?;
        self.show_selector(selector, 0, zero, 0x41f977)?;
        p.construction_cells()?.set_word(0x560920 + 4, 0)?;
        //Typed fields+8/+c carry these modern references; raw Cells retain
        //their static preimages. Never forge native addresses to fill them.
        self.node_mut(id)?.shown = 0;
        if let Subtype::List3Texts(l) = &mut self.node_mut(id)?.subtype {
            l.list.shown = 0;
        }
        self.show_selector(selector, 0, zero, 0x40a99d)?;
        self.child0 = Some(ChildZero {
            source: 0x560920,
            parent: self.root,
            state: 0,
            list: id,
            selector,
            native_return: -1,
        });
        Ok(())
    }
}
#[cfg(test)]
#[path = "main_construction_tests.rs"]
mod tests;

#[path = "main_property_construction.rs"]
mod property;
pub use property::{
    Amount, AwaitingChild2, AwaitingChild5, AwaitingParentTail, ChildFive, ChildOne, ChildTwo,
    CompletedMainLayout, SpecialList,
};
