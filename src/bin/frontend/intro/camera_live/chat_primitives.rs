//! Concrete43a770/43ce34 Chat storage in the SAME process allocation.
use super::*;
use super::super::primary_d_construction::PrimaryPhase;
const C:u32=0x57ef28;
const P:u32=0x571378;
const D:u32=0x574488;
const G:u32=0x57e710;
const ID:PanelId=PanelId::OtherPrimary(3);
impl Builder {
    pub(super) fn chat_static43a770(&mut self,texts:&mut BTreeMap<u32,Text>)->Result<(),String>{
        self.admit_span(C,0x80c)?;self.panel(C)?;self.spans.insert(C,0x80c);
        texts.insert(C+0x188,super::super::case_zero_factories::text_owner(&self.cells,C+0x188)?);
        for offset in [0x388,0x53c]{
            caption(&mut self.cells,C+offset,&mut self.steps)?;
            texts.insert(C+offset,super::super::case_zero_factories::text_owner(&self.cells,C+offset)?);
            self.caption_keys.insert(C+offset);
        }
        for i in 0..12{
            for o in [12,8,4,0]{self.cells.set_word(C+0x6f0+20*i+o,0)?;}
            self.cells.set_word(C+0x6f0+20*i+16,0.5f32.to_bits())?;
        }
        //Action1/+7e4 and recipient/input preimages have no constructor writer.
        self.cells.set_word(C,0x5258c0)
    }
}
impl Primitives {
    pub(in crate::intro::camera_live) fn check_chat_ready(&self)->Result<(),String>{
        if self.provenance!=Provenance::DeferredClaimed||self.primary_phase!=Some(PrimaryPhase::AwaitingChat)
            ||self.primary_pool.as_ref().and_then(|p|p.get(13))!=Some(&ID)
            ||self.cells().word(G+0x84)?!=C||self.cells().word(G+0x24)?!=17
            ||self.cells().word(G+8)?!=P||self.cells().word(G+12)?!=D
            ||self.cells().word(P+0x380)?!=0x573b38||self.cells().word(P+0x384)?!=0
            ||self.cells().word(0x573b38+0x384)?!=P||self.cells().byte(P+0x184)?!=1{
            return Err("Chat requires SAME actually returned/registered EventList before descriptor13; no replay".into())
        }
        if self.panels.get(&P).is_none_or(|v|v.phase!=Phase::Bound||v.descriptor!=Some(DescriptorIdentity::Archive(PanelId::OtherPrimary(2)))){
            return Err("Chat prior EventList bind owner differs".into())
        }
        let panel=self.panels.get(&C).ok_or("Chat static Panel absent")?;
        if panel.phase!=Phase::Constructed||panel.descriptor.is_some()||self.cells().byte(C+0x184)?!=0
            ||self.cells().word(C)?!=0x5258c0||self.cells().word(0x5258c4)?!=0x41a510{
            return Err("Chat static owner/actual41a510 dispatch differs".into())
        }
        if self.resources.resolve(ID)?!=Some(ID){return Err("Chat descriptor13 has missing/null/different model".into())}
        self.texts.get(&(C+0x188)).ok_or("Chat SAME embedded caption absent")?;
        for at in [C+0x388,C+0x53c]{self.texts.get(&at).ok_or("Chat SAME title/to Text absent")?;}
        for (at,target) in [(0x52590c,0x42f140),(0x525934,0x43b370),(0x5259f8,0x42f140),(0x5259a4,0x432890),(0x5259a8,0x442600)]{
            if self.cells().word(at)?!=target{return Err("Chat concrete Text dispatch differs".into())}
        }Ok(())
    }
    pub(in crate::intro::camera_live) fn chat_bind(&mut self)->Result<(),String>{
        self.check_chat_ready()?;self.primary_phase=Some(PrimaryPhase::LayingOutChat);
        let mut b=Builder{cells:self.cells.take().ok_or("process cells borrowed")?,
            panels:std::mem::take(&mut self.panels),caption_keys:std::mem::take(&mut self.caption_keys),
            spans:std::mem::take(&mut self.spans),resources:std::mem::replace(&mut self.resources,CpuResources::new()),
            steps:std::mem::take(&mut self.steps)};
        let result=b.bind_prefix(C,Some(ID),None,Group::Primary);
        self.cells=Some(b.cells);self.panels=b.panels;self.caption_keys=b.caption_keys;self.spans=b.spans;
        self.resources=b.resources;self.steps=b.steps;
        //SAME caption remains in Primitives, including after partial binding.
        let style=self.cells().word(C+0x188+0x1a0)?;let dirty=self.cells().byte(C+0x188+0x1a8)?;
        let caption=self.text_mut(C+0x188)?;caption.style=style;caption.dirty=dirty;result
    }
    pub(in crate::intro::camera_live) fn chat_cells(&mut self)->Result<&mut Cells,String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutChat){return Err("Chat storage outside reached body".into())}
        self.cells.as_mut().ok_or_else(||"process cells borrowed".into())
    }
    pub(in crate::intro::camera_live) fn chat_named(&mut self,name:u32)->Result<[f32;5],String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutChat){return Err("Chat query outside reached body".into())}
        let panel=self.panels.get(&C).ok_or("Chat actual Panel absent")?;
        if panel.phase!=Phase::AwaitingLayout||panel.descriptor!=Some(DescriptorIdentity::Archive(ID)){
            return Err("Chat query lacks SAME bound descriptor".into())
        }
        let frame=self.cells().word(C+0x180)?.checked_sub(1).ok_or("Chat live frame underflow")? as usize;
        let name=self.cells().ntbs(name,256)?;
        let key=self.resources.resolve(ID)?.ok_or("Chat actual model reference is null")?;
        let model=self.resources.models.get_mut(&key).ok_or("Chat model absent")?;
        if frame>=model.frames.len(){return Err("Chat live frame leaves SAME model".into())}
        let part=model.find(Some(&name))?;let mut e=[0.,0.,0.,0.,0.5];
        if let Some(r)=model.region(part,frame)?{e[..4].copy_from_slice(&r)}
        Ok(e) //43af30's defined missing-name fallback; never a missing model
    }
    pub(in crate::intro::camera_live) fn chat_bind_return_and_register(&mut self,product:&super::super::chat_construction::ChatProduct)->Result<(),String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutChat)
            ||product.phase()!=super::super::chat_construction::Phase::Returned||!product.same_process(self){
            return Err("Chat suffix outside SAME actual41a919 return".into())
        }
        let panel=self.panels.get_mut(&C).ok_or("Chat actual Panel absent")?;
        if panel.phase!=Phase::AwaitingLayout||panel.descriptor!=Some(DescriptorIdentity::Archive(ID)){
            return Err("Chat suffix lacks actual retained bind".into())
        }
        let c=self.cells.as_mut().ok_or("process cells borrowed")?;
        self.steps.push(Step::LayoutReturned(C));c.set_byte(C+0x184,1)?;
        panel.phase=Phase::Bound;self.steps.push(Step::Bound(C));
        registration::promote(c,Group::Primary,C)?;self.steps.push(Step::Registered(C));
        self.primary_phase=Some(PrimaryPhase::AwaitingG);Ok(())
    }
}

#[cfg(test)]
impl Primitives{
    pub(in crate::intro::camera_live) fn test_chat_inherited_frame(&mut self,frame:u32)->Result<(),String>{
        if self.provenance!=Provenance::DeferredClaimed||self.primary_phase!=Some(PrimaryPhase::Allocated){return Err("Chat frame canary outside untouched allocation".into())}
        self.cells.as_mut().ok_or("process cells borrowed")?.set_word(C+0x24,frame)
    }
}
