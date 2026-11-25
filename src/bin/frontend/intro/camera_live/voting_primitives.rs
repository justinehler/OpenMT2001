//! Voting's concrete birth/bind/layout storage in the SAME process allocation.
use super::*;
use super::super::primary_d_construction::PrimaryPhase;
const V:u32=0x573b38;
const D:u32=0x574488;
const G:u32=0x57e710;
const ID:PanelId=PanelId::OtherPrimary(1);
impl Builder {
    ///439df0, a finite CRT birth projection; never called by layout/bind.
    pub(super) fn voting_static(&mut self)->Result<(),String>{
        self.admit_span(V,0x5c8)?;self.panel(V)?;self.spans.insert(V,0x5c8);
        //Actual first-writer order: bottom/right/top/left/depth, seven records.
        let born=|c:&mut Cells,offset:u32|->Result<(),String>{
            for o in [12,8,4,0]{c.set_word(V+offset+o,0)?;}
            c.set_word(V+offset+16,0.5f32.to_bits())
        };
        for offset in [0x388,0x39c]{born(&mut self.cells,offset)?;}
        for base in [0x3b0,0x43c]{for i in 0..7{born(&mut self.cells,base+20*i)?;}}
        for offset in [0x4c8,0x4e4]{born(&mut self.cells,offset)?;}
        for i in 0..7{born(&mut self.cells,0x4f8+20*i)?;}
        //remaining/selected/aliases/padding are NOT constructor writers.
        self.cells.set_word(V,0x52571c)
    }
}
impl Primitives {
    pub(in crate::intro::camera_live) fn check_voting_ready(&self)->Result<(),String>{
        if self.provenance!=Provenance::DeferredClaimed||self.primary_phase!=Some(PrimaryPhase::AwaitingVoting)
            ||self.primary_pool.as_ref().and_then(|p|p.get(15))!=Some(&ID)
            ||self.cells().word(G+0x60)?!=V||self.cells().word(G+0x24)?!=17
            ||self.cells().word(G+8)?!=D||self.cells().word(G+12)?!=D{
            return Err("Voting requires actual fresh returned/registered D and pool15; no replay".into())
        }
        let panel=self.panels.get(&V).ok_or("Voting static Panel absent")?;
        if panel.phase!=Phase::Constructed||panel.descriptor.is_some()||self.cells().byte(V+0x184)?!=0
            ||self.cells().word(V)?!=0x52571c||self.cells().word(0x525720)?!=0x42c000{
            return Err("Voting static owner/actual42c000 dispatch differs".into())
        }
        if self.resources.resolve(ID)?!=Some(ID){return Err("Voting pool15 has missing/null/different model reference".into())}
        self.texts.get(&(V+0x188)).ok_or("Voting SAME embedded caption absent")?;
        for (at,target) in [(0x52590c,0x42f140),(0x525908,0x440900),(0x525934,0x43b370),(0x5259a8,0x442600)]{
            if self.cells().word(at)?!=target{return Err("Voting concrete Text/Visual dispatch differs".into())}
        }
        Ok(())
    }
    ///Concrete43cdeb. All loans, including failed binds, restore SAME owners.
    pub(in crate::intro::camera_live) fn voting_bind(&mut self)->Result<(),String>{
        self.check_voting_ready()?;
        self.primary_phase=Some(PrimaryPhase::LayingOutVoting);
        let mut b=Builder{cells:self.cells.take().ok_or("process cells borrowed")?,
            panels:std::mem::take(&mut self.panels),caption_keys:std::mem::take(&mut self.caption_keys),
            spans:std::mem::take(&mut self.spans),resources:std::mem::replace(&mut self.resources,CpuResources::new()),
            steps:std::mem::take(&mut self.steps)};
        let result=b.bind_prefix(V,Some(ID),None,Group::Primary);
        self.cells=Some(b.cells);self.panels=b.panels;self.caption_keys=b.caption_keys;self.spans=b.spans;
        self.resources=b.resources;self.steps=b.steps;
        //Caption has one writable typed authority. Project only reached raw
        //style writes, including a bind that later failed at frame prepare.
        let style=self.cells().word(V+0x188+0x1a0)?;
        let dirty=self.cells().byte(V+0x188+0x1a8)?;
        let caption=self.text_mut(V+0x188)?;caption.style=style;caption.dirty=dirty;
        result
    }
    pub(in crate::intro::camera_live) fn voting_cells(&mut self)->Result<&mut Cells,String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutVoting){return Err("Voting storage outside reached body".into())}
        self.cells.as_mut().ok_or_else(||"process cells borrowed".into())
    }
    ///43af30 re-reads the live frame/count and SAME descriptor/model each call.
    pub(in crate::intro::camera_live) fn voting_named(&mut self,name:u32)->Result<[f32;5],String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutVoting){return Err("Voting named lookup outside actual body".into())}
        let panel=self.panels.get(&V).ok_or("Voting actual Panel absent")?;
        if panel.phase!=Phase::AwaitingLayout||panel.descriptor!=Some(DescriptorIdentity::Archive(ID)){
            return Err("Voting named lookup lacks SAME bound descriptor".into())
        }
        let count=self.cells().word(V+0x180)?;
        let frame=count.checked_sub(1).ok_or("Voting live frame underflow")? as usize;
        let name=self.cells().ntbs(name,256)?;
        let key=self.resources.resolve(ID)?.ok_or("Voting actual model reference is null")?;
        let model=self.resources.models.get_mut(&key).ok_or("Voting model absent")?;
        if frame>=model.frames.len(){return Err("Voting live frame leaves SAME model".into())}
        let part=model.find(Some(&name))?;
        let mut e=[0.,0.,0.,0.,0.5];
        if let Some(r)=model.region(part,frame)?{e[..4].copy_from_slice(&r)}
        Ok(e) //native defined missing-name extent; not a missing model success
    }
    ///Only the concrete caller reaches this after the real layout returned.
    pub(in crate::intro::camera_live) fn voting_bind_return_and_register(&mut self,product:&super::super::voting_construction::VotingProduct)->Result<(),String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutVoting)||product.phase()!=super::super::voting_construction::Phase::Returned
            ||!product.same_process(self){return Err("Voting suffix outside SAME actually returned callback".into())}
        let panel=self.panels.get_mut(&V).ok_or("Voting actual Panel absent")?;
        if panel.phase!=Phase::AwaitingLayout||panel.descriptor!=Some(DescriptorIdentity::Archive(ID)){
            return Err("Voting suffix lacks actual retained bind".into())
        }
        let c=self.cells.as_mut().ok_or("process cells borrowed")?;
        self.steps.push(Step::LayoutReturned(V));
        c.set_byte(V+0x184,1)?;panel.phase=Phase::Bound;self.steps.push(Step::Bound(V));
        registration::promote(c,Group::Primary,V)?;self.steps.push(Step::Registered(V));
        //Count was incremented by43cdda after D, not again by Voting.
        self.primary_phase=Some(PrimaryPhase::AwaitingEventList);Ok(())
    }
}

#[cfg(test)]
impl Primitives{
    ///Controlled inherited-frame branch only; no production context producer.
    pub(in crate::intro::camera_live) fn test_voting_inherited_frame(&mut self,frame:u32)->Result<(),String>{
        if self.provenance!=Provenance::DeferredClaimed||self.primary_phase!=Some(PrimaryPhase::Allocated){return Err("frame canary requires untouched fresh allocation".into())}
        self.cells.as_mut().ok_or("process cells borrowed")?.set_word(V+0x24,frame)
    }
}
