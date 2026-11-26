//! Completion receipt for this Inspection's actual Options request.
//! No native owner/token/phase is created; retiring the view drops its receiver.
use std::sync::mpsc::{Receiver,TryRecvError};
#[derive(Default)]
pub(super) struct Gate { pending:Option<(bool,Receiver<Result<(),String>>)>, confirmed_open:bool }
impl Gate {
    pub(super) fn cancel(&mut self){self.pending=None;self.confirmed_open=false;}
    pub(super) fn begin(&mut self,open:bool,reply:Receiver<Result<(),String>>){
        self.cancel();self.pending=Some((open,reply));
    }
    pub(super) fn ready(&self)->bool{self.confirmed_open&&self.pending.is_none()}
    pub(super) fn poll(&mut self)->Result<Option<bool>,String>{
        let Some((open,reply))=&self.pending else{return Ok(None)};
        let open=*open;
        match reply.try_recv(){
            Err(TryRecvError::Empty)=>Ok(None),
            result=>{
                self.pending=None;
                match result{
                    Ok(Ok(()))=>{self.confirmed_open=open;Ok(Some(open))},
                    Ok(Err(error))=>{self.confirmed_open=false;Err(error)},
                    Err(TryRecvError::Disconnected)=>{self.confirmed_open=false;Err("Options session closed".into())},
                    Err(TryRecvError::Empty)=>unreachable!(),
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;use std::sync::mpsc;
    #[test]
    fn save_waits_for_actual_request_reply_and_close_revokes_readiness(){
        let mut gate=Gate::default();let(tx,rx)=mpsc::channel();gate.begin(true,rx);
        assert!(!gate.ready());assert_eq!(gate.poll().unwrap(),None);assert!(!gate.ready());
        tx.send(Ok(())).unwrap();assert!(!gate.ready());assert_eq!(gate.poll().unwrap(),Some(true));assert!(gate.ready());
        let(tx,rx)=mpsc::channel();gate.begin(false,rx);assert!(!gate.ready());
        tx.send(Ok(())).unwrap();assert_eq!(gate.poll().unwrap(),Some(false));assert!(!gate.ready());
    }
    #[test]
    fn replaced_or_retired_pending_reply_cannot_admit_another_view(){
        let mut gate=Gate::default();let(old,rx)=mpsc::channel();gate.begin(true,rx);
        let(new,rx)=mpsc::channel();gate.begin(true,rx);assert!(old.send(Ok(())).is_err());assert!(!gate.ready());
        new.send(Ok(())).unwrap();assert_eq!(gate.poll().unwrap(),Some(true));assert!(gate.ready());
        gate.cancel();assert!(!gate.ready());
        let(retired,rx)=mpsc::channel();gate.begin(true,rx);gate.cancel();
        assert!(retired.send(Ok(())).is_err());assert!(!gate.ready());
    }
    #[test]
    fn failure_and_disconnect_never_become_successful_hold(){
        let mut gate=Gate::default();let(tx,rx)=mpsc::channel();gate.begin(true,rx);
        tx.send(Err("actual owner failure".into())).unwrap();assert_eq!(gate.poll().unwrap_err(),"actual owner failure");assert!(!gate.ready());
        let(tx,rx)=mpsc::channel();gate.begin(true,rx);drop(tx);
        assert!(gate.poll().is_err());assert!(!gate.ready());assert_eq!(gate.poll().unwrap(),None);
    }
}
