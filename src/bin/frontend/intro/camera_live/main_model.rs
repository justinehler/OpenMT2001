//! Concrete41fae0/41fc60 model/overlay lifecycle over supplied actual mesh.
//!43b5f0 prepare changes target/seeking and bounds; never snaps current frame.
use super::{owner::Owner,ui_selection::Selected,ui_graph::PanelId};
use super::main_panel::Access;
use openmt2001::{mesh::CachedMesh,pe::PeImage};
#[derive(Clone,Debug)]
pub struct Frame {
    pub parent:[f32;4],pub main:Option<[f32;4]>,pub caption:Option<[f32;4]>,
    pub toggle:[f32;5],
}
#[derive(Clone,Debug)]
pub struct Geometry {pub frames:Vec<Frame>}
fn part_bounds(mesh:&CachedMesh<'_>,positions:&[[f32;3]],part:usize)->Result<[f32;4],String>{
    let part=mesh.parts.get(part).ok_or("actual A region part outside bound mesh")?;
    //Native starts these extrema even with an empty index list.
    let mut b=[99999f32,99999f32,-99999f32,-99999f32];
    for index in part.vertex_indices(){
        let p=positions.get(index).ok_or("A region vertex outside actual mesh")?;
        if !p[0].is_finite()||!p[1].is_finite(){return Err("unsupported nonfinite A model geometry".into())}
        if p[0]<b[0]{b[0]=p[0];}if p[0]>b[2]{b[2]=p[0];}
        //Native negates the winning Y AFTER choosing extrema.
        if p[1]>-b[1]{b[1]=-p[1];}if p[1] < -b[3]{b[3]=-p[1];}
    }Ok(b)
}
impl Geometry {
    /// main/caption are actual bound native part ids, not name ordinals. The
    /// toggle name is read directly from supplied PE534dac. Missing named
    /// toggle is native zero-extent+0.5; missing bound regions retain cells.
    pub fn from_mesh(mesh:&CachedMesh<'_>,main:Option<usize>,caption:Option<usize>,image:&PeImage<'_>)
        ->Result<Self,String>{
        let bytes=image.at(0x534dac,64)?;let end=bytes.iter().position(|&b|b==0).ok_or("A toggle region name unterminated")?;
        let desired=&bytes[..end];let mut toggle=None;
        for record in mesh.opaque_records_35.chunks_exact(35){
            let end=record[..33].iter().position(|&b|b==0).ok_or("A bound region name unterminated")?;
            let upper:Vec<_>=record[..end].iter().map(u8::to_ascii_uppercase).collect();
            if upper==desired{toggle=Some(usize::from(record[33]));break;}
        }
        let mut frames=Vec::with_capacity(mesh.frames);
        for frame in 0..mesh.frames {
            let positions=mesh.positions(frame)?;
            //43b5f0 visits every vertex and has a distinct max/min ordering.
            let mut parent=[99999f32,99999f32,-99999f32,-99999f32];
            for p in &positions {
                if !p[0].is_finite()||!p[1].is_finite(){return Err("unsupported nonfinite A parent vertex".into())}
                if p[0]<parent[0]{parent[0]=p[0];}if p[0]>parent[2]{parent[2]=p[0];}
                if p[1]>-parent[1]{parent[1]=-p[1];}if p[1] < -parent[3]{parent[3]=-p[1];}
            }
            let main=main.map(|part|part_bounds(mesh,&positions,part)).transpose()?;
            let caption=caption.map(|part|part_bounds(mesh,&positions,part)).transpose()?;
            let toggle=if let Some(part)=toggle{let [l,t,r,b]=part_bounds(mesh,&positions,part)?;[l,t,r,b,0.5]}else{[0.,0.,0.,0.,0.5]};
            frames.push(Frame{parent,main,caption,toggle});
        }
        if frames.is_empty(){return Err("A bound model has no actual frames".into())}
        Ok(Self{frames})
    }
}
#[derive(Clone,Debug)]
pub struct State {
    pub geometry:Geometry,
    pub closed_bounds:[f32;4],pub open_bounds:[f32;4],
    pub caption_bounds:[f32;4],pub caption_visual_bounds:[f32;4],
    pub overlay_extent:[f32;5],pub overlay_image:u32,
    pub overlay_completion:u8,pub overlay_animation:u32,
    pub open_icon:u32,pub closed_icon:u32,
}
fn main(view:&impl Access)->Result<&super::main_panel::State,String>{view.main()}
fn main_mut(view:&mut impl Access)->Result<&mut super::main_panel::State,String>{view.main_mut()}
fn recognized(mode:u32)->bool{matches!(mode,0|1|2|5)}
pub fn check_prepare(view:&impl Access,frames:u32)->Result<(),String>{
    let p=view.panel()?;
    if p.independent!=0{return Ok(())}
    let count=usize::try_from(p.frame_count).map_err(|_|"A frame count negative")?;
    if main(view)?.model.geometry.frames.len()!=count{return Err("A supplied model/live frame count mismatch".into())}
    if frames==0||frames as usize>count{return Err(format!("A prepare{frames} leaves actual bound model{count}"))}
    Ok(())
}
pub fn prepare(view:&mut impl Access,frames:u32)->Result<(),String>{
    check_prepare(view,frames)?;
    if view.panel()?.independent!=0{return Ok(())}
    let count=view.panel()?.frame_count;
    let last=main(view)?.model.geometry.frames[(frames-1)as usize].clone();
    let first=main(view)?.model.geometry.frames[0].clone();
    let model=&mut main_mut(view)?.model;
    if count>1{if let Some(bounds)=first.main{model.closed_bounds=bounds;}}
    model.open_bounds=last.parent;
    if let Some(bounds)=last.main{model.caption_bounds=bounds;}
    if let Some(bounds)=last.caption{model.caption_visual_bounds=bounds;}
    let p=view.panel_mut()?;
    p.target=frames as i32;p.seeking=1;Ok(())
}
fn check_overlay(view:&impl Access)->Result<(),String>{
    let p=view.panel()?;
    let index=usize::try_from(p.frame_count.wrapping_sub(1)).map_err(|_|"A overlay frame count")?;
    main(view)?.model.geometry.frames.get(index).ok_or("A overlay full frame count outside actual mesh")?;Ok(())
}
/// Preflight the MODE THAT WILL BE LIVE after41f290 stores requested. The
/// older generic Step.check saw the old mode here and could miss child1 exit.
pub fn check_for(view:&impl Access,mode:u32,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    let current=owner.validate_ui_selected(current)?;
    let mut plan=super::main_panel::Prospective::capture(view,current)?;
    plan.main.mode=mode;
    refresh_owned(&mut plan,owner)
}
pub fn check(view:&impl super::main_panel::Access,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    check_for(view,main(view)?.mode,owner,current)
}
fn overlay(view:&mut impl Access,open:bool)->Result<(),String>{
    let count=view.panel()?.frame_count;
    let extent=main(view)?.model.geometry.frames.get((count-1)as usize).ok_or("A overlay full frame range")?.toggle;
    let m=&mut main_mut(view)?.model;m.overlay_extent=extent;
    m.overlay_image=if open{m.open_icon}else{m.closed_icon};
    m.overlay_completion=0;m.overlay_animation=if open{2}else{1};Ok(())
}
pub fn refresh(view:&mut impl Access,owner:&mut Owner)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|plan|refresh_owned(plan,owner))
}
pub fn ensure_expanded(view:&mut impl Access,_:&Owner)->Result<(),String>{
    if main(view)?.expanded!=0{return Ok(())}
    check_prepare(view,10)?;check_overlay(view)?;
    main_mut(view)?.expanded=1;
    // Exact expanded refresh branch; no child/camera/Host callback is reached.
    main_mut(view)?.transition_pending=1;prepare(view,10)?;overlay(view,true)
}
/// Same finite refresh as the live model, with child exits bound to actual
/// owned child0/1 algorithms instead of invoking arbitrary callbacks.
pub(super) fn refresh_owned(view:&mut super::main_panel::Prospective,owner:&Owner)->Result<(),String>{
    view.resolved_current(owner)?;
    check_overlay(view)?;
    let expanded=main(view)?.expanded;
    if expanded==1{check_prepare(view,10)?;}else{check_prepare(view,1)?;}
    main_mut(view)?.transition_pending=1;
    if expanded==1{prepare(view,10)?;overlay(view,true)?;return Ok(())}
    super::main_panel::clear_notice(view)?;
    super::main_panel::exit_owned(view,owner)?;
    for control in &mut main_mut(view)?.controls{control.set_selected(0);}
    overlay(view,false)?;prepare(view,1)
}
pub fn step()->super::main_panel::Step{super::main_panel::Step{check:process_check,apply:process_refresh}}

#[cfg(test)]
mod direct_refresh_transaction_tests {
    use super::*;
    use super::super::main_panel::regression_tests::{fixture,owner};
    #[test]
    fn direct_refresh_and_check_preserve_no_child_state_on_notice_failure_then_retry() {
        let mut p=fixture();let mut owner=owner(None);
        p.main.mode=3;p.main.expanded=0;p.main.transition_pending=0;
        p.main.notice_text.bounds[1]=f32::NAN;
        let before=format!("{p:?}");let raw=owner.capture();
        assert!(check_for(&p,3,&owner,None).is_err());
        assert_eq!(format!("{p:?}"),before);
        assert!(refresh(&mut p,&mut owner).is_err());
        assert_eq!(format!("{p:?}"),before);assert_eq!(owner.capture(),raw);
        p.main.notice_text.bounds[1]=0.;
        refresh(&mut p,&mut owner).unwrap();
        assert_eq!(p.main.transition_pending,1);assert_eq!(p.main.notice_text.group.count,0);
        assert_eq!(p.panel.as_ref().unwrap().target,1);assert_eq!(owner.capture(),raw);
    }
    #[test]
    fn direct_refresh_rejects_wrong_owned_exit_identity_without_commit() {
        let mut p=fixture();let mut owner=owner(None);
        p.main.mode=0;p.main.expanded=0;
        p.main.children[0].as_mut().unwrap().exit=super::super::property_child::child().exit;
        let before=format!("{p:?}");let raw=owner.capture();
        let error=refresh(&mut p,&mut owner).unwrap_err();
        assert!(error.contains("actual nonreentrant"));
        assert_eq!(format!("{p:?}"),before);assert_eq!(owner.capture(),raw);
        assert!(std::ptr::fn_addr_eq(step().apply,process_refresh as fn(&mut super::super::main_panel::ProcessTextAccess<'_>,&mut Owner)->Result<(),String>));
    }
}

//Concrete stored callback ABI; forwards once to the same finite owned transaction.
fn process_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    check(view,owner,current)
}
fn process_refresh(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    refresh(view,owner)
}
