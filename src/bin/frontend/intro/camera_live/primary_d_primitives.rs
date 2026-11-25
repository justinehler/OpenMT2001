//! D's actual static storage in the existing Primitives allocation.
use super::*;
use super::super::{primary_d_construction::{Currency,PrimaryPhase},ui_text_resources::UiTextResources};
const D:u32=0x574488;
const G:u32=0x57e710;
impl Primitives {
    ///43cc7c/91/a2, before the current Main child2 money helper. This does not
    ///certify the earlier bound F as occurring in native43cc60 order.
    pub(in crate::intro::camera_live) fn primary_currency(&mut self,text:&UiTextResources)->Result<Currency,String>{
        if self.provenance!=Provenance::Reordered{return Err("Fresh currency belongs to the concrete deferred Install prefix".into())}
        if self.primary_claimed||self.primary_currency_identity.is_some(){return Err("43cc60 prelude already claimed".into())}
        let currency=Currency::from_language(text)?;
        let c=self.cells.as_mut().ok_or("actual process allocation borrowed")?;
        c.read(0x57e6ec,currency.symbol.len())?;c.word(0x59050c)?;
        c.write(0x57e6ec,&currency.symbol)?;
        c.set_word(0x59050c,currency.suffix)?;
        self.primary_currency_identity=Some(text.construction_identity.clone());
        Ok(currency)
    }
    ///Move the SAME cells, models and panels into the shared Builder briefly.
    ///No model/F/A reconstruction; restoration occurs on every Result path.
    pub(in crate::intro::camera_live) fn primary_prepare(&mut self,models:Vec<CpuModel>)->Result<(),String>{
        if self.provenance!=Provenance::Reordered{return Err("Fresh static birth/pool/bind cannot use reordered primary_prepare".into())}
        if self.primary_claimed{return Err("Primary construction already claimed; no replay".into())}
        self.primary_claimed=true;self.primary_phase=Some(PrimaryPhase::Preparing);
        let mut b=Builder{cells:self.cells.take().ok_or("actual process cells borrowed")?,
            panels:std::mem::take(&mut self.panels),caption_keys:std::mem::take(&mut self.caption_keys),
            spans:std::mem::take(&mut self.spans),resources:std::mem::replace(&mut self.resources,CpuResources::new()),steps:std::mem::take(&mut self.steps)};
        let result=(||{
            //Actual43c570 constructor once; no alternate linked group authority.
            init::group43c570(&mut b.cells,G)?;
            for model in models {b.add_primary_model(model)?;}
            if b.resources.descriptors.len()!=16{return Err("Primary descriptor pool requires all16 actual archive products".into())}
            let mut pool=Vec::new();
            pool.try_reserve_exact(16).map_err(|_|"Primary descriptor pool allocation failed")?;
            b.cells.set_word(G+0x30,16)?;b.cells.set_byte(G+0x34,1)?;
            for ordinal in 0..16u8 {
                let entry=registration::PRIMARY.iter().find(|e|matches!(e.descriptor,registration::Descriptor::Model(i) if i==ordinal)).ok_or("actual16 pool slot absent")?;
                b.resources.resolve(entry.id)?.ok_or("archive pool slot has null model")?;
                pool.push(entry.id);b.cells.set_word(G+0x2c,u32::from(ordinal)+1)?;
            }
            self.primary_pool=Some(pool.try_into().map_err(|_|"actual pool length differs")?);
            b.cells.set_word(G+0x24,16)?;
            //Actual group storage uses typed CpuResources, never fake buffer pointer.
            for (offset,p) in [(0x38,0x58cdb0),(0x3c,0x56cdc8),(0x44,0x57f838),(0x48,D),
                (0x4c,0x58e8d0),(0x50,0x58e3c0),(0x58,0x57e798),(0x5c,0x571378),
                (0x68,0x56f3f0),(0x6c,0x58fb40),(0x54,0x58ca08),(0x70,0x575d98),
                (0x74,0x575280),(0x78,0x58f438),(0x7c,0x577048),(0x80,0x571808),
                (0x84,0x57ef28),(0x60,0x573b38)] {b.cells.set_word(G+offset,p)?;}
            b.panel(D)?;
            reset_descriptor(&mut b.cells,D+0x388)?;
            for offset in [0x4ac,0xa7c,0xc34] {
                caption(&mut b.cells,D+offset,&mut b.steps)?;
                self.texts.insert(D+offset,super::super::case_zero_factories::text_owner(&b.cells,D+offset)?);
            }
            b.cells.set_word(D,0x5255f0)?;
            self.texts.insert(D+0x188,super::super::case_zero_factories::text_owner(&b.cells,D+0x188)?);
            b.bind_descriptor_prefix(D,DescriptorIdentity::EmbeddedD,None,Group::Primary)
        })();
        self.cells=Some(b.cells);self.panels=b.panels;self.caption_keys=b.caption_keys;self.spans=b.spans;self.resources=b.resources;self.steps=b.steps;
        self.primary_phase=Some(if result.is_ok(){PrimaryPhase::AwaitingDLayout}else{PrimaryPhase::Failed});result
    }
    pub(in crate::intro::camera_live) fn primary_cells(&mut self)->Result<&mut Cells,String>{
        if !self.primary_claimed||self.primary_phase!=Some(PrimaryPhase::LayingOutD){return Err("D source write outside reached Primary layout".into())}
        self.cells.as_mut().ok_or_else(||"actual process allocation borrowed".into())
    }
    pub(in crate::intro::camera_live) fn primary_claim_d(&mut self)->Result<(),String>{
        if self.primary_phase!=Some(PrimaryPhase::AwaitingDLayout){return Err("D phase already claimed; no replay".into())}
        self.primary_phase=Some(PrimaryPhase::LayingOutD);Ok(())
    }
    pub(in crate::intro::camera_live) fn primary_fail_d(&mut self){self.primary_phase=Some(PrimaryPhase::Failed);}
    pub(in crate::intro::camera_live) fn primary_reset_d_descriptor(&mut self)->Result<(),String>{
        reset_descriptor(self.primary_cells()?,D+0x388)
    }
    pub(in crate::intro::camera_live) fn primary_park_d(&mut self)->Result<(),String>{
        if !matches!(self.primary_phase,Some(PrimaryPhase::AwaitingVoting|PrimaryPhase::AwaitingEventList|PrimaryPhase::AwaitingChat|PrimaryPhase::AwaitingG|PrimaryPhase::AwaitingDRebind)){return Err("D park requires same retained completed/parked owner".into())}
        let panel=self.panels.get_mut(&D).ok_or("D park static owner absent")?;
        if !matches!(panel.phase,Phase::Bound|Phase::Parked)||panel.descriptor!=Some(DescriptorIdentity::EmbeddedD){return Err("D park actual retained identity differs".into())}
        panel.phase=Phase::Parked;self.primary_phase=Some(PrimaryPhase::AwaitingDRebind);Ok(())
    }
    pub(in crate::intro::camera_live) fn primary_finish_d(&mut self)->Result<(),String>{
        if self.primary_phase!=Some(PrimaryPhase::LayingOutD){return Err("D finish outside reached layout phase".into())}
        let panel=self.panels.get_mut(&D).ok_or("D static Panel missing")?;
        if panel.phase!=Phase::AwaitingLayout||panel.descriptor!=Some(DescriptorIdentity::EmbeddedD){return Err("D bind suffix requires real completed layout owner".into())}
        let c=self.cells.as_mut().ok_or("actual process allocation borrowed")?;
        c.set_byte(D+0x184,1)?;panel.phase=Phase::Bound;self.steps.push(Step::Bound(D));
        registration::promote(c,Group::Primary,D)?; //actual43cdcd, only after184
        self.steps.push(Step::Registered(D));
        c.set_word(G+0x24,c.word(G+0x24)?.checked_add(1).ok_or("Primary count overflow")?)?;
        self.steps.push(Step::PrimaryCount(c.word(G+0x24)?));
        self.primary_phase=Some(PrimaryPhase::AwaitingVoting);Ok(())
    }
}
///4df9c0 changes only source-written fields, including the real old flags.
///Definition bytes44..b7, padding32..33 and other unwritten bytes survive.
pub(in crate::intro::camera_live) fn reset_descriptor(c:&mut Cells,p:u32)->Result<(),String>{
    let flags=(c.byte(p+0xb8)?&!2)|13;
    let identity=c.read(0x559730,48)?.to_vec();
    for o in [0x1c,0x20,0x24]{c.set_word(p+o,1f32.to_bits())?;}
    for o in [0x10,0x14,0x18,4,8,12,0,0x34,0x38]{c.set_word(p+o,0)?;}
    c.write(p+0x2c,&0u16.to_le_bytes())?;c.write(p+0x2e,&u16::MAX.to_le_bytes())?;
    c.set_word(p+0x28,u32::MAX)?;c.write(p+0x30,&0u16.to_le_bytes())?;
    for o in [0x3c,0x40]{c.set_word(p+o,u32::MAX)?;}
    c.set_byte(p+0xb8,flags)?;c.write(p+0xbc,&identity)?;c.write(p+0xec,&identity)
}
