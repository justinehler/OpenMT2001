//! Finite F525578 +14/+18/+20/+24/+2c, source-only and inactive.
//! Only F's +34/+38 RETs are admitted. No externally supplied callbacks run
//! during prepare; commit has no fallible consumer, replay or callback queue.
use super::{cursor::Cursor, navigation::Group, owned_factories::Content,
    process_image::Cells, registration, ui_graph::{Panel,PanelId,WidgetId,WidgetKind},
    super::Inspection};
use crate::navigator::upper::{Geometry,Message,Pointer,State};
use openmt2001::{native_input::{RetainedCells,StagedInput},x87::Sum};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T,String>;
const F:WidgetId=WidgetId{panel:PanelId::F,kind:WidgetKind::Panel};

/// Only the F layout is owned here. These offsets overlap different fields
/// in some other panel subtypes; no universal generic/default initializer.
#[derive(Clone,Debug)]
pub struct PanelCells {
    pub drag346:u8, pub pending354:u32, pub offsets:[f32;2], pub velocity_z:f32,
    pub title164:[f32;4], pub caption18c:[f32;4], pub caption_region340:u32,
    ///Identity/reference only. Panel.position is THE live descriptor X/Y
    ///authority used by renderer, hit, animation/layout and this controller.
    pub descriptor:Option<u32>, pub reference:Option<u32>,
}
impl PanelCells {
    pub fn capture(c:&Cells,p:u32)->Result<Self>{
        if p!=0x58cdb0||c.word(p)?!=0x525578{return Err("finite generic factory only owns actual F525578".into());}
        let d=c.word(p+0x37c)?;
        Ok(Self{drag346:c.byte(p+0x346)?,pending354:c.word(p+0x354)?,
            offsets:[c.float(p+0x358)?,c.float(p+0x35c)?],velocity_z:c.float(p+0x36c)?,
            title164:c.rect(p+0x164)?,caption18c:c.rect(p+0x18c)?,
            caption_region340:c.word(p+0x340)?,descriptor:(d!=0).then_some(d),
            reference:if d==0{None}else{let r=c.word(d)?;(r!=0).then_some(r)}})
    }
}
#[derive(Clone,Debug)]
pub struct ModelSlot {pub available:u8,pub model:Option<(u32,u16)>}
///Actual940cb0 indirections, independent of cached Panel.frame_count178.
///Root must maintain these at real reference/model replacement/retirement
///writers. An absent reached entry rejects; old frame_count is never used.
#[derive(Clone,Debug)]
pub struct Models {
    pub manager:u32, pub references:BTreeMap<u32,u32>, pub slots:BTreeMap<u32,ModelSlot>,
}
impl Models {
    fn capture(c:&Cells,p:&PanelCells)->Result<Self>{
        let manager=c.word(0x940cb0)?;
        let mut out=Self{manager,references:BTreeMap::new(),slots:BTreeMap::new()};
        if let Some(reference)=p.reference {
            let handle=c.word(reference)?;out.references.insert(reference,handle);
            if manager!=0 {
                let available=c.byte(manager.checked_add(0x5dc3).and_then(|v|v.checked_add(handle))
                    .ok_or("model availability address overflow")?)?;
                let model=if available==0{None}else{
                    let index=handle.checked_sub(1).ok_or("available model handle0 outside finite directory")?;
                    let address=manager.checked_add(index.checked_mul(4).ok_or("model index overflow")?)
                        .ok_or("model directory address overflow")?;
                    let model=c.word(address)?;
                    if model==0{None}else{Some((model,c.half(model.checked_add(4).ok_or("model address overflow")?)?))}
                };
                out.slots.insert(handle,ModelSlot{available,model});
            }
        }
        Ok(out)
    }
    fn frames(&self,p:&PanelCells)->Result<u16>{
        p.descriptor.ok_or("F opening reached null bound descriptor")?;
        if self.manager==0{return Err("F opening reached missing actual940cb0 manager".into());}
        let reference=p.reference.ok_or("F opening reached null descriptor reference")?;
        let handle=self.references.get(&reference).ok_or("F opening current reference has no live first-handle owner")?;
        let slot=self.slots.get(handle).ok_or("F opening current handle has no live model-directory slot")?;
        if slot.available==0{return Err("F opening reaches native null+4 model branch; no fallback frames".into());}
        let (identity,frames)=slot.model.ok_or("F opening available model has no actual metadata owner")?;
        if identity==0{return Err("F opening actual model identity is null".into());}
        Ok(frames)
    }
}
///Process-wide590504, input and model owners. Not an F-private override.
///Other panel families must use this SAME owner when their ports are joined.
#[derive(Debug)]
pub struct Shared {
    pub input:StagedInput, pub override590504:Option<WidgetId>, pub models:Models,
    pub frame_parents:Vec<(WidgetId,Option<PanelId>)>,
}
fn panel_from_raw(raw:u32)->Option<PanelId>{
    [Group::Primary,Group::Secondary,Group::Tertiary].into_iter()
        .flat_map(registration::entries).find(|e|e.object==raw).map(|e|e.id)
}
fn widget_from_raw(content:&Content,raw:u32)->Result<WidgetId>{
    if let Some(panel)=panel_from_raw(raw){Ok(WidgetId{panel,kind:WidgetKind::Panel})}else{content.id(raw)}
}
impl Shared {
    ///Move the already source-produced public native_input owner exactly
    ///once. No constructor/refresh seeds a second mouse/keyboard/CHAR owner.
    pub fn capture(c:&Cells,content:&Content,input:StagedInput,p:&PanelCells)->Result<Self>{
        let raw=c.word(0x590504)?;
        let override590504=if raw==0{None}else{Some(widget_from_raw(content,raw)?)};
        let mut frame_parents:Vec<(WidgetId,Option<PanelId>)>=Vec::new();
        let all=content.identities.iter().map(|(&raw,&id)|(raw,id)).chain(
            [Group::Primary,Group::Secondary,Group::Tertiary].into_iter().flat_map(registration::entries)
                .map(|e|(e.object,WidgetId{panel:e.id,kind:WidgetKind::Panel})));
        for (raw,id) in all {
            let parent=c.word(raw+0x24)?;
            let parent=if parent==0{None}else{Some(panel_from_raw(parent).ok_or("actual frame-parent lacks retained panel identity")?)};
            if let Some((_,old))=frame_parents.iter().find(|(candidate,_)|*candidate==id){
                if *old!=parent{return Err("one widget identity aliases inconsistent native frame-parent cells".into());}
            }else{frame_parents.push((id,parent));}
        }
        Ok(Self{input,override590504,models:Models::capture(c,p)?,frame_parents})
    }
}
fn graph(v:&Inspection)->Result<&super::ui_graph::UiGraphState>{v.ui_graph.as_ref().ok_or_else(||"F graph absent".into())}
///Pointer is a read projection. Restriction cells remain distinct from the
///native packet, retain their actual process writers, and are never inferred
///from physical UI/Scene/camera mode. Projection has no public setter.
pub fn pointer(v:&Inspection)->Result<Pointer>{
    let g=graph(v)?;let upper=g.upper.as_ref().ok_or("F upper owner absent")?;
    let input=g.pointer_owner.as_ref().ok_or("actual staged GUI input owner absent")?.input.cells();
    Ok(project(input,upper.pointer))
}
fn project(c:&RetainedCells,restrictions:Pointer)->Pointer{
    Pointer{xy:[c.logical_pointer.x,c.logical_pointer.y],blocked:c.mouse_current.right(),
        restricted:restrictions.restricted,allowed:restrictions.allowed}
}
fn contains(geometry:Geometry,rect:[f32;4],pointer:Pointer,p:&Panel)->Result<bool>{
    if p.f_pointer.as_ref().ok_or("F generic fields absent")?.descriptor.is_none(){return Ok(false);}
    Geometry{bounds:rect,..geometry}.contains(pointer,p.position)
}
///+20 means recompute, not retain. No input/mouse-down retention policy.
pub fn slot20()->u8{1}
///+2c: shown OR moving picks open parent bounds. No outer wrapper gates here.
pub fn slot2c(v:&Inspection)->Result<bool>{
    let g=graph(v)?;let p=g.panel(PanelId::F)?;let geo=g.upper.as_ref().ok_or("F upper owner absent")?.geometry;
    contains(geo,if p.requested!=0||p.moving!=0{geo.parent_open}else{geo.parent_closed},pointer(v)?,p)
}
///+24: after base hit, F's actual child-head proof admits panel itself.
pub fn slot24(v:&Inspection)->Result<Option<WidgetId>>{
    if !slot2c(v)?{return Ok(None);}
    let p=graph(v)?.panel(PanelId::F)?;
    if p.requested!=0&&!p.children.is_empty(){return Err("F+24 reached unowned actual child tree".into());}
    Ok(Some(F))
}
fn difference(integer:i32,value:f32)->Result<f32>{
    let mut result=Sum::from_i32(integer);result.add(Sum::from_f32(value).map_err(str::to_owned)?.negated());Ok(result.to_f32())
}
fn closing_velocity(value:f32,end:i32)->Result<f32>{
    let mut result=Sum::from_f32(value).map_err(str::to_owned)?;
    result.div(Sum::from_i32(end-1)).map_err(str::to_owned)?;Ok(result.negated().to_f32())
}
///Fully owned finite evaluation; no borrowing external callback invocation.
///Not returned to callers, so an old plan cannot overwrite a later callback.
#[cfg_attr(test,derive(Clone))]
struct Plan {
    panel:Panel, upper:State, input:RetainedCells, models:Models,
    selected:Option<WidgetId>,frame_parents:Vec<(WidgetId,Option<PanelId>)>,
    override590504:Option<WidgetId>,modal:u8,blocked:u8,cursor:Cursor,force_default:bool,
    #[cfg(test)] observed:Vec<&'static str>,
}
impl Plan {
    fn from_view(v:&Inspection)->Result<Self>{
        let g=graph(v)?;let shared=g.pointer_owner.as_ref().ok_or("actual shared input/override owner absent")?;
        let ui=v.camera_ui.as_ref().ok_or("F cursor requires SAME process cursor owner")?;
        Ok(Self{panel:g.panel(PanelId::F)?.clone(),upper:g.upper.as_ref().ok_or("F upper owner absent")?.clone(),
            input:shared.input.cells().clone(),models:shared.models.clone(),
            selected:g.group(g.panel(PanelId::F)?.group).selected,frame_parents:shared.frame_parents.clone(),
            override590504:shared.override590504,modal:g.panel(PanelId::G)?.requested,blocked:g.media_dispatch_blocked()?,
            cursor:ui.prospective_cursor(),force_default:ui.globals.force_default_cursor,
            #[cfg(test)] observed:Vec::new()})
    }
    fn note(&mut self,operation:&'static str){#[cfg(test)] self.observed.push(operation);#[cfg(not(test))] let _=operation;}
    fn cells(&self)->Result<&PanelCells>{self.panel.f_pointer.as_ref().ok_or_else(||"actual F generic fields absent".into())}
    fn cells_mut(&mut self)->Result<&mut PanelCells>{self.panel.f_pointer.as_mut().ok_or_else(||"actual F generic fields absent".into())}
    fn point(&self)->Pointer{project(&self.input,self.upper.pointer)}
    fn hit(&self,rect:[f32;4])->Result<bool>{contains(self.upper.geometry,rect,self.point(),&self.panel)}
    fn base_hit(&self)->Result<bool>{self.hit(if self.panel.requested!=0||self.panel.moving!=0{
        self.upper.geometry.parent_open}else{self.upper.geometry.parent_closed})}
    fn descriptor(&self)->Result<()>{self.cells()?.descriptor.ok_or("F drag/velocity reached null bound descriptor")?;Ok(())}
    ///Concrete F43aaf0/43ab00 are RET; test observations mark the actual
    ///call boundary, not a configurable callback or successful fake effect.
    fn opened(&mut self){self.note("F43aaf0 RET");}
    fn closed(&mut self){self.note("F43ab00 RET");}
    fn specialized(&mut self,event:u32,parameter:i32)->Result<()>{
        if !self.panel.children.is_empty(){return Err("F slot18 reached outside actual null-child family".into());}
        self.upper.pointer=self.point(); //read projection, not another owner
        if self.panel.disabled1d==0{
            let hover=self.upper.hover(self.panel.position,self.modal,true)?;
            self.note("43dd90 hover");
            for request in hover.cursor{self.cursor.select(request,self.force_default)?;self.note("4391e0 cursor");}
        }
        if let Message::Click(category)=self.upper.message_suffix(event,parameter,self.modal){
            return Err(format!("F positive specialized5 category{category} unjoined: TutorialEvent103/registry/whole mode0/camera/F11/A1 transaction required"));
        }
        self.note("43d130 suffix");Ok(())
    }
    fn generic(&mut self,event:u32,parameter:i32)->Result<()>{
        //43c090: event11 exception is only for1d, and10/11 skip busy.
        if self.panel.draw_enabled==0||self.panel.block34c!=0
            ||self.panel.disabled1d!=0&&event!=11||self.blocked!=0{return Ok(());}
        if !(4..=11).contains(&event)||(7..=9).contains(&event){return Ok(());}
        if event<=6 {
            if self.cells()?.drag346!=0 {
                self.descriptor()?;
                self.panel.position[0]=difference(self.input.logical_pointer.x,self.cells()?.offsets[0])?;
                self.note("descriptor X store");
                //Native reload of current37c before independent Y store.
                self.descriptor()?;
                self.panel.position[1]=difference(self.input.logical_pointer.y,self.cells()?.offsets[1])?;
                self.note("descriptor Y store");
            }else if !self.base_hit()?&&self.panel.block34d==0{return Ok(());}
            let buttons=self.input.mouse_current;
            if buttons.left()==0||buttons.right()!=0 {
                if self.cells()?.drag346==1{
                    self.cells_mut()?.drag346=0;self.override590504=None;self.note("release drag1/override clear");
                }
                self.cells_mut()?.pending354=0;self.note("pending clear");
            }else{
                if self.panel.moving!=0{return Ok(());}
                if self.cells()?.pending354==0 {
                    let rect=if self.panel.requested!=0{self.cells()?.title164}else{self.upper.geometry.parent_closed};
                    if self.input.mouse_prior.left()==0&&self.hit(rect)? {
                        if self.panel.requested!=0 {
                            let end=self.panel.transition_end;
                            self.panel.moving=1;self.panel.requested=0;self.note("close busy/shown stores");
                            if end>1{
                                self.descriptor()?;
                                self.panel.velocity[0]=closing_velocity(self.panel.position[0],end)?;
                                self.panel.velocity[1]=closing_velocity(self.panel.position[1],end)?;
                            }
                            self.closed();
                            //Reload selection after callback; exact frame-parent,
                            //not WidgetId.panel equivalence for arbitrary child.
                            if let Some(selected)=self.selected {
                                let clear=if selected==F{true}else{
                                    self.frame_parents.iter().find(|(id,_)|*id==selected)
                                        .ok_or("closing selected widget lacks actual frame-parent owner")?.1==Some(PanelId::F)};
                                if clear{self.selected=None;self.note("selected clear");}
                            }
                            if self.panel.independent!=0{self.panel.moving=0;}
                        }else{
                            let direct=self.panel.independent;
                            let code=(if direct!=0{event}else{u32::from(self.models.frames(self.cells()?)?).wrapping_sub(1)}).wrapping_add(2);
                            if code!=0||direct!=0{
                                self.panel.moving=1;self.panel.requested=1;self.note("open busy/shown stores");self.opened();
                            }
                            if self.panel.independent!=0{self.panel.moving=0;}
                            self.note("recursive slot14(11)");self.generic(11,0)?;
                        }
                        //93c72d is CURRENT keyboard index1d, any nonzero byte.
                        if self.input.keyboard_current[0x1d]!=0{
                            self.cells_mut()?.velocity_z=0.;self.note("velocity Z clear");
                            self.panel.velocity[1]=0.;self.note("velocity Y clear");
                            self.panel.velocity[0]=0.;self.note("velocity X clear");
                        }
                    }
                    if self.panel.requested!=0&&self.cells()?.drag346==0&&self.hit(self.cells()?.caption18c)?
                        &&(self.override590504.is_none()||self.override590504==Some(F)){
                        self.descriptor()?;self.cells_mut()?.drag346=1;
                        let x=difference(self.input.logical_pointer.x,self.panel.position[0])?;
                        self.cells_mut()?.offsets[0]=x;
                        let y=difference(self.input.logical_pointer.y,self.panel.position[1])?;
                        self.cells_mut()?.offsets[1]=y;
                        self.override590504=Some(F);self.note("drag offsets/override publish");
                    }
                }
            }
            if self.panel.moving!=0{return Ok(());}
        }
        if self.panel.requested==0{return Ok(());}
        self.specialized(event,parameter)?;
        //F retained actual child-head0; no arbitrary child callback provider.
        if event==11&&!self.panel.children.is_empty(){return Err("F recursive11 reached unowned child tree".into());}
        Ok(())
    }
    fn commit(self,v:&mut Inspection){
        //All lookups were established above; no callback, input poll or worker
        //runs between evaluation and these infallible writes. Only F fields,
        //its live group.selected, shared override and SAME cursor are written.
        let g=v.ui_graph.as_mut().unwrap();let group=self.panel.group;
        *g.panels.get_mut(&PanelId::F).unwrap()=self.panel;
        *g.upper.as_mut().unwrap()=self.upper;
        g.group_mut(group).selected=self.selected;
        g.pointer_owner.as_mut().unwrap().override590504=self.override590504;
        v.camera_ui.as_mut().unwrap().publish_cursor(self.cursor);
    }
}
fn evaluate(mut plan:Plan,event:u32,parameter:i32,specialized:bool)->Result<Plan>{
    if specialized{plan.specialized(event,parameter)?;}else{plan.generic(event,parameter)?;}
    Ok(plan)
}
fn prepare(v:&Inspection,event:u32,parameter:i32,specialized:bool)->Result<Plan>{
    evaluate(Plan::from_view(v)?,event,parameter,specialized)
}
pub fn check_message(v:&Inspection,event:u32,parameter:i32)->Result<()>{prepare(v,event,parameter,false).map(|_|())}
pub fn message(v:&mut Inspection,event:u32,parameter:i32)->Result<()>{prepare(v,event,parameter,false)?.commit(v);Ok(())}
pub fn check_specialized(v:&Inspection,event:u32,parameter:i32)->Result<()>{prepare(v,event,parameter,true).map(|_|())}
pub fn specialized_message(v:&mut Inspection,event:u32,parameter:i32)->Result<()>{prepare(v,event,parameter,true)?.commit(v);Ok(())}

#[cfg(test)]
#[path="f_generic_tests.rs"]
mod tests;
