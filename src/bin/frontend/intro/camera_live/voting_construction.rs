//! Concrete439df0/42c000 retained Voting owner. No election/render/input owner.
//! Static Cells own all native bounds/preimages. Typed aliases never invent
//! native heap pointers. Root caption remains in Primitives' SAME Text map.
use super::{case_zero_text::{Text,Fonts},main_construction::Field,
    primitive_factory::{Primitives,Step},process_images::Images,
    ui_text_resources::UiTextResources};
use std::{collections::BTreeMap,sync::atomic::{AtomicU64,Ordering}};
use openmt2001::x87::Sum;
pub const SOURCE:u32=0x573b38;
static NEXT:AtomicU64=AtomicU64::new(1);
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]
pub struct NodeId{owner:u64,slot:usize}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Phase{StaticPending,Unbound,Binding,LayingOut,Returned,Registered,Failed}
///These labels name source storage, never real participants or a voter roster.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Label{Ok,Percentage(usize),Count(usize),TitleLabel,Title}
pub use super::visual_construction::Visual;
use super::visual_construction::ImageRef;
pub enum Subtype{Text(Text),Visual(Visual)}
pub struct Node{
    pub id:NodeId,pub label:Label,pub allocation_size:u32,
    ///Text geometry lives only in Subtype::Text; Visual has separate outer
    ///geometry and its genuinely embedded caption geometry.
    pub visual_outer:Option<([f32;4],f32)>,
    pub field18:u32,pub shown:u8,pub disabled:u8,pub name:[u8;255],pub padding1e:Field<[u8;2]>,
    pub event_sink:u32,pub frame:u32,pub next:Option<NodeId>,pub number:Option<u32>,
    pub animation34:u32,pub completion38:u8,pub subtype:Subtype,
}
impl Node{
    pub fn bounds(&self)->[f32;4]{match &self.subtype{Subtype::Text(t)=>t.bounds,Subtype::Visual(_)=>self.visual_outer.expect("Visual outer birth").0}}
    pub fn depth(&self)->f32{match &self.subtype{Subtype::Text(t)=>t.depth,Subtype::Visual(_)=>self.visual_outer.expect("Visual outer birth").1}}
}
#[derive(Clone,Debug,PartialEq)]
pub enum Effect{
    Query{source:u32,name:u32},Allocated{source:u32,node:NodeId},
    Slot04{node:NodeId,target:u32},Number{node:NodeId,key:u32},Append(NodeId),
    Alias{offset:u32,node:NodeId},TextCall{source:u32,node:NodeId,target:u32},
    Scalar{source:u32,offset:u32,value:u32},Returned,
}
///Move-stable and retained before ANY reached birth/bind/layout effect.
///There is no success-only temporary product and no reconstruction on error.
pub struct VotingProduct{
    pub source:u32,pub caption_source:u32,pub(super) phase:Phase,pub failure:Option<String>,
    pub static_preimage:Vec<u8>,owner:u64,process:super::main_construction::ResourceIdentity,
    nodes:Vec<Box<Node>>,children:Vec<NodeId>,aliases:BTreeMap<u32,NodeId>,effects:Vec<Effect>,
}
impl VotingProduct{
    pub(super) fn allocated(p:&Primitives)->Result<Box<Self>,String>{
        let preimage=p.cells().read(SOURCE,0x5c8)?.to_vec();
        let owner=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|v|v.checked_add(1)).map_err(|_|"Voting owner IDs exhausted")?;
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
        if id.owner!=self.owner{return Err("Voting node belongs to another retained product".into())}
        self.nodes.get(id.slot).map(Box::as_ref).ok_or_else(||"Voting node absent".into())
    }
    fn node_mut(&mut self,id:NodeId)->Result<&mut Node,String>{
        if id.owner!=self.owner{return Err("Voting node belongs to another retained product".into())}
        self.nodes.get_mut(id.slot).map(Box::as_mut).ok_or_else(||"Voting node absent".into())
    }
    pub fn text(&self,id:NodeId)->Result<&Text,String>{match &self.node(id)?.subtype{Subtype::Text(t)=>Ok(t),Subtype::Visual(v)=>Ok(&v.caption)}}
    fn text_mut(&mut self,id:NodeId)->Result<&mut Text,String>{match &mut self.node_mut(id)?.subtype{Subtype::Text(t)=>Ok(t),Subtype::Visual(v)=>Ok(&mut v.caption)}}
    pub fn visual(&self,id:NodeId)->Result<&Visual,String>{match &self.node(id)?.subtype{Subtype::Visual(v)=>Ok(v),_=>Err("Voting node is not Visual".into())}}
    fn visual_mut(&mut self,id:NodeId)->Result<&mut Visual,String>{match &mut self.node_mut(id)?.subtype{Subtype::Visual(v)=>Ok(v),_=>Err("Voting node is not Visual".into())}}
    fn allocate(&mut self,p:&Primitives,label:Label,e:[f32;5],subtype:Subtype,source:u32)->Result<NodeId,String>{
        //43e9a0/43ed20 copy parent.frame if nonnull, else parent itself.
        let frame=p.cells().word(SOURCE+0x24)?;let frame=if frame==0{SOURCE}else{frame};
        let allocation_size=match &subtype{Subtype::Text(_)=>0x1b4,Subtype::Visual(_)=>0x374};
        let visual_outer=if matches!(&subtype,Subtype::Visual(_)){Some((bounds(e),0.5))}else{None};
        let id=NodeId{owner:self.owner,slot:self.nodes.len()};
        self.nodes.push(Box::new(Node{id,label,allocation_size,visual_outer,field18:1,shown:1,disabled:0,
            name:[0;255],padding1e:Field::Unknown,event_sink:SOURCE,frame,next:None,number:None,
            animation34:0,completion38:0,subtype}));
        self.effects.push(Effect::Allocated{source,node:id});Ok(id)
    }
    fn append_returned(&mut self,id:NodeId,key:u32,target:u32)->Result<(),String>{
        self.effects.push(Effect::Slot04{node:id,target});
        self.node_mut(id)?.number=Some(key);self.effects.push(Effect::Number{node:id,key});
        if let Some(tail)=self.children.last().copied(){self.node_mut(tail)?.next=Some(id);}
        self.children.push(id);self.effects.push(Effect::Append(id));Ok(())
    }
    fn publish(&mut self,offset:u32,id:NodeId)->Result<(),String>{
        self.node(id)?;
        if self.aliases.contains_key(&offset){return Err("Voting alias already published; no replay".into())}
        self.aliases.insert(offset,id);self.effects.push(Effect::Alias{offset,node:id});Ok(())
    }
    fn make_text(&mut self,p:&Primitives,label:Label,e:[f32;5],key:u32,source:u32)->Result<NodeId,String>{
        let mut text=Text::normal_fields();text.bounds=bounds(e);
        let id=self.allocate(p,label,e,Subtype::Text(text),source)?;
        //Normal factory concrete42f140 is RET; never440900 on this path.
        self.append_returned(id,key,0x42f140)?;Ok(id)
    }
    fn make_visual(&mut self,p:&Primitives,e:[f32;5])->Result<NodeId,String>{
        let id=self.allocate(p,Label::Ok,e,Subtype::Visual(Visual::normal_fields()),0x42c13d)?;
        self.effects.push(Effect::TextCall{source:0x44267c,node:id,target:0x440900});
        self.visual_mut(id)?.initialize442600(bounds(e))?;
        self.append_returned(id,0,0x442600)?;Ok(id)
    }
    fn query(&mut self,p:&mut Primitives,name:u32,source:u32)->Result<[f32;5],String>{
        self.effects.push(Effect::Query{source,name});p.voting_named(name)
    }
    fn named_store(&mut self,p:&mut Primitives,name:u32,offset:u32,source:u32)->Result<[f32;5],String>{
        let e=self.query(p,name,source)?;write_extent(p,offset,e)?;Ok(e)
    }
    fn style(&mut self,id:NodeId,source:u32)->Result<(),String>{
        self.effects.push(Effect::TextCall{source,node:id,target:0x440650});
        self.text_mut(id)?.style440650(4,-999,-999);Ok(())
    }
    fn font(&mut self,id:NodeId,fonts:&mut Fonts,font:u32,source:u32)->Result<(),String>{
        self.effects.push(Effect::TextCall{source,node:id,target:0x440b70});
        self.text_mut(id)?.set_font(fonts,font)
    }
    fn identifier(&mut self,id:NodeId,fonts:&mut Fonts,identifier:u32,source:u32)->Result<(),String>{
        self.effects.push(Effect::TextCall{source,node:id,target:0x441220});
        //Native44124d selects the born Text font before4412ab consumes
        //packed glyphs. Retain scale/line-mode/width effects on refusal.
        self.text_mut(id)?.identifier441220_unpadded(fonts,identifier)
    }
    fn literal(&mut self,id:NodeId,fonts:&mut Fonts,format:&[u8],source:u32)->Result<(),String>{
        self.effects.push(Effect::TextCall{source,node:id,target:0x441770});
        self.text_mut(id)?.literal(fonts,format,&[])
    }
    ///Only PrimaryConstruction's concrete Install continuation calls this.
    pub(super) fn layout(&mut self,p:&mut Primitives,text:&mut UiTextResources,images:&Images)->Result<(),String>{
        if self.phase!=Phase::Binding{return Err("Voting body already reached or birth incomplete; no replay".into())}
        self.phase=Phase::LayingOut;p.steps.push(Step::Layout(SOURCE));
        self.named_store(p,0x535854,0x388,0x42c021)?;
        self.named_store(p,0x535840,0x39c,0x42c04d)?;
        for i in 0..6u32{
            let name=p.cells().word(0x53570c+4*i)?;self.named_store(p,name,0x3b0+20*i,0x42c08a)?;
            let name=p.cells().word(0x535724+4*i)?;self.named_store(p,name,0x43c+20*i,0x42c0bc)?;
        }
        self.named_store(p,0x53582c,0x4c8,0x42c0fa)?;
        let e=self.query(p,0x535818,0x42c126)?;
        let ok=self.make_visual(p,e)?;self.publish(0x4dc,ok)?;
        //Actual read order, then actual write order. No dimension/scales call.
        let a=images.id(0x68af68)?;let b=images.id(0x68b064)?;
        let c=images.id(0x68af6c)?;let d=images.id(0x68af64)?;
        {let v=self.visual_mut(ok)?;
            for (offset,image) in [(0x304,ImageRef::Image(c)),(0x300,ImageRef::Image(d)),(0x348,ImageRef::Image(d)),(0x308,ImageRef::Image(b)),(0x30c,ImageRef::Image(a)),(0x310,ImageRef::Invalid)]{v.images.insert(offset,image);}
            v.scalars.insert(0x13c,3);v.scalars.insert(0x33c,0); //442ab0
        }
        self.font(ok,text.fonts_mut(),2,0x42c1ab)?;self.style(ok,0x42c1c8)?;
        self.effects.push(Effect::TextCall{source:0x42c1d9,node:ok,target:0x441940});
        self.text_mut(ok)?.reset(text.fonts_mut())?;
        self.identifier(ok,text.fonts_mut(),0x59b,0x42c1ef)?;
        for i in 0..6u32{
            let r=read_extent(p,0x43c+20*i)?;let graph_bottom=p.cells().float(SOURCE+0x4d4)?;
            let placement=0x4f8+20*i;
            {let c=p.voting_cells()?;
                c.set_word(SOURCE+placement,r[0].to_bits())?;
                c.set_word(SOURCE+placement+8,r[2].to_bits())?;
                c.set_word(SOURCE+placement+12,graph_bottom.to_bits())?;
                c.set_word(SOURCE+placement+4,graph_bottom.to_bits())?;
            }
            //Explicit x87 operation -> binary32 store at each source cut.
            let e=[add(r[0],p.cells().float(0x52516c)?)?,graph_bottom,r[2],
                add(graph_bottom,p.cells().float(0x5250a4)?)?,0.5];
            let percent=self.make_text(p,Label::Percentage(i as usize),e,1+2*i,0x42c272)?;
            self.publish(0x588+4*i,percent)?;
            let format=p.cells().ntbs(0x535814,32)?;
            self.literal(percent,text.fonts_mut(),&format,0x42c289)?;
            self.style(percent,0x42c2a0)?;self.font(percent,text.fonts_mut(),0,0x42c2aa)?;
            let count=self.make_text(p,Label::Count(i as usize),r,2+2*i,0x42c2c8)?;
            self.publish(0x5a4+4*i,count)?;
            let format=p.cells().ntbs(0x535810,32)?;
            self.literal(count,text.fonts_mut(),&format,0x42c2de)?;
            self.style(count,0x42c2f4)?;self.font(count,text.fonts_mut(),1,0x42c2fd)?;
            //Direct Node rectangle stores, NOT extent440900; leave visible64.
            for (axis,source) in [(1,0x42c31a),(3,0x42c326)]{
                let old=self.text(count)?.bounds[axis];let value=sub(old,p.cells().float(0x52501c)?)?;
                self.text_mut(count)?.bounds[axis]=value;
                self.effects.push(Effect::Scalar{source,offset:4+axis as u32*4,value:value.to_bits()});
            }
        }
        let e=self.named_store(p,0x5357fc,0x4e4,0x42c34b)?;
        let label=self.make_text(p,Label::TitleLabel,e,13,0x42c372)?;self.publish(0x5c0,label)?;
        self.style(label,0x42c38b)?;self.font(label,text.fonts_mut(),3,0x42c398)?;
        let e=self.query(p,0x535854,0x42c3b5)?;
        let title=self.make_text(p,Label::Title,e,14,0x42c3ca)?;self.publish(0x5c4,title)?;
        self.identifier(title,text.fonts_mut(),0x603,0x42c3dc)?;
        self.style(title,0x42c3f3)?;self.font(title,text.fonts_mut(),1,0x42c400)?;
        p.voting_cells()?.set_word(SOURCE+0x584,u32::MAX)?;
        self.effects.push(Effect::Scalar{source:0x42c407,offset:0x584,value:u32::MAX});
        p.voting_cells()?.set_word(SOURCE+0x350,1)?;
        self.effects.push(Effect::Scalar{source:0x42c411,offset:0x350,value:1});
        self.phase=Phase::Returned;self.effects.push(Effect::Returned);Ok(())
    }
}
fn bounds(e:[f32;5])->[f32;4]{e[..4].try_into().unwrap()}
fn write_extent(p:&mut Primitives,offset:u32,e:[f32;5])->Result<(),String>{
    let c=p.voting_cells()?;for (i,v) in e.into_iter().enumerate(){c.set_word(SOURCE+offset+i as u32*4,v.to_bits())?;}Ok(())
}
fn read_extent(p:&Primitives,offset:u32)->Result<[f32;5],String>{
    let mut e=[0.;5];for (i,v) in e.iter_mut().enumerate(){*v=p.cells().float(SOURCE+offset+i as u32*4)?;}Ok(e)
}
fn add(a:f32,b:f32)->Result<f32,String>{
    let mut sum=Sum::from_f32(a).map_err(str::to_owned)?;sum.add_f32(b).map_err(str::to_owned)?;store(sum)
}
fn sub(a:f32,b:f32)->Result<f32,String>{
    let mut sum=Sum::from_f32(a).map_err(str::to_owned)?;sum.add(Sum::from_f32(b).map_err(str::to_owned)?.negated());store(sum)
}
fn store(sum:Sum)->Result<f32,String>{let v=sum.to_f32();if v.is_finite(){Ok(v)}else{Err("Voting layout left finite binary32 store domain".into())}}
