//! Actual406690/408da0 child2, including mandatory layout-time409d20.
//! Stops at41e759 before the parent tail. Bounded null-business construction.
use super::*;
use super::super::super::super::other_child_resets::{self,Currency,NullReset};
const BUSINESS:u32=0x560930;

pub struct ChildTwo {
    pub source:u32,pub parent:NodeId,pub fields:BTreeMap<u32,NodeId>,
    pub globals:BTreeMap<u32,NodeId>,pub static_preimage:Vec<u8>,
    pub native_return:Field<i32>,pub currency_buffer:Field<Vec<u8>>,
}
impl ChildTwo {
    pub fn field(&self,offset:u32)->Result<NodeId,String>{self.fields.get(&offset).copied().ok_or_else(||format!("business+{offset:x} actual owner absent"))}
    pub fn global(&self,address:u32)->Option<NodeId>{self.globals.get(&address).copied()}
    pub fn require_selected_business(&self)->Result<(),String>{Err("selected590f60 business/report/input families are not implemented".into())}
}
pub struct AwaitingParentTail {prefix:AwaitingChild2}
impl Deref for AwaitingParentTail {type Target=AwaitingChild2;fn deref(&self)->&Self::Target{&self.prefix}}
impl AwaitingParentTail {
    pub fn phase(&self)->MainPhase{self.prefix.phase()}
    pub fn child2(&self)->&ChildTwo{self.prefix.prefix.prefix.inner.child2.as_ref().expect("successful408da0 owns child2")}
    pub fn require_full_main(&self)->Result<(),String>{Err("41e759 incomplete: parent tail/switch1,43bed0,184,43c600/full41eb40 and group/Graph/input/timers remain".into())}
    pub fn continue_child2(slot:&mut Option<AwaitingChild2>,executable:&[u8],text:&mut UiTextResources,
        p:&mut Primitives,images:&mut Images)->Result<Self,String>{
        let prior=slot.as_ref().ok_or("408da0 prefix already consumed; no duplicate continuation")?;
        let product=&prior.prefix.prefix.inner;
        if product.phase!=MainPhase::AwaitingChild2{return Err("408da0 retained product is not AwaitingChild2; no replay".into())}
        p.check_main_business_owner(product.owner)?;
        if !product.text_identity.same(&text.construction_identity)||!product.image_identity.same(&images.construction_identity){
            return Err("408da0 requires SAME retained Fonts/language and image owners".into())
        }
        if images.ui_dimensions!=[640,480]{return Err("408da0 requires actual retained640 image domain".into())}
        let pe=PeImage::parse(executable)?;
        for (at,n) in [(0x406690,0x35),(0x407010,0x20),(0x407890,0xd3),(0x408880,0x520),
            (0x408da0,0xe03),(0x409d20,0xa5),(0x41e73f,0x1a),(0x41f150,0x40),
            (0x41f510,0x240),(0x43a8c0,0x8c),(0x43ac10,0x50),(0x442960,0xc0),
            (0x440b10,0x35),(0x43eb40,0xf0),(0x43ec30,0xf0),(0x43ed20,0x110),
            (0x43f510,0x2f0),(0x42f3d0,0x140),(0x42fdf0,0x96),(0x430790,0x250),
            (0x431e80,0x30),(0x440160,0x100),(0x440650,0x570),(0x441940,0xb0),(0x52501c,0x98),(0x525908,52),(0x534d78,4),(0x536400,0x20),(0x533278,8)]{
            if pe.at(at,n)?!=p.cells().read(at,n)?{return Err(format!("408da0 source{at:x} differs from SAME retained PE"))}
        }
        if word(&pe,0x525908+0x2c)?!=0x43b370{return Err("408da0 requires default Text reset RET43b370".into())}
        if p.cells().word(A+0x398)?!=BUSINESS{return Err("408da0 actual parent+398 static child identity differs".into())}
        if p.cells().word(A+0x388)?!=product.counter{return Err("408da0 actual parent counter differs".into())}
        for (i,v) in product.main_window.iter().enumerate(){if p.cells().word(A+0x4d4+i as u32*4)?!=v.to_bits(){return Err("408da0 SAME parent geometry differs".into())}}
        if pe.at(BUSINESS,0xd0)?!=p.cells().read(BUSINESS,0xd0)?{return Err("406690 static birth preimage differs".into())}
        //Two independent authoritative cells. Do not force either to NULL.
        if p.cells().word(0x590f60)?!=0{return Err("408da0 layout-time409d20 selected590f60 branch unsupported".into())}
        if p.cells().word(0x822d50)?!=0{return Err("408da0 source-born no-selection domain requires actual822d50 null".into())}
        for at in [0x4b8,0x4a4,0x4a8,0x4ac,0x4b0,0x4b4,0x48c]{
            let id=*product.aliases.get(&at).ok_or("408da0 required shared parent alias absent")?;
            match (at,&product.node(id)?.subtype){(0x4b8,Subtype::Text(_))=>(),(_,Subtype::Visual(_)) if at!=0x4b8=>(),_=>return Err("408da0 parent Text/control subtype mismatch".into())}
        }
        p.claim_main_business(product.owner)?;
        slot.as_mut().unwrap().prefix.prefix.inner.phase=MainPhase::ConstructingBusiness;
        let result=slot.as_mut().unwrap().prefix.prefix.inner.business_layout(&pe,p,text.fonts_mut(),images)
            .map_err(|e|format!("406690/408da0 actual child2: {e}"));
        match result {
            Ok(())=>{p.main_complete_business();slot.as_mut().unwrap().prefix.prefix.inner.phase=MainPhase::AwaitingParentTail;
                Ok(Self{prefix:slot.take().expect("checked SAME retained child2 prefix")})},
            Err(e)=>{p.main_failed();slot.as_mut().unwrap().prefix.prefix.inner.phase=MainPhase::Failed;Err(e)},
        }
    }
}
fn plus_sum(a:Sum,b:f32)->Result<Sum,String>{let mut a=a;a.add_f32(b).map_err(str::to_owned)?;Ok(a)}
fn less_sum(mut a:Sum,b:f32)->Result<Sum,String>{a.add(sum(b)?.negated());Ok(a)}
fn int_store(v:i32)->Result<f32,String>{store(Sum::from_i32(v))}
fn extent_xywh(x:f32,y:f32,w:f32,h:f32)->Result<[f32;5],String>{Ok([x,y,add(x,w)?,add(y,h)?,0.5])}
impl Product {
    fn business_field(&self,offset:u32)->Result<NodeId,String>{self.child2.as_ref().ok_or("business birth absent")?.field(offset)}
    fn business_publish(&mut self,offset:u32,id:NodeId,source:u32)->Result<(),String>{
        self.node(id)?;if self.child2.as_mut().ok_or("business birth absent")?.fields.insert(offset,id).is_some(){return Err("business field published twice".into())}
        self.record(source,id,Event::Alias(BUSINESS+offset));Ok(())
    }
    fn business_global(&mut self,address:u32,id:NodeId,source:u32)->Result<(),String>{
        self.node(id)?;if self.child2.as_mut().ok_or("business birth absent")?.globals.insert(address,id).is_some(){return Err("business global published twice".into())}
        self.record(source,id,Event::Alias(address));Ok(())
    }
    fn business_word(&mut self,p:&mut Primitives,o:u32,v:u32,source:u32)->Result<(),String>{
        p.construction_cells()?.set_word(BUSINESS+o,v)?;self.record(source,self.root,Event::ScalarStore{field:BUSINESS+o,value:v});Ok(())
    }
    fn business_text(&mut self,o:u32,e:[f32;5],source:u32)->Result<NodeId,String>{
        let id=self.make_text(Label::Business(o),e,self.counter,source)?;self.business_publish(o,id,source)?;Ok(id)
    }
    fn business_visual(&mut self,o:u32,e:[f32;5],source:u32)->Result<NodeId,String>{
        let id=self.make_visual(Label::Business(o),e,self.root,self.counter,source)?;self.business_publish(o,id,source)?;Ok(id)
    }
    fn business_style(&mut self,id:NodeId,style:u32,fonts:&mut Fonts,source:u32)->Result<(),String>{
        self.text_mut(id)?.set_font(fonts,3)?;self.record(source,id,Event::TextCall(0x440b70));
        self.text_mut(id)?.style440650(style,-999,-999);self.record(source,id,Event::TextCall(0x440650));Ok(())
    }
    fn business_columns(&mut self,o:u32,e:[f32;5],fonts:&mut Fonts,source:u32)->Result<NodeId,String>{
        let id=self.make_column_node(Label::Business(o),e,false,fonts,source)?;
        self.business_publish(o,id,source)?;
        //Unlike property41f510, these ordinary lists use secondary right4.
        let l=self.columns_mut(id)?;l.primary.style440650(3,0,0);l.primary.padding=1;l.primary.layout(fonts)?;
        l.secondary.style440650(2,4,3);l.primary.gap=2;l.secondary.gap=2;
        l.primary.set_font(fonts,3)?;l.secondary.set_font(fonts,3)?;
        for call in [0x440650,0x4406a0,0x440650,0x440b70,0x440b70]{self.record(source,id,Event::TextCall(call));}Ok(id)
    }
    fn business_button(&mut self,o:u32,e:[f32;5],identifier:u32,fonts:&mut Fonts,images:&Images,source:u32)->Result<NodeId,String>{
        let id=self.business_visual(o,e,source)?;
        self.images(id,images,[Some(0x68af58),Some(0x68af60),Some(0x68b064),Some(0x68af5c),None])?;
        self.visual_identifier(id,identifier,fonts,source)?;
        self.visual_mut(id)?.caption.set_font(fonts,2)?;self.record(source,id,Event::TextCall(0x440b70));
        self.visual_mut(id)?.caption.style440650(4,-999,-999);self.record(source,id,Event::TextCall(0x440650));
        self.caption_mode(id,3)?;Ok(id)
    }
    fn business_color9(&mut self,id:NodeId,source:u32)->Result<(),String>{
        //440b10 calls440330 then writes row0 color/blink-color/blink bytes.
        let t=self.text_mut(id)?;t.resize()?;
        *t.colors.get_mut(0).ok_or("440b10 row0 color storage absent")?=9;
        *t.blink_colors.get_mut(0).ok_or("440b10 row0 blink-color storage absent")?=0;
        *t.blink.get_mut(0).ok_or("440b10 row0 blink storage absent")?=0;
        self.record(source,id,Event::TextCall(0x440b10));Ok(())
    }
    fn business_currency(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,reset:bool,source:u32)->Result<(),String>{
        let id=self.business_field(0xa4)?;
        if reset{self.text_mut(id)?.reset(fonts)?;self.record(source,id,Event::TextCall(0x441940));}
        let c=p.cells();let mut buffer=c.ntbs(0x57f734,1024)?;buffer.push(0);
        let mut currency=Currency{raw_symbol:c.ntbs(0x57e6ec,1024)?,suffix:c.word(0x59050c)?,buffer,
            prefix_format:ntbs(pe,0x536410)?,suffix_format:ntbs(pe,0x536400)?};
        let raw=currency.zero()?;
        //Keep43a8c0's shared native buffer authoritative; this is a retained
        //observation, not another language/currency manager or source.
        p.construction_cells()?.write(0x57f734,&currency.buffer)?;
        self.child2.as_mut().unwrap().currency_buffer=Field::Known(currency.buffer);
        self.record(source,id,Event::TextCall(0x43a8c0));
        self.text_mut(id)?.literal(fonts,&raw,&[])?;self.record(source,id,Event::TextCall(0x441770));Ok(())
    }
    fn business_pair(&mut self,y:i32,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,images:&mut Images)->Result<(),String>{
        let top=store(Sum::from_f64(f64::from(y as u32)).map_err(str::to_owned)?)?;
        let e=[self.main_window[0],top,store(sub(self.main_window[2],self.scrollbar_width)?)?,store(sub(self.main_window[3],float(pe,0x525028)?)?)?,0.5];
        let list=self.make_column_node(Label::Business(0x98),e,true,fonts,0x41f64c)?;
        self.business_publish(0x98,list,0x41f661)?;self.configure_columns(list,fonts,0x41f66a)?;
        self.node_mut(list)?.shown=0;self.columns_mut(list)?.shown=0;self.increment(p,0x41f6d1)?;
        let h=ftol(sub(self.main_window[3],top)?)?;let x=ftol(sub(self.main_window[2],self.scrollbar_width)?)?;
        let selector=self.make_selector(Label::Business(0xb8),x,y,h,self.counter,pe,images,0x41f707)?;
        self.business_publish(0xb8,selector,0x41f710)?;self.increment(p,0x41f715)?;
        let Subtype::SpecialList3Texts(l)=&mut self.node_mut(list)?.subtype else{unreachable!()};l.indicator=Some(selector);
        let Subtype::Selector2Arrows(s)=&mut self.node_mut(selector)?.subtype else{unreachable!()};s.last_list=Some(list);
        self.record(0x431ea0,selector,Event::Attach(list));self.show_selector(selector,0,float(pe,0x525040)?,0x41f737)?;
        let Subtype::SpecialList3Texts(l)=&mut self.node_mut(list)?.subtype else{unreachable!()};l.selectable680=1;Ok(())
    }
    fn business_layout(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,images:&mut Images)->Result<(),String>{
        if self.child2.is_some(){return Err("business child already materialized".into())}
        let preimage=p.cells().read(BUSINESS,0xd0)?.to_vec();
        //406690 actual static constructor; preserve all other source bytes.
        {let c=p.construction_cells()?;for o in [0x48,0x44]{c.set_word(BUSINESS+o,0)?;}
            for o in [0x94,0xcc,0xb0]{c.set_byte(BUSINESS+o,0)?;}c.set_word(BUSINESS+0xb4,u32::MAX)?;
            for i in 0..8{c.set_word(BUSINESS+0x68+i*4,0)?;}}
        self.record(0x406690,self.root,Event::Constructor(0x406690));
        self.child2=Some(ChildTwo{source:BUSINESS,parent:self.root,fields:BTreeMap::new(),globals:BTreeMap::new(),static_preimage:preimage,native_return:Field::Unknown,currency_buffer:Field::Unknown});
        self.business_word(p,0x44,A,0x408dbc)?;self.business_word(p,0x28,0,0x408dc1)?;
        let m=self.main_window;let f=|at|float(pe,at);
        let button_w=images.dimension(0x68af58,0)? as i32;let button_h=images.dimension(0x68af58,1)? as i32;
        let mut e=m;e[0]=add(e[0],f(0x52501c)?)?;e[2]=store(sub(e[2],f(0x52501c)?)?)?;
        //408e5e FST rounds top, while bottom adds to the retained PC64 ST0.
        let top=plus_sum(sum(e[1])?,f(0x5250a4)?)?;e[1]=store(top)?;e[3]=store(plus_sum(top,f(0x5250a0)?)?)?;
        let id=self.business_text(0x50,e,0x408e77)?;self.business_style(id,4,fonts,0x408e83)?;self.increment(p,0x408ea7)?;
        //The source loop tests i==2 but iterates only i=0,1: both extents
        //are the same. Do not invent a second-row offset for54/58.
        for (o,at) in [(0x54,0x408ede),(0x58,0x408ede)]{let id=self.business_text(o,e,at)?;self.text_mut(id)?.set_font(fonts,3)?;self.record(at,id,Event::TextCall(0x440b70));self.increment(p,0x408efe)?;}
        let id=self.business_field(0x54)?;self.text_mut(id)?.identifier(fonts,0x1b)?;self.text_mut(id)?.style440650(0,-999,-999);
        let id=self.business_field(0x58)?;self.text_mut(id)?.style440650(2,-999,-999);
        e[1]=add(e[1],f(0x5250a4)?)?;
        let nav_w=images.dimension(0x68afac,0)? as i32;let nav_h=images.dimension(0x68afac,1)? as i32;
        let half=nav_w.wrapping_sub(nav_w>>31)>>1;let half=int_store(half)?;let nw=int_store(nav_w)?;let nh=int_store(nav_h)?;
        let left=store(plus_sum(plus_sum(Sum::from_i32(nav_w),m[0])?,half)?)?;
        let top=add(m[1],f(0x52509c)?)?;
        //409011 FST rounds nh for later consumers but retains the signed
        //FILD integer in ST0 for this first navigation bottom's FADD.
        let first_bottom=store(plus_sum(Sum::from_i32(nav_h),top)?)?;
        let n=[left,top,add(nw,left)?,first_bottom,0.5];
        let id=self.business_visual(0x1c,n,0x409029)?;self.images(id,images,[Some(0x68afac),Some(0x68afb4),None,Some(0x68afb0),None])?;self.caption_mode(id,3)?;
        self.business_global(0x86d3d0,id,0x40907a)?;self.business_word(p,0x14,self.counter,0x409094)?;self.node_mut(id)?.shown=0;self.increment(p,0x4090a5)?;
        let left=store(less_sum(less_sum(sub(m[2],nw)?,nw)?,half)?)?;
        let n=[left,top,add(nw,left)?,add(nh,top)?,0.5];
        let id=self.business_visual(0x20,n,0x4090f9)?;self.images(id,images,[Some(0x68afbc),Some(0x68afc4),None,Some(0x68afc0),None])?;
        self.business_word(p,0x18,self.counter,0x40914e)?;self.node_mut(id)?.shown=0;self.business_global(0x86d3d4,id,0x40915c)?;self.increment(p,0x40916e)?;
        //409165 FLD stored nav width, FADD ST0,ST0, then parent x.
        let mut left=sum(nw)?;left.add(sum(nw)?);let left=store(less_sum(plus_sum(left,m[0])?,f(0x525020)?)?)?;
        let right=store(less_sum(less_sum(sub(m[2],nw)?,half)?,f(0x525020)?)?)?;
        let top=store(less_sum(plus_sum(sum(m[1])?,f(0x52509c)?)?,f(0x525028)?)?)?;
        let id=self.business_text(0x24,[left,top,right,add(nh,top)?,0.5],0x4091da)?;
        self.text_mut(id)?.set_font(fonts,3)?;self.text_mut(id)?.identifier(fonts,word(pe,0x534d78)?)?;self.text_mut(id)?.style440650(4,-999,-999);self.increment(p,0x409222)?;
        let mut list_e=e;list_e[0]=m[0];list_e[2]=m[2];list_e[3]=m[3];
        let id=self.business_columns(0x64,list_e,fonts,0x409268)?;self.node_mut(id)?.disabled=1;self.columns_mut(id)?.disabled=1;self.increment(p,0x409334)?;
        e=m;e[0]=add(e[0],f(0x52501c)?)?;e[1]=add(e[1],f(0x525020)?)?;e[2]=store(sub(e[2],f(0x52501c)?)?)?;
        //The preceding push makes40933f/40961f read stored TOP at S+28h.
        e[3]=add(e[1],f(0x525078)?)?;
        let id=self.business_text(0xc4,e,0x409352)?;self.increment(p,0x409369)?;self.business_style(id,4,fonts,0x409375)?;
        let top=plus_sum(sum(e[1])?,f(0x525098)?)?;e[1]=store(top)?;e[3]=store(plus_sum(top,f(0x5250a4)?)?)?;
        let id=self.business_text(0xbc,e,0x4093c3)?;self.increment(p,0x4093da)?;self.business_style(id,0,fonts,0x4093e6)?;self.text_mut(id)?.identifier(fonts,0x54)?;
        let id=self.business_text(0xc0,e,0x409429)?;self.increment(p,0x40943e)?;self.business_style(id,2,fonts,0x40944c)?;
        self.text_mut(id)?.literal(fonts,&ntbs(pe,0x533278)?,&[])?;self.business_color9(id,0x40948a)?;
        let half_button=int_store(button_w.wrapping_sub(button_w>>31)>>1)?;
        let centered=|top:f32|->Result<[f32;5],String>{
            let mut x=sub(m[2],m[0])?;x.mul_f32(f(0x52502c)?).map_err(str::to_owned)?;x=less_sum(x,half_button)?;
            let x=store(plus_sum(Sum::from_i32(ftol(x)?),m[0])?)?;
            extent_xywh(x,top,int_store(button_w)?,int_store(button_h)?)
        };
        let button_top=add(e[1],f(0x525094)?)?;
        let id=self.business_button(0xc8,centered(button_top)?,0x58,fonts,images,0x40951c)?;
        self.business_global(0x86d3dc,id,0x4095bc)?;self.business_word(p,8,self.counter,0x4095c8)?;self.increment(p,0x4095d6)?;
        e=m;e[0]=add(e[0],f(0x52501c)?)?;e[1]=add(e[1],f(0x525020)?)?;e[2]=store(sub(e[2],f(0x52501c)?)?)?;e[3]=add(e[1],f(0x525078)?)?;
        let id=self.business_text(0x9c,e,0x409632)?;self.increment(p,0x409649)?;self.business_style(id,4,fonts,0x409655)?;
        let top=plus_sum(sum(e[1])?,f(0x525098)?)?;e[1]=store(top)?;e[3]=store(plus_sum(top,f(0x5250a4)?)?)?;
        let id=self.business_text(0xa0,e,0x4096a3)?;self.increment(p,0x4096ba)?;self.business_style(id,0,fonts,0x4096c6)?;self.text_mut(id)?.identifier(fonts,0x57)?;
        let id=self.business_text(0xa4,e,0x409709)?;self.increment(p,0x409720)?;self.business_style(id,2,fonts,0x40972c)?;
        self.business_currency(pe,p,fonts,false,0x40974a)?;self.business_color9(id,0x409770)?;
        let id=self.business_button(0xac,centered(button_top)?,0x55,fonts,images,0x4097cf)?;
        self.business_global(0x86d3d8,id,0x40986c)?;self.business_word(p,0x10,self.counter,0x409877)?;self.increment(p,0x40988b)?;
        self.business_word(p,0xc,self.counter,0x40989c)?;
        //After push0,409880 loads stored row TOP at S+28h, then parent TOP.
        let y=ftol(plus_sum(plus_sum(sum(e[1])?,m[1])?,f(0x525078)?)?)?;
        self.business_pair(y,pe,p,fonts,images)?;let id=self.business_field(0x98)?;self.business_global(0x86d3e0,id,0x4098c6)?;
        let mut le=m;le[3]=add(le[1],f(0x525058)?)?;self.business_columns(0x88,le,fonts,0x409903)?;self.increment(p,0x4099a1)?;
        let top=plus_sum(sum(le[1])?,f(0x525078)?)?;le[1]=store(top)?;le[3]=store(plus_sum(top,f(0x525058)?)?)?;
        self.business_columns(0x8c,le,fonts,0x4099e3)?;self.increment(p,0x409a8a)?;
        let top=add(m[1],f(0x525090)?)?;self.business_button(0x90,centered(top)?,0x587,fonts,images,0x409ae3)?;
        self.business_word(p,4,self.counter,0x409b83)?;self.increment(p,0x409b8d)?;
        //409b93 calls409d20 BEFORE the layout returns. The same null reset
        //algorithm works directly on this arena and SAME mutable manager.
        other_child_resets::null_reset(&mut BirthReset{product:self,primitives:p,fonts,pe,zero:f(0x525040)?})?;
        self.record(0x409b93,self.root,Event::Geometry(0x409d20));
        self.child2.as_mut().unwrap().native_return=Field::Known(-1);
        if p.cells().byte(A+0x184)?!=0{return Err("A184 must remain incomplete through41e759".into())}
        self.record(0x41e759,self.root,Event::Stop);Ok(())
    }
}
struct BirthReset<'a,'p>{product:&'a mut Product,primitives:&'a mut Primitives,fonts:&'a mut Fonts,pe:&'a PeImage<'p>,zero:f32}
impl BirthReset<'_,'_>{
    fn parent(&self,o:u32)->Result<NodeId,String>{self.product.aliases.get(&o).copied().ok_or_else(||format!("business shared parent+{o:x} absent"))}
    fn null(&self)->Result<(),String>{if self.primitives.cells().word(0x590f60)?!=0{return Err("business409d20 reached selected590f60 unavailable".into())}Ok(())}
    fn select(&mut self,o:u32)->Result<(),String>{let id=self.parent(o)?;let v=self.product.visual_mut(id)?;if v.policy338==1{v.selected336=1;}Ok(())}
}
impl NullReset for BirthReset<'_,'_>{
    fn scalar(&mut self,o:u32,v:u32,byte:bool)->Result<(),String>{
        if byte{self.primitives.construction_cells()?.set_byte(BUSINESS+o,v as u8)?;
            self.product.record(0x409d20,self.product.root,Event::ScalarStore{field:BUSINESS+o,value:v});Ok(())}
        else{self.product.business_word(self.primitives,o,v,0x409d20)}
    }
    fn hide(&mut self,o:u32,_optional:bool)->Result<bool,String>{
        //Every reached pointer is concretely allocated; missing is an error,
        //not an optional projection or a successful fabricated NULL.
        let id=self.product.business_field(o)?;let n=self.product.node_mut(id)?;n.shown=0;
        match &mut n.subtype{Subtype::List3Texts(l)=>l.list.shown=0,Subtype::SpecialList3Texts(l)=>l.list.shown=0,Subtype::Visual(_)|Subtype::Text(_)=>(),_=>return Err("business hide actual subtype mismatch".into())}Ok(true)
    }
    fn block(&mut self,o:u32)->Result<(),String>{let id=self.product.business_field(o)?;let n=self.product.node_mut(id)?;let Subtype::Visual(v)=&mut n.subtype else{return Err("business block needs actual Visual".into())};v.disabled333=1;v.down335=0;Ok(())}
    fn prepare(&mut self)->Result<(),String>{for i in 0..5{let id=self.parent(0x4a4+i*4)?;self.product.reset_control_node(id)?;}Ok(())}
    fn selector(&mut self)->Result<(),String>{let id=self.product.business_field(0xb8)?;self.product.show_selector(id,0,self.zero,0x4326b0)}
    fn zero_notice(&mut self)->Result<(),String>{self.product.business_currency(self.pe,self.primitives,self.fonts,true,0x408ae7)}
    fn mode_zero(&mut self)->Result<(),String>{
        let id=self.parent(0x4b8)?;self.product.text_mut(id)?.reset(self.fonts)?;self.info_visibility()?;
        self.product.text_mut(id)?.identifier(self.fonts,0x1e)?;
        let list=self.product.business_field(0x64)?;self.product.node_mut(list)?.shown=1;self.product.columns_mut(list)?.shown=1;
        self.null()?;let t=self.product.text_mut(id)?;t.resize()?;for c in &mut t.colors[..t.group.count as usize]{*c=16;}Ok(())
    }
    fn select_control_zero(&mut self)->Result<(),String>{self.select(0x48c)}
    fn select_info_zero(&mut self)->Result<(),String>{self.select(0x4a4)}
    fn info_visibility(&mut self)->Result<(),String>{
        if self.primitives.cells().byte(A+0x509)?!=1&&self.primitives.cells().byte(A+0x508)?!=0{
            for o in [0x4a4,0x4a8]{let id=self.parent(o)?;self.product.node_mut(id)?.shown=1;}}
        Ok(())
    }
    fn activation_null(&mut self)->Result<(),String>{self.null()}
}
#[cfg(test)]
#[path="main_business_construction_tests.rs"]
mod tests;

#[path="main_parent_tail_construction.rs"]
mod parent_tail;
pub use parent_tail::CompletedMainLayout;

impl Product {
    fn parent_business_exit(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts)->Result<(),String>{
        if p.cells().word(0x590f60)?!=0{return Err("407890 selected590f60 unavailable".into())}
        other_child_resets::null_exit(&mut BirthReset{product:self,primitives:p,fonts,pe,zero:float(pe,0x525040)?})
    }
}
