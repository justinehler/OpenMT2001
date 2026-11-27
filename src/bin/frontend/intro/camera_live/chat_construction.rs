//! Concrete Chat57ef28 construction ONLY. No show/reset/input/transport owner.
//! Cells retain static preimages; typed IDs never fabricate native pointers.
use super::{
    case_zero_text::Text,
    main_construction::{Field, ResourceIdentity},
    primitive_factory::{Primitives, Step},
    process_images::Images,
    ui_text_resources::UiTextResources,
    visual_construction::{ImageRef, Visual},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};
pub const SOURCE: u32 = 0x57ef28;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    owner: u64,
    slot: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    StaticPending,
    Unbound,
    Binding,
    LayingOut,
    Returned,
    Registered,
    Failed,
}
///Only the successful normal43f340 birth. Unknown heap/context/padding never
///becomes a usable default. This subtype owns the sole embedded Text+1c4.
pub struct Input {
    pub text: Text,
    pub embedded_context24: Field<u32>,
    pub node_context138: Field<[u8; 4]>,
    pub padding13d: Field<[u8; 3]>,
    pub padding389: Field<[u8; 3]>,
    pub focused: u8,
    pub count: u32,
    pub raw: [Option<u8>; 128],
    pub width_limit: i32,
    pub char_limit: i32,
    pub enabled: u8,
    pub flag381: u8,
    pub active382: Field<u8>,
    pub ready383: u8,
    pub color: u32,
    pub flag388: u8,
    pub pulse: u32,
}
impl Input {
    fn normal_fields() -> Self {
        let mut raw = [None; 128];
        raw[0] = Some(0);
        Self {
            text: Text::normal_fields(),
            embedded_context24: Field::Unknown,
            node_context138: Field::Unknown,
            padding13d: Field::Unknown,
            padding389: Field::Unknown,
            focused: 0,
            count: 0,
            raw,
            width_limit: -1,
            char_limit: -1,
            enabled: 0,
            flag381: 1,
            active382: Field::Unknown,
            ready383: 0,
            color: u32::MAX,
            flag388: 1,
            pulse: 0,
        }
    }
}
pub enum Subtype {
    Visual(Visual),
    Input(Input),
}
pub struct Node {
    pub id: NodeId,
    pub vtable: u32,
    pub allocation_size: u32,
    pub bounds: [f32; 4],
    pub depth: f32,
    pub field18: u32,
    pub shown: u8,
    pub disabled: u8,
    pub name: [u8; 255],
    pub padding1e: Field<[u8; 2]>,
    pub event_sink: u32,
    pub frame: u32,
    pub next: Option<NodeId>,
    pub number: Option<u32>,
    pub animation34: u32,
    pub completion38: u8,
    pub subtype: Subtype,
}
impl Node {
    pub fn text(&self) -> &Text {
        match &self.subtype {
            Subtype::Visual(v) => &v.caption,
            Subtype::Input(i) => &i.text,
        }
    }
    fn text_mut(&mut self) -> &mut Text {
        match &mut self.subtype {
            Subtype::Visual(v) => &mut v.caption,
            Subtype::Input(i) => &mut i.text,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Query {
        source: u32,
        name: u32,
    },
    Allocated {
        source: u32,
        node: NodeId,
    },
    Slot04 {
        node: NodeId,
        target: u32,
    },
    Number {
        node: NodeId,
        key: u32,
    },
    Append(NodeId),
    Alias {
        offset: u32,
        node: NodeId,
    },
    TextCall {
        node: Option<NodeId>,
        text_source: u32,
        target: u32,
    },
    ImageRead(u32),
    ImageStore {
        node: NodeId,
        offset: u32,
    },
    OuterSlot00(NodeId),
    ProcessAlias,
    Returned,
}
pub struct ChatProduct {
    pub source: u32,
    pub caption_source: u32,
    pub title_source: u32,
    pub to_source: u32,
    pub static_preimage: Vec<u8>,
    pub(super) phase: Phase,
    pub failure: Option<String>,
    owner: u64,
    process: ResourceIdentity,
    nodes: Vec<Box<Node>>,
    children: Vec<NodeId>,
    aliases: BTreeMap<u32, NodeId>,
    effects: Vec<Effect>,
    title_x: Vec<Option<i16>>,
    to_x: Vec<Option<i16>>,
    #[cfg(test)]
    fault_final_notify: bool,
    #[cfg(test)]
    fault_first_identifier: Option<u32>,
}
impl ChatProduct {
    pub(super) fn allocated(p: &Primitives) -> Result<Box<Self>, String> {
        let preimage = p.cells().read(SOURCE, 0x80c)?.to_vec();
        let owner = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| "Chat owner IDs exhausted")?;
        Ok(Box::new(Self {
            source: SOURCE,
            caption_source: SOURCE + 0x188,
            title_source: SOURCE + 0x388,
            to_source: SOURCE + 0x53c,
            static_preimage: preimage,
            phase: Phase::StaticPending,
            failure: None,
            owner,
            process: p.construction_identity.clone(),
            nodes: Vec::new(),
            children: Vec::new(),
            aliases: BTreeMap::new(),
            effects: Vec::new(),
            title_x: Vec::new(),
            to_x: Vec::new(),
            #[cfg(test)]
            fault_final_notify: false,
            #[cfg(test)]
            fault_first_identifier: None,
        }))
    }
    pub(super) fn same_process(&self, p: &Primitives) -> bool {
        self.process.same(&p.construction_identity)
    }
    pub(super) fn fail(&mut self, error: String) {
        self.phase = Phase::Failed;
        self.failure = Some(error);
    }
    #[cfg(test)]
    pub(super) fn test_final_font_notify(&mut self) {
        self.fault_final_notify = true;
    }
    #[cfg(test)]
    pub(super) fn test_first_identifier(&mut self, id: u32) {
        self.fault_first_identifier = Some(id);
    }
    #[cfg(test)]
    pub(super) fn test_input_factory(
        &mut self,
        p: &Primitives,
        e: [f32; 5],
    ) -> Result<NodeId, String> {
        self.make_input(p, e)
    }
    #[cfg(test)]
    pub(super) fn test_visual_images(
        &mut self,
        p: &Primitives,
        text: &mut UiTextResources,
        images: &Images,
    ) -> Result<(), String> {
        self.visual(
            p,
            text,
            images,
            0,
            [1., 2., 100., 40., 0.75],
            [Some(0x52), Some(0x53), Some(0x54), None],
            Some(0x5cb),
            &[],
        )
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().map(Box::as_ref)
    }
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
    pub fn title_x(&self) -> &[Option<i16>] {
        &self.title_x
    }
    pub fn to_x(&self) -> &[Option<i16>] {
        &self.to_x
    }
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    ///None means alias unproduced; static_preimage still owns its native word.
    pub fn alias(&self, offset: u32) -> Option<NodeId> {
        self.aliases.get(&offset).copied()
    }
    pub fn node(&self, id: NodeId) -> Result<&Node, String> {
        if id.owner != self.owner {
            return Err("Chat node belongs to another product".into());
        }
        self.nodes
            .get(id.slot)
            .map(Box::as_ref)
            .ok_or_else(|| "Chat node absent".into())
    }
    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, String> {
        if id.owner != self.owner {
            return Err("Chat node belongs to another product".into());
        }
        self.nodes
            .get_mut(id.slot)
            .map(Box::as_mut)
            .ok_or_else(|| "Chat node absent".into())
    }
    pub fn input(&self) -> Result<&Input, String> {
        match &self
            .node(self.alias(0x808).ok_or("Chat Input not yet returned")?)?
            .subtype
        {
            Subtype::Input(i) => Ok(i),
            _ => Err("Chat808 is not Input".into()),
        }
    }
    fn visual_mut(&mut self, id: NodeId) -> Result<&mut Visual, String> {
        match &mut self.node_mut(id)?.subtype {
            Subtype::Visual(v) => Ok(v),
            _ => Err("Chat child not Visual".into()),
        }
    }
    fn allocate(
        &mut self,
        p: &Primitives,
        e: [f32; 5],
        subtype: Subtype,
        source: u32,
    ) -> Result<NodeId, String> {
        let frame = p.cells().word(SOURCE + 0x24)?;
        let frame = if frame == 0 { SOURCE } else { frame };
        let (allocation_size, vtable) = match &subtype {
            Subtype::Visual(_) => (0x374, 0x5259a4),
            Subtype::Input(_) => (0x390, 0x5259f4),
        };
        let id = NodeId {
            owner: self.owner,
            slot: self.nodes.len(),
        };
        self.nodes.push(Box::new(Node {
            id,
            vtable,
            allocation_size,
            bounds: bounds(e),
            depth: 0.5,
            field18: 1,
            shown: 1,
            disabled: 0,
            name: [0; 255],
            padding1e: Field::Unknown,
            event_sink: SOURCE,
            frame,
            next: None,
            number: None,
            animation34: 0,
            completion38: 0,
            subtype,
        }));
        self.effects.push(Effect::Allocated { source, node: id });
        Ok(id)
    }
    fn append(&mut self, id: NodeId, key: u32) -> Result<(), String> {
        if let Some(tail) = self.children.last().copied() {
            self.node_mut(tail)?.next = Some(id);
        }
        self.children.push(id);
        self.effects.push(Effect::Append(id));
        debug_assert_eq!(self.node(id)?.number, Some(key));
        Ok(())
    }
    fn number(&mut self, id: NodeId, key: u32) {
        self.nodes[id.slot].number = Some(key);
        self.effects.push(Effect::Number { node: id, key });
    }
    fn publish(&mut self, offset: u32, id: NodeId) -> Result<(), String> {
        self.node(id)?;
        if self.aliases.contains_key(&offset) {
            return Err("Chat alias already produced; no replay".into());
        }
        self.aliases.insert(offset, id);
        self.effects.push(Effect::Alias { offset, node: id });
        Ok(())
    }
    fn text_call(&mut self, id: Option<NodeId>, text_source: u32, target: u32) {
        self.effects.push(Effect::TextCall {
            node: id,
            text_source,
            target,
        });
    }
    fn query_store(
        &mut self,
        p: &mut Primitives,
        name: u32,
        offset: u32,
        source: u32,
    ) -> Result<(), String> {
        self.effects.push(Effect::Query { source, name });
        let e = p.chat_named(name)?;
        for (i, v) in e.into_iter().enumerate() {
            p.chat_cells()?
                .set_word(SOURCE + offset + 4 * i as u32, v.to_bits())?;
        }
        Ok(())
    }
    fn embedded_label(
        &mut self,
        p: &mut Primitives,
        text: &mut UiTextResources,
        offset: u32,
        extent: u32,
        style: u32,
        identifier: u32,
    ) -> Result<(), String> {
        let key = SOURCE + offset;
        let e = read_extent(p, extent)?;
        self.text_call(None, key, 0x440650);
        p.text_mut(key)?.style440650(style, -999, -999);
        self.text_call(None, key, 0x440900);
        p.text_mut(key)?.extent440900(bounds(e))?;
        self.text_call(None, key, 0x440b70);
        p.text_mut(key)?
            .set_font440b70_retained(text.fonts_mut(), 3)?;
        self.text_call(None, key, 0x441940);
        p.text_mut(key)?.reset_unaligned(if offset == 0x388 {
            &mut self.title_x
        } else {
            &mut self.to_x
        })?;
        p.text_mut(key)?.line_mode = 1;
        self.text_call(None, key, 0x441220);
        p.text_mut(key)?.identifier441220_unaligned(
            text.fonts_mut(),
            identifier,
            if offset == 0x388 {
                &mut self.title_x
            } else {
                &mut self.to_x
            },
        )
    }
    fn visual(
        &mut self,
        p: &Primitives,
        text: &mut UiTextResources,
        images: &Images,
        key: u32,
        e: [f32; 5],
        ordinals: [Option<u32>; 4],
        identifier: Option<u32>,
        literal: &[u8],
    ) -> Result<(), String> {
        let mut refs = [ImageRef::Invalid; 4];
        //Actual normal,hover,pressed,disabled reads precede ANY factory effect.
        for (i, ordinal) in ordinals.into_iter().enumerate() {
            if let Some(ordinal) = ordinal {
                let at = 0x68ae28 + 4 * ordinal;
                self.effects.push(Effect::ImageRead(at));
                refs[i] = ImageRef::Image(images.id(at)?);
            }
        }
        let id = self.allocate(p, e, Subtype::Visual(Visual::normal_fields()), 0x43ed20)?;
        self.text_call(Some(id), 0, 0x440900);
        self.visual_mut(id)?.initialize442600(bounds(e))?;
        self.effects.push(Effect::Slot04 {
            node: id,
            target: 0x442600,
        });
        self.number(id, key);
        self.append(id, key)?;
        self.publish(0x7e0 + 4 * key, id)?;
        //No animation34/38 write: wrapper operand is native -1.
        for (offset, image) in [
            (0x300, refs[0]),
            (0x348, refs[0]),
            (0x304, refs[2]),
            (0x308, refs[1]),
            (0x30c, refs[3]),
            (0x310, ImageRef::Invalid),
        ] {
            self.visual_mut(id)?.images.insert(offset, image);
            self.effects.push(Effect::ImageStore { node: id, offset });
        }
        {
            let v = self.visual_mut(id)?;
            v.scalars.insert(0x13c, 3);
            v.scalars.insert(0x33c, 0);
        }
        if let Some(identifier) = identifier {
            self.text_call(Some(id), 0, 0x442960);
            self.visual_mut(id)?
                .caption_identifier442960(text.fonts_mut(), identifier)?;
        } else {
            self.text_call(Some(id), 0, 0x442a20);
            self.visual_mut(id)?
                .caption_literal442a20(text.fonts_mut(), literal)?;
        }
        //Outer Visual slot00=432890 RET20; never update outer extent here.
        self.effects.push(Effect::OuterSlot00(id));
        self.text_call(Some(id), 0, 0x440900);
        self.node_mut(id)?.text_mut().extent440900(bounds(e))?;
        self.text_call(Some(id), 0, 0x440b70);
        self.node_mut(id)?
            .text_mut()
            .set_font440b70_retained(text.fonts_mut(), 3)?;
        self.text_call(Some(id), 0, 0x440650);
        self.node_mut(id)?.text_mut().style440650(4, -999, -999);
        self.node_mut(id)?.shown = 1;
        Ok(())
    }
    fn make_input(&mut self, p: &Primitives, e: [f32; 5]) -> Result<NodeId, String> {
        let id = self.allocate(p, e, Subtype::Input(Input::normal_fields()), 0x43f340)?;
        //Input table is exactly0x28 bytes. Its slot04 is42f140 RET, NOT Row.
        self.effects.push(Effect::Slot04 {
            node: id,
            target: 0x42f140,
        });
        self.number(id, 0);
        self.node_mut(id)?.text_mut().bounds = bounds(e); //direct four-word copy
        self.node_mut(id)?.bounds = bounds(e); //repeated outer copy before append
        self.append(id, 0)?;
        Ok(id) //both depths retain normal birth0.5
    }
    pub(super) fn layout(
        &mut self,
        p: &mut Primitives,
        text: &mut UiTextResources,
        images: &Images,
    ) -> Result<(), String> {
        if self.phase != Phase::Binding || !self.same_process(p) {
            return Err("Chat body outside SAME bound owner; no replay".into());
        }
        self.phase = Phase::LayingOut;
        p.steps.push(Step::Layout(SOURCE));
        for (name, offset, source) in [
            (0x5341f0, 0x6f0, 0x41a537),
            (0x5341e0, 0x704, 0x41a55f),
            (0x5341d0, 0x718, 0x41a591),
            (0x5341c0, 0x72c, 0x41a5bd),
            (0x5341b0, 0x740, 0x41a5ec),
            (0x53419c, 0x768, 0x41a618),
            (0x53418c, 0x77c, 0x41a644),
            (0x53417c, 0x790, 0x41a670),
            (0x53416c, 0x7a4, 0x41a69f),
            (0x53415c, 0x7b8, 0x41a6cb),
            (0x53414c, 0x7cc, 0x41a6f7),
        ] {
            self.query_store(p, name, offset, source)?;
        }
        self.embedded_label(p, text, 0x388, 0x6f0, 4, 0x5c9)?;
        self.embedded_label(p, text, 0x53c, 0x718, 3, 0x5ca)?;
        let identifier = 0x5cb;
        #[cfg(test)]
        let identifier = self.fault_first_identifier.unwrap_or(identifier);
        self.visual(
            p,
            text,
            images,
            0,
            read_extent(p, 0x740)?,
            [Some(0x52), Some(0x53), Some(0x54), None],
            Some(identifier),
            &[],
        )?;
        self.visual(
            p,
            text,
            images,
            2,
            read_extent(p, 0x768)?,
            [Some(0x55), Some(0x56), Some(0x57), None],
            Some(0x6e1),
            &[],
        )?;
        for (key, offset) in [(3, 0x77c), (4, 0x790), (5, 0x7a4), (6, 0x7b8), (7, 0x7cc)] {
            let literal = p.cells().ntbs(0x5607d0, 1)?;
            self.visual(
                p,
                text,
                images,
                key,
                read_extent(p, offset)?,
                [None; 4],
                None,
                &literal,
            )?;
        }
        let input = self.make_input(p, read_extent(p, 0x72c)?)?;
        self.publish(0x808, input)?;
        {
            let Subtype::Input(i) = &mut self.node_mut(input)?.subtype else {
                unreachable!()
            };
            i.width_limit = 230;
            i.char_limit = 215;
            i.enabled = 1;
        }
        #[cfg(test)]
        if self.fault_final_notify {
            //Negative manager-state injection at the actual last-font cut.
            //Forces cache miss, current store then real unsupported notify.
            let f = text.fonts_mut();
            f.current = 0;
            f.selector.cached = 0;
            f.selector.pending = [0, 0];
            f.selector.notify = 7;
        }
        self.text_call(Some(input), 0, 0x440b70);
        self.node_mut(input)?
            .text_mut()
            .set_font440b70_retained(text.fonts_mut(), 3)?;
        //The real source writes its static process alias AFTER the last Font3.
        p.chat_cells()?.set_word(0x86d494, SOURCE)?;
        self.effects.push(Effect::ProcessAlias);
        self.phase = Phase::Returned;
        self.effects.push(Effect::Returned);
        Ok(())
    }
}
fn bounds(e: [f32; 5]) -> [f32; 4] {
    e[..4].try_into().unwrap()
}
fn read_extent(p: &Primitives, offset: u32) -> Result<[f32; 5], String> {
    let mut e = [0.; 5];
    for (i, v) in e.iter_mut().enumerate() {
        *v = p.cells().float(SOURCE + offset + 4 * i as u32)?;
    }
    Ok(e)
}
