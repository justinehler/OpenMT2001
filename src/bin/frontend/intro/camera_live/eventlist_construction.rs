//! Retained normal EventList439ce0/41cdc0 product, not its later renderer.
//! One private Text per child; SAME root caption is retained in Primitives.
use super::{case_zero_text::{Text,Group},main_construction::{Field,ResourceIdentity},
    primitive_factory::{Primitives,Step},ui_text_resources::UiTextResources};
use std::{collections::BTreeMap,sync::atomic::{AtomicU64,Ordering}};
use openmt2001::x87::Sum;
pub const SOURCE:u32=0x571378;
static NEXT:AtomicU64=AtomicU64::new(1);
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]
pub struct NodeId{owner:u64,slot:usize}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Phase{StaticPending,Unbound,Binding,LayingOut,Returned,Registered,Failed}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Label{Green(usize),Body(usize),Title}
enum PackedId{Word(u32),Immediate(u32)}
///Read-only unaligned fields. There is deliberately no Text/layout/render
///escape: a convenience zero or shortened Text.x is not native capacity.
pub struct TextView<'a>{
    pub bounds:[f32;4],pub depth:f32,pub font:u32,pub pitch:i32,pub gap:i8,
    pub visible:u32,pub line_mode:u8,pub scale:f32,pub padding:i32,
    pub group:&'a Group,pub colors:&'a [u8],pub blink_colors:&'a [u8],pub blink:&'a [u8],
    ///Length is retained allocation capacity. None is unknown heap content;
    ///existing Some prefixes survive growth. No source position writer ran.
    pub x:&'a [Option<i16>],pub extent:[f32;4],pub y:i16,pub inset:i32,pub scroll:i32,
    pub style:u32,pub first:u32,pub dirty:u8,pub geometry_dirty:u8,
    pub background_color:u32,pub background:u8,
}
pub struct Node{
    pub id:NodeId,pub label:Label,pub allocation_size:u32,
    pub field18:u32,pub shown:u8,pub disabled:u8,pub name:[u8;255],pub padding1e:Field<[u8;2]>,
    pub parent:u32,pub root:u32,pub next:Option<NodeId>,pub number:Option<u32>,
    pub animation34:u32,pub completion38:u8,text:Text,x:Vec<Option<i16>>,
}
impl Node{
    pub fn text(&self)->TextView<'_>{
        let t=&self.text;
        TextView{bounds:t.bounds,depth:t.depth,font:t.font,pitch:t.pitch,gap:t.gap,
            visible:t.visible,line_mode:t.line_mode,scale:t.scale,padding:t.padding,
            group:&t.group,colors:&t.colors,blink_colors:&t.blink_colors,blink:&t.blink,
            x:&self.x,extent:t.extent,y:t.y,inset:t.inset,scroll:t.scroll,style:t.style,
            first:t.first,dirty:t.dirty,geometry_dirty:t.geometry_dirty,
            background_color:t.background_color,background:t.background}
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Effect{
    Query{source:u32,name:u32},Allocated{source:u32,node:NodeId},
    Slot04{node:NodeId,target:u32},Number{node:NodeId,key:u32},Append(NodeId),
    Alias{source:u32,offset:u32,node:NodeId},TextCall{source:u32,node:NodeId,target:u32},
    LeftStore{source:u32,offset:u32,word:u32},Returned,
}
///Allocated before reached static birth; boxes/typed IDs survive wrapper moves
///and Session changes. Cells own extents; this product owns typed heap links.
pub struct EventListProduct{
    pub source:u32,pub caption_source:u32,pub(super) phase:Phase,pub failure:Option<String>,
    pub static_preimage:Vec<u8>,owner:u64,process:ResourceIdentity,
    nodes:Vec<Box<Node>>,children:Vec<NodeId>,aliases:BTreeMap<u32,NodeId>,effects:Vec<Effect>,
}
impl EventListProduct{
    pub(super) fn allocated(p:&Primitives)->Result<Box<Self>,String>{
        let preimage=p.cells().read(SOURCE,0x490)?.to_vec();
        let owner=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|v|v.checked_add(1)).map_err(|_|"EventList owner IDs exhausted")?;
        Ok(Box::new(Self{source:SOURCE,caption_source:SOURCE+0x188,phase:Phase::StaticPending,failure:None,
            static_preimage:preimage,owner,process:p.construction_identity.clone(),nodes:Vec::new(),children:Vec::new(),aliases:BTreeMap::new(),effects:Vec::new()}))
    }
    pub(super) fn same_process(&self,p:&Primitives)->bool{self.process.same(&p.construction_identity)}
    pub(super) fn fail(&mut self,error:String){self.phase=Phase::Failed;self.failure=Some(error);}
    pub fn phase(&self)->Phase{self.phase}
    pub fn children(&self)->&[NodeId]{&self.children}
    pub fn nodes(&self)->impl Iterator<Item=&Node>{self.nodes.iter().map(Box::as_ref)}
    pub fn effects(&self)->&[Effect]{&self.effects}
    pub fn alias(&self,offset:u32)->Option<NodeId>{self.aliases.get(&offset).copied()}
    pub fn node(&self,id:NodeId)->Result<&Node,String>{
        if id.owner!=self.owner{return Err("EventList child belongs to another retained owner".into())}
        self.nodes.get(id.slot).map(Box::as_ref).ok_or_else(||"EventList child absent".into())
    }
    fn node_mut(&mut self,id:NodeId)->Result<&mut Node,String>{
        if id.owner!=self.owner{return Err("EventList child belongs to another retained owner".into())}
        self.nodes.get_mut(id.slot).map(Box::as_mut).ok_or_else(||"EventList child absent".into())
    }
    fn make_text(&mut self,p:&Primitives,label:Label,e:[f32;5],key:u32,source:u32)->Result<NodeId,String>{
        //Finite successful440160/440540 fields; old heap pitch/status and
        //allocator unwind remain explicit lower-layer domains, not zeroed.
        let mut text=Text::normal_fields();
        let root=p.cells().word(SOURCE+0x24)?;let root=if root==0{SOURCE}else{root};
        text.bounds=e[..4].try_into().unwrap(); //wrapper ignores extent depth
        let id=NodeId{owner:self.owner,slot:self.nodes.len()};
        self.nodes.push(Box::new(Node{id,label,allocation_size:0x1b4,
            field18:1,shown:1,disabled:0,name:[0;255],padding1e:Field::Unknown,
            parent:SOURCE,root,next:None,number:None,animation34:0,completion38:0,text,x:Vec::new()}));
        self.effects.push(Effect::Allocated{source,node:id});
        //Concrete Text+04=42f140 RET precedes number/append. Never440900.
        self.effects.push(Effect::Slot04{node:id,target:0x42f140});
        self.node_mut(id)?.number=Some(key);self.effects.push(Effect::Number{node:id,key});
        if let Some(tail)=self.children.last().copied(){self.node_mut(tail)?.next=Some(id);}
        self.children.push(id);self.effects.push(Effect::Append(id));Ok(id)
    }
    fn publish(&mut self,offset:u32,id:NodeId,source:u32)->Result<(),String>{
        self.node(id)?;if self.aliases.contains_key(&offset){return Err("EventList alias already produced; no replay".into())}
        self.aliases.insert(offset,id);self.effects.push(Effect::Alias{source,offset,node:id});Ok(())
    }
    fn query_store(&mut self,p:&mut Primitives,name:u32,offset:u32,source:u32)->Result<(),String>{
        self.effects.push(Effect::Query{source,name});let e=p.eventlist_named(name)?;
        let c=p.eventlist_cells()?;for (i,v) in e.into_iter().enumerate(){c.set_word(SOURCE+offset+4*i as u32,v.to_bits())?;}Ok(())
    }
    fn configure(&mut self,id:NodeId,p:&Primitives,text:&mut UiTextResources,identifier:PackedId,style:u32,sources:[u32;3])->Result<(),String>{
        self.effects.push(Effect::TextCall{source:sources[0],node:id,target:0x440b70});
        self.node_mut(id)?.text.set_font440b70_retained(text.fonts_mut(),3)?;
        let identifier=match identifier{PackedId::Word(at)=>p.cells().word(at)?,PackedId::Immediate(value)=>value};
        self.effects.push(Effect::TextCall{source:sources[1],node:id,target:0x441220});
        let n=self.node_mut(id)?;n.text.identifier441220_unaligned(text.fonts_mut(),identifier,&mut n.x)?;
        self.effects.push(Effect::TextCall{source:sources[2],node:id,target:0x440650});
        self.node_mut(id)?.text.style440650(style,-999,-999);Ok(())
    }
    pub(super) fn layout(&mut self,p:&mut Primitives,text:&mut UiTextResources)->Result<(),String>{
        if self.phase!=Phase::Binding||!self.same_process(p){return Err("EventList body outside SAME bound owner; no replay".into())}
        self.phase=Phase::LayingOut;p.steps.push(Step::Layout(SOURCE));
        self.query_store(p,0x534af8,0x388,0x41cde7)?;
        for i in 0..5u32{
            let name=p.cells().word(0x534a08+4*i)?;self.query_store(p,name,0x39c+20*i,0x41ce37)?;
            let name=p.cells().word(0x534a1c+4*i)?;self.query_store(p,name,0x400+20*i,0x41ce63)?;
            let green=self.make_text(p,Label::Green(i as usize),read_extent(p,0x39c+20*i)?,2*i,0x41ce8d)?;
            self.publish(0x468+4*i,green,0x41ce96)?;
            self.configure(green,p,text,PackedId::Word(0x534a30+4*i),3,[0x41ce99,0x41ceac,0x41cec0])?;
            let offset=0x400+20*i;
            let mut left=Sum::from_f32(p.cells().float(SOURCE+offset)?).map_err(str::to_owned)?;
            left.add_f32(p.cells().float(0x52516c)?).map_err(str::to_owned)?;
            let value=left.to_f32();if !value.is_finite(){return Err("EventList inset left leaves finite binary32 domain".into())}
            p.eventlist_cells()?.set_word(SOURCE+offset,value.to_bits())?;
            self.effects.push(Effect::LeftStore{source:0x41cee0,offset,word:value.to_bits()});
            let body=self.make_text(p,Label::Body(i as usize),read_extent(p,offset)?,2*i+1,0x41ceeb)?;
            self.publish(0x47c+4*i,body,0x41cef4)?;
            self.configure(body,p,text,PackedId::Word(0x534a44+4*i),3,[0x41cef7,0x41cf0a,0x41cf1e])?;
        }
        let title=self.make_text(p,Label::Title,read_extent(p,0x388)?,10,0x41cf5f)?;
        self.publish(0x464,title,0x41cf68)?;
        self.configure(title,p,text,PackedId::Immediate(1501),4,[0x41cf6e,0x41cf7e,0x41cf95])?;
        self.phase=Phase::Returned;self.effects.push(Effect::Returned);Ok(())
    }
}
fn read_extent(p:&Primitives,offset:u32)->Result<[f32;5],String>{
    let mut e=[0.;5];for (i,v) in e.iter_mut().enumerate(){*v=p.cells().float(SOURCE+offset+4*i as u32)?;}Ok(e)
}
