//! Owned original UI graph and concrete consumers.
//! Missing specialized effects return precise errors.
//! No successful Noop is installed for an absent reached implementation.
use super::{navigation::{Consumers, Group, Object}, owner::Owner,
    ui_selection::Selected, super::Inspection};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PanelId {
    A, B, C, D, E, F, G, H, Clock, Comparison,
    /// Exact group registration ordinal, supplied from43cc60/4b initialization.
    OtherPrimary(u8), Frontend(u8), Auction(u8),
}
impl From<Object> for PanelId {
    fn from(value: Object) -> Self {
        match value { Object::A=>Self::A, Object::B=>Self::B, Object::C=>Self::C,
            Object::D=>Self::D, Object::E=>Self::E, Object::F=>Self::F,
            Object::G=>Self::G, Object::H=>Self::H }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind { Panel, Compass, Time(crate::time_panel::Button), LocatorChild(u32), Unported(u32) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetId { pub panel: PanelId, pub kind: WidgetKind }

#[derive(Clone, Debug)]
pub struct PointerGroup {
    pub enabled: u8,
    pub blocks_input:u8, //actual group+4, recomputed by complete43c8e0 only
    ///Sole live typed head/tail/next/previous authority.
    pub links: super::group_dispatch::Links,
    pub dwell: Option<WidgetId>,
    pub selected: Option<WidgetId>,
    pub hovered: Option<WidgetId>,
    ///43c570 does not write+1c. The install producer must supply this cell.
    pub key_target: Option<WidgetId>,
}
impl PointerGroup {
    #[cfg(test)] //synthetic fixture only; process factory reads actual cells
    pub fn after_constructor(registration: Vec<PanelId>,
        actual_key_target: Option<WidgetId>) -> Self {
        Self { enabled:0, blocks_input:0, links:super::group_dispatch::Links::synthetic_draw_order(&registration), dwell:None, selected:None,
            hovered:None, key_target:actual_key_target }
    }
    pub fn has_selection(&self) -> bool { self.selected.is_some() && self.enabled & 1 != 0 }
}

#[derive(Clone, Debug)]
pub struct Panel {
    pub group: Group,
    pub requested: u8, pub moving: u8, pub seeking: u8, pub draw_enabled: u8,
    pub force_show: u8, pub independent: u8,
    pub block34c:u8,pub disabled1d:u8,pub node_shown1c:u8,pub children:Vec<WidgetId>,
    pub promote34b:u8,pub block34d:u8,
    ///Only F has an owned generic pointer field layout in this packet.
    pub f_pointer:Option<super::f_generic::PanelCells>,
    pub frame_count: i32, pub target: i32, pub transition_end: i32,
    pub frame: u16, pub position: [f32;2], pub velocity: [f32;2],
    pub main_region: Option<u32>,
    pub alpha: u8, pub fading: u8, pub fade_step: i8,
}
impl Panel {
    ///43b770 bind result. model values are decoded from the SUPPLIED bound
    ///descriptor, not invented frame counts or instant-show approximations.
    #[cfg(test)] //synthetic fixture only; never seed unknown production bytes
    pub fn bound(group: Group, frames: u16, frame: u16, position:[f32;2],
        independent: bool, main_region:Option<u32>, alpha:u8) -> Result<Self,String> {
        if !independent && frames == 0 { return Err("Bound panel model has zero frames".into()); }
        let count = if independent {1} else {i32::from(frames)};
        Ok(Self { group, requested:0, moving:0, seeking:0, draw_enabled:1,
            force_show:0, independent:u8::from(independent), block34c:0,disabled1d:0,node_shown1c:1,children:Vec::new(),
            promote34b:0,block34d:0,f_pointer:None,frame_count:count,
            target:count, transition_end:count, frame, position, velocity:[0.;2],
            main_region, alpha, fading:0, fade_step:0 })
    }
    ///43c3a0 includes a closed model's named main region even when idle/hidden.
    pub fn shell(&self) -> bool {
        self.draw_enabled != 0 && (self.requested != 0 || self.moving != 0
            || self.frame_count != 1 && self.main_region.is_some())
    }
    pub fn body(&self) -> bool {
        self.draw_enabled != 0 && self.requested != 0 && self.moving == 0
    }
    ///Child-body predicate for existing presentation/input callers. The group
    ///dispatcher still calls real slot24 on hidden/closed parent panels.
    pub fn children(&self) -> bool { self.requested != 0 && self.moving == 0 }
    ///43bce0. Return true only on actual event11 completion; caller dispatches.
    pub fn advance(&mut self) -> bool {
        if self.fading != 0 {
            self.alpha = self.alpha.wrapping_add(self.fade_step as u8);
            if self.fade_step > 0 {
                if self.alpha == 255 { self.fading=0; }
            } else if self.alpha <=128 { self.fading=0; }
        }
        if self.seeking != 0 {
            let difference = (i32::from(self.frame as i16) as u32)
                .wrapping_sub(self.target as u32).wrapping_add(1) as i32;
            if difference >0 { self.frame=self.frame.wrapping_sub(1); return false; }
            if difference <0 { self.frame=self.frame.wrapping_add(1); return false; }
            self.seeking=0; self.moving=0; return true;
        }
        if self.moving == 0 { return false; }
        if self.requested !=0 {
            self.frame=self.frame.wrapping_add(1);
            if i32::from(self.frame as i16) >= self.transition_end.wrapping_sub(1) {
                self.seeking=1; self.frame=(self.transition_end as u16).wrapping_sub(1);
            }
        } else {
            self.frame=self.frame.wrapping_sub(1);
            if (self.frame as i16)<=0 { self.moving=0; self.frame=0; }
        }
        if self.frame_count<=1 { self.moving=0; }
        false
    }
}

/// Real D inputs, sampled by the live UI producer at its protected boundary.
/// Keep600 slots, including inactive records. Never read stale Scene owners.
#[derive(Clone, Copy, Debug)]
pub struct Building { pub active:u8, pub owner_color:Option<u32> }
#[derive(Clone, Copy, Debug)]
pub struct Property { pub active:u8, pub excluded:u8, pub title:i32 }
#[derive(Clone, Debug)]
pub struct ReportInputs {
    pub buildings: [Building;600], pub properties:[Property;52],
    /// Exact title words from supplied PE table5385a0,88-byte stride.
    pub titles:Vec<u32>,
}
#[derive(Clone, Debug)]
pub struct LegendRow { pub category:usize, pub title:i32, pub caption:u32 }
#[derive(Clone, Debug)]
pub struct StrategicWorld {
    pub mode:u32, pub dirty:u8, pub messages:u32,
    /// Actual UI tint owner; all footprint draw consumers read this array.
    pub colors:[u32;600], pub rows:Vec<LegendRow>,
}
impl StrategicWorld {
    pub fn installed(mode:u32, dirty:u8, messages:u32, colors:[u32;600],rows:Vec<LegendRow>)->Self {
        Self {mode,dirty,messages,colors,rows}
    }
    fn recolor(&mut self,input:&ReportInputs) {
        for (i,b) in input.buildings.iter().enumerate() {
            if b.active!=0 {self.colors[i]=b.owner_color.unwrap_or(0xff808080);}
        }
    }
    fn show(&mut self,input:&ReportInputs)->Result<(),String> {
        //42a600 exact admission, including category41 exception and signed<=45.
        //Negative title would index before the mapped table natively; refuse
        //unsupported data before mutation rather than silently skip it.
        let mut rows=Vec::new();
        for (category,p) in input.properties.iter().enumerate() {
            if ((p.active==1 && p.excluded&1==0)||category==41) && p.title<=45 {
                let title=usize::try_from(p.title).map_err(|_|"D legend negative title requires its actual before-table mapping")?;
                let label=*input.titles.get(title).ok_or("D legend title leaves supplied table")?;
                rows.push(LegendRow{category,title:p.title,caption:label.wrapping_add(1)});
            }
        }
        self.recolor(input); self.rows.clear(); self.rows.extend(rows); Ok(())
    }
    fn hide(&mut self,input:&ReportInputs) {
        for (i,b) in input.buildings.iter().enumerate() {
            if b.active!=0 {self.colors[i]=0xffffffff;}
        }
    }
}

/// Source-complete family callbacks may be installed here. None means an
/// explicit reached error, never an empty successful implementation.
#[derive(Clone, Copy)]
pub struct Family {
    pub check:fn(&Inspection,&Owner,Option<Selected>)->Result<(),String>,
    pub apply:fn(&mut Inspection,&mut Owner)->Result<(),String>,
}
#[derive(Clone, Copy, Default)]
pub struct Families {
    pub c_show:Option<Family>,
    pub b_show:Option<Family>,pub b_hide:Option<Family>,
    pub h_show:Option<Family>,pub h_hide:Option<Family>,
    pub g_show:Option<Family>,pub g_hide:Option<Family>,
    ///Actual reached per-panel event11, including A active-child activation.
    pub phase11:Option<fn(&mut Inspection,PanelId,&mut Owner)->Result<(),String>>,
    pub phase11_check:Option<fn(&Inspection,PanelId,&Owner)->Result<(),String>>,
    pub visible1:Option<fn(&mut Inspection,PanelId,&mut Owner)->Result<(),String>>,
    pub visible1_check:Option<fn(&Inspection,PanelId,&Owner)->Result<(),String>>,
    pub tree11:Option<fn(&mut Inspection,PanelId,&mut Owner)->Result<(),String>>,
    pub tree11_check:Option<fn(&Inspection,PanelId,&Owner)->Result<(),String>>,
    pub event73:Option<fn(&mut Inspection,&mut Owner)->Result<(),String>>,
}

pub struct UiGraphState {
    pub panels:BTreeMap<PanelId,Panel>, pub groups:[PointerGroup;3],
    ///Live current-human+10 supplier, updated at actual protected UI boundary.
    pub human_enabled:u8,
    pub dispatch_blocked:u8, //legacy fixture/install receipt; NEVER live authority when source bound
    process_runtime:Option<crate::live_host::process_runtime::Handle>,
    chat_primary84_verified:bool,

    pub upper:Option<crate::navigator::upper::State>,
    ///Moved public native_input owner and process-wide590504/model operands.
    pub pointer_owner:Option<super::f_generic::Shared>,
    pub retained_c:Option<Selected>,
    pub report_inputs:ReportInputs, pub world:StrategicWorld,
    pub families:Families,
    pub main:super::main_panel::State,
    pub compass_literal:u8, pub compass_identifier:u32,
    ///4253a0 reset state: caption425400 never modifies these.
    pub speed_disabled:u8,pub speed_blink:u8,
    ///43e350 current secondary focus identity, independently maintained.
    pub focus_child:Option<WidgetId>,
}
impl UiGraphState {
    ///Only actual load/fresh UI owner producer may construct Installed. No
    ///Default and no guessed mode, visibility, frame, selection or group state.
    pub fn installed(panels:BTreeMap<PanelId,Panel>, groups:[PointerGroup;3],
        human_enabled:u8, dispatch_blocked:u8, retained_c:Option<Selected>, report_inputs:ReportInputs,
        world:StrategicWorld,families:Families,main:super::main_panel::State,compass_literal:u8,
        compass_identifier:u32,speed_disabled:u8,speed_blink:u8,
        focus_child:Option<WidgetId>) -> Result<Self,String> {
        for id in [PanelId::A,PanelId::B,PanelId::C,PanelId::D,PanelId::E,
            PanelId::F,PanelId::G,PanelId::H] {
            if !panels.contains_key(&id) {return Err(format!("Graph install missing {id:?}"));}
        }
        let mut seen=std::collections::BTreeSet::new();
        for (i,g) in groups.iter().enumerate() {
            g.links.validate()?;
            if g.links.len()!=[18,23,7][i]{return Err("Graph registration does not match actual18/23/7 owner census".into());}
            for id in g.links.identities() {
                if !seen.insert(id){return Err("Panel registered more than once".into());}
                let p=panels.get(&id).ok_or("Registered panel absent from graph")?;
                if group_index(p.group)!=i {return Err("Registered panel group mismatch".into());}
            }
        }
        if seen.len()!=panels.len(){return Err("Graph contains unregistered panels".into());}
        for id in [PanelId::A,PanelId::B,PanelId::C,PanelId::D,PanelId::E,PanelId::F,PanelId::G,PanelId::H]{
            if panels[&id].group!=Group::Primary{return Err("A..H must all belong to Primary".into());}
        }
        Ok(Self {panels,groups,human_enabled,dispatch_blocked,process_runtime:None,chat_primary84_verified:false,upper:None,pointer_owner:None,retained_c,report_inputs,world,
            families,main,compass_literal,compass_identifier,speed_disabled,speed_blink,focus_child})
    }
    pub fn bind_process_source(&mut self,process:crate::live_host::process_runtime::Handle,c:&super::process_image::Cells)->Result<(),String>{
        if self.process_runtime.is_some(){return Err("process graph source already bound".into())}
        let actual=c.word(0x57e710+0x84)?;
        let entry=super::registration::entries(Group::Primary).into_iter()
            .find(|entry|entry.id==PanelId::OtherPrimary(3)).ok_or("actual chat registration absent")?;
        if actual!=0x57ef28||entry.object!=actual{return Err("actual Primary+84 is not installed chat57ef28".into())}
        self.panel(PanelId::OtherPrimary(3))?;
        self.process_runtime=Some(process);self.chat_primary84_verified=true;Ok(())
    }
    pub fn chat_guard57e794_plus345(&self)->Result<u8,String>{
        if !self.chat_primary84_verified{return Err("chat guard has no actual Primary+84 install provenance".into())}
        Ok(self.panel(PanelId::OtherPrimary(3))?.requested)
    }
    pub(super) fn media_dispatch_blocked(&self)->Result<u8,String>{
        if let Some(process)=&self.process_runtime{return process.byte(crate::live_host::process_runtime::Byte::Media8fca98).map_err(|e|e.to_string());}
        //Legacy graph fixtures remain explicitly outside joined source route.
        //No production source installation uses the copied byte.
        #[cfg(test)] {return Ok(self.dispatch_blocked);}
        #[cfg(not(test))] {Err("graph source media producer absent".into())}
    }
    pub fn group(&self,g:Group)->&PointerGroup {&self.groups[group_index(g)]}
    pub fn group_mut(&mut self,g:Group)->&mut PointerGroup {&mut self.groups[group_index(g)]}
    pub fn panel(&self,id:PanelId)->Result<&Panel,String> {
        self.panels.get(&id).ok_or_else(||format!("Uninstalled panel {id:?}"))
    }
    pub fn panel_mut(&mut self,id:PanelId)->Result<&mut Panel,String> {
        self.panels.get_mut(&id).ok_or_else(||format!("Uninstalled panel {id:?}"))
    }
    pub fn shell(&self,id:PanelId)->bool {self.panel(id).is_ok_and(|p|
        self.group(p.group).enabled!=0 && p.shell())}
    pub fn body(&self,id:PanelId)->bool {self.panel(id).is_ok_and(|p|
        self.group(p.group).enabled!=0 && p.body())}
    pub fn input(&self,id:PanelId)->bool {self.panel(id).is_ok_and(|p|
        self.group(p.group).enabled!=0 && p.children())}
    pub fn group_enabled(&self,g:Group)->bool {self.group(g).enabled!=0}
    pub fn has_selection(&self,g:Group)->bool {self.group(g).has_selection()}
}
pub fn group_index(g:Group)->usize {match g {Group::Primary=>0,Group::Secondary=>1,Group::Tertiary=>2}}
fn graph(view:&impl LiveAccess)->Result<&UiGraphState,String> {
    view.inspection().ui_graph.as_ref().ok_or_else(||"UI graph lacks actual load-boundary producer".into())
}
fn graph_mut(view:&mut impl LiveAccess)->Result<&mut UiGraphState,String> {
    view.inspection_mut().ui_graph.as_mut().ok_or_else(||"UI graph lacks actual load-boundary producer".into())
}
fn required(family:Option<Family>,name:&str)->Result<Family,String> {
    family.ok_or_else(||format!("Navigation reached unimplemented {name}"))
}
fn family(view:&impl LiveAccess,id:PanelId,shown:bool)->Result<Family,String> {
    let f=graph(view)?.families;
    match (id,shown) {
        (PanelId::C,true)=>required(f.c_show,"C40f550 active-page/poll/prestige/ownership rebuild"),
        (PanelId::B,true)=>required(f.b_show,"B418ee0 validation/report show"),
        (PanelId::B,false)=>required(f.b_hide,"B418f70 human464060/A show"),
        (PanelId::H,true)=>required(f.h_show,"H40c480 apartment report show"),
        (PanelId::H,false)=>required(f.h_hide,"H40c500 human464060/A show"),
        (PanelId::G,true)=>required(f.g_show,"G423300 build show"),
        (PanelId::G,false)=>required(f.g_hide,"G423490 global/property445d10 writes"),
        _=>Err(format!("No finite specialized family for {id:?}/{shown}")),
    }
}
fn check_phase11(view:&impl LiveAccess,id:PanelId,owner:&Owner)->Result<(),String> {
    check_phase11_shown(view,id,owner,graph(view)?.panel(id)?.requested!=0)
}
fn check_phase11_shown(view:&impl LiveAccess,id:PanelId,owner:&Owner,shown:bool)->Result<(),String>{
    let g=graph(view)?;let p=g.panel(id)?;
    //43c090 event11 bypasses moving but not348/34c/global/shown gates.
    if p.draw_enabled==0||p.block34c!=0||g.media_dispatch_blocked()?!=0||!shown{return Ok(());}
    if id==PanelId::C {
        super::strategic_panel::check_tree11(view.inspection(),id,owner)?;
    } else if !p.children.is_empty(){
        let f=g.families;
        let check=f.tree11_check.ok_or("Reached actual child-tree phase11 checker absent")?;
        if f.tree11.is_none(){return Err("Reached actual child-tree phase11 consumer absent".into());}
        check(view.inspection(),id,owner)?;
    }
    match id {
        PanelId::C=>return super::strategic_panel::check_phase11(view.inspection(),id,owner),
        PanelId::E|PanelId::D=>return Ok(()),
        PanelId::A=>return super::main_panel::check_message(view,11,owner,g.panel(id)?.frame),
        PanelId::F=>return super::upper_f::check_phase11(view.inspection(),owner),
        _=>{},
    }
    let f=g.families;
    let check=f.phase11_check.ok_or_else(||format!("Reached {id:?} phase11 checker missing"))?;
    if f.phase11.is_none(){return Err(format!("Reached {id:?} phase11 consumer missing"));}
    check(view.inspection(),id,owner)
}
pub(super) fn phase11(view:&mut impl LiveAccess,id:PanelId,owner:&mut Owner)->Result<(),String> {
    let g=graph(view)?;let p=g.panel(id)?;
    if p.draw_enabled==0||p.block34c!=0||g.media_dispatch_blocked()?!=0||p.requested==0{return Ok(());}
    phase11_body(view,id,owner)?;
    //Original rereads the child head AFTER the specialized callback.
    if id==PanelId::C {
        super::strategic_panel::tree11(view.inspection_mut(),id,owner)?;
    } else if !graph(view)?.panel(id)?.children.is_empty(){
        let call=graph(view)?.families.tree11.ok_or("Reached child-tree phase11 consumer absent")?;
        call(view.inspection_mut(),id,owner)?;
    }
    Ok(())
}
fn phase11_body(view:&mut impl LiveAccess,id:PanelId,owner:&mut Owner)->Result<(),String>{
    match id {
        PanelId::C=>return super::strategic_panel::phase11(view.inspection_mut(),id,owner),
        PanelId::A=>return super::main_panel::message(view,11,owner),
        PanelId::E=>return Ok(()), //425490 returns unless message12
        PanelId::D=>{let world=&mut graph_mut(view)?.world;
            world.messages=world.messages.wrapping_add(1);return Ok(());}, //42bff0
        PanelId::F=>return super::upper_f::phase11(view.inspection_mut(),owner),
        _=>{},
    }
    let call=graph(view)?.families.phase11.ok_or("Reached phase11 implementation absent")?;
    call(view.inspection_mut(),id,owner)
}
///All reached work is checked before43be20 mutates common bytes. A complete
///navigation preflight below checks the complete transaction before raw copies.
pub fn check_visibility(view:&impl SelectionAccess,id:PanelId,shown:bool,owner:&Owner)->Result<(),String> {
    let g=view.selection_graph()?;let p=g.panel(id)?;
    if shown && p.requested!=0 && p.force_show==0 {return Ok(());}
    if !shown && p.requested==0 {return Ok(());}
    match (id,shown) {
        (PanelId::A,_)|(PanelId::E,_)|(PanelId::F,_)|(PanelId::C,false)=>{},
        // Real C show reads its Locator suppliers, not this copied facade.
        // Keep the actual family checker; pointer-only admission must not
        // require a live descriptor allocation.
        (PanelId::C,true)=>view.check_family(id,shown,owner,owner.ui_selection()?.current)?,
        (PanelId::D,true)=>{let mut preview=g.world.clone();preview.show(&g.report_inputs)?;},
        (PanelId::D,false)=>{},
        _=>{let current=owner.current_ui_selected()?;
            view.check_family(id,shown,owner,current)?;}
    }
    if id==PanelId::F {
        view.check_upper_tree()?;
    }
    if shown {view.check_shown_phase11(id,owner,!((id==PanelId::A||id==PanelId::C)&&g.human_enabled==0))?;}
    Ok(())
}
pub fn visible(view:&mut impl LiveAccess,object:Object,_owner:&mut Owner)->Result<u8,String> {
    Ok(graph(view)?.panel(object.into())?.requested)
}
pub fn visibility(view:&mut impl LiveAccess,object:Object,shown:bool,owner:&mut Owner)->Result<(),String> {
    set_visibility(view,object.into(),shown,owner)
}
pub fn set_visibility(view:&mut impl LiveAccess,id:PanelId,shown:bool,owner:&mut Owner)->Result<(),String> {
    check_visibility(view,id,shown,owner)?;
    if shown {
        let p=graph_mut(view)?.panel_mut(id)?;
        if p.requested!=0 && p.force_show==0 {return Ok(());}
        p.moving=1;p.requested=1;
        match id {
            PanelId::A=>{if graph(view)?.human_enabled==0 {set_visibility(view,id,false,owner)?;}},
            PanelId::E|PanelId::F=>{}, //exact specialized RET bodies; common stays real
            PanelId::D=>{let g=graph_mut(view)?;g.world.show(&g.report_inputs)?;},
            _=>{let call=family(view,id,true)?.apply;call(view.inspection_mut(),owner)?;}
        }
        if graph(view)?.panel(id)?.independent!=0 {graph_mut(view)?.panel_mut(id)?.moving=0;}
        phase11(view,id,owner)?;
    } else {
        if graph(view)?.panel(id)?.requested!=0 {
            let p=graph_mut(view)?.panel_mut(id)?;
            p.moving=1;p.requested=0;
            if p.transition_end>1 {p.velocity=p.position.map(|v|(-f64::from(v)/f64::from(p.transition_end-1))as f32);}
            match id {
                PanelId::A|PanelId::E|PanelId::F=>{}, //verified specialized RETs
                PanelId::C=>{let retained=graph(view)?.retained_c;owner.ui_selection_mut()?.strategic_hide(retained);},
                PanelId::D=>{let g=graph_mut(view)?;g.world.hide(&g.report_inputs);},
                _=>{let call=family(view,id,false)?.apply;call(view.inspection_mut(),owner)?;}
            }
            let g=graph_mut(view)?;
            let group=g.panel(id)?.group;
            if g.group(group).selected.is_some_and(|target|target.panel==id) {
                g.group_mut(group).selected=None;
            }
        }
        if graph(view)?.panel(id)?.independent!=0 {graph_mut(view)?.panel_mut(id)?.moving=0;}
    }
    Ok(())
}
///425400 caption-only. Real focus child is updated by navigation/focus helper.
pub fn refresh_bottom_hud(view:&mut impl LiveAccess,_owner:&mut Owner)->Result<(),String> {
    let mode=view.inspection().camera_navigation.as_ref().ok_or("HUD lacks live UI mode owner")?.mode();
    let g=graph_mut(view)?;g.compass_literal=0;g.compass_identifier=if mode==3{1143}else{1142};
    Ok(())
}
///4253a0 is deliberately separate and called only at the actual reset boundary.
pub fn reset_bottom_hud(view:&mut Inspection,network_active:bool)->Result<(),String> {
    let g=graph_mut(view)?;g.speed_disabled=u8::from(network_active);if network_active{g.speed_blink=0;}Ok(())
}
pub fn check_reset(view:&Inspection,which:Group)->Result<(),String> {
    let Some(target)=graph(view)?.group(which).dwell else{return Ok(());};
    if Some(target)==graph(view)?.focus_child&&view.camera_focus.retained_primary_identifier().is_some(){
        return Err("Group clear with retained primary caption needs real synchronous43e0a0 font measurement".into());
    }
    match (target.panel,target.kind) {
        (PanelId::F,WidgetKind::Panel)=>super::upper_f::check_tree(view),
        (PanelId::E,WidgetKind::Compass)|(PanelId::E,WidgetKind::Time(crate::time_panel::Button::Speed))
        |(PanelId::E,WidgetKind::Time(crate::time_panel::Button::Manager))
        |(PanelId::Clock,WidgetKind::Time(crate::time_panel::Button::Options))=>Ok(()),
        _=>Err(format!("Group {which:?} dwell {target:?} requires its actual phase3 consumer")),
    }
}
pub fn reset_group(view:&mut impl LiveAccess,which:Group,_owner:&mut Owner)->Result<(),String> {
    check_reset(view.inspection(),which)?;
    if let Some(target)=graph(view)?.group(which).dwell {
        match target.kind {
            //Generic compass phase3 clears its one-shot flag; visual down stays.
            WidgetKind::Panel if target.panel==PanelId::F=>{}, //43c090 event3 returns before43d130
            WidgetKind::Compass=>view.inspection_mut().navigation_click_latched=false,
            WidgetKind::Time(button)=>view.inspection_mut().time_panel.leave_child(button),
            _=>return Err("Checked phase3 family changed before dispatch".into()),
        }
        //Native rereads group.dwell AFTER event3, compares even null/null.
        let after=graph(view)?.group(which).dwell;
        if after==graph(view)?.focus_child {
            view.inspection_mut().camera_focus.clear_child();
            view.inspection_mut().tooltip=None;view.inspection_mut().tooltip_state=None;
            graph_mut(view)?.focus_child=None;
        }
        graph_mut(view)?.group_mut(which).dwell=None;
    }
    Ok(())
}
pub fn enable_group(view:&mut impl LiveAccess,which:Group,enabled:bool,_owner:&mut Owner)->Result<(),String> {
    graph_mut(view)?.group_mut(which).enabled=u8::from(enabled);Ok(())
}
pub fn toolbar_mode(view:&mut impl LiveAccess,mode:u32,owner:&mut Owner)->Result<(),String> {
    super::main_panel::switch(view,mode,owner)
}
/// Narrow access to the actual owners used by picked-panel restore and
/// navigation admission. Inspection supplies all concrete family checks.
/// Tests can own this finite graph/Main/navigation state without GPU/Host.
pub(super) trait SelectionAccess:super::main_panel::Access {
    fn selection_graph(&self)->Result<&UiGraphState,String>;
    fn navigation_state(&self)->Result<&super::navigation::State,String>;
    fn tutorial_session(&self)->Result<bool,String>;
    fn check_group(&self,which:Group)->Result<(),String>;
    fn check_panel(&self,id:PanelId,shown:bool,owner:&Owner)->Result<(),String>;
    fn check_family(&self,id:PanelId,shown:bool,owner:&Owner,current:Option<Selected>)->Result<(),String>;
    fn check_upper_tree(&self)->Result<(),String>;
    fn check_shown_phase11(&self,id:PanelId,owner:&Owner,shown:bool)->Result<(),String>;
}
pub(super) trait LiveAccess:SelectionAccess {
    fn inspection(&self)->&Inspection;
    fn inspection_mut(&mut self)->&mut Inspection;
}
impl LiveAccess for Inspection {
    fn inspection(&self)->&Inspection{self}
    fn inspection_mut(&mut self)->&mut Inspection{self}
}
impl LiveAccess for super::main_panel::ProcessTextAccess<'_> {
    fn inspection(&self)->&Inspection{self.view}
    fn inspection_mut(&mut self)->&mut Inspection{self.view}
}
impl SelectionAccess for super::main_panel::ProcessTextAccess<'_> {
    fn selection_graph(&self)->Result<&UiGraphState,String>{graph(self)}
    fn navigation_state(&self)->Result<&super::navigation::State,String>{self.view.navigation_state()}
    fn tutorial_session(&self)->Result<bool,String>{self.view.tutorial_session()}
    fn check_group(&self,which:Group)->Result<(),String>{self.view.check_group(which)}
    fn check_panel(&self,id:PanelId,shown:bool,owner:&Owner)->Result<(),String>{check_visibility(self,id,shown,owner)}
    fn check_family(&self,id:PanelId,shown:bool,owner:&Owner,current:Option<Selected>)->Result<(),String>{
        self.view.check_family(id,shown,owner,current)
    }
    fn check_upper_tree(&self)->Result<(),String>{self.view.check_upper_tree()}
    fn check_shown_phase11(&self,id:PanelId,owner:&Owner,shown:bool)->Result<(),String>{check_phase11_shown(self,id,owner,shown)}
}
impl SelectionAccess for Inspection {
    fn selection_graph(&self)->Result<&UiGraphState,String>{graph(self)}
    fn navigation_state(&self)->Result<&super::navigation::State,String>{
        self.camera_navigation.as_ref().ok_or_else(||"Navigation state absent".into())
    }
    fn tutorial_session(&self)->Result<bool,String>{
        Ok(self.camera_ui.as_ref().ok_or("Graph lacks current admission")?.admission().tutorial_session)
    }
    fn check_group(&self,which:Group)->Result<(),String>{check_reset(self,which)}
    fn check_panel(&self,id:PanelId,shown:bool,owner:&Owner)->Result<(),String>{check_visibility(self,id,shown,owner)}
    fn check_family(&self,id:PanelId,shown:bool,owner:&Owner,current:Option<Selected>)->Result<(),String>{
        (family(self,id,shown)?.check)(self,owner,current)
    }
    fn check_upper_tree(&self)->Result<(),String>{super::upper_f::check_tree(self)}
    fn check_shown_phase11(&self,id:PanelId,owner:&Owner,shown:bool)->Result<(),String>{check_phase11_shown(self,id,owner,shown)}
}
pub fn restore_picked_panel(view:&mut impl SelectionAccess,owner:&mut Owner)->Result<(),String> {
    let selected=owner.resolve_ui_selected(owner.ui_selection()?.published)?
        .ok_or("4c39c0 called with null published selection")?;
    let mode=view.navigation_state()?.mode();
    let requested=match selected.kind() {
        i16::MIN..=0=>return Ok(()),1|2=>5,
        4 if mode!=3=>if owner.ui_selection()?.sequence_index.is_some_and(|i|i != -1){2}else{1},
        _=>return Ok(()), //source-proven kinds3/5+ or kind4 while mode3 return
    };
    super::main_panel::switch(view,requested,owner)
}
pub fn event73(view:&mut impl LiveAccess,owner:&mut Owner)->Result<(),String> {
    if !view.inspection().camera_ui.as_ref().ok_or("Event73 lacks actual admission")?.admission().tutorial_session {return Ok(());}
    let call=graph(view)?.families.event73.ok_or("Tutorial73 synchronous Lua consumer absent")?;
    call(view.inspection_mut(),owner)
}
//Concrete stored callback signatures preserve both adapter lifetimes.
pub fn consumers()->Consumers {
    Consumers{visible:consume_visible,visibility:consume_visibility,
        reset_group:consume_reset_group,enable_group:consume_enable_group,
        toolbar_mode:consume_toolbar_mode,restore_picked_panel:consume_restore,
        refresh_bottom_hud:consume_refresh,event73:Some(consume_event73)}
}
fn consume_visible(v:&mut super::main_panel::ProcessTextAccess<'_>,id:Object,o:&mut Owner)->Result<u8,String>{visible(v,id,o)}
fn consume_visibility(v:&mut super::main_panel::ProcessTextAccess<'_>,id:Object,shown:bool,o:&mut Owner)->Result<(),String>{visibility(v,id,shown,o)}
fn consume_reset_group(v:&mut super::main_panel::ProcessTextAccess<'_>,g:Group,o:&mut Owner)->Result<(),String>{reset_group(v,g,o)}
fn consume_enable_group(v:&mut super::main_panel::ProcessTextAccess<'_>,g:Group,enabled:bool,o:&mut Owner)->Result<(),String>{enable_group(v,g,enabled,o)}
fn consume_toolbar_mode(v:&mut super::main_panel::ProcessTextAccess<'_>,mode:u32,o:&mut Owner)->Result<(),String>{toolbar_mode(v,mode,o)}
fn consume_restore(v:&mut super::main_panel::ProcessTextAccess<'_>,o:&mut Owner)->Result<(),String>{restore_picked_panel(v,o)}
fn consume_refresh(v:&mut super::main_panel::ProcessTextAccess<'_>,o:&mut Owner)->Result<(),String>{refresh_bottom_hud(v,o)}
fn consume_event73(v:&mut super::main_panel::ProcessTextAccess<'_>,o:&mut Owner)->Result<(),String>{event73(v,o)}


// Scene refresh uses intro::refresh: full preparation precedes every owner take.

///Preflight called BEFORE43e370 and either raw-copy branch. It deliberately
///rejects unjoined families before any mode/visibility/selection mutation.
///No guessed claims about hidden B/H/G effects are needed: exact byte==1
///matches the native pre-camera tests.
pub fn preflight(view:&impl SelectionAccess,owner:&Owner)->Result<(),String> {
    use PanelId::*;
    let g=view.selection_graph()?;
    // Resolve the value that the reached Main branch will consume BEFORE
    // invoking any family preflight or allowing primary-clear/raw-copy work.
    // A visible C restores its independent retained cell; do not substitute
    // fresh current for that retained identity or touch unrelated published.
    let current=if owner.kind()==4 && g.panel(C)?.requested!=0 {
        owner.resolve_ui_selected(g.retained_c)?
    }else{
        owner.resolve_ui_selected(owner.ui_selection()?.current)?
    };
    for group in [Group::Secondary,Group::Primary,Group::Tertiary] {view.check_group(group)?;}
    if view.tutorial_session()?
        && g.families.event73.is_none() {return Err("Tutorial73 synchronous Lua consumer absent".into());}
    if owner.kind()==4 {
        let nav=view.navigation_state()?;
        if nav.mode()==3 {
            let saved=nav.saved_visibility();
            if saved[0]!=0{view.check_panel(A,true,owner)?;}
            if saved[2]!=0{view.check_panel(B,true,owner)?;}
        }
        for id in [C,D]{view.check_panel(id,false,owner)?;}
        for id in [E,F]{view.check_panel(id,true,owner)?;}
        //C hide may restore a different selection; check that value BEFORE raw copy.
        let source=current.map(|selected|selected.source());
        super::main_panel::preflight(view,1,owner,source)?;
        if let Some(selected)=current {
            let mode=match selected.kind() {1|2=>Some(5),4=>Some(if owner.ui_selection()?.sequence_index.is_some_and(|i|i != -1){2}else{1}),_=>None};
            if let Some(mode)=mode{super::main_panel::preflight(view,mode,owner,source)?;}
        }
    } else {
        for id in [G,H,B] {if g.panel(id)?.requested==1 {view.check_panel(id,false,owner)?;}}
        view.check_panel(A,true,owner)?;
        //A will be shown before it is hidden, even if entry requested was0.
        //A hide is the exact RET. F's actual current byte chooses its hide.
        view.check_panel(F,false,owner)?;
        for id in [D,C]{view.check_panel(id,true,owner)?;}
    }
    Ok(())
}

///Full live43c8e0. Explicit assets/provider seam; installation and native
///activation remain the containing producer's responsibility.
pub fn advance_group<P:super::group_bridge::Families>(view:&mut Inspection,which:Group,
    owner:&mut Owner,assets:&mut super::super::Assets,providers:&mut P)->Result<(),String>{
    super::group_dispatch::advance(&mut super::group_bridge::Bridge{view,owner,assets,providers},which)
}
pub(super) fn visible_message(view:&mut impl LiveAccess,id:PanelId,owner:&mut Owner)->Result<(),String>{
    match id {
        PanelId::C=>super::strategic_panel::visible1(view.inspection_mut(),id,owner),
        PanelId::D=>{let world=&mut graph_mut(view)?.world;
            world.messages=world.messages.wrapping_add(1);Ok(())}, //42bff0
        PanelId::A=>super::main_panel::message(view,1,owner),
        PanelId::E=>Ok(()), //425490 returns unless event12
        PanelId::F=>super::upper_f::visible1(view.inspection_mut(),owner),
        _=>{
            let call=graph(view)?.families.visible1.ok_or("Reached visible event1 implementation absent")?;
            call(view.inspection_mut(),id,owner)
        }
    }
}

#[cfg(test)]
pub(crate) fn refresh_fixture(colors:[u32;600])->UiGraphState {
    use super::navigation::Group;
    let registrations = [
        [PanelId::A,PanelId::B,PanelId::C,PanelId::D,PanelId::E,PanelId::F,PanelId::G,PanelId::H]
            .into_iter().chain((0..10).map(PanelId::OtherPrimary)).collect::<Vec<_>>(),
        (0..23).map(PanelId::Frontend).collect(),
        (0..7).map(PanelId::Auction).collect(),
    ];
    let mut panels=BTreeMap::new();
    for (i, ids) in registrations.iter().enumerate() {
        for &id in ids {
            panels.insert(id,Panel::bound([Group::Primary,Group::Secondary,Group::Tertiary][i],
                10,7,[0.,0.],false,None,255).unwrap());
        }
    }
    panels.get_mut(&PanelId::C).unwrap().children=vec![WidgetId{panel:PanelId::C,kind:WidgetKind::LocatorChild(5)}];
    let mut groups=registrations.map(|ids|PointerGroup::after_constructor(ids,None));
    groups[0].enabled=1;
    groups[0].selected=Some(WidgetId{panel:PanelId::C,kind:WidgetKind::LocatorChild(5)});
    let mut g=UiGraphState::installed(panels,groups,1,0,None,
        ReportInputs{buildings:[Building{active:0,owner_color:None};600],
            properties:[Property{active:0,excluded:0,title:0};52],titles:vec![0;46]},
        StrategicWorld::installed(3,1,17,colors,vec![]),Families::default(),
        super::main_panel::regression_tests::fixture().main,0,1143,0,0,None).unwrap();
    use crate::navigator::upper::{Geometry,Inputs,Pointer,State};
    g.upper=Some(State::installed(Geometry{bounds:[0.,0.,10.,10.],second:[0.;4],backing:[0.;4],
        parent_open:[0.;4],parent_closed:[0.;4]},7,1,1,
        Pointer{xy:[2,3],blocked:0,restricted:0,allowed:-1},
        std::sync::Arc::new(Inputs{source_epoch:crate::live_host::camera_bridge::PropertyEpoch::synthetic(1),rows:[[0;380];52],owners:[None;52],count:52,phase:17,
            presentation_mode:3,property_labels:[0;52]})));
    g
}
