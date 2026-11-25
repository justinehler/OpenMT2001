//! Actual C callbacks for intro/camera_live. Requires the real Locator producer;
//! no Host query, FreshRequest, wait, clock, TaskEffect, or missing-success path.
use super::{owner::Owner,ui_graph::{Family,Families,PanelId,WidgetKind},ui_selection::Selected,super::Inspection};
fn graph(view:&Inspection)->Result<&super::ui_graph::UiGraphState,String>{view.ui_graph.as_ref().ok_or_else(||"C graph absent".into())}
fn locator(view:&Inspection)->Result<&crate::locator::Locator,String>{view.locator.as_ref().ok_or_else(||"C actual Locator absent".into())}
fn effects(owner:&mut Owner,e:crate::locator::strategic::Effects)->Result<(),String>{
    let selection=owner.ui_selection_mut()?;
    if e.clear_current{selection.current=None;}
    if let Some(value)=e.business{selection.list_business=value;}
    if let Some(value)=e.commodity{selection.list_commodity=value;}
    Ok(())
}
pub fn install(families:&mut Families){families.c_show=Some(Family{check:check_show,apply:show});}
fn project(view:&mut Inspection,owner:&Owner)->Result<(),String>{
    let selected=owner.ui_selection()?;
    let locator=view.locator.as_mut().ok_or("C projection actual Locator absent")?;
    {let s=locator.strategic_mut()?;s.world_business=selected.list_business;s.world_commodity=selected.list_commodity;}
    locator.publish_strategic()?;locator.project_strategic_owned(&view.scene)
}
fn check_show(view:&Inspection,owner:&Owner,prospective:Option<Selected>)->Result<(),String>{
    check_show_owner(graph(view)?,locator(view)?,owner,prospective)
}
/// Same read-only C admission body used by the installed family and direct
/// show. The raw prospective facade is unused: C reads its own Locator/list
/// suppliers, not selection kind/category/key.
pub(super) fn check_show_owner(g:&super::ui_graph::UiGraphState,locator:&crate::locator::Locator,
    owner:&Owner,_prospective:Option<Selected>)->Result<(),String>{
    owner.ui_selection()?;
    let mut s=locator.strategic()?.clone();let mut colors=g.world.colors;
    let result=s.show(&mut colors)?;
    if result.close{return Ok(());}
    //The common dispatcher calls phase11 AFTER show. Validate its changed
    //guards/pages and tree, rather than preflighting only entry state.
    let p=g.panel(PanelId::C)?;
    if p.draw_enabled!=0&&p.block34c==0&&g.media_dispatch_blocked()?==0{
        let geometry=&s.geometry;
        let inside=geometry.contains(geometry.open,p.position)?;
        let owned=geometry.contains(geometry.owned,p.position)?;
        s.message(11,inside,owned,p.disabled1d,&mut colors)?;s.tree.check()?;
    }
    Ok(())
}
pub fn show(view:&mut Inspection,owner:&mut Owner)->Result<(),String>{
    check_show(view,owner,owner.ui_selection()?.current)?;
    graph(view)?;
    //40f557 invalidation belongs to actual C State;40f55d is graph retention.
    view.locator.as_mut().unwrap().strategic_mut()?.cache=u32::MAX;
    view.ui_graph.as_mut().unwrap().retained_c=None;
    let outcome={
        let locator=view.locator.as_mut().ok_or("C actual Locator absent")?;
        let g=view.ui_graph.as_mut().ok_or("C actual graph absent")?;
        let s=locator.strategic_mut()?;s.show_after_retention_clear(&mut g.world.colors)?
    };
    let close=outcome.close;effects(owner,outcome)?;
    if close{
        //Actual43bed0, including40f890 selection restoration and common motion.
        return super::ui_graph::set_visibility(view,PanelId::C,false,owner);
    }
    project(view,owner)
}
pub fn check_message(view:&Inspection,id:PanelId,_owner:&Owner,event:u32)->Result<(),String>{
    if id!=PanelId::C{return Err("C callback received another panel identity".into());}
    let g=graph(view)?;let p=g.panel(id)?;let mut s=locator(view)?.strategic()?.clone();
    let geometry=&s.geometry;let bounds=if p.requested!=0||p.moving!=0{geometry.open}else{geometry.closed};
    let inside=geometry.contains(bounds,p.position)?;let owned=geometry.contains(geometry.owned,p.position)?;
    let mut colors=g.world.colors;
    s.message(event,inside,owned,p.disabled1d,&mut colors)?;Ok(())
}
pub fn message(view:&mut Inspection,id:PanelId,owner:&mut Owner,event:u32)->Result<(),String>{
    check_message(view,id,owner,event)?;
    let outcome={
        let g=view.ui_graph.as_mut().ok_or("C graph absent")?;let p=g.panel(id)?.clone();
        let locator=view.locator.as_mut().ok_or("C Locator absent")?;let s=locator.strategic_mut()?;
        let geometry=&s.geometry;let bounds=if p.requested!=0||p.moving!=0{geometry.open}else{geometry.closed};
        let inside=geometry.contains(bounds,p.position)?;let owned=geometry.contains(geometry.owned,p.position)?;
        let e=s.message(event,inside,owned,p.disabled1d,&mut g.world.colors)?;
        if let Some(mode)=e.report_mode{g.world.mode=mode;}e
    };
    effects(owner,outcome)?;
    project(view,owner)
}
pub fn check_phase11(view:&Inspection,id:PanelId,owner:&Owner)->Result<(),String>{check_message(view,id,owner,11)}
pub fn phase11(view:&mut Inspection,id:PanelId,owner:&mut Owner)->Result<(),String>{message(view,id,owner,11)}
pub fn check_visible1(view:&Inspection,id:PanelId,owner:&Owner)->Result<(),String>{check_message(view,id,owner,1)}
pub fn visible1(view:&mut Inspection,id:PanelId,owner:&mut Owner)->Result<(),String>{message(view,id,owner,1)}
///Compare graph identity against the actual retained C tree head. Passing an
///empty graph vector cannot omit a real child-tree callback.
pub fn check_tree11(view:&Inspection,id:PanelId,_owner:&Owner)->Result<(),String>{
    if id!=PanelId::C{return Err("C tree callback received another panel".into());}
    let tree=&locator(view)?.strategic()?.tree;let p=graph(view)?.panel(id)?;
    let expected=tree.first.ok_or("C actual child head null after layout")?;
    if p.children.as_slice()!=[super::ui_graph::WidgetId{panel:id,kind:WidgetKind::LocatorChild(expected)}]{
        return Err("C graph child head differs from actual retained child owner".into());}
    tree.check()
}
pub fn tree11(view:&mut Inspection,id:PanelId,owner:&mut Owner)->Result<(),String>{
    check_tree11(view,id,owner)?;
    view.locator.as_mut().unwrap().strategic_mut()?.tree.dispatch11()?;
    //The same actual control owner must feed pressed/down visuals. Invalidate
    //the real batch after pulse/changed writes, even though no click occurred.
    view.locator.as_mut().unwrap().dirty=true;Ok(())
}
///Invoke once from Context's successful ack construction, after all callbacks.
///The worker commits before resuming; this is NOT a queued runtime action.
pub fn take_writeback(view:&mut Inspection)->Result<Option<crate::locator::strategic::Writeback>,String>{
    let colors=graph(view)?.world.colors;
    view.locator.as_mut().ok_or("C writeback actual owner absent")?.take_strategic_writeback(colors)
}
/// All fallible owner/identity/dirty checks precede either supplier mutation.
pub fn check_suppliers(view:&Inspection,value:Option<&crate::locator::strategic::Inputs>,owner:Option<&Owner>)->Result<(),String>{
    let g=graph(view)?;
    check_supplier_owner(view.locator.as_ref(),&g.world.colors,
        g.panels.get(&PanelId::C).map(|p|p.children.as_slice()),value,owner)
}
pub(crate) fn check_supplier_owner(locator:Option<&crate::locator::Locator>,colors:&[u32;600],
    children:Option<&[super::ui_graph::WidgetId]>,value:Option<&crate::locator::strategic::Inputs>,owner:Option<&Owner>)->Result<(),String>{
    let s=locator.ok_or("C actual Locator absent")?.strategic()?;
    let expected=s.tree.first.ok_or("C actual child head null after layout")?;
    if children.ok_or("C graph panel absent")?!=[super::ui_graph::WidgetId{
        panel:PanelId::C,kind:WidgetKind::LocatorChild(expected)}]{
        return Err("C graph child head differs from actual retained child owner".into());
    }
    if let Some(owner)=owner{
        let selection=owner.ui_selection()?;
        // SelectionState owns the CURRENT globals; State.world_* is only the
        // last C projection (C hide legitimately clears globals separately).
        // Check the current selectors against their actual supplied owners,
        // never require equality with that historical display projection.
        let input=value.unwrap_or(&s.input);
        if selection.list_business < -1||selection.list_business>=input.catalog.rows().len()as i32
            ||selection.list_commodity < -1||selection.list_commodity>=input.commodities.len()as i32{
            return Err("C current list selection leaves actual supplied owner".into());
        }
        for selected in [owner.current_ui_selected()?,owner.published_ui_selected()?].into_iter().flatten(){
            if selected.category>=input.categories.len()as u32{return Err("C live selection category leaves actual supplier".into());}
        }
    }
    if let Some(value)=value{s.check_suppliers(value,colors)?;}else{s.tree.check()?;}
    Ok(())
}
pub fn suppliers(view:&mut Inspection,value:std::sync::Arc<crate::locator::strategic::Inputs>)->Result<(),String>{
    check_suppliers(view,Some(&value),None)?;
    // Check confirmed no local C/D color or poll write is pending. +90 cells
    // belong to the current suspended World supplier, not an old display cut:
    // preserve retained mode/legend/selections, refresh only working colors.
    // C/D callbacks can then change those cells and return the exact before
    // image at Install/CentralTail. Uncommitted Apply writes reject above.
    let g=view.ui_graph.as_mut().unwrap();
    replace_supplier_owner(view.locator.as_mut().unwrap(),&mut g.world.colors,value.clone())?;
    g.human_enabled=value.human_enabled;
    g.report_inputs.buildings=std::array::from_fn(|i|super::ui_graph::Building{
        active:value.building_active[i],owner_color:Some(value.owner_colors[i])});
    g.report_inputs.properties=std::array::from_fn(|i|{let r=&value.categories[i];super::ui_graph::Property{
        active:r[4],excluded:r[0x90],title:i32::from_le_bytes(r[0x8c..0x90].try_into().unwrap())}});
    g.report_inputs.titles=value.tables.property_labels[..46].to_vec();
    Ok(())
}

/// Same real-owner operation used by supplier publication; no metadata is
/// changed on rejection, and external current colors become the new base.
pub(crate) fn replace_supplier_owner(locator:&mut crate::locator::Locator,colors:&mut [u32;600],value:std::sync::Arc<crate::locator::strategic::Inputs>)->Result<(),String>{
    locator.strategic()?.check_suppliers(&value,colors)?;
    locator.replace_strategic_suppliers(value.clone())?;
    *colors=value.before_colors;Ok(())
}
