//! Actual containing child1 401000/4055f0. Stops at parent41e725.
//! No FullMain, group registration, selected report/provider or input activation.
use super::*;
use super::super::{native_initializers,property_child::finite};
use std::ops::Deref;
const CHILD:u32=0x560820;

///430790 is a separate concrete subtype. Its common column algorithm backing
///uses List; that backing's normal-only flag67c is not a special native field.
///Extra native658 selection flags and680/681 bytes belong to this subtype.
pub struct SpecialList {
    pub list:List<NodeId>,pub indicator:Option<NodeId>,
    pub selection658:Vec<u8>,pub selectable680:u8,pub flag681:u8,
}
impl SpecialList {
    pub fn range(&self)->super::super::property_child::ListRange{self.list.range()}
}
pub struct Amount {
    pub dirty13c:u8,pub value140:f32,pub step144:f32,pub minimum148:f32,pub maximum14c:f32,
    pub first150:Field<NodeId>,pub second154:Field<NodeId>,
}
impl Amount {
    fn birth()->Self{Self{dirty13c:0,value140:0.,step144:1.,minimum148:f32::from_bits(0xc9742400),maximum14c:f32::from_bits(0x49742400),first150:Field::Unknown,second154:Field::Unknown}}
    ///42efb0 does not clamp, round or set dirty13c.
    pub fn range42efb0(&mut self,step:f32,minimum:f32,maximum:f32){self.minimum148=minimum;self.maximum14c=maximum;self.step144=step;}
    ///42ee70 is an unconditional dirty store and raw binary32 replacement.
    pub fn amount42ee70(&mut self,value:f32){self.dirty13c=1;self.value140=value;}
}
///All scalar bytes remain in the actual static Cells allocation. Pointers to
///modern nodes are typed aliases here, never fabricated native heap addresses.
pub struct ChildOne {
    pub source:u32,pub parent:NodeId,pub fields:BTreeMap<u32,NodeId>,
    pub global_aliases:BTreeMap<u32,NodeId>,pub static_preimage:Vec<u8>,
    pub native_return:i32,
}
impl ChildOne {
    pub fn field(&self,offset:u32)->Result<NodeId,String>{self.fields.get(&offset).copied().ok_or_else(||format!("child1+{offset:x} actual product absent"))}
    pub fn global(&self,address:u32)->Option<NodeId>{self.global_aliases.get(&address).copied()}
    pub fn require_selected_reports(&self)->Result<(),String>{Err("selected report population, linked-value/attribute creators and rights republication remain unavailable".into())}
}
///Move-only product of the SAME consumed prefix. Deref exposes retained nodes
///and child0 without rebuilding/copying any model, Text, list, font or image.
pub struct AwaitingChild5 {prefix:AwaitingOtherMainChildren}
impl Deref for AwaitingChild5 {type Target=AwaitingOtherMainChildren;fn deref(&self)->&Self::Target{&self.prefix}}
impl AwaitingChild5 {
    pub fn phase(&self)->MainPhase{self.prefix.phase()}
    pub fn child1(&self)->&ChildOne{self.prefix.inner.child1.as_ref().expect("successful4055f0 owns child1")}
    pub fn require_full_main(&self)->Result<(),String>{Err("41e725 incomplete: child5+394/409ec0, child2+398/408da0, parent tail, switch1,43bed0,184,43c600 and41eb40 remain".into())}
    ///Checks phase and all allocation owners BEFORE moving the prefix. A
    ///wrong/duplicate continuation changes nothing; a reached failure retains
    ///the same failed candidate in its slot and cannot replay construction.
    pub fn continue_child1(slot:&mut Option<AwaitingOtherMainChildren>,executable:&[u8],
        text:&mut UiTextResources,p:&mut Primitives,images:&mut Images)->Result<Self,String>{
        let prefix=slot.as_ref().ok_or("4055f0 prefix already consumed; no duplicate continuation")?;
        p.check_main_property_owner(prefix.inner.owner)?;
        if !prefix.inner.text_identity.same(&text.construction_identity)||!prefix.inner.image_identity.same(&images.construction_identity){
            return Err("4055f0 requires SAME retained Fonts/packed-language and GenericTextures640 owners".into())
        }
        let pe=PeImage::parse(executable)?;
        //Use this prefix's actual source image, not a new static/default world.
        //Validate the reached code and constant/table bytes against that image.
        for (at,n) in [(0x401000,0x53f0),(0x41f120,0x60),(0x41f510,0x240),(0x42ebe0,0x560),(0x430790,0x250),(0x430b10,0x20),(0x431290,0xa0),(0x431e80,0x30),(0x43ec30,0xf0),(0x43f800,0x200),(0x442960,0xc0),(0x52501c,0xf8),(0x525290,52),(0x525308,52),(0x534d70,20),(0x53322c,12)]{
            if pe.at(at,n)?!=p.cells().read(at,n)?{return Err(format!("4055f0 source{at:x} differs from retained prefix PE"))}
        }
        for (at,target) in [(0x525308+4,0x431290),(0x525290+4,0x42f140),(0x525908+0x2c,0x43b370)]{
            if word(&pe,at)?!=target{return Err("4055f0 required concrete subtype dispatch differs".into())}
        }
        if images.ui_dimensions!=[640,480]{return Err("4055f0 retained640 image domain changed".into())}
        //822d50 is zero-filled .data virtual tail at actual process birth.
        //No reached prefix constructor writes it. The guard reads that same
        //retained source each time; selected contexts never enter this domain.
        if p.cells().word(0x822d50)?!=0{return Err("4055f0 selected822d50 context unsupported before continuation".into())}
        if p.cells().word(A+0x390)?!=CHILD{return Err("4055f0 actual parent+390 static child identity differs".into())}
        p.claim_main_property(prefix.inner.owner)?;
        slot.as_mut().unwrap().inner.phase=MainPhase::ConstructingProperty;
        let result=slot.as_mut().unwrap().inner.property_layout(&pe,p,text.fonts_mut(),images)
            .map_err(|e|format!("401000/4055f0 actual child1: {e}"));
        match result {
            Ok(())=>{p.main_complete_property();slot.as_mut().unwrap().inner.phase=MainPhase::AwaitingChild5;Ok(Self{prefix:slot.take().expect("checked retained prefix")})},
            Err(error)=>{p.main_failed();slot.as_mut().unwrap().inner.phase=MainPhase::Failed;Err(error)},
        }
    }
}
fn ftol_added(a:f32,b:f32)->Result<i32,String>{let mut value=sum(a)?;value.add_f32(b).map_err(str::to_owned)?;ftol(value)}
fn minus(a:f32,b:f32)->Result<f32,String>{store(sub(a,b)?)}
fn integer(v:i32)->Result<f32,String>{store(Sum::from_i32(v))}
fn unsigned(v:u32)->Result<f32,String>{store(Sum::from_f64(f64::from(v)).map_err(str::to_owned)?)}
impl Product {
    fn child_field(&self,offset:u32)->Result<NodeId,String>{self.child1.as_ref().ok_or("child1 birth absent")?.field(offset)}
    fn publish_child(&mut self,offset:u32,node:NodeId,source:u32)->Result<(),String>{
        self.node(node)?;
        if self.child1.as_mut().ok_or("child1 birth absent")?.fields.insert(offset,node).is_some(){return Err("child1 field published twice".into())}
        self.record(source,node,Event::Alias(offset));Ok(())
    }
    fn publish_global(&mut self,address:u32,node:NodeId,source:u32)->Result<(),String>{
        self.node(node)?;
        if self.child1.as_mut().ok_or("child1 birth absent")?.global_aliases.insert(address,node).is_some(){return Err("child1 global alias published twice".into())}
        self.record(source,node,Event::Alias(address));Ok(())
    }
    fn columns_mut(&mut self,id:NodeId)->Result<&mut List<NodeId>,String>{match &mut self.node_mut(id)?.subtype{
        Subtype::List3Texts(l)=>Ok(&mut l.list),Subtype::SpecialList3Texts(l)=>Ok(&mut l.list),
        _=>Err("property column operation requires concrete ordinary/special List".into()),
    }}
    fn configure_columns(&mut self,id:NodeId,fonts:&mut Fonts,source:u32)->Result<(),String>{
        let l=self.columns_mut(id)?;
        l.primary.style440650(3,0,0);l.primary.padding=1;
        l.primary.layout(fonts)?; //4406a0 direct, never4409c0
        l.secondary.style440650(2,3,3);l.primary.gap=2;l.secondary.gap=2;
        l.primary.set_font(fonts,3)?;l.secondary.set_font(fonts,3)?;
        for call in [0x440650,0x4406a0,0x440650,0x440b70,0x440b70]{self.record(source,id,Event::TextCall(call));}Ok(())
    }
    fn make_columns(&mut self,offset:u32,e:[f32;5],special:bool,fonts:&mut Fonts,source:u32)->Result<NodeId,String>{
        let id=self.make_column_node(Label::Property(offset),e,special,fonts,source)?;
        self.publish_child(offset,id,source)?;self.configure_columns(id,fonts,source)?;Ok(id)
    }
    fn make_column_node(&mut self,label:Label,e:[f32;5],special:bool,fonts:&mut Fonts,source:u32)->Result<NodeId,String>{
        let family=if special{Family::SpecialList3Texts}else{Family::List3Texts};
        let id=self.allocate(label,family,bounds(e),Subtype::ConstructingList,source);
        self.record(source,id,Event::Constructor(if special{0x430790}else{0x42f3d0}));
        for _ in 0..3{self.record(source,id,Event::Constructor(0x440160));}
        self.bind_node(id,self.root)?;
        //Both actual slot04s set all three Text extents, clear indicator, set
        //node18=0 and perform the same column/row reset at capacity0. The
        //special-only658 flag allocation remains truly empty at this birth.
        let list={let n=self.node_mut(id)?;ListBirth::new(id).layout(bounds(e),fonts,&mut n.field18)}?;
        self.node_mut(id)?.subtype=if special{Subtype::SpecialList3Texts(SpecialList{list,indicator:None,selection658:Vec::new(),selectable680:0,flag681:0})}
            else{Subtype::List3Texts(ListProduct{list,indicator:None})};
        for _ in 0..3{self.record(source,id,Event::TextCall(0x440900));}
        self.record(source,id,Event::TextCall(if special{0x4308d0}else{0x42f510}));
        self.append_bound(id,self.root,self.counter,if special{0x431290}else{0x42fdf0},source)?;
        //Containing callers publish aliases/configure/increment at their exact boundary.
        Ok(id)
    }
    ///41f510 no-title branch, selector output present. Unlike41f750 this
    ///constructs actual430790 special list, including680 selection policy.
    fn property_pair(&mut self,list_offset:u32,selector_offset:u32,y:i32,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,images:&mut Images,source:u32)->Result<(),String>{
        let top=unsigned(y as u32)?;let right=minus(self.main_window[2],self.scrollbar_width)?;
        let bottom=minus(self.main_window[3],float(pe,0x525028)?)?;
        let e=[self.main_window[0],top,right,bottom,0.5];
        let list=self.make_columns(list_offset,e,true,fonts,0x41f64c)?;
        self.node_mut(list)?.shown=0;self.columns_mut(list)?.shown=0;self.increment(p,0x41f6d1)?;
        let h=ftol(sub(self.main_window[3],top)?)?;let x=ftol(sub(self.main_window[2],self.scrollbar_width)?)?;
        let selector=self.make_selector(Label::Property(selector_offset),x,y,h,self.counter,pe,images,0x41f707)?;
        self.publish_child(selector_offset,selector,source)?;self.increment(p,0x41f715)?;
        {let Subtype::SpecialList3Texts(l)=&mut self.node_mut(list)?.subtype else{unreachable!()};l.indicator=Some(selector);}
        {let Subtype::Selector2Arrows(s)=&mut self.node_mut(selector)?.subtype else{unreachable!()};s.last_list=Some(list);}
        self.record(0x431ea0,selector,Event::Attach(list));self.show_selector(selector,0,float(pe,0x525040)?,0x41f737)?;
        //430b10(1) occurs AFTER attachment/show, unlike the standalone6c.
        let Subtype::SpecialList3Texts(l)=&mut self.node_mut(list)?.subtype else{unreachable!()};l.selectable680=1;
        self.record(source,list,Event::Geometry(0x430b10));Ok(())
    }
    fn property_text(&mut self,offset:u32,e:[f32;5],source:u32)->Result<NodeId,String>{
        let id=self.make_text(Label::Property(offset),e,self.counter,source)?;self.publish_child(offset,id,source)?;Ok(id)
    }
    fn property_visual(&mut self,offset:u32,e:[f32;5],source:u32)->Result<NodeId,String>{
        let id=self.make_visual(Label::Property(offset),e,self.root,self.counter,source)?;self.publish_child(offset,id,source)?;Ok(id)
    }
    ///442960 synchronously resets REAL embedded Text, appends REAL glyphs,
    ///resizes/clears reached colors, THEN publishes identifier2f4.
    fn visual_identifier(&mut self,id:NodeId,identifier:u32,fonts:&mut Fonts,source:u32)->Result<(),String>{
        self.visual_mut(id)?.caption.reset(fonts)?;
        self.record(source,id,Event::TextCall(0x441940));
        self.visual_mut(id)?.caption.identifier(fonts,identifier)?;
        self.record(source,id,Event::TextCall(0x441220));
        {let v=self.visual_mut(id)?;v.caption.resize()?;
            for color in &mut v.caption.colors[..v.caption.group.count as usize]{*color=0;}}
        self.record(source,id,Event::TextCall(0x440330));
        self.visual_mut(id)?.caption_identifier2f4=Field::Known(identifier);
        self.record(source,id,Event::CaptionStore(identifier));
        self.record(source,id,Event::TextCall(0x442960));Ok(())
    }
    ///43f800: outer amount's RET slot04/number/append precede42ec20's
    ///two actual Visual children. The source image dimensions write scale1.
    fn make_amount(&mut self,x:f32,y:f32,width:f32,images:&mut Images,source:u32)->Result<NodeId,String>{
        let w=images.dimension(0x68b000,0)? as i32;let h=images.dimension(0x68b000,1)? as i32;
        let right=add(x,width)?;
        //43f8a9 FSTs h to a binary32 argument while retaining the signed
        //integer value in ST0 for the outer bottom's following FADD.
        let stored_height=integer(h)?;let mut bottom=Sum::from_i32(h);bottom.add_f32(y).map_err(str::to_owned)?;let bottom=store(bottom)?;
        let child_bottom=add(stored_height,y)?;
        let id=self.allocate(Label::Property(0xb0),Family::Amount2Visuals,[x,y,right,bottom],Subtype::Amount2Visuals(Amount::birth()),source);
        self.record(source,id,Event::Constructor(0x42ebe0));self.append(id,self.root,self.counter,0x42f140,source)?;
        //Source43f96a reuses signed dimension integer, not a rounded width.
        let mut left_right=Sum::from_i32(w);left_right.add_f32(x).map_err(str::to_owned)?;
        let mut second_left=sum(right)?;second_left.add(Sum::from_i32(w).negated());
        let left_end=store(left_right)?;let right_start=store(second_left)?;
        //43f9ce..43f9de pushes right extent as first argument, left second.
        let first=self.make_visual(Label::AmountFirst150,[right_start,y,right,child_bottom,0.5],id,0,0x42ec39)?;
        {let Subtype::Amount2Visuals(a)=&mut self.node_mut(id)?.subtype else{unreachable!()};a.first150=Field::Known(first);}
        self.record(0x42ec42,first,Event::Alias(0x150));self.caption_mode(first,4)?;
        self.images(first,images,[Some(0x68b010),Some(0x68b018),Some(0x68b01c),Some(0x68b014),None])?;
        let second=self.make_visual(Label::AmountSecond154,[x,y,left_end,child_bottom,0.5],id,1,0x42eca7)?;
        {let Subtype::Amount2Visuals(a)=&mut self.node_mut(id)?.subtype else{unreachable!()};a.second154=Field::Known(second);}
        self.record(0x42ecb0,second,Event::Alias(0x154));self.caption_mode(second,4)?;
        self.images(second,images,[Some(0x68b000),Some(0x68b008),Some(0x68b00c),Some(0x68b004),None])?;
        self.publish_child(0xb0,id,0x405ce0)?;Ok(id)
    }
    fn centered_button(&self,pe:&PeImage<'_>,images:&mut Images,top:f32)->Result<[f32;5],String>{
        let w=images.dimension(0x68af58,0)? as i32;let h=images.dimension(0x68af58,1)? as i32;
        let half=w.wrapping_sub(w>>31)>>1; //CDQ/SUB/SAR, native truncation toward0
        let mut x=sub(self.main_window[2],self.main_window[0])?;x.mul_f32(float(pe,0x52502c)?).map_err(str::to_owned)?;x.add(Sum::from_i32(half).negated());
        let x=ftol(x)?;let mut x=Sum::from_i32(x);x.add_f32(self.main_window[0]).map_err(str::to_owned)?;
        let left=store(x)?;let right=add(left,integer(w)?)?;let bottom=add(top,integer(h)?)?;
        Ok([left,top,right,bottom,0.5])
    }
    fn property_layout(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,images:&mut Images)->Result<(),String>{
        if self.child1.is_some(){return Err("child1 already materialized".into())}
        let preimage=p.cells().read(CHILD,0xc8)?.to_vec();
        native_initializers::property401000(p.construction_cells()?,CHILD)?;
        self.record(0x401000,self.root,Event::Constructor(0x401000));
        self.child1=Some(ChildOne{source:CHILD,parent:self.root,fields:BTreeMap::new(),global_aliases:BTreeMap::new(),static_preimage:preimage,native_return:-1});
        p.construction_cells()?.set_word(CHILD+0x28,A)?;
        //40560c descriptor+8 is the SAME bound Primary4 descriptor's Y.
        let descriptor=p.resources.descriptor(PanelId::A)?;
        if descriptor.slot!=PanelId::A||descriptor.reference!=Some(PanelId::A){return Err("40560c requires actual retained A descriptor/model".into())}
        let mut baseline=sum(descriptor.position[1])?;baseline.add_f32(float(pe,0x525078)?).map_err(str::to_owned)?;let baseline=ftol(baseline)?;
        let m=self.main_window;
        let e=[m[0],add(m[1],float(pe,0x525074)?)?,minus(m[2],float(pe,0x525028)?)?,m[3],m[4]];
        let list=self.make_columns(0x6c,e,true,fonts,0x405664)?;
        {let Subtype::SpecialList3Texts(l)=&mut self.node_mut(list)?.subtype else{unreachable!()};l.selectable680=1;}
        self.increment(p,0x4056f6)?;
        let e=[m[0],add(m[1],float(pe,0x525078)?)?,minus(m[2],float(pe,0x525034)?)?,m[3],m[4]];
        self.make_columns(0x38,e,false,fonts,0x40573f)?;self.increment(p,0x4057c4)?;
        let y=ftol_added(m[1],float(pe,0x525078)?)?;
        self.property_pair(0x64,0x68,y,pe,p,fonts,images,0x4057e8)?;
        self.publish_global(0x86d3c0,self.child_field(0x64)?,0x4057f5)?;
        let wi=images.dimension(0x68afac,0)? as i32;let hi=images.dimension(0x68afac,1)? as i32;
        let w=integer(wi)?;let h=integer(hi)?;
        //405862/40589e FST binary32 copies do not pop the FILD value used
        //by the immediately following FADD. Later reloads use rounded w/h.
        let mut left=Sum::from_i32(wi);left.add_f32(m[0]).map_err(str::to_owned)?;let left=store(left)?;
        let top=add(m[1],float(pe,0x525070)?)?;let mut bottom=Sum::from_i32(hi);bottom.add_f32(top).map_err(str::to_owned)?;
        let prev=self.property_visual(0x2c,[left,top,add(w,left)?,store(bottom)?,0.5],0x4058b5)?;
        self.images(prev,images,[Some(0x68afac),Some(0x68afb4),None,Some(0x68afb0),None])?;self.caption_mode(prev,3)?;
        self.publish_global(0x86d3c4,prev,0x405906)?;p.construction_cells()?.set_word(CHILD+8,self.counter)?;self.increment(p,0x405925)?;
        //Three source FSUBs precede this FSTP; do not fold to right-2*w.
        let mut left=sub(m[2],w)?;left.add(sum(w)?.negated());left.add(sum(w)?.negated());let left=store(left)?;
        let next=self.property_visual(0x30,[left,top,add(w,left)?,add(h,top)?,0.5],0x40597c)?;
        self.images(next,images,[Some(0x68afbc),Some(0x68afc4),None,Some(0x68afc0),None])?;
        p.construction_cells()?.set_word(CHILD+0xc,self.counter)?;self.publish_global(0x86d3c8,next,0x4059cc)?;self.caption_mode(next,3)?;self.increment(p,0x4059ea)?;
        let y60=ftol_added(m[1],float(pe,0x52506c)?)?;
        self.property_pair(0x74,0x78,y60,pe,p,fonts,images,0x405a0f)?;
        let mut right=sub(m[2],w)?;right.add(sum(w)?.negated());let title_top=add(m[1],float(pe,0x525068)?)?;
        let title=self.property_text(0x34,[add(w,m[0])?,title_top,store(right)?,add(h,title_top)?,0.5],0x405a6c)?;
        self.text_mut(title)?.set_font(fonts,3)?;
        let category=p.cells().word(CHILD+0x70)?;
        if category>=5{return Err("405a83 constructor category leaves actual title table".into())}
        self.text_mut(title)?.identifier(fonts,word(pe,0x534d70+category*4)?)?;self.text_mut(title)?.style440650(4,-999,-999);self.increment(p,0x405aaf)?;
        self.property_pair(0x80,0x84,y60,pe,p,fonts,images,0x405ada)?;
        let space=ntbs(pe,0x533278)?;
        for index in [0u32,2,4]{
            let row=baseline.wrapping_add((index as i32).wrapping_mul(6));
            //Literal bottom20 and zero-high QWORD top are intentional native
            //stores, including negative-height extents. Do not normalize them.
            let e=[add(m[0],float(pe,0x52501c)?)?,unsigned(row as u32)?,minus(m[2],float(pe,0x52501c)?)?,20.,0.5];
            let first=self.property_text(0x94+index*4,e,0x405b55)?;
            self.text_mut(first)?.set_font(fonts,3)?;self.text_mut(first)?.identifier(fonts,word(pe,0x53322c+index*2)?)?;
            self.text_mut(first)?.style440650(if index==4{1}else{0},-999,-999);
            if index==4{let n=self.node_mut(first)?;n.bounds[1]=add(n.bounds[1],float(pe,0x525064)?)?;n.bounds[3]=add(n.bounds[3],float(pe,0x525064)?)?;let Subtype::Text(t)=&mut n.subtype else{unreachable!()};t.bounds=n.bounds;}
            self.increment(p,0x405bdd)?;
            let second=self.property_text(0x98+index*4,e,0x405bf7)?;
            self.text_mut(second)?.style440650(2,-999,-999);self.text_mut(second)?.set_font(fonts,3)?;self.text_mut(second)?.literal(fonts,&space,&[])?;
            self.increment(p,0x405c55)?;
        }
        let left=add(m[0],float(pe,0x525064)?)?;let right=minus(m[2],float(pe,0x525064)?)?;
        let amount_y=unsigned(baseline.wrapping_add(0x3c) as u32)?;
        let amount_bottom=add(amount_y,float(pe,0x525064)?)?;
        let amount=self.make_amount(left,amount_y,minus(right,left)?,images,0x405cca)?;
        {let Subtype::Amount2Visuals(a)=&mut self.node_mut(amount)?.subtype else{unreachable!()};a.range42efb0(100.,1000.,100000.);a.amount42ee70(1000.);}
        self.record(0x405ce6,amount,Event::Geometry(0x42efb0));self.record(0x405cf6,amount,Event::Geometry(0x42ee70));self.increment(p,0x405d0a)?;
        let t=self.property_text(0xb4,[left,amount_y,right,amount_bottom,0.5],0x405d24)?;
        self.text_mut(t)?.set_font(fonts,3)?;self.text_mut(t)?.style440650(1,-999,-999);self.text_mut(t)?.literal(fonts,&space,&[])?;self.increment(p,0x405d6c)?;
        let e=self.centered_button(pe,images,add(amount_y,float(pe,0x525060)?)?)?;
        let rights=self.property_visual(0xac,e,0x405e44)?;
        self.images(rights,images,[Some(0x68af58),Some(0x68af60),None,Some(0x68af5c),None])?;
        self.visual_identifier(rights,47,fonts,0x405e90)?;
        {let v=self.visual_mut(rights)?;v.caption.set_font(fonts,2)?;v.caption.style440650(4,-999,-999);}
        self.caption_mode(rights,3)?;self.publish_global(0x86d3cc,rights,0x405ed8)?;p.construction_cells()?.set_word(CHILD,self.counter)?;self.increment(p,0x405eed)?;
        let explanatory_top=add(m[1],float(pe,0x52505c)?)?;
        let e=[add(m[0],float(pe,0x525020)?)?,explanatory_top,minus(m[2],float(pe,0x525020)?)?,add(explanatory_top,float(pe,0x525058)?)?,0.5];
        for (offset,source,increment) in [(0xc4,0x405f7d,0x405fc8),(0xb8,0x405fe2,0x406025)]{
            let t=self.property_text(offset,e,source)?;self.text_mut(t)?.style440650(0,-999,-999);self.text_mut(t)?.set_font(fonts,3)?;self.text_mut(t)?.literal(fonts,&space,&[])?;self.increment(p,increment)?;
        }
        let e=self.centered_button(pe,images,add(m[1],float(pe,0x525064)?)?)?;
        let aux=self.property_visual(0xbc,e,0x406104)?;self.images(aux,images,[Some(0x68af58),Some(0x68af60),None,Some(0x68af5c),None])?;
        self.visual_identifier(aux,53,fonts,0x406150)?;
        {let v=self.visual_mut(aux)?;v.caption.set_font(fonts,0)?;v.caption.style440650(4,-999,-999);}
        self.caption_mode(aux,3)?;self.publish_global(0x86d4ac,aux,0x40619a)?;p.construction_cells()?.set_word(CHILD+4,self.counter)?;self.increment(p,0x4061b0)?;
        self.property_pair(0x8c,0x90,y,pe,p,fonts,images,0x4061db)?;
        {let c=p.construction_cells()?;c.set_word(CHILD+0x88,1)?;c.set_word(CHILD+0x7c,1)?;}
        for offset in [0xbc,0xac,0xb0,0x2c,0x30]{let id=self.child_field(offset)?;self.node_mut(id)?.shown=0;}
        for i in 0..6{let id=*self.aliases.get(&(0x48c+i*4)).ok_or("41f120 actual mode control absent")?;self.reset_control_node(id)?;}
        self.record(0x40621d,self.root,Event::Geometry(0x41f120));
        {let c=p.construction_cells()?;c.set_word(CHILD+0x20,0)?;c.set_word(CHILD+0x24,0)?;}
        finite::no_selection_cycle(&mut LayoutView{product:self,primitives:p,fonts,zero:float(pe,0x525040)?})?;
        self.record(0x41e725,self.root,Event::Stop);Ok(())
    }
    fn reset_control_node(&mut self,id:NodeId)->Result<(),String>{
        let n=self.node_mut(id)?;let Subtype::Visual(v)=&mut n.subtype else{return Err("mode/info reset requires real Visual".into())};
        finite::reset_control(&mut n.shown,v.policy338,&mut v.selected336,&mut v.disabled333);Ok(())
    }
}
struct LayoutView<'a>{product:&'a mut Product,primitives:&'a mut Primitives,fonts:&'a mut Fonts,zero:f32}
impl LayoutView<'_>{
    fn null(&self)->Result<(),String>{if self.primitives.cells().word(0x822d50)?!=0{return Err("property startup guard reached selected822d50".into())}Ok(())}
    fn parent(&self,offset:u32)->Result<NodeId,String>{self.product.aliases.get(&offset).copied().ok_or_else(||format!("property actual parent+{offset:x} owner absent"))}
}
impl finite::ResetView for LayoutView<'_>{
    fn flag10(&mut self,value:u8)->Result<(),String>{self.primitives.construction_cells()?.set_byte(CHILD+0x10,value)}
    fn infos_reset(&mut self)->Result<(),String>{for i in 0..5{let id=self.parent(0x4a4+i*4)?;self.product.reset_control_node(id)?;}Ok(())}
    fn title_hide(&mut self)->Result<(),String>{let id=self.parent(0x4b8)?;self.product.node_mut(id)?.shown=0;Ok(())}
    fn hide(&mut self,offset:u32)->Result<(),String>{let id=self.product.child_field(offset)?;let n=self.product.node_mut(id)?;n.shown=0;match &mut n.subtype{Subtype::List3Texts(l)=>l.list.shown=0,Subtype::SpecialList3Texts(l)=>l.list.shown=0,_=>()}Ok(())}
    fn selector_hide(&mut self,offset:u32)->Result<(),String>{let id=self.product.child_field(offset)?;self.product.show_selector(id,0,self.zero,0x402c30)}
    fn active1c(&mut self,value:u8)->Result<(),String>{self.primitives.construction_cells()?.set_byte(CHILD+0x1c,value)}
}
impl finite::ExitView for LayoutView<'_>{fn notice_reset(&mut self)->Result<(),String>{
    let id=self.parent(0x3a0)?;self.product.text_mut(id)?.reset(self.fonts)?;
    self.product.record(0x402d22,id,Event::TextCall(0x441940));Ok(())
}}
impl finite::NoSelectionView for LayoutView<'_>{
    fn info_visibility(&mut self)->Result<(),String>{
        if self.primitives.cells().byte(A+0x509)?==1{return Ok(())}
        self.null()?; //4048a0 NULL returns0, retaining401000 flags/indices
        for i in 0..5{let id=self.parent(0x4a4+i*4)?;self.product.node_mut(id)?.shown=1;}Ok(())
    }
    fn report_zero(&mut self)->Result<(),String>{self.primitives.construction_cells()?.set_word(CHILD+0x24,0)}
    fn title_zero(&mut self)->Result<(),String>{
        let id=self.parent(0x4b8)?;let n=self.product.node_mut(id)?;
        let Subtype::Text(t)=&mut n.subtype else{return Err("property title must alias actual Text".into())};
        finite::report_zero_title(t,self.fonts,&mut n.shown)?;
        for (source,call) in [(0x40494d,0x441940),(0x404981,0x441220),(0x404991,0x440ad0)]{self.product.record(source,id,Event::TextCall(call));}Ok(())
    }
    fn dispatch_zero(&mut self)->Result<(),String>{self.null()} //4044b0 NULL returns before rows
    fn select_info_zero(&mut self)->Result<(),String>{let id=self.parent(0x4a4)?;let v=self.product.visual_mut(id)?;if v.policy338==1{v.selected336=1;}Ok(())}
    fn child_title_reset(&mut self)->Result<(),String>{
        let id=self.product.child_field(0x34)?;self.product.text_mut(id)?.reset(self.fonts)?;
        self.product.record(0x441940,id,Event::TextCall(0x441940));Ok(())
    }
    fn mode_zero(&mut self)->Result<(),String>{self.primitives.construction_cells()?.set_word(CHILD+0x20,0)}
    fn activate(&mut self)->Result<(),String>{
        self.null()?;self.primitives.construction_cells()?.set_word(A+0x4f4,u32::MAX)?;
        self.product.record(0x402366,self.product.root,Event::ScalarStore{field:A+0x4f4,value:u32::MAX});Ok(())
    }
}

#[cfg(test)]
#[path="main_property_construction_tests.rs"]
mod tests;

#[path="main_child_five_construction.rs"]
mod five;
pub use five::{CompletedMainLayout,AwaitingChild2,AwaitingParentTail,ChildTwo,ChildFive};

//Finite no-selection4024b0 entry on the full typed constructor arena. The
//shared reset/exit/info algorithms keep their real Text/list/selector owners.
impl Product {
    fn parent_property_exit(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts)->Result<(),String>{
        finite::exit_owned(&mut LayoutView{product:self,primitives:p,fonts,zero:float(pe,0x525040)?})
    }
    fn parent_property_activate(&mut self,p:&mut Primitives,fonts:&mut Fonts,zero:f32)->Result<(),String>{
        //4021a0's independently sampled NULL branch; no selected projection.
        let mut v=LayoutView{product:self,primitives:p,fonts,zero};
        finite::NoSelectionView::activate(&mut v)
    }
    fn parent_property_enter(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,images:&Images)->Result<(),String>{
        let zero=float(pe,0x525040)?;
        for i in 0..3u32 {
            let id=*self.aliases.get(&(0x48c+i*4)).ok_or("4024b0 mode owner absent")?;
            let b=self.node(id)?.bounds;
            let image=images.id(0x68ae28+word(pe,0x533244+i*4)?.checked_mul(4).ok_or("4024b0 image label overflow")?)?;
            self.node_mut(id)?.shown=1;
            let visual=self.visual_mut(id)?;visual.icon_bounds=[b[0],b[1],b[2],b[3],0.5];visual.icon=Some(image);
            let animation=word(pe,0x533238+i*4)?;let node=self.node_mut(id)?;node.completion38=0;node.animation34=animation;
            self.record(0x402538,id,Event::Geometry(0x4024b0));
        }
        //4048a0 returns0 on the actual NULL cell, retaining static flags.
        if p.cells().word(0x822d50)?!=0{return Err("40253c/4048a0 selected property classification unavailable".into())}
        for (o,source) in [(0x490,0x40266b),(0x494,0x4026d8)] {
            //The real null guards disable the amount and rights modes.
            if p.cells().word(0x822d50)?!=0{return Err("4024b0 selected mode enable unavailable".into())}
            let id=*self.aliases.get(&o).ok_or("4024b0 mode field absent")?;
            let v=self.visual_mut(id)?;v.disabled333=1;v.down335=0;
            self.record(source,id,Event::Geometry(0x442ac0));
        }
        if p.cells().word(0x822d50)?!=0{return Err("402701 selected lock-state4053a0 unavailable".into())}
        if p.cells().byte(A+0x509)?==1||p.cells().byte(A+0x508)?==0{return Ok(())}
        for i in 0..5u32 {
            let id=*self.aliases.get(&(0x4a4+i*4)).ok_or("402761 info owner absent")?;
            self.node_mut(id)?.shown=1;let b=self.node(id)?.bounds;
            let factor=Sum::from_f64(double(pe,0x525038)?).map_err(str::to_owned)?;
            let mut extent=[0.;5];extent[4]=0.5;
            for j in 0..4 {
                let mut scaled=sum(b[j])?;scaled.mul(factor);
                let mut v=sum(b[j])?;v.add(if j<2{scaled}else{scaled.negated()});extent[j]=store(v)?;
            }
            let image=images.id(0x68ae28+word(pe,0x533250+i*8)?.checked_mul(4).ok_or("4028a6 image label overflow")?)?;
            let v=self.visual_mut(id)?;v.icon_bounds=extent;v.icon=Some(image);
            let animation=word(pe,0x533254+i*8)?;let n=self.node_mut(id)?;n.completion38=0;n.animation34=animation;
            self.record(0x4028e3,id,Event::Geometry(0x4024b0));
        }
        if p.cells().word(0x822d50)?!=0{return Err("4028ef/4048a0 selected classification unavailable".into())}
        if p.cells().word(CHILD+0x20)?!=0||p.cells().word(CHILD+0x24)?!=0{
            return Err("402a78 requires actual source-born child1 mode0/report0; selected/other-mode entry unavailable".into())
        }
        //402aa9 ensure-expanded is an actual guard read, not a model prepare.
        if p.cells().byte(A+0x508)?==0{return Err("402aa9/41fc60 collapsed model consumer unavailable".into())}
        self.record(0x402aa9,self.root,Event::Callback(0x41fc60));
        finite::restore_no_selection_mode(&mut LayoutView{product:self,primitives:p,fonts,zero})?;
        let id=*self.aliases.get(&0x48c).ok_or("402afd mode0 owner absent")?;
        let v=self.visual_mut(id)?;if v.policy338==1{v.selected336=1;}
        self.record(0x402afd,id,Event::Geometry(0x442af0));
        self.parent_property_activate(p,fonts,zero)?; //402b04, repeated by source
        //402b0e exit and restore the actual retained mode0, then info-mode.
        finite::restore_no_selection_mode(&mut LayoutView{product:self,primitives:p,fonts,zero})?;
        Ok(())
    }
}
