//! One-time bridge from actual constructor/layout products to the finite
//! typed graph. This does not manufacture any missing layout or subtype.
use super::{process_image::Cells,owned_factories::{self,Content},registration,
    navigation::Group,ui_graph::{self,PointerGroup,StrategicWorld,LegendRow},owner::Owner,
    super::Inspection};
use std::collections::BTreeMap;
use super::group_dispatch::{Links,Link};
fn panel_id(raw:u32)->Result<Option<ui_graph::PanelId>,String>{
    if raw==0{return Ok(None)}
    [Group::Primary,Group::Secondary,Group::Tertiary].into_iter().flat_map(registration::entries)
        .find(|entry|entry.object==raw).map(|entry|Some(entry.id))
        .ok_or_else(||format!("actual link{raw:08x} lacks registered identity"))
}
fn target(c:&Cells,content:&Content,a:u32)->Result<Option<ui_graph::WidgetId>,String>{
    let raw=c.word(a)?;if raw==0{return Ok(None)}
    if let Ok(Some(id))=panel_id(raw){return Ok(Some(super::group_dispatch::panel(id)))}
    Ok(Some(content.id(raw)?))
}
pub fn groups(c:&Cells,content:&Content)->Result<[PointerGroup;3],String>{
    let mut out=Vec::new();
    for group in [Group::Primary,Group::Secondary,Group::Tertiary]{
        let a=registration::group_address(group);let expected=registration::entries(group);
        let mut nodes=BTreeMap::new();
        for entry in &expected{
            nodes.insert(entry.id,Link{next:panel_id(c.word(entry.object+0x380)?)?,
                previous:panel_id(c.word(entry.object+0x384)?)?});
        }
        let links=Links::from_source(panel_id(c.word(a+8)?)?,panel_id(c.word(a+12)?)?,nodes)?;
        //Original first-process bind/registration gate remains mandatory.
        //No vector is retained as a second link or draw-order authority.
        let mut at=links.tail;
        for entry in expected{
            if at!=Some(entry.id){return Err("process graph factory called before full registration or after promotion".into())}
            at=links.previous(entry.id)?;
        }
        if at.is_some(){return Err("actual group registration has extra panel".into())}
        out.push(PointerGroup{enabled:c.byte(a+5)?,blocks_input:c.byte(a+4)?,links,
            dwell:target(c,content,a+0x10)?,selected:target(c,content,a+0x14)?,
            hovered:target(c,content,a+0x18)?,key_target:target(c,content,a+0x1c)?});
    }
    out.try_into().map_err(|_|"actual three pointer groups".into())
}
pub fn install(view:&mut Inspection,owner:&mut Owner,c:&Cells,content:&Content,
    process:crate::live_host::process_runtime::Handle,
    input:super::live_inputs::Inputs,main:super::main_panel::State,families:ui_graph::Families,
    upper_input:std::sync::Arc<crate::navigator::upper::Inputs>,sequence_index:Option<i32>,
    native_input:openmt2001::native_input::StagedInput)->Result<(),String>{
    if view.ui_graph.is_some()||view.camera_navigation.is_some(){return Err("once-only process UI factory cannot replace retained state".into())}
    let panels=owned_factories::panels(c,content)?;let groups=groups(c,content)?;
    //42a370 initial D layout builds precisely these rows. The helper keeps
    //actual child control objects in the graph; this is their caption mapping.
    let mut rows=Vec::new();
    for (category,p) in input.reports.properties.iter().enumerate(){
        if ((p.active==1&&p.excluded&1==0)||category==41)&&p.title<=45{
            let title=usize::try_from(p.title).map_err(|_|"actual D negative before-table title requires its mapping")?;
            let caption=input.reports.titles.get(title).ok_or("actual D supplied title mapping absent")?.wrapping_add(1);
            rows.push(LegendRow{category,title:p.title,caption});
        }
    }
    if rows.len()!=c.word(0x574488+0x4a4)? as usize{return Err("actual D initial row factories differ from captured52 inputs".into())}
    let world=StrategicWorld::installed(c.word(0x574488+0x4a8)?,c.byte(0x574488+0xde8)?,c.word(0x560b14)?,input.colors,rows);
    let compass=c.word(0x58ca08+0x388)?;let speed=c.word(0x58ca08+0x38c)?;
    if compass==0||speed==0{return Err("E actual layout missing compass/speed child".into())}
    let mut graph=ui_graph::UiGraphState::installed(panels,groups,input.human_enabled,c.byte(0x8fca98)?,
        content.selection(c.word(0x58e8d0+0x43c)?)?,input.reports,
        world,families,main,c.byte(compass+0x38)?,c.word(compass+0x34)?,c.byte(speed+0x333)?,c.byte(speed+0x335)?,
        target(c,content,0x590da4)?)?;
    let pointer=super::f_generic::Shared::capture(c,content,native_input,
        graph.panel(ui_graph::PanelId::F)?.f_pointer.as_ref().ok_or("actual F generic cells absent")?)?;
    graph.pointer_owner=Some(pointer);
    graph.bind_process_source(process,c)?;
    let selection=owned_factories::selection(c,content,sequence_index)?;
    super::bindings::install(view,owner,graph,owned_factories::navigation(c)?,selection)?;
    super::upper_f::install(view,c,upper_input)
}
