//! Missing native process products. The shared handle shares identity, never a
//! flag snapshot. Every method releases its mutex before returning/callbacks.
use std::sync::{Arc,Mutex};
use crate::intro::camera_live::process_image::Cells;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Error {
    Absent, Poisoned, UnbornSequence, MediaProbeUnavailable, SequenceCallbackUnavailable,
    TimingModeCallbackUnavailable, SourceLoadOrderUnavailable, UnsupportedStartupOption,
    CentralAlreadyActive, CentralTokenMismatch, CentralTokenExhausted, NoActiveCentral,
}
impl std::fmt::Display for Error {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"process source unavailable: {self:?}")}
}
impl std::error::Error for Error {}
pub type Result<T>=std::result::Result<T,Error>;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Byte { Media8fca98,Suppressed8b0700,QuitOnEscape8b23b9,Network93bbb9,
    Debug8b23c5,Alternative8fca96,Controller920888,Test8b23bc }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MediaSite { Startup4b94eb,Late4b9beb,Periodic4b921d }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MediaOutcome { Valid, OriginalBypass }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MediaProducer { Loader, Awaiting(MediaSite), Validated(MediaSite) }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum QuitWriter { Central,Window,Menu }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PickingWrite { DisableAll,EnableAll,CameraDisable,Block(Option<i32>),Building(Option<i32>) }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct PickingProjection { pub common:u8,pub camera:u8,pub block:u8,pub block_id:i32,
    pub building:u8,pub building_id:i32 }
struct Options { quit_on_escape:u8,debug:u8,test:u8,test_b:u8 }
impl Options {
    fn defaults4b9140()->Self {Self{quit_on_escape:0,debug:0,test:0,test_b:0}}
    fn parse4c7500(&mut self,command:&str)->Result<()> {
        // Admitted parser slice: exact source delimiters and ASCII option
        // names. Native ignores unknown tokens. Recognized unported options
        // are explicit errors; no accidental HOST/JOIN policy activation.
        for token in command.split('\0').next().unwrap_or("").split(|c:char|matches!(c,' '|','|'\t'|'\n'|'-'|'+')).filter(|s|!s.is_empty()) {
            match token.to_ascii_uppercase().as_str() {
                "DEBUGSTUFF"=>self.debug=1,
                "TESTMODE"=>{self.test=1;self.test_b=1;},
                "NOEXCEPTIONHANDLING"|"L"|"BUILDINGEDITOR"|"HOST"|"PORT"|"JOIN"|"SESSIONNAME"|"MAXPLAYERS"=>return Err(Error::UnsupportedStartupOption),
                _=>{},
            }
        }
        Ok(())
    }
}
struct Network { enabled:u8 }
impl Network {fn constructor4e1290()->Self{Self{enabled:0}}fn teardown4e1c50(&mut self){self.enabled=0;}}
struct Sequence { active:u8,second:u8,bypass:u8,bypass_tail:u8 }
impl Sequence {fn constructor4b1790()->Self{Self{active:0,second:0,bypass:1,bypass_tail:0}}}
struct Media {byte:u8,producer:MediaProducer}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct CentralId { serial:u64,operation:super::camera_bridge::CentralIdentity }
///Move-only, private construction. Completion/failure consumes the lease
///created for this exact checked Worker operation, not a later World/Host.
pub struct CentralToken(CentralId);
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum TailObligation {Pending(CentralId),Completed(CentralId),Failed(CentralId)}
struct Quit {requested:u8,active:Option<CentralId>,next_serial:u64,obligation:Option<TailObligation>}
struct ProcessRuntime {
    options:Options,network:Network,alternative:u8,media:Media,sequence:Option<Sequence>,
    picking:PickingProjection,count:u32,quit:Quit,
    // This is only availability/provenance, NEVER a second scene/clock word.
    // Scene822d30 is the already-existing Session.world.clock.timing[5].
    scene_error:Option<Error>,
}
#[derive(Clone)]
pub struct Handle(Arc<Mutex<ProcessRuntime>>);
/// UI-owned pair. Not Clone, not in Host/Session, not rebuilt on World reset.
/// Assets owns it before any graph, and retains it across park/attach/refresh.
pub struct Activity { a4:u32,a8:u32 }
impl Activity {
    pub fn a4(&self)->u32{self.a4}
    pub fn a8(&self)->u32{self.a8}
    pub fn store_a4(&mut self,value:u32){self.a4=value;}
    pub fn store_a8(&mut self,value:u32){self.a8=value;}
    ///4c2bdc's actual operand. The source controller uses the very same a4.
    pub fn presentation4c2bdc(&self)->u32{self.a4}
}
/// Construct once at actual Assets startup, from supplied PE loader products,
/// source defaults/options and native offline/profile/sequence constructors.
/// No code from the PE is loaded or executed. No caller-filled flag builder.
pub fn startup(pe:&[u8],command:&str)->std::result::Result<(Handle,Activity),String>{
    let cells=Cells::process_allocation(pe)?;
    startup_from_loader(&cells,command)
}
fn startup_from_loader(cells:&Cells,command:&str)->std::result::Result<(Handle,Activity),String>{
    // Each loader preimage is verified, including opaque96 and both activity
    // words. This reads actual supplied virtual-tail zero-fill, not constants
    // returned by getters. Constructor/default stores supersede loader bytes.
    for address in [0x8b23b9,0x8b23c5,0x8b23bc,0x8fca96,0x8fca98,0x93bbb9,
        0x920888,0x920889,0x92088a,0x920890,0x93442c] {
        if cells.byte(address)?!=0{return Err(format!("unexpected process loader preimage {address:08x}"));}
    }
    for address in [0x822d38,0x8fcaa4,0x8fcaa8] {
        if cells.word(address)?!=0{return Err(format!("unexpected process loader word {address:08x}"));}
    }
    let mut options=Options::defaults4b9140();options.parse4c7500(command).map_err(|e|e.to_string())?;
    let mut runtime=ProcessRuntime{options,network:Network::constructor4e1290(),
        alternative:cells.byte(0x8fca96)?,media:Media{byte:cells.byte(0x8fca98)?,producer:MediaProducer::Loader},
        sequence:Some(Sequence::constructor4b1790()),
        //4c2b30 first store,4c2b60 constructor and4c2b80 reset: common1,
        //camera1, block0/id-1, building0/id-1. No tutorial-derived negation.
        picking:PickingProjection{common:1,camera:1,block:0,block_id:-1,building:0,building_id:-1},
        count:cells.word(0x822d38)?,quit:Quit{requested:cells.byte(0x93442c)?,
            active:None,next_serial:0,obligation:None},scene_error:None};
    //4b9480 samples the native network constructor product once. Teardown
    //later cannot undo the city-entry b9 writer.
    runtime.city_main_entry();
    //Actual startup clear is committed. The missing probe is represented as
    //awaiting, NOT admitted as known valid0 or silently ignored success.
    runtime.media.byte=0;runtime.media.producer=MediaProducer::Awaiting(MediaSite::Startup4b94eb);
    Ok((Handle(Arc::new(Mutex::new(runtime))),Activity{a4:cells.word(0x8fcaa4)?,a8:cells.word(0x8fcaa8)?}))
}
impl ProcessRuntime {fn city_main_entry(&mut self){if self.network.enabled!=0{self.options.quit_on_escape=1;}}}
impl Handle {
    fn with<T>(&self,f:impl FnOnce(&mut ProcessRuntime)->Result<T>)->Result<T>{
        let mut state=self.0.lock().map_err(|_|Error::Poisoned)?;f(&mut state)
    }
    pub fn byte(&self,cell:Byte)->Result<u8>{self.with(|r|Ok(match cell{
        Byte::Media8fca98=>{if matches!(r.media.producer,MediaProducer::Awaiting(_)){return Err(Error::MediaProbeUnavailable)}r.media.byte},
        Byte::Suppressed8b0700=>r.sequence.as_ref().ok_or(Error::UnbornSequence)?.active,
        Byte::QuitOnEscape8b23b9=>r.options.quit_on_escape,Byte::Network93bbb9=>r.network.enabled,
        Byte::Debug8b23c5=>r.options.debug,Byte::Test8b23bc=>r.options.test,
        Byte::Alternative8fca96=>r.alternative,Byte::Controller920888=>r.picking.common,
    }))}
    pub fn increment_count4ba580(&self)->Result<()>{self.with(|r|{r.count=r.count.wrapping_add(1);Ok(())})}
    pub fn count822d38(&self)->Result<u32>{self.with(|r|Ok(r.count))}
    ///Called after the actual camera reset final fields, before imported camera
    ///restore. Refresh/Host::new/Admission never invoke this writer.
    pub fn world_reset_post_camera4b9eb1(&self)->Result<()>{self.with(|r|{r.count=0;Ok(())})}
    pub fn world_controller_reset4c2b80(&self)->Result<()>{self.with(|r|{
        r.picking=PickingProjection{common:1,camera:1,block:0,block_id:-1,building:0,building_id:-1};Ok(())})}
    pub fn picking_write(&self,write:PickingWrite)->Result<()>{self.with(|r|{match write{
        PickingWrite::DisableAll=>r.picking.common=0,PickingWrite::EnableAll=>r.picking.common=1,
        PickingWrite::CameraDisable=>r.picking.camera=0,
        PickingWrite::Block(id)=>{r.picking.common=1;r.picking.block=1;r.picking.block_id=id.unwrap_or(-1);r.picking.building=0;},
        PickingWrite::Building(id)=>{r.picking.common=1;r.picking.building=1;r.picking.building_id=id.unwrap_or(-1);r.picking.block=0;},
    }Ok(())})}
    pub fn picking_projection(&self)->Result<PickingProjection>{self.with(|r|Ok(r.picking))}
    pub fn network_teardown4e1c50(&self)->Result<()>{self.with(|r|{r.network.teardown4e1c50();Ok(())})}
    pub fn scene_available(&self)->Result<()>{self.with(|r|r.scene_error.map_or(Ok(()),Err))}
    pub fn source_load_unjoined(&self)->Result<()>{self.with(|r|{r.scene_error=Some(Error::SourceLoadOrderUnavailable);Ok(())})}
    pub fn source_fresh_world(&self)->Result<()>{self.with(|r|{r.scene_error=None;Ok(())})}
    pub fn source_timing_mode_unjoined(&self)->Result<()>{self.with(|r|{r.scene_error=Some(Error::TimingModeCallbackUnavailable);Ok(())})}
    pub fn source_timing_completed(&self)->Result<()>{self.with(|r|{r.scene_error=None;Ok(())})}
    pub fn media_producer(&self)->Result<MediaProducer>{self.with(|r|Ok(r.media.producer))}
    ///No lock crosses a media probe (or its external error/cleanup callback).
    pub fn validate_media(&self,site:MediaSite,probe:impl FnOnce()->Result<MediaOutcome>)->Result<()>{
        self.with(|r|{r.media.byte=0;r.media.producer=MediaProducer::Awaiting(site);Ok(())})?;
        let outcome=probe()?;
        self.with(|r|{r.media.byte=u8::from(outcome==MediaOutcome::OriginalBypass);r.media.producer=MediaProducer::Validated(site);Ok(())})
    }
    pub fn reached_periodic_media(&self)->Result<()>{self.validate_media(MediaSite::Periodic4b921d,||Err(Error::MediaProbeUnavailable))}
    ///The source start callback must own the actual scene/Lua initialization.
    ///This method cannot turn modern result booleans into a source sequence.
    pub fn start_sequence4b1800<'data>(&self,session:&mut super::Session<'data>,initialize:impl FnOnce(&mut super::Session<'data>)->Result<()>)->Result<()>{
        self.with(|r|{r.sequence.as_mut().ok_or(Error::UnbornSequence)?.active=0;Ok(())})?;
        session.world.clock.timing[5]=3;
        initialize(session)?;
        self.with(|r|{r.sequence.as_mut().ok_or(Error::UnbornSequence)?.active=1;Ok(())})
    }
    pub fn end_sequence4b1eb0(&self,cleanup:impl FnOnce()->Result<()>)->Result<()>{
        cleanup()?;self.with(|r|{r.sequence.as_mut().ok_or(Error::UnbornSequence)?.active=0;Ok(())})
    }
    pub fn reached_sequence4b1980(&self)->Result<()>{self.with(|r|{
        let _=r.sequence.as_ref().ok_or(Error::UnbornSequence)?;
        Err(Error::SequenceCallbackUnavailable)
    })}
    pub fn begin_central(&self,operation:super::camera_bridge::CentralIdentity)->Result<CentralToken>{self.with(|r|{
        if r.quit.active.is_some(){return Err(Error::CentralAlreadyActive)}
        let next=r.quit.next_serial.checked_add(1).ok_or(Error::CentralTokenExhausted)?;
        let id=CentralId{serial:r.quit.next_serial,operation};r.quit.next_serial=next;r.quit.active=Some(id);
        Ok(CentralToken(id))
    })}
    pub fn request_quit(&self,writer:QuitWriter)->Result<()>{self.with(|r|{
        if writer==QuitWriter::Central&&r.quit.active.is_none(){return Err(Error::NoActiveCentral)}
        r.quit.requested=1;
        if r.quit.obligation.is_none(){
            if let Some(id)=r.quit.active{r.quit.obligation=Some(TailObligation::Pending(id));}
        }
        Ok(())
    })}
    pub fn complete_central_tail(&self,token:CentralToken)->Result<()>{self.with(|r|{
        if r.quit.active!=Some(token.0){return Err(Error::CentralTokenMismatch)}
        r.quit.active=None;
        if r.quit.obligation==Some(TailObligation::Pending(token.0)){
            r.quit.obligation=Some(TailObligation::Completed(token.0));
        }
        Ok(())
    })}
    pub fn fail_central(&self,token:CentralToken)->Result<()>{self.with(|r|{
        if r.quit.active!=Some(token.0){return Err(Error::CentralTokenMismatch)}
        r.quit.active=None;
        if r.quit.obligation==Some(TailObligation::Pending(token.0)){
            r.quit.obligation=Some(TailObligation::Failed(token.0));
        }
        Ok(())
    })}
    pub fn quit_requested(&self)->Result<u8>{self.with(|r|Ok(r.quit.requested))}
    pub fn exit_ready(&self)->Result<bool>{self.with(|r|Ok(r.quit.requested!=0&&r.quit.active.is_none()
        &&matches!(r.quit.obligation,None|Some(TailObligation::Completed(_))))) }
    #[cfg(test)] pub(crate) fn mutex_available(&self)->bool{self.0.try_lock().is_ok()}
}
#[cfg(test)] pub(crate) fn test_fixture()->Handle{tests::fixture()}
#[cfg(test)]
#[path="process_runtime/tests.rs"] mod tests;
