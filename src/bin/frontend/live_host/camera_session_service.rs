//! Actual Host-stack Session handler, invoked by the existing central request.
//! No copied World/Session, Lua move, clock, FreshRequest or callback replay.
use super::{camera_bridge::{SessionCall,SessionReply,RequestError},Session};

pub(super) fn handle(session:&mut Session<'_>,call:SessionCall)->Result<SessionReply,RequestError> {
    // Borrow only the source-owned pieces while the same Session stays with Host.
    // This inner function is also the CPU fixture seam; it is used by production,
    // not a separately reimplemented test dispatcher.
    apply(&mut session.world.pause,&mut session.hooks.timers,&session.catalog,call)
}
///The production Host lends its SAME process handle and SAME Session on this
///stack. No process lock exists while Worker waits for the UI operation.
pub(super) fn handle_with_process(session:&mut Session<'_>,process:Option<&super::process_runtime::Handle>,
    call:SessionCall)->Result<SessionReply,RequestError>{
    use SessionCall::*;
    let process_call=matches!(call,ProcessByte(_)|Count822d38|IncrementCount4ba580|Scene822d30|Quit93442c|Media4b9210|Sequence4b1980);
    if !process_call{return handle(session,call)}
    apply_process(process,&session.world.clock.timing[5],call)
}
fn apply_process(process:Option<&super::process_runtime::Handle>,scene:&u32,call:SessionCall)->Result<SessionReply,RequestError>{
    use SessionCall::*;
    let process=process.ok_or_else(||super::process_runtime::Error::Absent.to_string())?;
    match call{
        ProcessByte(cell)=>process.byte(cell).map(SessionReply::Byte).map_err(|e|e.to_string().into()),
        Count822d38=>process.count822d38().map(SessionReply::Word).map_err(|e|e.to_string().into()),
        IncrementCount4ba580=>process.increment_count4ba580().map(|()|SessionReply::Completed).map_err(|e|e.to_string().into()),
        //SAME native timing sixth-word storage, already written by real map,
        //auction prepare/settlement and source timing writers; no mode inference.
        Scene822d30=>{process.scene_available().map_err(|e|e.to_string())?;Ok(SessionReply::Word(*scene))},
        Quit93442c=>process.request_quit(super::process_runtime::QuitWriter::Central).map(|()|SessionReply::Completed).map_err(|e|e.to_string().into()),
        Media4b9210=>process.reached_periodic_media().map(|()|SessionReply::Completed).map_err(|e|e.to_string().into()),
        Sequence4b1980=>process.reached_sequence4b1980().map(|()|SessionReply::Completed).map_err(|e|e.to_string().into()),
        _=>unreachable!(),
    }
}
fn apply(pause:&mut openmt2001::pause::State,timers:&mut [openmt2001::timers::Task],
    catalog:&openmt2001::level_catalog::Catalog,call:SessionCall,
)->Result<SessionReply,RequestError> {
    use SessionCall::*;
    match call {
        PauseByte(byte)=>Ok(SessionReply::Byte(pause.bytes[match byte {
            super::camera_bridge::PauseByte::Latch8b22d8=>8,
            super::camera_bridge::PauseByte::SignedReason3_8b22d3=>3,
        }])),
        // Actual optional selected record and byte, not TutorialEvent's ==1
        // gate or a guessed participant/tutorial/menu snapshot.
        Busy48dbf0=>Ok(SessionReply::Byte(catalog.selected_level().map_or(0,|level|level.bytes[0x85c]))),
        Pause4bac90(reason)=>{
            pause.pause(reason.index(),|event,_|{event.apply(timers);Ok(())})?;
            Ok(SessionReply::Completed)
        }
        Unpause4bacc0(reason)=>{
            pause.unpause(reason.index(),|event,_|{event.apply(timers);Ok(())})?;
            Ok(SessionReply::Completed)
        }
        Missing(family)=>Err(format!("Session source/callback join unavailable: {family:?}").into()),
        ProcessByte(_)|Count822d38|IncrementCount4ba580|Scene822d30|Quit93442c|Media4b9210|Sequence4b1980=>Err(super::process_runtime::Error::Absent.to_string().into()),
    }
}

#[cfg(test)]
#[path="camera_session_service/process_tests.rs"] mod process_tests;
#[cfg(test)]
#[path="camera_session_service/tests.rs"]
mod tests;
