//! Exact root callback bindings and guarded transfer/installation helper.
//! Intro owns channel/lifetime/save composition; this file starts no worker.
use super::{context::UiJoins,navigation,owner::Owner,ui_graph::{self,UiGraphState},
    ui_selection::SelectionState,super::{Assets,Inspection}};
pub struct ModeValues {pub mode:i32,pub shadow:i32,pub previous:i32,pub saved:[u8;3]}
pub fn install(view:&mut Inspection,owner:&mut Owner,mut graph:UiGraphState,
    values:ModeValues,selection:SelectionState)->Result<(),String>{
    if view.ui_graph.is_some()||view.camera_navigation.is_some(){
        return Err("UI graph installation would replace retained process state".into());
    }
    super::strategic_panel::install(&mut graph.families);
    owner.install_ui_selection(selection)?;
    view.ui_graph=Some(graph);
    view.camera_navigation=Some(navigation::State::new(values.mode,values.shadow,
        values.previous,values.saved,Some(ui_graph::consumers())));
    Ok(())
}
pub type Tutorial=fn(&mut Inspection,&mut Assets,crate::live_host::camera_bridge::NativeOperation,
    &mut Owner)->Result<(),String>;
///No successful missing family: navigation::primary_clear first performs the
///actual branch preflight. Tutorial callback remains explicitly supplied.
pub fn joins(tutorial:Option<Tutorial>)->UiJoins{
    UiJoins{primary_clear:Some(navigation::primary_clear),navigation:Some(navigation::apply),tutorial}
}
