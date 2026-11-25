//! Read-only actual Session World producer; no install row snapshot authority.
use crate::live_host::camera_bridge::{ResidentSource,PropertyEpoch,SessionReply,RequestError};
pub(crate) fn read(world:&openmt2001::level_startup::World,source:&ResidentSource,row:Option<u8>)->Result<SessionReply,RequestError>{
    source.validate(PropertyEpoch::World(world.topology.epoch()),&world.creation.auxiliary)?;
    if world.categories.len()!=52||world.topology.count617624()!=52{return Err("D actual World category domain is not52".into())}
    let Some(row)=row else{
        #[cfg(test)] observer::record(world.topology.epoch().serial(),None,None);
        return Ok(SessionReply::PrimaryCategory(None))
    };
    let record=world.categories.get(usize::from(row)).ok_or("D category read outside actual52 records")?;
    //Exact42a3ae/3b7/3c9 inputs, reborrowed after every earlier call.
    let words=[u32::from(record[4]),u32::from(record[0x90]),
        u32::from_le_bytes(record[0x8c..0x90].try_into().unwrap())];
    #[cfg(test)] observer::record(world.topology.epoch().serial(),Some(row),Some(words));
    Ok(SessionReply::PrimaryCategory(Some(words)))
}

/// Post-validation observations only. No hook or callback can change the read,
/// mint a source, override a reply, or supply a World. Each guard is Host-thread
/// local and !Send/!Sync; nested observers reject and Drop clears all records.
#[cfg(test)]
pub(crate) mod observer {
    use std::{cell::RefCell,marker::PhantomData,rc::Rc};
    pub(crate) type Read=(u64,Option<u8>,Option<[u32;3]>);
    thread_local! {static READS:RefCell<Option<Vec<Read>>>=const {RefCell::new(None)};}
    pub(crate) struct Observe{_thread:PhantomData<Rc<()>>}
    impl Observe {
        pub(crate) fn start()->Self{
            READS.with(|r|{let mut r=r.borrow_mut();assert!(r.is_none(),"nested actual-category observer");*r=Some(Vec::new());});
            Self{_thread:PhantomData}
        }
        pub(crate) fn records(&self)->Vec<Read>{READS.with(|r|r.borrow().as_ref().expect("live observer").clone())}
        pub(crate) fn clear(&self){READS.with(|r|r.borrow_mut().as_mut().expect("live observer").clear());}
    }
    impl Drop for Observe{fn drop(&mut self){READS.with(|r|*r.borrow_mut()=None);}}
    pub(super) fn record(epoch:u64,row:Option<u8>,words:Option<[u32;3]>){
        READS.with(|r|if let Some(reads)=r.borrow_mut().as_mut(){reads.push((epoch,row,words));});
    }
}
