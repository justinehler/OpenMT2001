//! Actual41dcf0 continuation41e759..41ea70, source-born no selection.
//! SAME move-only Product; not43cc60 completion, FullMain or Graph activation.
use super::*;
use crate::intro::camera_live::case_zero;

///The entire41dcf0 layout returned. Native43ba52 binding suffix and43cf4e
///registration still require the actual containing Primary factory/group.
///No writable facade, Clone, register/install conversion or input is exported.
pub struct CompletedMainLayout {prefix:AwaitingParentTail}
impl Deref for CompletedMainLayout {type Target=AwaitingParentTail;fn deref(&self)->&Self::Target{&self.prefix}}
impl CompletedMainLayout {
    pub fn phase(&self)->MainPhase{self.prefix.phase()}
    pub fn require_full_main(&self)->Result<(),String>{
        Err("41dcf0 completed/AwaitingBinding:43ba52 A184 store and43cf4e/43c600 require actual43cc60 Primary group after Clock and all12 preceding owned registrations; containing factory,41eb40/Graph/input/timers unavailable".into())
    }
    pub fn require_input_or_render(&self)->Result<(),String>{Err("Completed Main layout is AwaitingBinding; FullMain/Graph/input/render activation stays closed".into())}
    ///Foreign source/resource/owner/phase rejects before any move or write.
    ///Reached failures retain SAME partial owners in slot, faulted, no replay.
    pub fn continue_parent_tail(slot:&mut Option<AwaitingParentTail>,executable:&[u8],text:&mut UiTextResources,
        p:&mut Primitives,images:&mut Images)->Result<Self,String>{
        let prior=slot.as_ref().ok_or("41e759 parent tail already consumed; no replay")?;
        let product=&prior.prefix.prefix.prefix.inner;
        if product.phase!=MainPhase::AwaitingParentTail{return Err("41e759 requires retained AwaitingParentTail; no replay".into())}
        p.check_main_tail_owner(product.owner)?;
        if !product.text_identity.same(&text.construction_identity)||!product.image_identity.same(&images.construction_identity){
            return Err("41e759 requires SAME boxed Fonts/language and retained image owner".into())
        }
        if images.ui_dimensions!=[640,480]{return Err("41e759 requires actual retained640 image domain".into())}
        let pe=PeImage::parse(executable)?;
        for (at,n) in [(0x41dcf0,0xe04),(0x40a910,0x37),(0x4021a0,0x9ec),(0x402ba0,0x130),
            (0x4048a0,0x270),(0x407890,0xd3),(0x409e90,0x2b),(0x41f080,0x2a),
            (0x41f120,0x60),(0x41fc60,0x20),(0x43bed0,0x92),(0x41ed90,1),(0x43b370,1),
            (0x43b040,0xb0),(0x43e9a0,0xa0),(0x440160,0x100),(0x440650,0x570),
            (0x441770,0x60),(0x441940,0xb0),(0x442af0,0x20),(0x4326b0,0x1c0),
            (0x52501c,0xa8),(0x525668,0x48),(0x525908,0x34),(0x533238,0x44),
            (0x533278,8),(0x533848,0x30),(0x534ce8,16)]{
            if pe.at(at,n)?!=p.cells().read(at,n)?{return Err(format!("41e759 source{at:x} differs from SAME retained PE"))}
        }
        for (table,expected) in [(0x41eaac,[0x41e95d,0x41e96a,0x41e977,0x41e98f,0x41e98f,0x41e984]),
            (0x41eac4,[0x41e9d0,0x41e9dd,0x41e9ea,0x41ea02,0x41ea02,0x41e9f7]),
            (0x41eadc,[0x41ea37,0x41ea54,0x41ea71,0x41ea99,0x41ea99,0x41ea8e])]{
            for (i,v) in expected.into_iter().enumerate(){if word(&pe,table+i as u32*4)?!=v{return Err(format!("41dcf0 dispatch table{table:x} mode{i} differs"))}}
        }
        if word(&pe,0x525908+0x2c)?!=0x43b370||word(&pe,0x525668+0x38)?!=0x41ed90{
            return Err("41e759 concrete Text default RET or A specialized hide RET differs".into())
        }
        if p.cells().byte(A+0x184)?!=0||p.cells().word(A+0x388)?!=product.counter{
            return Err("41e759 binding/counter phase differs from retained constructor".into())
        }
        for (o,address) in [(0x38c,0x560920),(0x390,0x560820),(0x394,0x5608e8),(0x398,0x560930)]{
            if p.cells().word(A+o)?!=address{return Err(format!("41e759 actual child pointer{o:x} differs"))}
        }
        //Distinct real source-born authorities; neither is written here.
        if p.cells().word(0x590f60)?!=0{return Err("41e759 no-selection birth requires independent590f60 NULL; selected business unavailable".into())}
        if p.cells().word(0x822d50)?!=0{return Err("41e759 actual822d50 selected property/input family unavailable".into())}
        if p.cells().word(0x560820+0x20)?!=0||p.cells().word(0x560820+0x24)?!=0{
            return Err("41e759 actual child1 mode/report differs from completed source-born no-selection layout".into())
        }
        if product.child0.is_none()||product.child1.is_none()||product.child5.is_none()||product.child2.is_none(){
            return Err("41e759 mandatory retained child lifecycle owner absent".into())
        }
        for o in [0x3a0,0x4b8]{let id=product.parent_alias(o)?;if !matches!(product.node(id)?.subtype,Subtype::Text(_)){return Err("41e759 required shared Text subtype differs".into())}}
        for o in (0..6).map(|i|0x48c+i*4).chain((0..5).map(|i|0x4a4+i*4)).chain([0x4c4,0x4c8]){
            let id=product.parent_alias(o)?;if !matches!(product.node(id)?.subtype,Subtype::Visual(_)){return Err("41e759 actual mode/info/navigation Visual subtype differs".into())}
        }
        let selector=product.parent_alias(0x4cc)?;
        if !matches!(product.node(selector)?.subtype,Subtype::Selector2Arrows(_)){return Err("41e759 parent selector subtype differs".into())}
        if product.aliases.contains_key(&0x4bc)||product.aliases.contains_key(&0x4c0){return Err("41e759 additional Text already allocated; no replay".into())}
        let panel=p.panels.get(&A).ok_or("41e759 actual A primitive absent")?;
        if panel.phase!=crate::intro::camera_live::primitive_factory::Phase::AwaitingLayout||panel.descriptor!=Some(crate::intro::camera_live::primitive_factory::DescriptorIdentity::Archive(PanelId::A)){
            return Err("41e759 actual A bind-prefix owner differs".into())
        }
        p.claim_main_tail(product.owner)?;
        slot.as_mut().unwrap().prefix.prefix.prefix.inner.phase=MainPhase::ConstructingParentTail;
        let result=slot.as_mut().unwrap().prefix.prefix.prefix.inner.parent_tail(&pe,p,text.fonts_mut(),images)
            .map_err(|e|format!("41dcf0 parent tail: {e}"));
        match result{
            Ok(())=>{
                p.main_layout_returned();slot.as_mut().unwrap().prefix.prefix.prefix.inner.phase=MainPhase::AwaitingBinding;
                Ok(Self{prefix:slot.take().expect("checked SAME retained parent tail")})
            },
            Err(e)=>{
                let product=&mut slot.as_mut().unwrap().prefix.prefix.prefix.inner;
                product.record(0x41dcf0,product.root,Event::Fault(e.clone()));product.phase=MainPhase::Failed;p.main_failed();Err(e)
            },
        }
    }
}
impl Product {
    fn parent_alias(&self,o:u32)->Result<NodeId,String>{self.aliases.get(&o).copied().ok_or_else(||format!("41dcf0 actual parent+{o:x} owner absent"))}
    fn parent_word(&mut self,p:&mut Primitives,o:u32,value:u32,source:u32)->Result<(),String>{
        p.construction_cells()?.set_word(A+o,value)?;self.record(source,self.root,Event::ScalarStore{field:A+o,value});Ok(())
    }
    fn parent_byte(&mut self,p:&mut Primitives,o:u32,value:u8,source:u32)->Result<(),String>{
        p.construction_cells()?.set_byte(A+o,value)?;self.record(source,self.root,Event::ScalarStore{field:A+o,value:u32::from(value)});Ok(())
    }
    fn parent_clear_notice(&mut self,fonts:&mut Fonts,source:u32)->Result<(),String>{
        let id=self.parent_alias(0x3a0)?;self.text_mut(id)?.reset(fonts).map_err(|e|format!("{source:x} parent+3a0 node{id:?} Text/441940: {e}"))?;
        self.record(source,id,Event::TextCall(0x441940));Ok(())
    }
    fn parent_colors(&mut self,pe:&PeImage<'_>,p:&mut Primitives,source:u32)->Result<(),String>{
        let mut colors=[0;4];
        for (i,color) in colors.iter_mut().enumerate(){
            *color=word(pe,0x534ce8+i as u32*4)?;
            self.parent_word(p,0x40c+i as u32*4,*color,source)?;
        }
        self.regions.get_mut(2).ok_or("41f080 actual region2 color owner missing")?.1=colors;Ok(())
    }
    fn parent_conditional_select(&mut self,id:NodeId,value:u8,source:u32)->Result<(),String>{
        let v=self.visual_mut(id)?;if v.policy338==1{v.selected336=value;}
        self.record(source,id,Event::Geometry(0x442af0));Ok(())
    }
    fn parent_tail(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts,images:&mut Images)->Result<(),String>{
        let selector=self.parent_alias(0x4cc)?;self.show_selector(selector,0,float(pe,0x525040)?,0x41e761)?;
        for (o,source) in [(0x4c4,0x41e76c),(0x4c8,0x41e77e)]{
            let id=self.parent_alias(o)?;self.node_mut(id)?.shown=0;self.record(source,id,Event::Show(0));
        }
        let mode0=self.parent_alias(0x48c)?;self.parent_conditional_select(mode0,1,0x41e784)?;
        self.parent_byte(p,0x508,1,0x41e7a4)?;self.parent_byte(p,0x509,0,0x41e7ab)?;
        self.parent_word(p,0x4f4,u32::MAX,0x41e7b2)?;
        let title=self.temporary(pe,p,0x53385c,0x41e7bc)?;
        //Second real43b040 executes and destroys its Text but its extent is
        //not copied. Do not replace the first title extent with its result.
        let _discarded=self.temporary(pe,p,0x533848,0x41e801)?;
        let left=add(title[0],float(pe,0x5250a0)?)?;
        let right=add(title[2],float(pe,0x5250a0)?)?;
        let first=[left,title[1],right,add(title[1],float(pe,0x525030)?)?,0.5];
        let id=self.make_text(Label::Additional(0x4bc),first,self.counter,0x41e86b)?;
        self.alias(0x4bc,id,0x41e87d)?;self.increment(p,0x41e883)?;
        self.text_mut(id)?.literal(fonts,&ntbs(pe,0x533278)?,&[]).map_err(|e|format!("41e889 parent+4bc Text/441770: {e}"))?;self.record(0x41e889,id,Event::TextCall(0x441770));
        self.text_mut(id)?.style440650(4,-999,-999);self.record(0x41e8a3,id,Event::TextCall(0x440650));
        self.text_mut(id)?.set_font(fonts,0)?;self.record(0x41e8b0,id,Event::TextCall(0x440b70));
        //41e8d5 FST retains unrounded ST0 for the following ADD/41e8e7 FSTP.
        let mut top=sum(title[1])?;top.add_f32(float(pe,0x5250a0)?).map_err(str::to_owned)?;
        let second_top=store(top)?;top.add_f32(float(pe,0x525030)?).map_err(str::to_owned)?;
        let second=[left,second_top,right,store(top)?,0.5];
        let id=self.make_text(Label::Additional(0x4c0),second,self.counter,0x41e8f6)?;
        self.alias(0x4c0,id,0x41e908)?;self.increment(p,0x41e90e)?;
        self.text_mut(id)?.literal(fonts,&ntbs(pe,0x533278)?,&[]).map_err(|e|format!("41e914 parent+4c0 Text/441770: {e}"))?;self.record(0x41e914,id,Event::TextCall(0x441770));
        self.text_mut(id)?.style440650(4,-999,-999);self.record(0x41e92e,id,Event::TextCall(0x440650));
        self.text_mut(id)?.set_font(fonts,0)?;self.record(0x41e93b,id,Event::TextCall(0x440b70));
        self.parent_clear_notice(fonts,0x41e946)?;
        //Actual stored OLD39c, not Scene/camera or the newly requested mode.
        match p.cells().word(A+0x39c)? {
            0=>{
                self.record(0x41e963,self.root,Event::Callback(0x40a910));
                case_zero::exit_finite(&mut BirthZeroExit{product:self,p,fonts,pe})?;
            },
            1=>{self.record(0x41e970,self.root,Event::Callback(0x402d10));self.parent_property_exit(pe,p,fonts)?;},
            2=>{self.record(0x41e97d,self.root,Event::Callback(0x407890));self.parent_business_exit(pe,p,fonts)?;},
            5=>{
                self.record(0x41e98a,self.root,Event::Callback(0x409e90));
                self.parent_clear_notice(fonts,0x409e9c)?;self.parent_colors(pe,p,0x409ea9)?;
                let id=self.child5.as_ref().ok_or("409e90 actual child5 absent")?.list()?;
                self.node_mut(id)?.shown=0;self.columns_mut(id)?.shown=0;
            },
            _=>(), //actual switch3/4 and unsigned>5 default has no callback
        }
        self.parent_word(p,0x39c,1,0x41e98f)?;
        for i in 0..6u32{
            let id=self.parent_alias(0x48c+i*4)?;self.reset_control_node(id)?;
            self.record(0x41e9aa,id,Event::Geometry(0x41f120));
        }
        //The actual enter table now selects child1; its null entry has real
        //mode/info image/animation, reset/exit/title/caption/font effects.
        self.record(0x41e9e3,self.root,Event::Callback(0x4024b0));
        self.parent_property_enter(pe,p,fonts,images)?;
        if p.cells().byte(A+0x508)?==0{
            return Err("41ea0e collapsed41fae0 model/child consumer unavailable outside source-born expanded branch".into())
        }
        self.record(0x41ea5a,self.root,Event::Callback(0x4021a0));
        self.parent_property_activate(p,fonts,float(pe,0x525040)?)?;
        self.record(0x41ea61,self.root,Event::Callback(0x43bed0));
        self.parent_hide_birth(pe,p)?;
        self.record(0x41ea70,self.root,Event::Returned(0x41dcf0));Ok(())
    }
    ///Actual43bed0 over SAME panel Cells/descriptor. At this birth345==0:
    ///skip motion/slot38/group14 cleanup, then apply independent-byte guard.
    ///A non-birth requested branch retains reached common/motion effects and
    ///requires a real group owner at43bf29; raw zero cells are not that owner.
    fn parent_hide_birth(&mut self,pe:&PeImage<'_>,p:&mut Primitives)->Result<(),String>{
        if p.cells().byte(A+0x345)?!=0 {
            let count=p.cells().word(A+0x180)? as i32;
            self.parent_byte(p,0x344,1,0x43bee4)?;self.parent_byte(p,0x345,0,0x43beee)?;
            if count>1{
                let descriptor=p.resources.descriptor(PanelId::A)?;
                if p.panels.get(&A).ok_or("43bed0 actual panel absent")?.descriptor!=Some(crate::intro::camera_live::primitive_factory::DescriptorIdentity::Archive(descriptor.slot)){
                    return Err("43bf06 actual descriptor owner mismatch".into())
                }
                let position=descriptor.position;let divisor=Sum::from_i32(count.wrapping_sub(1));
                for i in 0..2{let mut velocity=sum(position[i])?;velocity.div(divisor).map_err(str::to_owned)?;
                    self.parent_word(p,0x364+i as u32*4,store(velocity.negated())?.to_bits(),0x43bf0d+i as u32*13)?;}
            }
            if word(pe,0x525668+0x38)?!=0x41ed90{return Err("43bf26 A specialized hide target unavailable".into())}
            self.record(0x43bf26,self.root,Event::Callback(0x41ed90)); //actual RET, no child exits
            let group=p.cells().word(A+0x138)?;
            if group!=0{
                return Err(format!("43bf29/43bf37 group{group:08x}+14 cleanup needs actual retained containing group and typed target/root owners;43cc60 factory unavailable"));
            }
        }
        if p.cells().byte(A+0x34e)?!=0{self.parent_byte(p,0x344,0,0x43bf58)?;}
        Ok(())
    }
}
struct BirthZeroExit<'a,'p>{product:&'a mut Product,p:&'a mut Primitives,fonts:&'a mut Fonts,pe:&'a PeImage<'p>}
impl case_zero::ExitView for BirthZeroExit<'_,'_>{
    fn clear_notice(&mut self)->Result<(),String>{self.product.parent_clear_notice(self.fonts,0x40a91b)}
    fn restore_colors(&mut self)->Result<(),String>{self.product.parent_colors(self.pe,self.p,0x40a927)}
    fn hide_list(&mut self)->Result<(),String>{
        let id=self.product.child0.as_ref().ok_or("40a910 actual child0 absent")?.list;
        self.product.node_mut(id)?.shown=0;self.product.columns_mut(id)?.shown=0;Ok(())
    }
    fn hide_selector(&mut self)->Result<(),String>{
        let id=self.product.child0.as_ref().ok_or("40a910 actual child0 absent")?.selector;
        self.product.show_selector(id,0,float(self.pe,0x525040)?,0x40a941)
    }
}
#[cfg(test)]
#[path="main_parent_tail_construction_tests.rs"]
mod tests;
