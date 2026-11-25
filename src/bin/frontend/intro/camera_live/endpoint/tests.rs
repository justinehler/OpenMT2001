use super::*;
use openmt2001::camera::{bank::Bank,primary::{Mode12Input,Mode13Input}};
use std::{sync::mpsc,thread,time::{Duration,Instant}};
#[derive(Default)]
pub(crate) struct Ui {fail_continuation:bool,fail_focus:bool,fail_validate:bool,fail_finalize:bool,save_pending:bool,save_published:usize,save_failed:usize,writeback_cuts:usize,trace:Vec<&'static str>,staged:Vec<TaskEffect>,fail_cursor:bool,clear_changes_kind:bool,consumed:usize,queued_target:Option<[u32;3]>,pre_captures:Vec<Capture>}
fn input()->Input {
    Input {mode12:Mode12Input{build_visible:false,enable_translation:true,enable_keyboard:true,enable_rotation:true,enable_zoom:true,buttons:[false;2],delta:[0;2],keys:[true,false,false,false],sensitivity:50,legacy_yaw:false,invert_vertical:false,low_height:false},
        mode13:Mode13Input{enable_translation:true,enable_keyboard:true,enable_zoom:true,buttons:[false;2],delta:[0;2],keys:[true,false,false,false],sensitivity:50,legacy_yaw:false,invert_vertical:false,low_height:false}}
}
impl Context for Ui {
    fn validate(&self,_:&Operation,_:&Owner)->Result<(),String>{
        if self.fail_validate{Err("read-only preflight rejected".into())}else{Ok(())}
    }
    fn publish_save_captures(&mut self){if self.save_pending{self.save_pending=false;self.save_published+=1;}}
    fn take_strategic_writeback(&mut self)->Result<Option<crate::locator::strategic::Writeback>,String>{self.writeback_cuts+=1;
        if self.fail_finalize&&self.consumed>0{Err("late C finalization failure".into())}else{Ok(None)}}
    fn admission(&mut self,_:&Admission)->Result<(),String>{Ok(())}
    fn pre_camera(&mut self,owner:&mut Owner,_:&Admission)->Result<(),String>{
        if let Some(target)=self.queued_target.take(){owner.target(target);self.pre_captures.push(owner.capture());}
        self.save_pending=true;Ok(())
    }
    fn input(&mut self,_:&Admission)->Result<Input,String>{self.trace.push("input");self.staged.push(TaskEffect::ClearElapsed);Ok(input())}
    fn consumed_input(&mut self){self.trace.push("consume");self.consumed+=1;}
    fn cursor(&mut self,_:Vec<CursorRequest>)->Result<(),String>{self.trace.push("cursor");if self.fail_cursor{Err("cursor failure".into())}else{Ok(())}}
    fn navigate(&mut self,owner:&mut Owner)->Result<(),String>{
        self.trace.push("clear");if self.clear_changes_kind{owner.set_kind(4);}
        owner.strategic_after_ui_clear();self.trace.push("navigation");Ok(())
    }
    fn tutorial(&mut self,_:NativeOperation,_:&mut Owner)->Result<(),String>{Err("test does not implement tutorial operations".into())}
    fn pending_sound(&mut self,_:Pose)->Result<(),String>{self.trace.push("sound-current");Ok(())}
    fn present(&mut self,_:Pose)->Result<(),String>{self.trace.push("present-sample");Ok(())}
    fn take_task_effects(&mut self)->Vec<TaskEffect>{std::mem::take(&mut self.staged)}
    fn discard_task_effects(&mut self){self.staged.clear();if self.save_pending{self.save_pending=false;self.save_failed+=1;}}
}
pub(super) fn capture()->Capture {
    let mut p=[0u8;196];p[0]=6;p[0x70]=1;p[0x71]=1;p[0x74]=10;
    for (at,v) in [(0x34,100f32),(0x38,120.),(0x68,1.),(0x90,1.),(0xb0,80.)]{p[at..at+4].copy_from_slice(&v.to_le_bytes());}
    Capture::from_records([p,[0xcd;196]])
}
fn drive<T:Send+'static>(ui:Ui,run:impl FnOnce(&mut bridge::Worker,Capture,std::sync::Arc<bridge::PropertySource>)->T+Send+'static)->(T,Capture,Ui,Vec<String>) {
    let assets=crate::selection::Assets::descriptor_test_assets(vec![],[0;116]);
    let (mut worker,inbox)=bridge::channel(47);let initial=capture();
    let source=std::sync::Arc::new(bridge::PropertySource::synthetic(47,[[0;380];52],52,&assets));
    let mut endpoint=Endpoint::with_property_install(Owner::new(Bank::loader_zero_storage(),9),inbox,PropertyInstall::synthetic(&assets));
    let mut ui=ui;
    let (done,result)=mpsc::channel();let (release,held)=mpsc::channel();
    let handle=thread::spawn(move||{let r=run(&mut worker,initial,source);done.send(r).unwrap();let _=held.recv();});
    let start=Instant::now();let mut errors=Vec::new();let value=loop {
        if let Ok(value)=result.try_recv(){break value;}
        assert!(start.elapsed()<Duration::from_secs(3),"camera protocol did not complete");
        match endpoint.service(&mut ui){Ok(Service::Idle)=>thread::yield_now(),Ok(Service::Completed)=>{},Err(e)=>errors.push(e)}
        // A real refresh moves the endpoint with its owner/inbox/sequence.
        let mut slot=Some(endpoint);endpoint=slot.take().unwrap();
    };
    let saved=endpoint.capture();release.send(()).unwrap();handle.join().unwrap();(value,saved,ui,errors)
}
fn install(w:&mut bridge::Worker,c:Capture,source:std::sync::Arc<bridge::PropertySource>){
    w.request(Operation::Install{property:Some(source),capture:c,admission:Admission::default()}).unwrap();
}
#[test]
fn acknowledged_central_actual_ratio_and_sound_order_survive_moves() {
    let (observation,saved,ui,errors)=drive(Ui::default(),|w,c,source|{
        install(w,c,source);for _ in 0..2{w.request(Operation::CurrentPose).unwrap();}
        let c=w.request(Operation::CentralTail{admission:Admission{camera_disabled:true,..Admission::default()}}).unwrap();
        assert_eq!(c.current.eye[0],10f32.to_bits());assert_eq!(c.task_effects,vec![TaskEffect::ClearElapsed]);
        w.request(Operation::Sample{ratio_bits:2f32.to_bits()}).unwrap()
    });
    assert!(errors.is_empty());assert_eq!(observation.current.eye[0],10f32.to_bits());assert_eq!(observation.sampled.unwrap().eye[0],28f32.to_bits());assert_eq!(saved.records()[1],[0xcd;196]);
    assert_eq!(ui.trace,vec!["input","consume","cursor","sound-current","present-sample"]);assert_eq!(ui.consumed,1);
}
#[test]
fn failed_owner_work_discards_uncommitted_timer_effects_and_latches() {
    let (failed,_,ui,errors)=drive(Ui{fail_cursor:true,..Ui::default()},|w,c,source|{
        install(w,c,source);assert!(w.request(Operation::CentralTail{admission:Admission::default()}).is_err());
        w.request(Operation::Sample{ratio_bits:0f32.to_bits()}).is_err()
    });
    assert!(failed);assert!(!errors.is_empty());assert!(ui.staged.is_empty());assert!(!ui.trace.contains(&"sound-current"));assert!(!ui.trace.contains(&"present-sample"));
}
#[test]
fn external_navigation_clear_precedes_raw_kind_branch() {
    let (_,saved,ui,errors)=drive(Ui{clear_changes_kind:true,..Ui::default()},|w,c,source|{
        install(w,c,source);w.request(Operation::Apply{operation:NativeOperation::StrategicAfterUiClear,admission:Admission::default()}).unwrap();
    });
    assert!(errors.is_empty());assert_eq!(saved.records()[0],[0xcd;196]);assert_eq!(saved.records()[1],[0xcd;196]);assert_eq!(ui.trace,vec!["clear","navigation"]);
}
#[test]
fn queued_ui_setter_and_capture_precede_same_central_common() {
    let (observation,saved,ui,errors)=drive(Ui{queued_target:Some([20f32.to_bits(),120f32.to_bits(),(-30f32).to_bits()]),..Ui::default()},|w,c,source|{
        install(w,c,source);w.request(Operation::CentralTail{admission:Admission::default()}).unwrap()
    });
    assert!(errors.is_empty());assert_eq!(observation.current.eye[0],2f32.to_bits());assert_eq!(ui.pre_captures.len(),1);
    let word=|row:&[u8;196],at|u32::from_le_bytes(row[at..at+4].try_into().unwrap());
    assert_eq!(word(&ui.pre_captures[0].records()[0],4),0);assert_eq!(word(&ui.pre_captures[0].records()[0],0x34),20f32.to_bits());
    assert_eq!(word(&saved.records()[0],4),2f32.to_bits());assert_eq!(word(&saved.records()[0],0x34),20f32.to_bits());
    assert_eq!(ui.pre_captures[0].records()[1],saved.records()[1]);
}


#[test]
fn explicit_into_owner_cancels_without_rewriting_pair_or_dirty_byte() {
    let (mut worker, inbox) = bridge::channel(301);
    let mut bank = Bank::loader_zero_storage();
    bank.restore(capture());
    let owner = Owner::new(bank, 0x91);
    let before = owner.capture();
    let endpoint = Endpoint::new(owner, inbox);
    let owner = endpoint.into_owner();
    assert_eq!(owner.capture(), before);
    assert_eq!(owner.kind_dirty(), 0x91);
    assert_eq!(worker.request(Operation::Install { capture: before,property:None,
        admission: Admission::default() }), Err(bridge::RequestError::Retired));
}

#[test]
fn dropping_endpoint_without_into_owner_is_still_a_failure() {
    let (mut worker, inbox) = bridge::channel(302);
    let endpoint = Endpoint::new(Owner::new(Bank::loader_zero_storage(), 8), inbox);
    drop(endpoint);
    assert!(matches!(worker.request(Operation::Install { capture: capture(),property:None,
        admission: Admission::default() }), Err(bridge::RequestError::Failed(_))));
    assert!(!worker.retired());
}

#[test]
fn world_writeback_cut_occurs_only_at_protected_install_and_central() {
    let (_,_,ui,errors)=drive(Ui::default(),|w,c,source|{
        install(w,c,source);
        w.request(Operation::CurrentPose).unwrap();
        w.request(Operation::Sample{ratio_bits:0f32.to_bits()}).unwrap();
        w.request(Operation::Admission(Admission::default())).unwrap();
        w.request(Operation::Apply{operation:NativeOperation::SetKind(6),admission:Admission::default()}).unwrap();
        let ack=w.request(Operation::CentralTail{admission:Admission::default()}).unwrap();
        assert_eq!(ack.task_effects,vec![TaskEffect::ClearElapsed]);
    });
    assert!(errors.is_empty());
    assert_eq!(ui.writeback_cuts,2,"pure pose/sample/admission/setter must not consume world writeback");
}

#[test]
fn preflight_rejection_precedes_restore_and_leaves_queued_save_and_input_untouched(){
    let (failed,saved,ui,errors)=drive(Ui{fail_validate:true,save_pending:true,..Ui::default()},|w,c,source|{
        w.request(Operation::Install{property:Some(source),capture:c,admission:Admission::default()}).is_err()
    });
    assert!(failed);assert!(!errors.is_empty());
    assert_eq!(saved,Capture::from_records([[0;196];2]));
    assert_eq!(ui.consumed,0);assert!(ui.trace.is_empty());assert_eq!(ui.writeback_cuts,0);
    assert!(ui.save_pending);assert_eq!((ui.save_published,ui.save_failed),(0,0));
}
#[test]
fn completed_central_ack_keeps_effects_on_late_extractor_fault_without_save_success(){
    let ((ack,next),_,ui,errors)=drive(Ui{fail_finalize:true,..Ui::default()},|w,c,source|{
        install(w,c,source);
        let ack=w.request(Operation::CentralTail{admission:Admission::default()}).unwrap();
        let next=w.request(Operation::CurrentPose);
        (ack,next)
    });
    assert_eq!(ack.task_effects,vec![TaskEffect::ClearElapsed]);
    assert_eq!(ack.finalization_fault.as_deref(),Some("late C finalization failure"));
    assert!(ack.strategic_writeback.is_none());
    assert_eq!(next,Err(bridge::RequestError::Failed("late C finalization failure".into())));
    assert_eq!((ui.consumed,ui.save_published,ui.save_failed),(1,0,1));
    assert!(ui.staged.is_empty());assert!(errors.iter().any(|e|e=="late C finalization failure"));
}

// Actual Endpoint + existing Worker/Inbox callback route. The
// prefix receipt/Host primitives below are explicit synthetic fixtures, not
// a successful live Provider or full native central admission.
impl SessionContext for Ui {
    fn groups_and_continuation(&mut self,_:&mut Owner,_:&Admission,
        services:&mut SessionServices<'_>)->Result<super::super::continuation::BeforeTail,String> {
        use bridge::{PauseReason,PauseByte};
        self.trace.push("prefix");self.save_pending=true;
        services.pause4bac90(PauseReason::Action3).map_err(|e|e.to_string())?;
        assert_eq!(services.pause_byte(PauseByte::SignedReason3_8b22d3).map_err(|e|e.to_string())?,1);
        self.trace.push("continuation");
        if self.fail_continuation {return Err("continuation partial failure".into());}
        Ok(super::super::continuation::BeforeTail::synthetic())
    }
    fn focus_after_continuation(&mut self,_:&mut Owner,_:&Admission,
        before:super::super::continuation::BeforeTail,_:&mut SessionServices<'_>)->Result<(),String> {
        assert_eq!(before.phase(),super::super::continuation::Phase::ContinuationComplete);
        self.trace.push("focus");
        if self.fail_focus {Err("focus partial failure".into())} else {Ok(())}
    }
}
fn drive_session(mut ui:Ui)->(Result<Observation,bridge::RequestError>,u8,Ui) {
    let assets=crate::selection::Assets::descriptor_test_assets(vec![],[0;116]);
    let (mut worker,inbox)=bridge::channel(912);let initial=capture();
    let source=std::sync::Arc::new(bridge::PropertySource::synthetic(912,[[0;380];52],52,&assets));
    let mut endpoint=Endpoint::with_property_install(Owner::new(Bank::loader_zero_storage(),9),inbox,PropertyInstall::synthetic(&assets));
    let client=thread::spawn(move||{
        install(&mut worker,initial,source);
        let mut actual=0;
        let result=worker.request_with_handler(Operation::CentralTail{admission:Admission::default()},|call|{
            Ok(match call {
                bridge::SessionCall::Pause4bac90(_)=>{actual+=1;bridge::SessionReply::Completed},
                bridge::SessionCall::PauseByte(_)=>bridge::SessionReply::Byte(actual),
                _=>panic!("unexpected endpoint fixture callback"),
            })
        });
        (result,actual)
    });
    // The real request receive waits; callbacks are generated only by the
    // running Endpoint operation, never pre-enqueued to fake interleaving.
    endpoint.wait_service_session(&mut ui,Duration::from_secs(3)).unwrap();
    let _=endpoint.wait_service_session(&mut ui,Duration::from_secs(3));
    let (result,actual)=client.join().unwrap();(result,actual,ui)
}
#[test]
fn session_endpoint_orders_continuation_focus_camera_once_and_skips_legacy_pre_camera() {
    let (ack,actual,ui)=drive_session(Ui{queued_target:Some([0;3]),..Ui::default()});
    assert_eq!(actual,1);assert!(ack.is_ok());
    assert_eq!(ui.trace,vec!["prefix","continuation","focus","input","consume","cursor"]);
    assert_eq!(ui.consumed,1);assert_eq!(ui.save_published,1);
    assert!(ui.pre_captures.is_empty());assert_eq!(ui.queued_target,Some([0;3]),"legacy pre_camera must not run in joined route");
}
#[test]
fn session_endpoint_partial_fault_keeps_host_effect_without_focus_camera_or_save() {
    let (ack,actual,ui)=drive_session(Ui{fail_continuation:true,..Ui::default()});
    assert_eq!(actual,1);assert_eq!(ack,Err(bridge::RequestError::Failed("continuation partial failure".into())));
    assert_eq!(ui.trace,vec!["prefix","continuation"]);assert_eq!(ui.consumed,0);
    assert_eq!((ui.save_published,ui.save_failed),(0,1));
    let (ack,actual,ui)=drive_session(Ui{fail_focus:true,..Ui::default()});
    assert_eq!(actual,1);assert_eq!(ack,Err(bridge::RequestError::Failed("focus partial failure".into())));
    assert_eq!(ui.trace,vec!["prefix","continuation","focus"]);assert_eq!(ui.consumed,0);
    assert_eq!((ui.save_published,ui.save_failed),(0,1));
}
#[test]
fn session_endpoint_completed_task_effects_survive_finalizer_fault_and_no_save_success() {
    let (ack,actual,ui)=drive_session(Ui{fail_finalize:true,..Ui::default()});
    let ack=ack.unwrap();assert_eq!(actual,1);assert_eq!(ui.consumed,1);
    assert_eq!(ack.task_effects,vec![TaskEffect::ClearElapsed]);
    assert_eq!(ack.finalization_fault.as_deref(),Some("late C finalization failure"));
    assert_eq!((ui.save_published,ui.save_failed),(0,1));
}
