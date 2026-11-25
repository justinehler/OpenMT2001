//! Borrows existing graph/input/selection/camera authorities. No process-control
//! implementation is invented: each missing reached join remains required.
use super::{continuation::{self,Source,Byte,Word,Effect,Receiver,Poll,LiveDevice,ParticipantDescriptor},
    group_bridge::Bridge,navigation::Group,owner::Owner,chat_ring::Ring,
    ui_selection::SelectionKey,super::{Inspection,Assets}};
use openmt2001::native_input::StagedInput;
type Result<T>=std::result::Result<T,String>;
///Process/Session/device joins. NO defaults. Read actual cells/writers, never
///Host snapshots, copied RetainedCells, backend mode, modern roster guesses or
///a FreshRequest back to a worker blocked on this operation's direct reply.
///Callbacks must complete synchronously or return a partial error. This inert
///adapter does not implement a suspended Host protocol by pretending success.
pub trait Provider {
    fn continuation_byte(&mut self,v:&mut Inspection,o:&mut Owner,cell:Byte)->Result<u8>;
    fn continuation_word(&mut self,v:&mut Inspection,o:&mut Owner,cell:Word)->Result<u32>;
    fn continuation_write_word(&mut self,v:&mut Inspection,o:&mut Owner,cell:Word,value:u32)->Result<()>;
    fn continuation_quit(&mut self,v:&mut Inspection,o:&mut Owner)->Result<()>;
    fn continuation_busy48dbf0(&mut self,v:&mut Inspection,o:&mut Owner)->Result<u8>;
    fn continuation_receiver(&mut self,v:&mut Inspection,o:&mut Owner)->Result<Receiver>;
    ///Receiver+vtable were just resolved. Read actual flags/buffering NOW.
    ///Borrow provider's existing device. Mouse returns DeviceMode::Mouse
    ///without reading keyboard buffering; only Keyboard reads receiver+1c.
    fn continuation_device(&mut self,receiver:Receiver,poll:Poll)->Result<LiveDevice<'_>>;
    ///Actual captured receiver+19218 read after its device callback.
    fn continuation_recording_after_device(&mut self,receiver:Receiver,poll:Poll)->Result<u8>;
    ///Borrow the ONE process68b4c0 object, transferred from actual constructor/
    ///append/reset producers. No empty-per-frame or rivalQueue adapter.
    fn continuation_ring(&mut self)->Result<&mut Ring>;
    ///Actual current object+38 only after source kind1/2 and native-key check.
    fn continuation_participant_player(&mut self,v:&mut Inspection,o:&mut Owner,key:SelectionKey)->Result<u32>;
    fn continuation_participant_flags(&mut self,v:&mut Inspection,o:&mut Owner,index:i32)->Result<u8>;
    fn continuation_participant_lookup(&mut self,v:&mut Inspection,o:&mut Owner,index:i32)->Result<ParticipantDescriptor>;
    ///All remaining actual calls, including source graph/menu/pause/sequence/
    ///command/selection/script families. Return Err for every unsupplied branch.
    fn continuation_effect(&mut self,v:&mut Inspection,o:&mut Owner,a:&mut Assets,effect:Effect)->Result<()>;
}
impl<P:Provider> Source for Bridge<'_,P>{
    fn input(&self)->Result<&StagedInput>{Ok(&self.view.ui_graph.as_ref()
        .ok_or("continuation requires SAME installed graph")?.pointer_owner.as_ref()
        .ok_or("continuation requires SAME process staged input/CHAR")?.input)}
    fn input_mut(&mut self)->Result<&mut StagedInput>{Ok(&mut self.view.ui_graph.as_mut()
        .ok_or("continuation requires SAME installed graph")?.pointer_owner.as_mut()
        .ok_or("continuation requires SAME process staged input/CHAR")?.input)}
    fn byte(&mut self,cell:Byte)->Result<u8>{
        if cell==Byte::Guard57e794Plus345{return self.view.ui_graph.as_ref().ok_or("source chat guard graph absent")?.chat_guard57e794_plus345();}
        self.providers.continuation_byte(self.view,self.owner,cell)
    }
    fn word(&mut self,cell:Word)->Result<u32>{
        match cell{
            Word::Activity8fcaa4=>Ok(self.assets.activity.a4()),
            Word::Activity8fcaa8=>Ok(self.assets.activity.a8()),
            Word::TargetZ93af9c=>self.owner.continuation_target_word(2),
            Word::TargetY93af98=>self.owner.continuation_target_word(1),
            Word::TargetX93af94=>self.owner.continuation_target_word(0),
            _=>self.providers.continuation_word(self.view,self.owner,cell),
        }
    }
    fn write_word(&mut self,cell:Word,value:u32)->Result<()>{match cell{
        Word::Activity8fcaa4=>{self.assets.activity.store_a4(value);Ok(())},
        Word::Activity8fcaa8=>{self.assets.activity.store_a8(value);Ok(())},
        _=>Err(format!("central has no generic writer for {cell:?}")),
    }}
    fn write_quit93442c(&mut self)->Result<()>{self.providers.continuation_quit(self.view,self.owner)}
    fn busy48dbf0(&mut self)->Result<u8>{self.providers.continuation_busy48dbf0(self.view,self.owner)}
    fn resolve_receiver8b2278(&mut self)->Result<Receiver>{self.providers.continuation_receiver(self.view,self.owner)}
    fn live_device_input(&mut self,receiver:Receiver,poll:Poll)->Result<(&mut StagedInput,LiveDevice<'_>)>{
        let input=&mut self.view.ui_graph.as_mut().ok_or("continuation requires SAME installed graph")?
            .pointer_owner.as_mut().ok_or("continuation requires SAME process staged input/CHAR")?.input;
        let device=self.providers.continuation_device(receiver,poll)?;
        Ok((input,device))
    }
    fn recording_after_device(&mut self,receiver:Receiver,poll:Poll)->Result<u8>{
        self.providers.continuation_recording_after_device(receiver,poll)
    }
    fn secondary_enabled(&mut self)->Result<u8>{Ok(self.view.ui_graph.as_ref()
        .ok_or("continuation Secondary graph absent")?.group(Group::Secondary).enabled)}
    fn chat_ring68b4c0(&mut self)->Result<&mut Ring>{self.providers.continuation_ring()}
    fn active43c5c0(&mut self,g:Group)->Result<u8>{
        if g==Group::Tertiary{return Err("central never queries Tertiary43c5c0".into())}
        Ok(u8::from(self.view.ui_graph.as_ref().ok_or("continuation group graph absent")?.group(g).has_selection()))
    }
    fn mode4b52f0(&mut self)->Result<i32>{Ok(self.view.camera_navigation.as_ref()
        .ok_or("continuation actual navigation mode absent")?.mode())}
    fn current822d50(&mut self)->Result<Option<SelectionKey>>{Ok(self.owner.ui_selection()?.current.map(|s|s.key))}
    fn selected_kind30(&mut self,key:SelectionKey)->Result<i16>{
        let selected=self.owner.current_ui_selected()?.ok_or("current822d50 disappeared before kind read")?;
        if selected.key!=key{return Err("current822d50 identity changed before kind read".into())}
        Ok(selected.kind)
    }
    fn participant_player38(&mut self,key:SelectionKey)->Result<u32>{
        if key.native_key()?==0{return Err("participant+38 null identity".into())}
        self.providers.continuation_participant_player(self.view,self.owner,key)
    }
    fn participant_flags336(&mut self,index:i32)->Result<u8>{self.providers.continuation_participant_flags(self.view,self.owner,index)}
    fn participant6899a0(&mut self,index:i32)->Result<ParticipantDescriptor>{self.providers.continuation_participant_lookup(self.view,self.owner,index)}
    fn publish_current822d50(&mut self,value:ParticipantDescriptor)->Result<()>{
        self.owner.ui_selection_mut()?.current=Some(value.selected());Ok(())
    }
    fn transient823168_present(&mut self)->Result<bool>{Ok(self.owner.ui_selection()?.transient.is_some())}
    fn effect(&mut self,effect:Effect)->Result<()>{match effect{
        Effect::SetKind4e06c0(kind)=>{self.owner.set_kind(kind);Ok(())}
        Effect::Target4dfca0(words)=>{self.owner.target(words);Ok(())}
        other=>self.providers.continuation_effect(self.view,self.owner,self.assets,other),
    }}
}

/// Optional narrow delegation for a real Provider while it owns the ordinary
/// UI Inspection/Owner/mutable Assets loans. The port is borrowed from the
/// Inbox service closure for THIS operation; it cannot be stored in process
/// state, sent to a worker or used after the operation returns. It is not a
/// successful implementation of Provider and activates no source family.
pub struct SessionPort<'a,'operation> {
    services:&'a mut crate::live_host::camera_bridge::SessionServices<'operation>,
}
impl<'a,'operation> SessionPort<'a,'operation> {
    pub fn new(services:&'a mut crate::live_host::camera_bridge::SessionServices<'operation>)->Self { Self{services} }
    pub fn byte(&mut self,cell:Byte)->Result<u8> {
        use crate::live_host::camera_bridge::{PauseByte,SessionCall};
        match cell {
            Byte::PauseLatch8b22d8=>self.services.pause_byte(PauseByte::Latch8b22d8).map_err(|e|e.to_string()),
            Byte::SignedReason3_8b22d3=>self.services.pause_byte(PauseByte::SignedReason3_8b22d3).map_err(|e|e.to_string()),
            Byte::Guard57e794Plus345=>Err("chat guard must read actual installed UI graph".into()),
            other=>{
                use crate::live_host::process_runtime::Byte as P;
                let source=match other{
                    Byte::Media8fca98=>P::Media8fca98,Byte::Suppressed8b0700=>P::Suppressed8b0700,
                    Byte::QuitOnEscape8b23b9=>P::QuitOnEscape8b23b9,Byte::Network93bbb9=>P::Network93bbb9,
                    Byte::Debug8b23c5=>P::Debug8b23c5,Byte::Alternative8fca96=>P::Alternative8fca96,
                    Byte::Controller920888=>P::Controller920888,Byte::Test8b23bc=>P::Test8b23bc,
                    _=>unreachable!(),
                };
                match self.services.call(SessionCall::ProcessByte(source)).map_err(|e|e.to_string())?{
                    crate::live_host::camera_bridge::SessionReply::Byte(value)=>Ok(value),
                    _=>Err("process byte reply mismatch".into()),
                }
            },
        }
    }
    pub fn word(&mut self,cell:Word)->Result<u32>{
        use crate::live_host::camera_bridge::{SessionCall,SessionReply};
        let call=match cell{Word::Count822d38=>SessionCall::Count822d38,Word::Scene822d30=>SessionCall::Scene822d30,
            _=>return Err(format!("{cell:?} requires its actual UI/selection/resident source")),};
        match self.services.call(call).map_err(|e|e.to_string())?{SessionReply::Word(value)=>Ok(value),_=>Err("process word reply mismatch".into())}
    }
    pub fn increment_count4ba580(&mut self)->Result<()>{self.completed(crate::live_host::camera_bridge::SessionCall::IncrementCount4ba580)}
    pub fn quit93442c(&mut self)->Result<()>{self.completed(crate::live_host::camera_bridge::SessionCall::Quit93442c)}
    pub fn source_effect(&mut self,effect:Effect)->Result<()>{
        use crate::live_host::camera_bridge::{SessionCall,MissingFamily as M};
        let call=match effect{
            Effect::Media4b9210=>SessionCall::Media4b9210,Effect::Sequence4b1980=>SessionCall::Sequence4b1980,
            Effect::Graph4baf10=>SessionCall::Missing(M::Graph),Effect::Menu491720=>SessionCall::Missing(M::Menu),
            Effect::Pause43e5f0=>SessionCall::Missing(M::Pause43e5f0),
            Effect::DebugCommand4ba9f0_450490=>SessionCall::Missing(M::DebugCommand),
            Effect::Participant481a50(_)=>SessionCall::Missing(M::Participant481a50),
            Effect::Capture458af0Zero=>SessionCall::Missing(M::Screenshot),
            Effect::Selection4c3a20Zero=>SessionCall::Missing(M::Selection),Effect::Action4c3b80=>SessionCall::Missing(M::Action),
            Effect::Script4bc9d0Map|Effect::Script4d62b0MapOne=>SessionCall::Missing(M::Script),
            _=>return Err("UI/Owner effect must use SAME actual owner".into()),
        };self.completed(call)
    }
    fn completed(&mut self,call:crate::live_host::camera_bridge::SessionCall)->Result<()>{
        match self.services.call(call).map_err(|e|e.to_string())?{
            crate::live_host::camera_bridge::SessionReply::Completed=>Ok(()),_=>Err("process completion reply mismatch".into())}
    }
    pub fn busy48dbf0(&mut self)->Result<u8> { self.services.busy48dbf0().map_err(|e|e.to_string()) }
    // Pause43e5f0 is deliberately NOT mapped to pause4bac90. Its actual graph/
    // menu/toggle ordering has not been joined. Real callers of the established
    // reason-specific primitive may use SessionServices::pause4bac90 directly.
}
