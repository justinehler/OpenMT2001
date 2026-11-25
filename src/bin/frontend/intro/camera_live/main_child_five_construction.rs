//! Actual409dd0 static birth and409ec0 containing layout through41e73f.
//! The same move-only Main product stops BEFORE child2/408da0 and parent tail.
use super::*;
const FIVE:u32=0x5608e8;

///The full owner, distinct from the finite reset's state24 projection. The
///three embedded Texts and all retained rows live in the SAME arena's list.
///Known(None) records genuinely NULL arguments, never missing allocation.
pub struct ChildFive {
    pub source:u32,pub parent:NodeId,pub list:Field<NodeId>,pub state24:Field<u32>,
    pub selector_output:Field<Option<NodeId>>,pub title_output:Field<Option<NodeId>>,
    pub title_name:Field<Option<Vec<u8>>>,pub static_preimage:Vec<u8>,pub native_return:Field<i32>,
}
impl ChildFive {
    pub fn list(&self)->Result<NodeId,String>{match self.list{Field::Known(id)=>Ok(id),Field::Unknown=>Err("409ec0 list+28 has no reached publication".into())}}
}
///No Clone, graph/install conversion, registration or input/render admission.
///Deref retains child0/child1 and their exact allocation owners.
pub struct AwaitingChild2 {prefix:AwaitingChild5}
impl Deref for AwaitingChild2 {type Target=AwaitingChild5;fn deref(&self)->&Self::Target{&self.prefix}}
impl AwaitingChild2 {
    pub fn phase(&self)->MainPhase{self.prefix.phase()}
    pub fn child5(&self)->&ChildFive{self.prefix.prefix.inner.child5.as_ref().expect("successful409ec0 owns child5")}
    pub fn require_full_main(&self)->Result<(),String>{Err("41e73f incomplete: child2+398/408da0, parent tail/switch1,43bed0,184,43c600 and41eb40 remain".into())}
    ///409f00 does not clear rows or invoke42f510. No font/image/model access.
    pub fn reset_child5(&mut self,p:&mut Primitives)->Result<(),String>{
        let product=&mut self.prefix.prefix.inner;
        if product.phase!=MainPhase::AwaitingChild2||product.child5.is_none(){return Err("409f00 requires retained completed child5".into())}
        p.reset_retained_child5(product.owner)?;
        product.child5.as_mut().unwrap().state24=Field::Known(0);
        product.record(0x409f00,product.root,Event::ScalarStore{field:FIVE+0x24,value:0});Ok(())
    }
    ///Validate phase, all resource owners and reached supplied-source bytes
    ///BEFORE mutation or move. A reached failure stays in its original slot;
    ///phase Failed rejects replay even after the missing resource is restored.
    pub fn continue_child5(slot:&mut Option<AwaitingChild5>,executable:&[u8],
        text:&mut UiTextResources,p:&mut Primitives,images:&mut Images)->Result<Self,String>{
        let prefix=slot.as_ref().ok_or("409ec0 prefix already consumed; no duplicate continuation")?;
        let product=&prefix.prefix.inner;
        if product.phase!=MainPhase::AwaitingChild5{return Err("409ec0 retained prefix is not AwaitingChild5; no replay".into())}
        p.check_main_child5_owner(product.owner)?;
        if !product.text_identity.same(&text.construction_identity)||!product.image_identity.same(&images.construction_identity){
            return Err("409ec0 requires SAME retained Fonts/packed-language and GenericTextures640 owners".into())
        }
        if images.ui_dimensions!=[640,480]{return Err("409ec0 retained640 image domain changed".into())}
        let pe=PeImage::parse(executable)?;
        //409dd0 static constructor,409ec0 layout, actual caller, common
        //ordinary list/wrapper/Text dispatch and reached coordinate constants.
        for (at,n) in [(0x41dbc0,0x18),(0x409dd0,0x19),(0x409ec0,0x48),(0x41e725,0x1a),
            (0x41f750,0x236),(0x42f3d0,0x140),(0x42f510,0x230),(0x42fdf0,0x96),
            (0x43eb40,0xf0),(0x440160,0x100),(0x440650,0x570),(0x441940,0xb0),
            (0x525020,12),(0x5252e0,52),(0x525908,52)]{
            if pe.at(at,n)?!=p.cells().read(at,n)?{return Err(format!("409ec0 source{at:x} differs from retained prefix PE"))}
        }
        for (at,target) in [(0x5252e0+4,0x42fdf0),(0x525908,0x440900),
            (0x525908+4,0x42f140),(0x525908+0x2c,0x43b370)]{
            if word(&pe,at)?!=target{return Err("409ec0 required ordinary List/Text dispatch differs".into())}
        }
        if p.cells().word(A+0x394)?!=FIVE{return Err("409ec0 actual parent+394 static child identity differs".into())}
        if p.cells().word(A+0x388)?!=product.counter{return Err("409ec0 retained parent counter differs from SAME typed product".into())}
        for (index,value) in product.main_window.iter().enumerate(){
            if p.cells().word(A+0x4d4+index as u32*4)?!=value.to_bits(){return Err("409ec0 retained main-window geometry differs from SAME parent".into())}
        }
        if pe.at(FIVE,0x38)?!=p.cells().read(FIVE,0x38)?{
            return Err("409dd0 child5 static preimage differs from SAME retained source".into())
        }
        p.claim_main_child5(product.owner)?;
        slot.as_mut().unwrap().prefix.inner.phase=MainPhase::ConstructingChild5;
        let result=slot.as_mut().unwrap().prefix.inner.child_five(&pe,p,text.fonts_mut())
            .map_err(|e|format!("409dd0/409ec0 actual child5: {e}"));
        match result {
            Ok(())=>{
                p.main_complete_child5();slot.as_mut().unwrap().prefix.inner.phase=MainPhase::AwaitingChild2;
                Ok(Self{prefix:slot.take().expect("checked retained child5 prefix")})
            },
            Err(error)=>{p.main_failed();slot.as_mut().unwrap().prefix.inner.phase=MainPhase::Failed;Err(error)},
        }
    }
}
impl Product {
    fn child_five(&mut self,pe:&PeImage<'_>,p:&mut Primitives,fonts:&mut Fonts)->Result<(),String>{
        if self.child5.is_some(){return Err("child5 already materialized".into())}
        let preimage=p.cells().read(FIVE,0x38)?.to_vec();
        native_initializers::five409dd0(p.construction_cells()?,FIVE)?;
        self.record(0x409dd0,self.root,Event::Constructor(0x409dd0));
        p.construction_cells()?.set_word(FIVE+0x20,A)?;
        self.record(0x409ecc,self.root,Event::ScalarStore{field:FIVE+0x20,value:A});
        self.child5=Some(ChildFive{source:FIVE,parent:self.root,list:Field::Unknown,state24:Field::Unknown,
            selector_output:Field::Known(None),title_output:Field::Known(None),title_name:Field::Known(None),
            static_preimage:preimage,native_return:Field::Unknown});
        let mut y=sum(self.main_window[1])?;y.add_f32(float(pe,0x525020)?).map_err(str::to_owned)?;let y=ftol(y)?;
        //41f750 receives NULL selector output: right is full main_window[2],
        //not scrollbar-subtracted. Its zero high FILD word is intentional.
        let top=store(Sum::from_f64(f64::from(y as u32)).map_err(str::to_owned)?)?;
        let bottom=store(sub(self.main_window[3],float(pe,0x525028)?)?)?;
        let extent=[self.main_window[0],top,self.main_window[2],bottom,0.5];
        let list=self.make_plain_list(Label::Child5List,extent,fonts,0x41f88c)?;
        self.child5.as_mut().unwrap().list=Field::Known(list);
        self.record(0x41f8a1,list,Event::Alias(FIVE+0x28));
        self.configure_plain_list(list,fonts)?;
        self.increment(p,0x41f911)?;
        //The genuine NULL selector skips ALL43f510/431e80/4326b0 work.
        p.construction_cells()?.set_word(FIVE+0x24,0)?;
        self.child5.as_mut().unwrap().state24=Field::Known(0);
        self.record(0x409eee,self.root,Event::ScalarStore{field:FIVE+0x24,value:0});
        self.child5.as_mut().unwrap().native_return=Field::Known(-1);
        if p.cells().byte(A+0x184)?!=0{return Err("A184 must remain incomplete through41e73f".into())}
        self.record(0x41e73f,self.root,Event::Stop);Ok(())
    }
}
#[cfg(test)]
#[path="main_child_five_construction_tests.rs"]
mod tests;

#[path="main_business_construction.rs"]
mod business;
pub use business::{CompletedMainLayout,AwaitingParentTail,ChildTwo};
