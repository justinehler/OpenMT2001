//! Concrete graph bridge for group_dispatch. No second group/link/input owner.
//! This is a callable source seam, NOT a complete48-tree producer or endpoint
//! acknowledgment. Reached non-F families and central providers remain required.
use super::{group_dispatch::{self,Central,Dispatch,Result},navigation::Group,
    owner::Owner,ui_graph::{self,Panel,PanelId,PointerGroup,WidgetId,WidgetKind},
    super::{Inspection,Assets}};
use openmt2001::{native_input::StagedInput,camera::primary::CursorRequest};

///No successful defaults. Each unported reached family must return Err. A
///callback receives the real Inspection/Owner (generic14 also the SAME process
///text borrow) and may synchronously mutate
///links/selection/input/suppliers. It is called ONCE, never during preflight.
pub trait Families {
    fn slot20(&mut self,v:&mut Inspection,o:&mut Owner,id:WidgetId)->Result<u8>;
    fn slot24(&mut self,v:&mut Inspection,o:&mut Owner,id:PanelId)->Result<Option<WidgetId>>;
    fn generic14(&mut self,v:&mut super::main_panel::ProcessTextAccess<'_>,o:&mut Owner,id:WidgetId,event:u32,a:i32,b:i32)->Result<()>;
    fn child_block34c(&mut self,v:&mut Inspection,id:WidgetId)->Result<u8>;
    fn child_node1c(&mut self,v:&mut Inspection,id:WidgetId)->Result<u8>;
    fn show_focus43e310(&mut self,v:&mut Inspection,o:&mut Owner,assets:&mut Assets,id:WidgetId)->Result<()>;
    fn reset_old_key4439d0(&mut self,v:&mut Inspection,o:&mut Owner,id:WidgetId)->Result<()>;
}
///Useful finite production boundary, not a fake success provider. F's proved
///handlers are below; other reached generic/hit/focus/key families reject.
pub struct MissingFamilies;
fn missing<T>(operation:&str,id:impl std::fmt::Debug)->Result<T>{
    Err(format!("live group reached unsupplied {operation} for {id:?}; partial boundary, no rollback"))
}
impl Families for MissingFamilies {
    fn slot20(&mut self,_v:&mut Inspection,_o:&mut Owner,id:WidgetId)->Result<u8>{missing("slot20",id)}
    fn slot24(&mut self,_v:&mut Inspection,_o:&mut Owner,id:PanelId)->Result<Option<WidgetId>>{missing("slot24",id)}
    fn generic14(&mut self,_v:&mut super::main_panel::ProcessTextAccess<'_>,_o:&mut Owner,id:WidgetId,event:u32,_a:i32,_b:i32)->Result<()>{missing(&format!("generic14 event{event}"),id)}
    fn child_block34c(&mut self,_v:&mut Inspection,id:WidgetId)->Result<u8>{missing("search-node34c",id)}
    fn child_node1c(&mut self,_v:&mut Inspection,id:WidgetId)->Result<u8>{missing("key-target node1c",id)}
    fn show_focus43e310(&mut self,_v:&mut Inspection,_o:&mut Owner,_assets:&mut Assets,id:WidgetId)->Result<()>{missing("focus43e310/font",id)}
    fn reset_old_key4439d0(&mut self,_v:&mut Inspection,_o:&mut Owner,id:WidgetId)->Result<()>{missing("old-key4439d0",id)}
}

pub struct Bridge<'a,P>{pub view:&'a mut Inspection,pub owner:&'a mut Owner,
    pub assets:&'a mut Assets,pub providers:&'a mut P}
///Borrowed concrete adapter fields, not another focus/graph authority. The
///production caller lends its SAME owners and synchronous font consumer.
///Tests can fail that consumer without constructing an unrelated GPU/Host.
pub(super) struct FocusAdapter<'a>{
    pub graph:&'a mut ui_graph::UiGraphState,
    pub focus:&'a mut super::focus::Focus,
    pub tooltip:&'a mut Option<openmt2001::render::PreparedScene>,
    pub tooltip_state:&'a mut Option<(u32,[i32;2])>,
}
impl FocusAdapter<'_>{
    pub(super) fn refresh_caption(&mut self,measure:impl FnOnce(u32)->Result<()>)->Result<()>{
        if let Some(id)=self.focus.retained_text_identifier(){measure(id)?;}
        *self.tooltip=None;*self.tooltip_state=None;Ok(())
    }
    pub(super) fn clear_child(&mut self,measure:impl FnOnce(u32)->Result<()>)->Result<()>{
        //43e390..43e395: read the SAME590da4 identity, then immediate RET
        //when null. In particular, do not refresh a retained primary caption.
        if self.graph.focus_child.is_none(){return Ok(())}
        self.focus.clear_child();self.graph.focus_child=None;
        //Both prior clears survive a fallible43e0a0 font consumer.
        self.refresh_caption(measure)
    }
}
impl<P> Bridge<'_,P>{
    fn graph(&self)->Result<&ui_graph::UiGraphState>{self.view.ui_graph.as_ref().ok_or_else(||"live group requires actual complete process graph installation".into())}
    fn graph_mut(&mut self)->Result<&mut ui_graph::UiGraphState>{self.view.ui_graph.as_mut().ok_or_else(||"live group requires actual complete process graph installation".into())}
    fn shared(&self)->Result<&super::f_generic::Shared>{self.graph()?.pointer_owner.as_ref().ok_or_else(||"live group requires SAME retained staged input/shared override owner".into())}
    fn shared_mut(&mut self)->Result<&mut super::f_generic::Shared>{self.graph_mut()?.pointer_owner.as_mut().ok_or_else(||"live group requires SAME retained staged input/shared override owner".into())}
}
impl<P:Families> Dispatch for Bridge<'_,P>{
    fn group(&self,g:Group)->Result<&PointerGroup>{Ok(self.graph()?.group(g))}
    fn group_mut(&mut self,g:Group)->Result<&mut PointerGroup>{Ok(self.graph_mut()?.group_mut(g))}
    fn panel(&self,id:PanelId)->Result<&Panel>{self.graph()?.panel(id)}
    fn panel_mut(&mut self,id:PanelId)->Result<&mut Panel>{self.graph_mut()?.panel_mut(id)}
    fn input(&self)->Result<&StagedInput>{Ok(&self.shared()?.input)}
    fn override_target(&mut self)->Result<Option<WidgetId>>{Ok(self.shared()?.override590504)}
    fn block34c(&mut self,id:WidgetId)->Result<u8>{
        if id.kind==WidgetKind::Panel{Ok(self.graph()?.panel(id.panel)?.block34c)}
        else{self.providers.child_block34c(self.view,id)}
    }
    fn node1c(&mut self,id:WidgetId)->Result<u8>{
        if id.kind==WidgetKind::Panel{Ok(self.graph()?.panel(id.panel)?.node_shown1c)}
        else{self.providers.child_node1c(self.view,id)}
    }
    fn slot20(&mut self,id:WidgetId)->Result<u8>{
        if id==group_dispatch::panel(PanelId::F){Ok(super::f_generic::slot20())}
        else{self.providers.slot20(self.view,self.owner,id)}
    }
    fn slot24(&mut self,id:PanelId)->Result<Option<WidgetId>>{
        if id==PanelId::F{super::f_generic::slot24(self.view)}
        else{self.providers.slot24(self.view,self.owner,id)}
    }
    fn generic14(&mut self,id:WidgetId,event:u32,a:i32,b:i32)->Result<()>{
        if id==group_dispatch::panel(PanelId::F){
            if a!=0{return Err("finite F generic adapter reached unsupported parameter-a".into())}
            //PositiveF5 remains the EXISTING explicit error. No event/OS route
            //is fabricated and no transaction is admitted in this bridge.
            super::f_generic::message(self.view,event,b)
        }else if id.kind==WidgetKind::Panel&&event==11&&a==0&&b==0{
            ui_graph::phase11(&mut super::main_panel::ProcessTextAccess{view:self.view,resources:&mut self.assets.ui_text},id.panel,self.owner)
        }else{self.providers.generic14(&mut super::main_panel::ProcessTextAccess{view:self.view,resources:&mut self.assets.ui_text},self.owner,id,event,a,b)}
    }
    fn direct18_visible1(&mut self,id:PanelId)->Result<()>{
        ui_graph::visible_message(&mut super::main_panel::ProcessTextAccess{view:self.view,resources:&mut self.assets.ui_text},id,self.owner)
    }
    fn cursor0_priority1(&mut self)->Result<()>{
        self.view.camera_ui.as_mut().ok_or("group falling-right requires SAME cursor arbiter")?
            .ui_cursor(CursorRequest{state:0,priority:1})
    }
    fn focus_child(&self)->Result<Option<WidgetId>>{Ok(self.graph()?.focus_child)}
    fn show_focus43e310(&mut self,id:WidgetId)->Result<()>{
        if id==group_dispatch::panel(PanelId::F){super::upper_f::focus_dwell_panel(self.view,self.assets)}
        else{self.providers.show_focus43e310(self.view,self.owner,self.assets,id)}
    }
    fn clear_focus43e390(&mut self)->Result<()>{
        let v=&mut *self.view;let assets=&*self.assets;
        FocusAdapter{
            graph:v.ui_graph.as_mut().ok_or("live group requires actual complete process graph installation")?,
            focus:&mut v.camera_focus,tooltip:&mut v.tooltip,tooltip_state:&mut v.tooltip_state,
        }.clear_child(|id|assets.tooltip.measure_focus_identifier(id).map(|_|()))
    }
    fn reset_old_key4439d0(&mut self,id:WidgetId)->Result<()>{self.providers.reset_old_key4439d0(self.view,self.owner,id)}
    fn has_character(&mut self)->Result<bool>{Ok(self.shared()?.input.character_available_byte()!=0)}
    fn take_character(&mut self)->Result<u8>{Ok(self.shared_mut()?.input.take_character())}
}

///Root's actual central source boundary must implement these calls against
///its one retained process/scheduler owner. There is no local repeat/count or
///G-state copy here, and no constructor reading PE zero-fill as runtime input.
pub trait CentralProvider:super::continuation_bridge::Provider {
    fn increment_update822d38(&mut self,v:&mut Inspection,o:&mut Owner)->Result<()>;
    fn suppressed8b0700(&mut self,v:&mut Inspection,o:&mut Owner)->Result<u8>;
    fn signed8b22d3(&mut self,v:&mut Inspection,o:&mut Owner)->Result<i8>;
    fn repeat541918(&mut self,v:&mut Inspection,o:&mut Owner)->Result<i32>;
    fn g_prelude8b22dc(&mut self,v:&mut Inspection,o:&mut Owner)->Result<u8>;
    fn actual_g4207d0(&mut self,v:&mut Inspection,o:&mut Owner)->Result<()>;
    // The concrete Bridge now runs the bounded continuation itself. No
    // successful after-groups callback can stand in for the missing algorithm.
}
impl<P:Families+CentralProvider> Central for Bridge<'_,P>{
    fn increment_update822d38(&mut self)->Result<()>{self.providers.increment_update822d38(self.view,self.owner)}
    fn suppressed8b0700(&mut self)->Result<u8>{self.providers.suppressed8b0700(self.view,self.owner)}
    fn signed8b22d3(&mut self)->Result<i8>{self.providers.signed8b22d3(self.view,self.owner)}
    fn repeat541918(&mut self)->Result<i32>{self.providers.repeat541918(self.view,self.owner)}
    fn g_prelude8b22dc(&mut self)->Result<u8>{self.providers.g_prelude8b22dc(self.view,self.owner)}
    fn actual_g4207d0(&mut self)->Result<()>{self.providers.actual_g4207d0(self.view,self.owner)}
    fn after_groups4ba5eb(&mut self,prefix:super::group_dispatch::PrefixComplete)->Result<super::continuation::BeforeTail>{
        super::continuation::run(self,prefix).map_err(|error|error.to_string())
    }
}

///Full group draw admission remains a separate containing renderer join.
///Existing F body may be reused by that renderer; absent slot08/cursor family
///must error. This adapter never installs an F-only replacement draw list.
pub trait PresentationProvider {
    fn draw08(&mut self,v:&mut Inspection,o:&mut Owner,assets:&Assets,id:PanelId)->Result<()>;
    fn finish_cursor9c(&mut self,v:&mut Inspection,o:&mut Owner,assets:&Assets)->Result<()>;
    ///Controller presentation4c2bdc reads the retained UI a4 operand; callback
    ///must implement the reached picking/presentation family or return Err.
    fn controller4c2ba0(&mut self,_v:&mut Inspection,_o:&mut Owner,_assets:&Assets,_activity_a4:u32)->Result<()>{
        Err("actual controller4c2ba0 presentation callback absent".into())
    }
}
impl<P:Families+PresentationProvider> Bridge<'_,P>{
    pub fn controller4c2ba0(&mut self)->Result<()>{
        let activity=self.assets.activity.presentation4c2bdc();
        self.providers.controller4c2ba0(self.view,self.owner,self.assets,activity)
    }
}
impl<P:Families+PresentationProvider> group_dispatch::Presentation for Bridge<'_,P>{
    fn draw08(&mut self,id:PanelId)->Result<()>{self.providers.draw08(self.view,self.owner,self.assets,id)}
    fn finish_cursor9c(&mut self)->Result<()>{self.providers.finish_cursor9c(self.view,self.owner,self.assets)}
}

#[cfg(test)]
#[path="group_focus_tests.rs"] mod focus_tests;
