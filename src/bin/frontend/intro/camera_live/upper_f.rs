//! Concrete F graph callbacks. Nested intro/camera_live/upper_f.rs.
//! No family callback queue, worker query/wait, FreshRequest or TaskEffect.
use super::{owner::Owner,ui_graph::{PanelId,UiGraphState},super::{Inspection,Assets}};
use crate::navigator::upper::{self,State,Inputs,Geometry,Pointer,Draw};
use std::sync::Arc;
use openmt2001::render::{PreparedScene,Renderer};
fn graph(v:&Inspection)->Result<&UiGraphState,String>{v.ui_graph.as_ref().ok_or_else(||"F graph absent".into())}
fn state(v:&Inspection)->Result<&State,String>{graph(v)?.upper.as_ref().ok_or_else(||"F actual process owner absent".into())}
fn state_mut(v:&mut Inspection)->Result<&mut State,String>{v.ui_graph.as_mut().ok_or("F graph absent")?.upper.as_mut().ok_or_else(||"F actual process owner absent".into())}
pub fn producer_cells(c:&super::process_image::Cells,image:&openmt2001::pe::PeImage<'_>)
    ->Result<upper::Producer,String>{
    Ok(upper::Producer{tables:upper::Tables::capture(image)?,
        mode8b2318:c.word(0x8b2318)?})
}

///Reads actual initialized source-owned Cells after43b770->43d010 binding.
///F has no factory children: node43e700 sets+28=0;4393a0 and43d010 do not
///append any. Enforce the actual cell ANDgraph identity, not an empty override.
pub fn install(v:&mut Inspection,c:&super::process_image::Cells,input:Arc<Inputs>)->Result<(),String>{
    let p=0x58cdb0;
    if c.word(p)?!=0x525578||c.word(p+0x28)?!=0{return Err("F actual vtable/child-head differs from proved first-process owner".into());}
    if c.word(p+0x34)?!=0||!c.ntbs(p+0x39,255)?.is_empty(){
        return Err("F actual panel focus is not source-proved empty-caption branch".into());}
    let g=v.ui_graph.as_mut().ok_or("F install before graph")?;
    if !g.panel(PanelId::F)?.children.is_empty(){return Err("F graph child vector differs from actual null head".into());}
    if g.upper.is_some(){return Err("F process owner cannot be replaced on refresh".into());}
    let geometry=Geometry{bounds:c.rect(p+0x15c8)?,second:c.rect(p+0x15e4)?,backing:c.rect(p+0x15f8)?,
        parent_open:c.rect(p+0x13c)?,parent_closed:c.rect(p+0x150)?};
    let pointer=Pointer{xy:[c.word(0x93c5c0)? as i32,c.word(0x93c5dc)? as i32],
        blocked:c.byte(0x93c60d)?,restricted:c.byte(0x92088a)?,allowed:c.word(0x92088c)? as i32};
    g.upper=Some(State::installed(geometry,c.word(p+0x15c0)? as i32,c.byte(p+0x15c4)?,c.byte(p+0x15e0)?,pointer,input));
    Ok(())
}
///Layout-reset43d010 does not clear retained hover/category/focus or input.
pub fn reset_layout(v:&mut Inspection,geometry:Geometry)->Result<(),String>{state_mut(v)?.reset_layout(geometry);Ok(())}
pub fn check_suppliers(v:&Inspection,input:Option<&Inputs>)->Result<(),String>{
    let g=graph(v)?;
    check_supplier_owner(g.upper.as_ref(),g.panels.get(&PanelId::F).map(|p|p.children.as_slice()),input)
}
pub(super) fn check_supplier_owner(owner:Option<&State>,children:Option<&[super::ui_graph::WidgetId]>,input:Option<&Inputs>)->Result<(),String>{
    owner.ok_or("F actual process owner absent")?;
    if !children.ok_or("F graph panel absent")?.is_empty(){return Err("F child tree mutated outside proved null-head family".into());}
    if let Some(input)=input{
        if input.count>52{return Err("F supplier count leaves actual52-row owner".into());}
        for i in 0..input.count.max(0) as usize{
            let row=&input.rows[i];
            if (row[4]!=0&&row[5]!=0)!=input.owners[i].is_some(){
                return Err("F supplied live owner/category identity mismatch".into());
            }
        }
    }
    Ok(())
}
pub fn suppliers(v:&mut Inspection,input:Arc<Inputs>)->Result<(),String>{
    check_suppliers(v,Some(&input))?;state_mut(v)?.input=input;Ok(())
}
pub fn check_tree(v:&Inspection)->Result<(),String>{
    check_tree_owner(graph(v)?)
}
fn check_tree_owner(g:&UiGraphState)->Result<(),String>{
    g.upper.as_ref().ok_or("F actual process owner absent")?;
    if !g.panel(PanelId::F)?.children.is_empty(){return Err("F child tree mutated outside proved null-head family".into());}
    Ok(())
}
///Direct specialized slot18; deliberately separate from generic slot14.
pub fn check_message(v:&Inspection,_owner:&Owner,event:u32,parameter:i32)->Result<(),String>{
    super::f_generic::check_specialized(v,event,parameter)
}
pub fn message(v:&mut Inspection,_owner:&mut Owner,event:u32,parameter:i32)->Result<(),String>{
    super::f_generic::specialized_message(v,event,parameter)
}
//The graph caller has already checked prospective shown/generic gates;
//validate its specialized11 consumer even when the entry panel is hidden.
pub fn check_phase11(v:&Inspection,o:&Owner)->Result<(),String>{check_message(v,o,11,0)}
pub fn phase11(v:&mut Inspection,_o:&mut Owner)->Result<(),String>{super::f_generic::message(v,11,0)}
pub fn check_visible1(v:&Inspection,o:&Owner)->Result<(),String>{check_message(v,o,1,0)}
pub fn visible1(v:&mut Inspection,o:&mut Owner)->Result<(),String>{message(v,o,1,0)}
pub fn generic_message(v:&mut Inspection,event:u32,parameter:i32)->Result<(),String>{
    super::f_generic::message(v,event,parameter)
}

///Source body43d0f0 runs inside native43c3a0 body predicate. Its material
///caption refresh and actual font measurement finish BEFORErawOwner reads.
///The endpoint lends read-only authority; rendering does not sample/step it.
pub fn body(v:&mut Inspection,assets:&Assets,renderer:&Renderer)->Result<Option<PreparedScene>,String>{
    if !graph(v)?.shell(PanelId::F){return Ok(None);}
    check_tree(v)?;
    let p=graph(v)?.panel(PanelId::F)?.clone();
    let mut projected=state(v)?.clone();projected.pointer=super::f_generic::pointer(v)?;
    let stable=p.body();
    if !stable{
        return Ok(Some(upper::prepare(&assets.navigator,renderer,&Draw{batches:Vec::new()},
            projected.geometry,p.frame,p.position,p.alpha,p.independent==0,false)?));
    }
    //All finite math/label lookup checked before publishing focus mutation.
    let mut batches=projected.map(p.position,p.alpha)?;
    let clear=projected.clears_primary(p.position)?;
    let category=projected.focus_category();
    let label=category.map(|i|projected.category_label(i)).transpose()?;
    if clear{v.camera_focus.clear_primary();}
    if let Some((category,label))=category.zip(label){v.camera_focus.show_category(category,label,true);}
    if clear||category.is_some(){
        if let Some(identifier)=v.camera_focus.retained_text_identifier(){assets.tooltip.measure_focus_identifier(identifier)?;}
        v.tooltip=None;v.tooltip_state=None;
    }
    //43d108 reads yaw, eyeZ, eyeX AFTER43d290 and its focus effects.
    let (eye,yaw)=v.camera_endpoint.as_ref().ok_or("F body lacks actual rawOwner endpoint")?.owner().upper_logic_words();
    let s=&projected;batches.push(upper::arrow(s.geometry,p.position,eye,yaw,s.input.phase)?);
    Ok(Some(upper::prepare(&assets.navigator,renderer,&Draw{batches},s.geometry,
        p.frame,p.position,p.alpha,p.independent==0,true)?))
}
///43bca0 panel-hit branch. F's actual child head is null. Body hit is the
///parent open/closed bounds, distinct from Navigator map hover bounds.
pub fn panel_hit(v:&Inspection)->Result<bool,String>{super::f_generic::slot2c(v)}
pub fn find_hit(v:&Inspection)->Result<Option<super::ui_graph::WidgetId>,String>{super::f_generic::slot24(v)}
pub fn recompute_selected()->u8{super::f_generic::slot20()}
///Full source group/poll continuation boundary. Root MUST supply live central
///providers, not a captured repeat scalar. This is not called by CentralTail;
///the actual pre-poll acknowledgment/continuation is still a Root join.
pub fn groups_before_controller<P:super::group_bridge::Families+super::group_bridge::CentralProvider>(
    v:&mut Inspection,o:&mut Owner,assets:&mut Assets,providers:&mut P)->Result<super::continuation::BeforeTail,String>{
    super::group_dispatch::central_source_boundary(&mut super::group_bridge::Bridge{
        view:v,owner:o,assets,providers})
}
///43e3b0 is once at actual central tail even if groups were suppressed. Call
///after controller/intents and before4e02d0, never per draw or repeat count.
pub fn central_focus_tail(v:&mut Inspection){v.camera_focus.advance();}
///At actual43c8e0 focus-show boundary, after its selected->dwell producer.
///F node43e700/4393a0/43d010 leaves id0/empty inline text.43e310 then
///disables the label, clears secondary and refreshes retained primary. It
///does not manufacture a new Compass/category child for the panel rectangle.
pub fn focus_dwell_panel(v:&mut Inspection,assets:&Assets)->Result<(),String>{
    focus_dwell_panel_with_measure(super::group_bridge::FocusAdapter{
        graph:v.ui_graph.as_mut().ok_or("F graph absent")?,focus:&mut v.camera_focus,
        tooltip:&mut v.tooltip,tooltip_state:&mut v.tooltip_state,
    },|id|assets.tooltip.measure_focus_identifier(id).map(|_|()))
}
///Same concrete graph/Focus adapter as production; only the synchronous
///measurement call is lent. No state snapshot, second owner or font default.
pub(super) fn focus_dwell_panel_with_measure(mut focus:super::group_bridge::FocusAdapter<'_>,
    measure:impl FnOnce(u32)->Result<(),String>)->Result<(),String>{
    use super::ui_graph::{WidgetId,WidgetKind};use super::navigation::Group;
    check_tree_owner(focus.graph)?;
    let actual=WidgetId{panel:PanelId::F,kind:WidgetKind::Panel};
    if focus.graph.group(Group::Primary).dwell!=Some(actual){
        return Err("F43e310 requires actual pointer-group dwell identity".into());}
    focus.focus.show_empty_panel_child();focus.graph.focus_child=None;
    //43e33f publishes null BEFORE43e34c refresh, including on font error.
    focus.refresh_caption(measure)
}
